//! Guest-physical memory ownership and stage-2 demand paging.

use core::ptr::{copy_nonoverlapping, write_bytes};

use alloc::vec::Vec;
use hyper::hal::cache::CacheMaintenance;
use hyper::mm::heap::PageOwner;
use hyper::mm::{BuddyError, PAGE_SIZE, PhysicalAddress};
use hyper::sync::InterruptSpinLock;

use crate::arch::{GuestMemoryAccess, GuestTranslationFault, Stage2AddressSpace, Stage2Error};
use crate::kernel::mm::page_block::PageBlock;
use crate::kernel::task::thread::VirtualMachineId;

type AddressSpaceLock = InterruptSpinLock<Vec<GuestAddressSpace>, crate::arch::LocalInterruptMask>;

static ADDRESS_SPACES: AddressSpaceLock = InterruptSpinLock::new(Vec::new());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation(BuddyError),
    Cache(hyper::hal::cache::CacheError),
    DuplicateVirtualMachine,
    InvalidRange,
    InvalidVmid,
    MetadataAllocation,
    NotInstalled,
    Stage2(Stage2Error),
    WrongVirtualMachine,
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
    virtual_machine: VirtualMachineId,
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
    pub fn new(virtual_machine: VirtualMachineId, ipa_base: u64, size: u64) -> Result<Self, Error> {
        let page_count = validate_region(ipa_base, size)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(page_count)
            .map_err(|_| Error::MetadataAllocation)?;
        pages.resize_with(page_count, || None);

        let table_capacity = Stage2AddressSpace::required_table_pages(ipa_base, size)?;
        let mut table_pages = Stage2PagePool::with_capacity(table_capacity)?;
        let mut allocate_table = || table_pages.allocate_zeroed();
        let vmid = u16::try_from(virtual_machine.0).map_err(|_| Error::InvalidVmid)?;
        let stage2 = Stage2AddressSpace::new(vmid, &mut allocate_table)?;
        Ok(Self {
            virtual_machine,
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

    pub fn write(&mut self, ipa: u64, source: &[u8]) -> Result<(), Error> {
        let mut offset = self.validate_access(ipa, source.len())?;
        let mut copied = 0;
        while copied < source.len() {
            let page_index = offset / PAGE_SIZE as usize;
            let page_offset = offset % PAGE_SIZE as usize;
            self.commit_page(page_index, false)?;
            let page = self.pages[page_index].as_ref().ok_or(Error::InvalidRange)?;
            let destination = linear_address(page.physical())?
                .checked_add(page_offset)
                .ok_or(Error::AddressOverflow)?;
            let length = (PAGE_SIZE as usize - page_offset).min(source.len() - copied);
            // SAFETY: The page is exclusively owned by this VM, and the
            // validated chunk lies entirely inside that page.
            unsafe {
                copy_nonoverlapping(source.as_ptr().add(copied), destination as *mut u8, length)
            };
            copied += length;
            offset += length;
        }
        Ok(())
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

    fn resolve_translation_fault(&mut self, fault: GuestTranslationFault) -> Result<bool, Error> {
        let Some(page_index) = self.page_index(fault.address) else {
            return Ok(false);
        };
        self.demand_faults = self.demand_faults.saturating_add(1);
        match fault.access {
            GuestMemoryAccess::Read => self.read_faults = self.read_faults.saturating_add(1),
            GuestMemoryAccess::Write => self.write_faults = self.write_faults.saturating_add(1),
            GuestMemoryAccess::Execute => {
                self.execute_faults = self.execute_faults.saturating_add(1)
            }
        }
        if fault.during_page_walk {
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
        if let Err(error) = self.commit_page(page_index, true) {
            self.failed_faults = self.failed_faults.saturating_add(1);
            return Err(error);
        }
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
        let mut allocate_table = || self.table_pages.allocate_zeroed();
        if active {
            // SAFETY: Called only from a lower-EL translation fault while this
            // VM is active, under the address-space lock.
            unsafe {
                self.stage2
                    .map_normal_page_active(ipa, physical.get(), &mut allocate_table)?
            };
        } else {
            self.stage2
                .map_normal_page(ipa, physical.get(), &mut allocate_table)?;
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
                    crate::arch::ArchitectureCache::publish_instruction_range(address, chunk)
                } else {
                    crate::arch::ArchitectureCache::publish_data_range(address, chunk)
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

pub(crate) fn install(address_space: GuestAddressSpace) -> Result<(), Error> {
    ADDRESS_SPACES.with(|address_spaces| {
        if address_spaces
            .iter()
            .any(|existing| existing.virtual_machine == address_space.virtual_machine)
        {
            return Err(Error::DuplicateVirtualMachine);
        }
        address_spaces
            .try_reserve(1)
            .map_err(|_| Error::MetadataAllocation)?;
        address_spaces.push(address_space);
        Ok(())
    })
}

/// Activates the installed VM's stage-2 hierarchy on the current CPU.
///
/// # Safety
///
/// The caller must own a stopped vCPU belonging to `virtual_machine`.
pub(crate) unsafe fn activate(virtual_machine: VirtualMachineId) -> Result<(), Error> {
    ADDRESS_SPACES.with(|address_spaces| {
        let address_space = find(address_spaces, virtual_machine)?;
        unsafe { address_space.stage2.activate() };
        Ok(())
    })
}

pub(crate) fn resolve_translation_fault(
    virtual_machine: VirtualMachineId,
    fault: GuestTranslationFault,
) -> Result<bool, Error> {
    ADDRESS_SPACES.with(|address_spaces| {
        let address_space = find_mut(address_spaces, virtual_machine)?;
        address_space.resolve_translation_fault(fault)
    })
}

pub fn statistics(virtual_machine: VirtualMachineId) -> Option<GuestMemoryStats> {
    ADDRESS_SPACES.with(|address_spaces| {
        address_spaces
            .iter()
            .find(|space| space.virtual_machine == virtual_machine)
            .map(GuestAddressSpace::statistics)
    })
}

fn find(
    address_spaces: &[GuestAddressSpace],
    virtual_machine: VirtualMachineId,
) -> Result<&GuestAddressSpace, Error> {
    address_spaces
        .iter()
        .find(|space| space.virtual_machine == virtual_machine)
        .ok_or(Error::WrongVirtualMachine)
}

fn find_mut(
    address_spaces: &mut [GuestAddressSpace],
    virtual_machine: VirtualMachineId,
) -> Result<&mut GuestAddressSpace, Error> {
    address_spaces
        .iter_mut()
        .find(|space| space.virtual_machine == virtual_machine)
        .ok_or(Error::WrongVirtualMachine)
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

    fn allocate_zeroed(&mut self) -> Option<PhysicalAddress> {
        let page = PageBlock::allocate_for(0, PageOwner::PageTable).ok()?;
        let physical = page.physical();
        let virtual_address = linear_address(physical).ok()?;
        // SAFETY: This new table page is exclusive and permanently mapped.
        unsafe { write_bytes(virtual_address as *mut u8, 0, PAGE_SIZE as usize) };
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
