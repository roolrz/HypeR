use core::ptr::{read_volatile, write_volatile};

use hyper::hal::memory::AddressTranslation;
use hyper::mm::{PAGE_SIZE, PhysicalAddress};

use super::memory::X86_64AddressTranslation;
use super::virtualization::Stage2Format;

const LEVEL_SHIFTS: [u64; 4] = [39, 30, 21, 12];
const EPT_READ: u64 = 1;
const EPT_WRITE: u64 = 1 << 1;
const EPT_EXECUTE: u64 = 1 << 2;
const EPT_MEMORY_WB: u64 = 6 << 3;
const EPT_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const NPT_PRESENT: u64 = 1;
const NPT_WRITE: u64 = 1 << 1;
const NPT_USER: u64 = 1 << 2;
const GUEST_LIMIT: u64 = 1 << 48;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation,
    BackendUnavailable,
    Conflict,
    InvalidAddress,
    InvalidRange,
    InvalidVmid,
    Invalidation,
}

pub struct Stage2AddressSpace {
    root: PhysicalAddress,
    backend: super::virtualization::Backend,
}

impl Stage2AddressSpace {
    pub fn required_table_pages(ipa: u64, size: u64) -> Result<usize, Error> {
        validate_range(ipa, ipa, size)?;
        let pages = size.div_ceil(1 << 21);
        usize::try_from(pages)
            .ok()
            .and_then(|count| count.checked_add(4))
            .ok_or(Error::AddressOverflow)
    }

    pub fn new(
        vmid: u16,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<Self, Error> {
        if vmid == 0 {
            return Err(Error::InvalidVmid);
        }
        let root = allocator(1, 1).ok_or(Error::Allocation)?;
        let backend = super::virtualization::selected().ok_or(Error::BackendUnavailable)?;
        Ok(Self { root, backend })
    }

    pub const fn root_address(&self) -> u64 {
        self.root.get()
    }

    pub fn map_normal_page(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        validate_range(ipa, physical, PAGE_SIZE)?;
        self.map_page(ipa, physical, allocator)
    }

    pub unsafe fn map_normal_page_active(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        self.map_normal_page(ipa, physical, allocator)?;
        self.invalidate()
    }

    pub unsafe fn invalidate_page_active(&self, ipa: u64) -> Result<(), Error> {
        if !ipa.is_multiple_of(PAGE_SIZE) || ipa >= GUEST_LIMIT {
            return Err(Error::InvalidAddress);
        }
        self.invalidate()
    }

    pub unsafe fn activate(&self) {
        self.backend.activate_stage2(self.root.get());
    }

    fn map_page(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        let mut table = self.root;
        for level in 0..3 {
            let slot = index(ipa, level);
            let entry = read_entry(table, slot)?;
            table = if entry == 0 {
                let child = allocator(1, 1).ok_or(Error::Allocation)?;
                write_entry(table, slot, child.get() | self.table_flags())?;
                child
            } else if entry & self.present_flags() != 0 {
                PhysicalAddress::new(entry & EPT_ADDRESS_MASK)
            } else {
                return Err(Error::Conflict);
            };
        }
        let slot = index(ipa, 3);
        let value = physical | self.leaf_flags();
        let existing = read_entry(table, slot)?;
        if existing != 0 && existing != value {
            return Err(Error::Conflict);
        }
        write_entry(table, slot, value)
    }

    fn table_flags(&self) -> u64 {
        match self.backend.stage2_format() {
            Stage2Format::Ept => EPT_READ | EPT_WRITE | EPT_EXECUTE,
            Stage2Format::Npt => NPT_PRESENT | NPT_WRITE | NPT_USER,
        }
    }

    fn leaf_flags(&self) -> u64 {
        self.table_flags()
            | match self.backend.stage2_format() {
                Stage2Format::Ept => EPT_MEMORY_WB,
                Stage2Format::Npt => 0,
            }
    }

    fn present_flags(&self) -> u64 {
        match self.backend.stage2_format() {
            Stage2Format::Ept => EPT_READ | EPT_WRITE | EPT_EXECUTE,
            Stage2Format::Npt => NPT_PRESENT,
        }
    }

    fn invalidate(&self) -> Result<(), Error> {
        self.backend
            .invalidate_stage2(self.root.get())
            .map_err(|_| Error::Invalidation)
    }
}

fn validate_range(ipa: u64, physical: u64, size: u64) -> Result<(), Error> {
    if size == 0
        || !ipa.is_multiple_of(PAGE_SIZE)
        || !physical.is_multiple_of(PAGE_SIZE)
        || !size.is_multiple_of(PAGE_SIZE)
    {
        return Err(Error::InvalidRange);
    }
    let end = ipa.checked_add(size).ok_or(Error::AddressOverflow)?;
    let physical_end = physical.checked_add(size).ok_or(Error::AddressOverflow)?;
    if end > GUEST_LIMIT || physical_end > super::registers::PHYSICAL_ADDRESS_LIMIT {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

fn index(address: u64, level: usize) -> usize {
    ((address >> LEVEL_SHIFTS[level]) & 511) as usize
}

fn table_pointer(table: PhysicalAddress) -> Result<usize, Error> {
    X86_64AddressTranslation::linear_address(table)
        .and_then(|address| usize::try_from(address.get()).ok())
        .ok_or(Error::InvalidAddress)
}

fn read_entry(table: PhysicalAddress, slot: usize) -> Result<u64, Error> {
    Ok(unsafe { read_volatile((table_pointer(table)? as *const u64).add(slot)) })
}

fn write_entry(table: PhysicalAddress, slot: usize, value: u64) -> Result<(), Error> {
    unsafe { write_volatile((table_pointer(table)? as *mut u64).add(slot), value) };
    Ok(())
}
