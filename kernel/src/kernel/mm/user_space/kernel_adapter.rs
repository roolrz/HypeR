// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Production accounting and physical-page adapters for native user memory.

use core::ptr::{copy_nonoverlapping, with_exposed_provenance_mut};
#[cfg(feature = "kernel-self-test")]
use core::sync::atomic::{AtomicUsize, Ordering};

use hyper::mm::allocator::heap::PageOwner;
use hyper::mm::{BuddyError, FallibleArc, PAGE_SIZE, PhysicalAddress};

use super::contract::UserAddressWindow;
use super::contract::{MemoryAccount, MemoryCharge, PageBackend};
use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::mm::page_block::PageBlock;

#[derive(Clone)]
pub(crate) struct DomainAccount(ResourceDomain);

impl DomainAccount {
    pub(crate) const fn new(domain: ResourceDomain) -> Self {
        Self(domain)
    }
}

pub(crate) struct DomainCharge {
    _charge: Option<CommittedCharge>,
}

impl MemoryAccount for DomainAccount {
    type Charge = DomainCharge;
    type Error = ResourceError;

    fn try_charge(&self, charge: MemoryCharge) -> Result<Self::Charge, Self::Error> {
        let amount = ResourceAmount::ZERO
            .with(ResourceKind::KernelMemoryBytes, charge.kernel_bytes)
            .with(ResourceKind::KernelObjects, charge.kernel_objects)
            .with(ResourceKind::CommittedPages, charge.committed_pages)
            .with(ResourceKind::PinnedPages, charge.pinned_pages)
            .with(ResourceKind::UserAddressSpaces, charge.address_spaces)
            .with(ResourceKind::UserMappings, charge.mappings);
        if amount.is_empty() {
            return Ok(DomainCharge { _charge: None });
        }
        Ok(DomainCharge {
            _charge: Some(self.0.reserve(amount)?.commit()),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KernelPageError {
    Allocation(BuddyError),
    AddressOverflow,
    MissingLinearMap,
    Range,
    Unsupported,
}

impl From<BuddyError> for KernelPageError {
    fn from(error: BuddyError) -> Self {
        Self::Allocation(error)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct KernelPageBackend;

#[cfg(feature = "kernel-self-test")]
static FAIL_EXPOSED_WRITE_AFTER_COPY_COUNTDOWN: AtomicUsize = AtomicUsize::new(0);

/// Injects a post-copy failure on one future exposed write.
///
/// The copied bytes remain visible, matching the documented partial-effect
/// contract, while the caller must treat the selected write as failed. A
/// countdown of one selects the next write, two the write after that, and zero
/// disables injection.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn fail_exposed_write_after_copy_for_test(countdown: usize) {
    FAIL_EXPOSED_WRITE_AFTER_COPY_COUNTDOWN.store(countdown, Ordering::Release);
}

pub(crate) fn address_window(
    plan: &crate::hal::user::AddressSpacePlan,
) -> Result<UserAddressWindow, crate::hal::user::AddressSpaceError> {
    // Consume the selected layout without interpreting architecture identity.
    match UserAddressWindow::from_limit(plan.address_limit(), plan.application_limit()) {
        Ok(window) => Ok(window),
        Err(_) => Err(crate::hal::user::AddressSpaceError::InvalidAddressLimit),
    }
}

/// Ordinary pages retain their existing one-block ownership. Explicit DMA VMOs
/// share a larger block without teaching the allocator to free its subpages.
pub(crate) struct KernelUserPage {
    storage: PageStorage,
}
enum PageStorage {
    Single(PageBlock),
    Contiguous {
        physical: PhysicalAddress,
        _owner: FallibleArc<ContiguousBlock>,
    },
}
struct ContiguousBlock {
    _block: PageBlock,
    _charge: CommittedCharge,
}
impl KernelUserPage {
    fn physical(&self) -> PhysicalAddress {
        match &self.storage {
            PageStorage::Single(page) => page.physical(),
            PageStorage::Contiguous { physical, .. } => *physical,
        }
    }
}

pub(super) fn contiguous_pages(
    size: u64,
    domain: &ResourceDomain,
) -> Result<(alloc::vec::Vec<KernelUserPage>, CommittedCharge), super::MemoryObjectError> {
    use super::MemoryObjectError;
    if size < PAGE_SIZE
        || !size.is_power_of_two()
        || size > hyper::abi::native::HYPER_NATIVE_VMO_MAX_CONTIGUOUS_SIZE_BYTES
    {
        return Err(MemoryObjectError::AllocationSize);
    }
    let count = usize::try_from(size / PAGE_SIZE).map_err(|_| MemoryObjectError::AllocationSize)?;
    let charge = domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::CommittedPages, count as u64)
                .with(
                    ResourceKind::KernelMemoryBytes,
                    FallibleArc::<ContiguousBlock>::allocation_size() as u64,
                ),
        )?
        .commit();
    let transient = domain
        .reserve(ResourceAmount::ZERO.with(
            ResourceKind::KernelMemoryBytes,
            (count * core::mem::size_of::<KernelUserPage>()) as u64,
        ))?
        .commit();
    let mut pages = alloc::vec::Vec::new();
    pages
        .try_reserve_exact(count)
        .map_err(|_| MemoryObjectError::AllocationSize)?;
    let block = PageBlock::allocate_for(count.trailing_zeros() as usize, PageOwner::User).map_err(
        |error| {
            MemoryObjectError::Vmo(super::VmoError::Backend(KernelPageError::Allocation(error)))
        },
    )?;
    let base = block.physical().get();
    base.checked_add(size)
        .ok_or(MemoryObjectError::AllocationSize)?;
    let address =
        crate::kernel::mm::memory::linear_address(base).ok_or(MemoryObjectError::AllocationSize)?;
    // SAFETY: A newly allocated, exclusive contiguous block is wholly covered
    // by the permanent RAM map. Zero before any page reference can escape.
    unsafe { with_exposed_provenance_mut::<u8>(address).write_bytes(0, size as usize) };
    let owner = FallibleArc::try_new(ContiguousBlock {
        _block: block,
        _charge: charge,
    })
    .map_err(|_| MemoryObjectError::AllocationSize)?;
    for index in 0..count {
        pages.push(KernelUserPage {
            storage: PageStorage::Contiguous {
                physical: PhysicalAddress::new(base + index as u64 * PAGE_SIZE),
                _owner: owner.clone(),
            },
        });
    }
    Ok((pages, transient))
}

impl PageBackend for KernelPageBackend {
    type Page = KernelUserPage;
    type Error = KernelPageError;
    type InstructionPublicationContext = dyn hyper::cpu::PinnedExecution;

    fn allocate_zeroed(&self) -> Result<Self::Page, Self::Error> {
        let page = PageBlock::allocate_for(0, PageOwner::User)?;
        let address = crate::kernel::mm::memory::linear_address(page.physical().get())
            .ok_or(KernelPageError::MissingLinearMap)?;
        // SAFETY: `page` uniquely owns exactly one PAGE_SIZE block covered by
        // the permanent writable linear map. No reference to it exists yet.
        unsafe { with_exposed_provenance_mut::<u8>(address).write_bytes(0, PAGE_SIZE as usize) };
        Ok(KernelUserPage {
            storage: PageStorage::Single(page),
        })
    }

    fn physical_address(&self, page: &Self::Page) -> PhysicalAddress {
        page.physical()
    }

    fn read_owned(
        &self,
        page: &Self::Page,
        offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let source = page_address(page, offset, destination.len())?;
        // SAFETY: page_address proves this source lies in the owned page. The
        // VMO page lock excludes a writer and destination is caller-owned.
        unsafe {
            copy_nonoverlapping(
                source.cast_const(),
                destination.as_mut_ptr(),
                destination.len(),
            )
        };
        Ok(())
    }

    fn write_owned(
        &self,
        page: &mut Self::Page,
        offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        let destination = page_address(page, offset, source.len())?;
        // SAFETY: page_address proves this destination lies in the uniquely
        // locked owned page, and source remains kernel-owned for this call.
        unsafe { copy_nonoverlapping(source.as_ptr(), destination, source.len()) };
        Ok(())
    }

    fn copy_owned(
        &self,
        source: &Self::Page,
        destination: &mut Self::Page,
    ) -> Result<(), Self::Error> {
        let source = page_address(source, 0, PAGE_SIZE as usize)?;
        let destination = page_address(destination, 0, PAGE_SIZE as usize)?;
        if source == destination {
            return Err(KernelPageError::Range);
        }
        // SAFETY: KernelUserPage owns page-aligned storage; page_address
        // validates both complete pages in the permanent linear map. Distinct
        // bases cannot overlap. The caller holds both page locks and excludes
        // source machine writers. The destination is unpublished and exclusively
        // borrowed.
        unsafe { copy_nonoverlapping(source.cast_const(), destination, PAGE_SIZE as usize) };
        Ok(())
    }

    fn read_exposed(
        &self,
        page: &Self::Page,
        offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let source = page_address(page, offset, destination.len())?;
        if pointer_ranges_overlap(
            source.cast_const(),
            destination.len(),
            destination.as_ptr(),
            destination.len(),
        )? {
            return Err(KernelPageError::Range);
        }
        // SAFETY: The source is a resident PageBlock range retained by the
        // mapping lease, destination is disjoint kernel memory, and the HAL
        // implements the selected architecture's non-faulting external-memory
        // sequence with defined byte-granular partial effects.
        unsafe {
            crate::hal::user::copy_from_exposed(
                source.cast_const(),
                destination.as_mut_ptr(),
                destination.len(),
            )
        }
        .map_err(|_| KernelPageError::Unsupported)
    }

    fn write_exposed(
        &self,
        page: &mut Self::Page,
        offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        let destination = page_address(page, offset, source.len())?;
        if pointer_ranges_overlap(
            source.as_ptr(),
            source.len(),
            destination.cast_const(),
            source.len(),
        )? {
            return Err(KernelPageError::Range);
        }
        // SAFETY: The destination is a resident PageBlock range retained by
        // the mapping lease, source is disjoint kernel memory, and the HAL
        // provides the external-memory sequence described above.
        unsafe { crate::hal::user::copy_to_exposed(source.as_ptr(), destination, source.len()) }
            .map_err(|_| KernelPageError::Unsupported)?;
        #[cfg(feature = "kernel-self-test")]
        let selected = FAIL_EXPOSED_WRITE_AFTER_COPY_COUNTDOWN.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |countdown| countdown.checked_sub(1),
        );
        #[cfg(feature = "kernel-self-test")]
        if matches!(selected, Ok(1)) {
            return Err(KernelPageError::Unsupported);
        }
        Ok(())
    }

    fn publish_instruction_pages(
        &self,
        context: &Self::InstructionPublicationContext,
        mut pages: impl FnMut(&mut dyn FnMut(&mut Self::Page)),
    ) -> Result<(), Self::Error> {
        let mut range_error = None;
        // SAFETY: Every enumerated PageBlock owns one complete aligned page,
        // its VMO page lock excludes modification/execution, and repeated
        // enumeration yields the same immutable set for every cache pass.
        let result = unsafe {
            crate::hal::cache::publish_instruction_ranges(context, |visit_range| {
                pages(
                    &mut |page| match page_address(page, 0, PAGE_SIZE as usize) {
                        Ok(start) => visit_range(start as usize, PAGE_SIZE as usize),
                        Err(error) => range_error = Some(error),
                    },
                );
            })
        };
        if let Some(error) = range_error {
            return Err(error);
        }
        result.map_err(|_| KernelPageError::Range)
    }
}

fn page_address(
    page: &KernelUserPage,
    offset: usize,
    length: usize,
) -> Result<*mut u8, KernelPageError> {
    let end = offset
        .checked_add(length)
        .ok_or(KernelPageError::AddressOverflow)?;
    if end > PAGE_SIZE as usize {
        return Err(KernelPageError::Range);
    }
    let base = crate::kernel::mm::memory::linear_address(page.physical().get())
        .ok_or(KernelPageError::MissingLinearMap)?;
    base.checked_add(offset)
        .map(with_exposed_provenance_mut)
        .ok_or(KernelPageError::AddressOverflow)
}

fn pointer_ranges_overlap(
    left: *const u8,
    left_length: usize,
    right: *const u8,
    right_length: usize,
) -> Result<bool, KernelPageError> {
    let left = left.addr();
    let right = right.addr();
    let left_end = left
        .checked_add(left_length)
        .ok_or(KernelPageError::AddressOverflow)?;
    let right_end = right
        .checked_add(right_length)
        .ok_or(KernelPageError::AddressOverflow)?;
    Ok(left < right_end && right < left_end)
}
