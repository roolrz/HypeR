// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest-physical memory ownership and stage-2 demand paging.

use core::ptr::{copy_nonoverlapping, write_bytes};

use alloc::vec::Vec;
use hyper::hal::cache::CacheMaintenance;
use hyper::mm::allocator::heap::PageOwner;
use hyper::mm::{
    BuddyError, ForeignCopyError, ForeignMemory, PAGE_SIZE, PhysicalAddress, copy_from_foreign,
    copy_to_foreign,
};
use hyper::vm::exit::{GuestMemoryFault, MemoryAccess};

use super::registry::{HardwareVmid, VmId};
use crate::arch::vm::{Stage2AddressSpace, Stage2Error};
use crate::kernel::mm::page_block::PageBlock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation(BuddyError),
    Cache(hyper::hal::cache::CacheError),
    InvalidRange,
    MetadataAllocation,
    Registry(super::registry::Error),
    Stage2(Stage2Error),
}

impl From<BuddyError> for Error {
    fn from(error: BuddyError) -> Self {
        Self::Allocation(error)
    }
}

impl From<Stage2Error> for Error {
    fn from(error: Stage2Error) -> Self {
        Self::Stage2(error)
    }
}

impl From<hyper::hal::cache::CacheError> for Error {
    fn from(error: hyper::hal::cache::CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<super::registry::Error> for Error {
    fn from(error: super::registry::Error) -> Self {
        Self::Registry(error)
    }
}

impl From<ForeignCopyError<Error>> for Error {
    fn from(error: ForeignCopyError<Error>) -> Self {
        match error {
            ForeignCopyError::AddressOverflow => Self::AddressOverflow,
            ForeignCopyError::Backend(error) => error,
            ForeignCopyError::InvalidPageSize | ForeignCopyError::InvalidRange => {
                Self::InvalidRange
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestMemoryStats {
    pub addressable_pages: usize,
    pub committed_pages: usize,
    pub boot_committed_pages: usize,
    pub demand_faults: u64,
    pub read_faults: u64,
    pub write_faults: u64,
    pub execute_faults: u64,
    pub page_walk_faults: u64,
    pub repeated_faults: u64,
    pub failed_faults: u64,
}

/// VM-owned sparse guest RAM and its architecture stage-2 hierarchy.
pub(crate) struct GuestAddressSpace {
    ipa_base: u64,
    size: u64,
    pages: Vec<Option<PageBlock>>,
    committed_pages: usize,
    boot_committed_pages: usize,
    demand_faults: u64,
    read_faults: u64,
    write_faults: u64,
    execute_faults: u64,
    page_walk_faults: u64,
    repeated_faults: u64,
    failed_faults: u64,
    stage2: Stage2AddressSpace,
    table_pages: Stage2PagePool,
}

impl GuestAddressSpace {
    pub(crate) fn new(
        hardware_vmid: HardwareVmid,
        ipa_base: u64,
        size: u64,
    ) -> Result<Self, Error> {
        let page_count = validate_region(ipa_base, size)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(page_count)
            .map_err(|_| Error::MetadataAllocation)?;
        pages.resize_with(page_count, || None);

        let table_capacity = Stage2AddressSpace::required_table_pages(ipa_base, size)?;
        let mut table_pages = Stage2PagePool::with_capacity(table_capacity)?;
        let mut allocate_table = |pages, alignment| table_pages.allocate_zeroed(pages, alignment);
        // SAFETY: Stage2PagePool returns uniquely owned, zeroed, naturally
        // aligned PageBlocks from writable RAM in the permanent linear map.
        // `table_pages` retains every block for at least as long as `stage2`,
        // and this not-yet-published address space has exclusive mutation.
        let stage2 = unsafe { Stage2AddressSpace::new(hardware_vmid.get(), &mut allocate_table)? };
        Ok(Self {
            ipa_base,
            size,
            pages,
            committed_pages: 0,
            boot_committed_pages: 0,
            demand_faults: 0,
            read_faults: 0,
            write_faults: 0,
            execute_faults: 0,
            page_walk_faults: 0,
            repeated_faults: 0,
            failed_faults: 0,
            stage2,
            table_pages,
        })
    }

    pub fn copy_from(&mut self, ipa: u64, destination: &mut [u8]) -> Result<(), Error> {
        copy_from_foreign(self, ipa, destination).map_err(Error::from)
    }

    pub fn copy_to(&mut self, ipa: u64, source: &[u8]) -> Result<(), Error> {
        copy_to_foreign(self, ipa, source).map_err(Error::from)
    }

    pub fn finish_boot_loading(&mut self) {
        self.boot_committed_pages = self.committed_pages;
    }

    pub fn publish_instruction(&self, ipa: u64, length: usize) -> Result<(), Error> {
        self.publish_range(ipa, length, true)
    }

    pub fn publish_data(&self, ipa: u64, length: usize) -> Result<(), Error> {
        self.publish_range(ipa, length, false)
    }

    pub const fn root_address(&self) -> u64 {
        self.stage2.root_address()
    }

    pub fn statistics(&self) -> GuestMemoryStats {
        GuestMemoryStats {
            addressable_pages: self.pages.len(),
            committed_pages: self.committed_pages,
            boot_committed_pages: self.boot_committed_pages,
            demand_faults: self.demand_faults,
            read_faults: self.read_faults,
            write_faults: self.write_faults,
            execute_faults: self.execute_faults,
            page_walk_faults: self.page_walk_faults,
            repeated_faults: self.repeated_faults,
            failed_faults: self.failed_faults,
        }
    }

    fn resolve_guest_memory_fault(&mut self, fault: GuestMemoryFault) -> Result<bool, Error> {
        let Some(page_index) = self.page_index(fault.address().get()) else {
            return Ok(false);
        };
        self.demand_faults = self.demand_faults.saturating_add(1);
        match fault.access() {
            MemoryAccess::Read => self.read_faults = self.read_faults.saturating_add(1),
            MemoryAccess::Write => self.write_faults = self.write_faults.saturating_add(1),
            MemoryAccess::Execute => self.execute_faults = self.execute_faults.saturating_add(1),
        }
        if fault.during_guest_page_walk() {
            self.page_walk_faults = self.page_walk_faults.saturating_add(1);
        }
        if self.pages[page_index].is_some() {
            self.repeated_faults = self.repeated_faults.saturating_add(1);
            let ipa = self.page_ipa(page_index)?;
            // SAFETY: Fault dispatch proves this VM's stage-2 is active on the
            // current CPU, and the address-space lock serializes invalidation.
            unsafe { self.stage2.invalidate_page_active(ipa)? };
            return Ok(true);
        }
        self.commit_page(page_index, true).inspect_err(|_| {
            self.failed_faults = self.failed_faults.saturating_add(1);
        })?;
        Ok(true)
    }

    fn commit_page(&mut self, page_index: usize, active: bool) -> Result<(), Error> {
        if self.pages.get(page_index).is_none() {
            return Err(Error::InvalidRange);
        }
        if self.pages[page_index].is_some() {
            return Ok(());
        }
        let page = PageBlock::allocate_for(0, PageOwner::Guest)?;
        let physical = page.physical();
        let virtual_address = linear_address(physical)?;
        // SAFETY: The new page is exclusively VM-owned and fully covered by
        // the permanent writable linear map.
        unsafe { write_bytes(virtual_address as *mut u8, 0, PAGE_SIZE as usize) };
        let ipa = self.page_ipa(page_index)?;
        let mut allocate_table =
            |pages, alignment| self.table_pages.allocate_zeroed(pages, alignment);
        if active {
            // SAFETY: Called only from a lower-EL translation fault while this
            // VM is active, under the address-space lock.
            unsafe {
                self.stage2
                    .map_normal_page_active(ipa, physical.get(), &mut allocate_table)?
            };
        } else {
            // SAFETY: Stage2PagePool preserves the allocation contract stated
            // at construction, and &mut self plus the owning VM's
            // address-space lock serializes all hierarchy mutation.
            unsafe {
                self.stage2
                    .map_normal_page(ipa, physical.get(), &mut allocate_table)?
            };
        }
        self.pages[page_index] = Some(page);
        self.committed_pages += 1;
        Ok(())
    }

    fn validate_access(&self, ipa: u64, length: usize) -> Result<usize, Error> {
        let offset = ipa.checked_sub(self.ipa_base).ok_or(Error::InvalidRange)?;
        let length = u64::try_from(length).map_err(|_| Error::AddressOverflow)?;
        let end = offset.checked_add(length).ok_or(Error::AddressOverflow)?;
        if end > self.size {
            return Err(Error::InvalidRange);
        }
        usize::try_from(offset).map_err(|_| Error::AddressOverflow)
    }

    fn publish_range(&self, ipa: u64, length: usize, instruction: bool) -> Result<(), Error> {
        let mut offset = self.validate_access(ipa, length)?;
        let mut published = 0;
        while published < length {
            let page_index = offset / PAGE_SIZE as usize;
            let page_offset = offset % PAGE_SIZE as usize;
            let page = self.pages[page_index].as_ref().ok_or(Error::InvalidRange)?;
            let address = linear_address(page.physical())?
                .checked_add(page_offset)
                .ok_or(Error::AddressOverflow)?;
            let chunk = (PAGE_SIZE as usize - page_offset).min(length - published);
            // SAFETY: Loading owns the VM and no vCPU can observe these pages
            // until the stage-2 hierarchy is installed and activated.
            unsafe {
                if instruction {
                    crate::arch::memory::Cache::publish_instruction_range(address, chunk)
                } else {
                    crate::arch::memory::Cache::publish_data_range(address, chunk)
                }
            }
            .map_err(Error::from)?;
            published += chunk;
            offset += chunk;
        }
        Ok(())
    }

    fn page_index(&self, address: u64) -> Option<usize> {
        let offset = address.checked_sub(self.ipa_base)?;
        if offset >= self.size {
            return None;
        }
        usize::try_from(offset / PAGE_SIZE).ok()
    }

    fn page_ipa(&self, page_index: usize) -> Result<u64, Error> {
        let offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .ok_or(Error::AddressOverflow)?;
        self.ipa_base
            .checked_add(offset)
            .ok_or(Error::AddressOverflow)
    }
}

impl ForeignMemory for GuestAddressSpace {
    type Error = Error;

    fn address_base(&self) -> u64 {
        self.ipa_base
    }

    fn address_size(&self) -> u64 {
        self.size
    }

    fn page_size(&self) -> usize {
        PAGE_SIZE as usize
    }

    fn read_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let Some(page) = self.pages.get(page_index).ok_or(Error::InvalidRange)? else {
            destination.fill(0);
            return Ok(());
        };
        let source = linear_address(page.physical())?
            .checked_add(page_offset)
            .ok_or(Error::AddressOverflow)?;
        // SAFETY: The generic copy layer bounds this chunk to one VM-owned
        // page, whose mapping remains stable under the address-space lock.
        unsafe {
            copy_nonoverlapping(
                source as *const u8,
                destination.as_mut_ptr(),
                destination.len(),
            )
        };
        Ok(())
    }

    fn write_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        self.commit_page(page_index, false)?;
        let page = self.pages[page_index].as_ref().ok_or(Error::InvalidRange)?;
        let destination = linear_address(page.physical())?
            .checked_add(page_offset)
            .ok_or(Error::AddressOverflow)?;
        // SAFETY: The generic copy layer bounds this chunk to one VM-owned
        // page, whose mapping remains stable under the address-space lock.
        unsafe { copy_nonoverlapping(source.as_ptr(), destination as *mut u8, source.len()) };
        Ok(())
    }
}

impl crate::arch::guest::PayloadMemory for GuestAddressSpace {
    type Error = Error;

    fn copy_to(
        &mut self,
        address: hyper::vm::exit::GuestPhysicalAddress,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        GuestAddressSpace::copy_to(self, address.get(), bytes)
    }

    fn publish_instruction(
        &self,
        address: hyper::vm::exit::GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error> {
        GuestAddressSpace::publish_instruction(self, address.get(), length)
    }

    fn publish_data(
        &self,
        address: hyper::vm::exit::GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error> {
        GuestAddressSpace::publish_data(self, address.get(), length)
    }
}

/// Activates the installed VM's stage-2 hierarchy on the current CPU.
///
/// # Safety
///
/// The caller must own the stopped vCPU carrying `vm`.
pub(in crate::kernel) unsafe fn activate(vm: &super::registry::VmBinding) -> Result<(), Error> {
    vm.with_address_space(|address_space| {
        // SAFETY: The caller owns the stopped vCPU, and the installed address
        // space is pinned in the VM registry for the active guest lifetime.
        unsafe { address_space.stage2.activate() };
    });
    Ok(())
}

pub(in crate::kernel) fn resolve_guest_memory_fault(
    vm: &super::registry::VmBinding,
    fault: GuestMemoryFault,
) -> Result<bool, Error> {
    vm.with_address_space(|address_space| address_space.resolve_guest_memory_fault(fault))
}

/// Copies guest bytes through VM ownership metadata, never by treating an IPA
/// as a host pointer. Uncommitted demand-zero pages read as zero without being
/// allocated.
pub fn copy_from_guest(
    virtual_machine: VmId,
    source_ipa: u64,
    destination: &mut [u8],
) -> Result<(), Error> {
    super::registry::with_address_space(virtual_machine, |address_space| {
        address_space.copy_from(source_ipa, destination)
    })?
}

/// Copies kernel bytes into guest RAM, committing demand-zero pages as needed.
/// Callers needing a coherent snapshot must stop or otherwise serialize the
/// target vCPU while the copy is in progress.
pub fn copy_to_guest(
    virtual_machine: VmId,
    destination_ipa: u64,
    source: &[u8],
) -> Result<(), Error> {
    super::registry::with_address_space(virtual_machine, |address_space| {
        address_space.copy_to(destination_ipa, source)
    })?
}

pub fn statistics(virtual_machine: VmId) -> Option<GuestMemoryStats> {
    super::registry::with_address_space(virtual_machine, |address_space| address_space.statistics())
        .ok()
}

struct Stage2PagePool {
    pages: Vec<PageBlock>,
}

impl Stage2PagePool {
    fn with_capacity(capacity: usize) -> Result<Self, Error> {
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(capacity)
            .map_err(|_| Error::MetadataAllocation)?;
        Ok(Self { pages })
    }

    fn allocate_zeroed(&mut self, pages: usize, alignment_pages: usize) -> Option<PhysicalAddress> {
        if pages == 0
            || !pages.is_power_of_two()
            || alignment_pages != pages
            || self.pages.len() == self.pages.capacity()
        {
            return None;
        }
        let byte_count = pages.checked_mul(PAGE_SIZE as usize)?;
        let order = pages.trailing_zeros() as usize;
        let page = PageBlock::allocate_for(order, PageOwner::PageTable).ok()?;
        let physical = page.physical();
        let virtual_address = linear_address(physical).ok()?;
        // SAFETY: This new table page is exclusive and permanently mapped.
        unsafe { write_bytes(virtual_address as *mut u8, 0, byte_count) };
        self.pages.push(page);
        Some(physical)
    }
}

fn validate_region(ipa_base: u64, size: u64) -> Result<usize, Error> {
    if size == 0 || ipa_base & (PAGE_SIZE - 1) != 0 || size & (PAGE_SIZE - 1) != 0 {
        return Err(Error::InvalidRange);
    }
    ipa_base.checked_add(size).ok_or(Error::AddressOverflow)?;
    usize::try_from(size / PAGE_SIZE).map_err(|_| Error::AddressOverflow)
}

fn linear_address(physical: PhysicalAddress) -> Result<usize, Error> {
    crate::kernel::mm::memory::linear_address(physical.get()).ok_or(Error::InvalidRange)
}
