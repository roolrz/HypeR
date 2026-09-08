// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::ptr::{read_volatile, write_volatile};

use hyper::hal::memory::AddressTranslation;
use hyper::mm::{PAGE_SIZE, PhysicalAddress};
use hyper::vm::translation::{ActiveMappingError, Stage2PagePermissions, publish_active_mapping};

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
const NPT_NO_EXECUTE: u64 = 1 << 63;
const GUEST_LIMIT: u64 = 1 << 48;

const _: () = {
    assert!(ept_execute_flags(Stage2PagePermissions::ReadWrite) == 0);
    assert!(ept_execute_flags(Stage2PagePermissions::ReadWriteExecute) == EPT_EXECUTE);
    assert!(npt_execute_flags(Stage2PagePermissions::ReadWrite) == NPT_NO_EXECUTE);
    assert!(npt_execute_flags(Stage2PagePermissions::ReadWriteExecute) == 0);
};
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

    /// Creates a guest address space backed by pages returned by `allocator`.
    ///
    /// # Safety
    ///
    /// Every successful allocation must return a uniquely owned, zeroed,
    /// page-aligned RAM page. Those pages must remain live and accessible
    /// through the kernel linear mapping for the lifetime of this address
    /// space.
    pub unsafe fn new(
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

    /// Maps one normal guest page.
    ///
    /// # Safety
    ///
    /// The address-space pages supplied to `new` and any pages returned by
    /// `allocator` must satisfy `new`'s ownership and accessibility contract.
    /// The caller must also serialize page-table mutation.
    pub unsafe fn map_normal_page(
        &mut self,
        ipa: u64,
        physical: u64,
        permissions: Stage2PagePermissions,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        validate_range(ipa, physical, PAGE_SIZE)?;
        // SAFETY: The method contract covers allocator pages and serialized mutation.
        unsafe { self.map_page(ipa, physical, permissions, allocator) }
    }

    /// Installs and publishes one mapping in the active EPT or NPT hierarchy.
    ///
    /// # Safety
    ///
    /// In addition to [`Self::map_normal_page`]'s requirements, the caller
    /// must guarantee that no sibling vCPU can retain translations for this
    /// address space. The current kernel admission policy enforces one vCPU
    /// per VM. Supporting concurrent vCPUs requires residency tracking and a
    /// synchronous remote INVEPT or NPT shootdown before relaxing that policy.
    pub unsafe fn map_normal_page_active(
        &mut self,
        ipa: u64,
        physical: u64,
        permissions: Stage2PagePermissions,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), ActiveMappingError<Error>> {
        publish_active_mapping(
            self,
            |stage2| {
                // SAFETY: This function has the same allocator and
                // serialization contract.
                unsafe { stage2.map_normal_page(ipa, physical, permissions, allocator) }
            },
            Stage2AddressSpace::invalidate,
        )
    }

    /// Grants execute permission to an inactive normal-memory leaf.
    pub fn make_normal_page_executable(&mut self, ipa: u64) -> Result<(), Error> {
        self.install_execute_permission(ipa)
    }

    /// Grants execute permission and invalidates the active EPT/NPT context.
    ///
    /// # Safety
    ///
    /// The active address space must satisfy the single-running-vCPU contract
    /// documented by [`Self::map_normal_page_active`].
    pub unsafe fn make_normal_page_executable_active(
        &mut self,
        ipa: u64,
    ) -> Result<(), ActiveMappingError<Error>> {
        publish_active_mapping(
            self,
            |stage2| stage2.install_execute_permission(ipa),
            Stage2AddressSpace::invalidate,
        )
    }

    /// Invalidates a page after a repeated fault in the active hierarchy.
    ///
    /// # Safety
    ///
    /// The active address space must satisfy the single-running-vCPU contract
    /// documented by [`Self::map_normal_page_active`].
    pub unsafe fn invalidate_page_active(&self, ipa: u64) -> Result<(), Error> {
        if !ipa.is_multiple_of(PAGE_SIZE) || ipa >= GUEST_LIMIT {
            return Err(Error::InvalidAddress);
        }
        self.invalidate()
    }

    pub unsafe fn activate(&self) {
        self.backend.activate_stage2(self.root.get());
    }

    unsafe fn map_page(
        &mut self,
        ipa: u64,
        physical: u64,
        permissions: Stage2PagePermissions,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        let mut table = self.root;
        for level in 0..3 {
            let slot = index(ipa, level);
            // SAFETY: The method contract guarantees every walked table is live and mapped.
            let entry = unsafe { read_entry(table, slot)? };
            table = if entry == 0 {
                let child = allocator(1, 1).ok_or(Error::Allocation)?;
                // SAFETY: The allocator contract provides a fresh live child table.
                unsafe { write_entry(table, slot, child.get() | self.table_flags())? };
                child
            } else if entry & self.present_flags() != 0 {
                PhysicalAddress::new(entry & EPT_ADDRESS_MASK)
            } else {
                return Err(Error::Conflict);
            };
        }
        let slot = index(ipa, 3);
        let value = physical | self.leaf_flags(permissions);
        // SAFETY: The walk established a live mapped leaf table.
        let existing = unsafe { read_entry(table, slot)? };
        if existing != 0 && existing != value {
            return Err(Error::Conflict);
        }
        // SAFETY: Mutation is serialized and the leaf table is live and mapped.
        unsafe { write_entry(table, slot, value) }
    }

    fn table_flags(&self) -> u64 {
        match self.backend.stage2_format() {
            Stage2Format::Ept => EPT_READ | EPT_WRITE | EPT_EXECUTE,
            Stage2Format::Npt => NPT_PRESENT | NPT_WRITE | NPT_USER,
        }
    }

    fn leaf_flags(&self, permissions: Stage2PagePermissions) -> u64 {
        match self.backend.stage2_format() {
            Stage2Format::Ept => {
                EPT_READ | EPT_WRITE | EPT_MEMORY_WB | ept_execute_flags(permissions)
            }
            Stage2Format::Npt => {
                NPT_PRESENT | NPT_WRITE | NPT_USER | npt_execute_flags(permissions)
            }
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

    fn install_execute_permission(&mut self, ipa: u64) -> Result<(), Error> {
        let (pointer, entry) = self.normal_page_leaf(ipa)?;
        let executable = match self.backend.stage2_format() {
            Stage2Format::Ept => entry | EPT_EXECUTE,
            Stage2Format::Npt => entry & !NPT_NO_EXECUTE,
        };
        if executable == entry {
            return Ok(());
        }
        // SAFETY: Serialized mutation owns this validated leaf.
        unsafe { write_volatile(pointer, executable) };
        Ok(())
    }

    fn normal_page_leaf(&self, ipa: u64) -> Result<(*mut u64, u64), Error> {
        if !ipa.is_multiple_of(PAGE_SIZE) || ipa >= GUEST_LIMIT {
            return Err(Error::InvalidAddress);
        }
        let mut table = self.root;
        for level in 0..3 {
            // SAFETY: The root and every validated child are retained by this
            // address space.
            let entry = unsafe { read_entry(table, index(ipa, level))? };
            if entry & self.present_flags() == 0 {
                return Err(Error::Conflict);
            }
            table = PhysicalAddress::new(entry & EPT_ADDRESS_MASK);
        }
        let pointer = table_pointer(table)? as *mut u64;
        // SAFETY: The walk established a live final-level table.
        let pointer = unsafe { pointer.add(index(ipa, 3)) };
        // SAFETY: Address-space mutation is serialized by the caller.
        let entry = unsafe { read_volatile(pointer) };
        let valid = match self.backend.stage2_format() {
            Stage2Format::Ept => entry & (EPT_READ | EPT_WRITE) == EPT_READ | EPT_WRITE,
            Stage2Format::Npt => {
                entry & (NPT_PRESENT | NPT_WRITE | NPT_USER) == NPT_PRESENT | NPT_WRITE | NPT_USER
            }
        };
        if !valid {
            return Err(Error::Conflict);
        }
        Ok((pointer, entry))
    }
}

const fn ept_execute_flags(permissions: Stage2PagePermissions) -> u64 {
    if permissions.is_executable() {
        EPT_EXECUTE
    } else {
        0
    }
}

const fn npt_execute_flags(permissions: Stage2PagePermissions) -> u64 {
    if permissions.is_executable() {
        0
    } else {
        NPT_NO_EXECUTE
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

unsafe fn read_entry(table: PhysicalAddress, slot: usize) -> Result<u64, Error> {
    // SAFETY: The caller guarantees a live mapped table and an in-range slot.
    Ok(unsafe { read_volatile((table_pointer(table)? as *const u64).add(slot)) })
}

unsafe fn write_entry(table: PhysicalAddress, slot: usize, value: u64) -> Result<(), Error> {
    // SAFETY: The caller guarantees exclusive table mutation and an in-range slot.
    unsafe { write_volatile((table_pointer(table)? as *mut u64).add(slot), value) };
    Ok(())
}
