// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded backing metadata and stage-2 table-page ownership.

use core::ptr::write_bytes;

use alloc::vec::Vec;
use hyper::mm::allocator::heap::PageOwner;
use hyper::mm::{PAGE_SIZE, PhysicalAddress};

use super::Error;
use crate::hal::vm::Stage2AddressSpace;
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::mm::page_block::PageBlock;
use crate::kernel::mm::user_space::GuestMemoryBacking as SharedGuestMemory;

pub(super) enum GuestMemoryBacking {
    #[cfg(feature = "kernel-self-test")]
    KernelOwned(Vec<Option<PageBlock>>),
    SharedVmo(SharedGuestMemory),
}

/// Frozen membership of the resident pages covered by one batched instruction
/// publication.
///
/// The compact bitmap bounds transient metadata for large guests. Every
/// selected address is validated before entering the HAL; backing ownership
/// then keeps its physical page and permanent linear mapping stable until the
/// guest address space retires.
pub(crate) struct ResidentInstructionSnapshot {
    pub(super) pages: FixedBitmap,
    pub(super) _metadata_charge: CommittedCharge,
}

#[cfg(feature = "kernel-self-test")]
impl ResidentInstructionSnapshot {
    pub(crate) fn retained_metadata_bytes_for_test(&self) -> u64 {
        self.pages.retained_bytes() as u64
    }
}

/// Fixed-size bitmap whose exact word allocation is admitted before creation.
///
/// An explicit word representation avoids `Vec<bool>`'s implementation-defined
/// capacity rounding and makes retained kernel-byte accounting auditable.
pub(super) struct FixedBitmap {
    words: Vec<usize>,
    bit_count: usize,
}

impl FixedBitmap {
    pub(super) fn try_new(bit_count: usize) -> Result<Self, Error> {
        let word_count = bitmap_word_count(bit_count).ok_or(Error::MetadataAllocation)?;
        let mut words = try_exact_capacity_vec(word_count)?;
        words.resize(word_count, 0);
        Ok(Self { words, bit_count })
    }

    pub(super) const fn len(&self) -> usize {
        self.bit_count
    }

    pub(super) fn get(&self, index: usize) -> Option<bool> {
        if index >= self.bit_count {
            return None;
        }
        let word_bits = usize::BITS as usize;
        Some(self.words[index / word_bits] & (1usize << (index % word_bits)) != 0)
    }

    pub(super) fn set(&mut self, index: usize, value: bool) -> Result<(), Error> {
        if index >= self.bit_count {
            return Err(Error::InvalidRange);
        }
        let word_bits = usize::BITS as usize;
        let bit = 1usize << (index % word_bits);
        let word = &mut self.words[index / word_bits];
        if value {
            *word |= bit;
        } else {
            *word &= !bit;
        }
        Ok(())
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = bool> + '_ {
        (0..self.bit_count).map(|index| self.get(index).unwrap_or(false))
    }

    #[cfg(feature = "kernel-self-test")]
    pub(super) fn retained_bytes(&self) -> usize {
        self.words.capacity() * core::mem::size_of::<usize>()
    }
}

pub(super) struct AddressSpaceMetadataLayout {
    pub(super) page_count: usize,
    pub(super) table_capacity: usize,
    pub(super) bytes: u64,
}

pub(super) struct AdmittedAddressSpaceMetadata {
    pub(super) page_count: usize,
    pub(super) table_capacity: usize,
    pub(super) charge: CommittedCharge,
}
pub(super) struct Stage2PagePool {
    pub(super) pages: Vec<PageBlock>,
    domain: ResourceDomain,
    charge: Option<CommittedCharge>,
    error: Option<Error>,
}

impl Stage2PagePool {
    pub(super) fn with_capacity(capacity: usize, domain: &ResourceDomain) -> Result<Self, Error> {
        let pages = try_exact_capacity_vec(capacity)?;
        Ok(Self {
            pages,
            domain: domain.clone(),
            charge: None,
            error: None,
        })
    }

    pub(super) fn allocate_zeroed(
        &mut self,
        pages: usize,
        alignment_pages: usize,
    ) -> Option<PhysicalAddress> {
        if pages == 0
            || !pages.is_power_of_two()
            || alignment_pages != pages
            || self.pages.len() == self.pages.capacity()
        {
            return None;
        }
        let byte_count = pages.checked_mul(PAGE_SIZE as usize)?;
        let page_count = u64::try_from(pages).ok()?;
        let byte_count_u64 = u64::try_from(byte_count).ok()?;
        let reservation = match self.domain.reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelMemoryBytes, byte_count_u64)
                .with(ResourceKind::CommittedPages, page_count)
                .with(ResourceKind::PinnedPages, page_count),
        ) {
            Ok(reservation) => reservation,
            Err(error) => {
                self.error = Some(Error::Resource(error));
                return None;
            }
        };
        let order = pages.trailing_zeros() as usize;
        let page = match PageBlock::allocate_for(order, PageOwner::PageTable) {
            Ok(page) => page,
            Err(error) => {
                self.error = Some(Error::Allocation(error));
                return None;
            }
        };
        let physical = page.physical();
        let virtual_address = match linear_address(physical) {
            Ok(address) => address,
            Err(error) => {
                self.error = Some(error);
                return None;
            }
        };
        // SAFETY: This new table page is exclusive and permanently mapped.
        unsafe { write_bytes(virtual_address as *mut u8, 0, byte_count) };
        self.pages.push(page);
        accumulate_charge(&mut self.charge, reservation.commit());
        Some(physical)
    }

    pub(super) fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }
}

pub(super) fn accumulate_charge(owner: &mut Option<CommittedCharge>, charge: CommittedCharge) {
    match owner {
        Some(owner) => owner.absorb_pre_admitted(charge),
        None => *owner = Some(charge),
    }
}

pub(super) fn address_space_metadata_layout(
    ipa_base: u64,
    size: u64,
    page_count: usize,
    backing_owner_bytes: usize,
) -> Result<AddressSpaceMetadataLayout, Error> {
    let table_capacity = Stage2AddressSpace::required_table_pages(ipa_base, size)?;
    let bitmap_bytes = bitmap_storage_bytes(page_count)
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(Error::MetadataAllocation)?;
    let table_owner_bytes = table_capacity
        .checked_mul(core::mem::size_of::<PageBlock>())
        .ok_or(Error::MetadataAllocation)?;
    let bytes = bitmap_bytes
        .checked_add(table_owner_bytes)
        .and_then(|bytes| bytes.checked_add(backing_owner_bytes))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(Error::MetadataAllocation)?;
    Ok(AddressSpaceMetadataLayout {
        page_count,
        table_capacity,
        bytes,
    })
}

pub(super) fn admit_metadata(
    domain: &ResourceDomain,
    layout: AddressSpaceMetadataLayout,
) -> Result<AdmittedAddressSpaceMetadata, Error> {
    let charge = domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, layout.bytes))?
        .commit();
    Ok(AdmittedAddressSpaceMetadata {
        page_count: layout.page_count,
        table_capacity: layout.table_capacity,
        charge,
    })
}

/// Returns the allocator payload required by a [`FixedBitmap`].
///
/// The final partial machine word remains fully retained and charged.
pub(super) fn bitmap_storage_bytes(bit_count: usize) -> Option<usize> {
    bitmap_word_count(bit_count).and_then(|words| words.checked_mul(core::mem::size_of::<usize>()))
}

fn bitmap_word_count(bit_count: usize) -> Option<usize> {
    let word_bits = usize::BITS as usize;
    bit_count
        .checked_add(word_bits - 1)
        .map(|bits| bits / word_bits)
}

/// Allocates a Vec whose retained allocator request is exactly the admitted
/// element payload.
///
/// `HypeR` charges requested heap bytes rather than the allocator's internal
/// slab/page rounding. Starting from an empty Vec and using `reserve_exact`
/// should retain precisely this capacity with the pinned Rust toolchain. The
/// explicit check turns any future library growth-policy change into a
/// failure before the storage is published under an undersized charge.
pub(super) fn try_exact_capacity_vec<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| Error::MetadataAllocation)?;
    if values.capacity() != capacity {
        return Err(Error::MetadataAllocation);
    }
    Ok(values)
}

pub(super) fn validate_region(ipa_base: u64, size: u64) -> Result<usize, Error> {
    if size == 0 || ipa_base & (PAGE_SIZE - 1) != 0 || size & (PAGE_SIZE - 1) != 0 {
        return Err(Error::InvalidRange);
    }
    ipa_base.checked_add(size).ok_or(Error::AddressOverflow)?;
    usize::try_from(size / PAGE_SIZE).map_err(|_| Error::AddressOverflow)
}

pub(super) fn linear_address(physical: PhysicalAddress) -> Result<usize, Error> {
    crate::kernel::mm::memory::linear_address(physical.get()).ok_or(Error::InvalidRange)
}
