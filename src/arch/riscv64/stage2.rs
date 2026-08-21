// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Sv39x4 guest-stage translation for the RISC-V hypervisor extension.

use core::ptr::{read_volatile, write_volatile};

use hyper::mm::{PAGE_SIZE, PhysicalAddress};

use super::memory::Riscv64AddressTranslation;
use super::registers;
use hyper::hal::memory::AddressTranslation;

const LEVEL_SHIFTS: [u64; 3] = [30, 21, 12];
const LEVEL_SIZES: [u64; 3] = [1 << 30, 1 << 21, PAGE_SIZE];
const ROOT_ENTRIES: u64 = 2048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation,
    Conflict,
    InvalidAddress,
    InvalidRange,
    InvalidVmid,
    RemoteFence(super::sbi::Error),
}

pub struct Stage2AddressSpace {
    root: PhysicalAddress,
    vmid: u16,
}

impl Stage2AddressSpace {
    pub fn required_table_pages(ipa: u64, size: u64) -> Result<usize, Error> {
        validate_range(ipa, ipa, size)?;
        let end = ipa.checked_add(size).ok_or(Error::AddressOverflow)?;
        let level1 = covering_regions(ipa, end, LEVEL_SIZES[0])?;
        let level2 = covering_regions(ipa, end, LEVEL_SIZES[1])?;
        4usize
            .checked_add(level1)
            .and_then(|pages| pages.checked_add(level2))
            .ok_or(Error::AddressOverflow)
    }

    /// Creates a guest address space backed by pages returned by `allocator`.
    ///
    /// # Safety
    ///
    /// Every successful allocation must return a uniquely owned, zeroed,
    /// suitably aligned group of RAM pages. Those pages must remain live and
    /// accessible through the kernel linear mapping for the lifetime of this
    /// address space.
    pub unsafe fn new(
        vmid: u16,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<Self, Error> {
        if vmid == 0 || u64::from(vmid) >= 1 << 14 {
            return Err(Error::InvalidVmid);
        }
        let root = allocator(4, 4).ok_or(Error::Allocation)?;
        if !root.get().is_multiple_of(4 * PAGE_SIZE) {
            return Err(Error::InvalidAddress);
        }
        Ok(Self { root, vmid })
    }

    pub const fn root_address(&self) -> u64 {
        self.root.get()
    }

    #[allow(dead_code)]
    /// Maps normal guest memory.
    ///
    /// # Safety
    ///
    /// The address-space pages supplied to `new` and any pages returned by
    /// `allocator` must satisfy `new`'s ownership and accessibility contract.
    /// The caller must also serialize page-table mutation.
    pub unsafe fn map_normal(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        validate_range(ipa, physical, size)?;
        let mut offset = 0;
        while offset < size {
            let level = best_level(ipa + offset, physical + offset, size - offset);
            // SAFETY: The method contract covers all allocator pages and serialized mutation.
            unsafe { self.map_leaf(ipa + offset, physical + offset, level, allocator)? };
            offset += LEVEL_SIZES[level];
        }
        Ok(())
    }

    /// Maps one normal guest page.
    ///
    /// # Safety
    ///
    /// The page-table allocation and serialization requirements from
    /// `map_normal` apply.
    pub unsafe fn map_normal_page(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        validate_range(ipa, physical, PAGE_SIZE)?;
        // SAFETY: The method contract covers allocator pages and serialized mutation.
        unsafe { self.map_leaf(ipa, physical, 2, allocator) }
    }

    pub unsafe fn map_normal_page_active(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        // SAFETY: This function has the same allocator and serialization contract.
        unsafe { self.map_normal_page(ipa, physical, allocator)? };
        // SAFETY: The caller guarantees this VMID is the active address space.
        unsafe { invalidate(self.vmid) }
    }

    pub unsafe fn invalidate_page_active(&self, ipa: u64) -> Result<(), Error> {
        if !ipa.is_multiple_of(PAGE_SIZE) || ipa >= registers::STAGE2_IPA_LIMIT {
            return Err(Error::InvalidAddress);
        }
        // SAFETY: The caller guarantees this VMID is the active address space.
        unsafe { invalidate(self.vmid) }
    }

    #[allow(dead_code)]
    /// Maps a guest device range.
    ///
    /// # Safety
    ///
    /// The page-table allocation and serialization requirements from
    /// `map_normal` apply.
    pub unsafe fn map_device(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        // SAFETY: This function has the same allocator and serialization contract.
        unsafe { self.map_normal(ipa, physical, size, allocator) }
    }

    pub unsafe fn activate(&self) {
        let value = registers::HGATP_MODE_SV39X4
            | (u64::from(self.vmid) << registers::HGATP_VMID_SHIFT)
            | (self.root.get() >> registers::PAGE_SHIFT);
        // SAFETY: The caller guarantees the hierarchy is complete and activation is serialized.
        unsafe { riscv64_activate_stage2(value) };
    }

    unsafe fn map_leaf(
        &mut self,
        ipa: u64,
        physical: u64,
        leaf_level: usize,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        let mut table = self.root;
        for level in 0..leaf_level {
            let slot = index(ipa, level);
            // SAFETY: The method contract guarantees every walked table is live and mapped.
            let entry = unsafe { read_entry(table, slot)? };
            table = if entry == 0 {
                let child = allocator(1, 1).ok_or(Error::Allocation)?;
                // SAFETY: The allocator contract provides a fresh live child table.
                unsafe { write_entry(table, slot, table_pte(child.get()))? };
                child
            } else if entry & registers::PTE_VALID != 0
                && entry & (registers::PTE_READ | registers::PTE_WRITE | registers::PTE_EXECUTE)
                    == 0
            {
                PhysicalAddress::new(pte_address(entry))
            } else {
                return Err(Error::Conflict);
            };
        }
        let slot = index(ipa, leaf_level);
        let value = (physical >> 2)
            | registers::PTE_VALID
            | registers::PTE_READ
            | registers::PTE_WRITE
            | registers::PTE_EXECUTE
            | registers::PTE_USER
            | registers::PTE_ACCESSED
            | registers::PTE_DIRTY;
        // SAFETY: The walk established a live mapped leaf table.
        let existing = unsafe { read_entry(table, slot)? };
        if existing != 0 && existing != value {
            return Err(Error::Conflict);
        }
        // SAFETY: Mutation is serialized and the leaf table is live and mapped.
        unsafe { write_entry(table, slot, value) }
    }
}

fn index(ipa: u64, level: usize) -> usize {
    let mask = if level == 0 { ROOT_ENTRIES - 1 } else { 511 };
    ((ipa >> LEVEL_SHIFTS[level]) & mask) as usize
}

#[allow(dead_code)]
fn best_level(ipa: u64, physical: u64, remaining: u64) -> usize {
    LEVEL_SIZES
        .iter()
        .position(|size| {
            ipa.is_multiple_of(*size) && physical.is_multiple_of(*size) && remaining >= *size
        })
        .unwrap_or(2)
}

fn table_pte(address: u64) -> u64 {
    (address >> 2) | registers::PTE_VALID
}
fn pte_address(entry: u64) -> u64 {
    (entry >> 10) << 12
}

unsafe fn read_entry(table: PhysicalAddress, slot: usize) -> Result<u64, Error> {
    let pointer = table_pointer(table)? as *const u64;
    // SAFETY: The caller guarantees a live table and `slot` is produced by index().
    Ok(unsafe { read_volatile(pointer.add(slot)) })
}

unsafe fn write_entry(table: PhysicalAddress, slot: usize, value: u64) -> Result<(), Error> {
    let pointer = table_pointer(table)? as *mut u64;
    // SAFETY: The caller guarantees exclusive table mutation and an in-range slot.
    unsafe { write_volatile(pointer.add(slot), value) };
    Ok(())
}

fn table_pointer(table: PhysicalAddress) -> Result<usize, Error> {
    Riscv64AddressTranslation::linear_address(table)
        .and_then(|address| usize::try_from(address.get()).ok())
        .ok_or(Error::InvalidAddress)
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
    if end > registers::STAGE2_IPA_LIMIT || physical_end > registers::PHYSICAL_ADDRESS_LIMIT {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

fn covering_regions(start: u64, end: u64, span: u64) -> Result<usize, Error> {
    let first = start / span;
    let last = end.checked_sub(1).ok_or(Error::InvalidRange)? / span;
    usize::try_from(last - first + 1).map_err(|_| Error::AddressOverflow)
}

unsafe fn invalidate(vmid: u16) -> Result<(), Error> {
    // SAFETY: The caller guarantees this VMID is active and invalidation is serialized.
    unsafe { riscv64_invalidate_stage2_vmid(usize::from(vmid)) };
    super::smp::for_each_online_remote_hart(|hart_id| {
        super::sbi::remote_hfence_gvma_vmid(hart_id, vmid)
    })
    .map_err(Error::RemoteFence)
}

unsafe extern "C" {
    fn riscv64_activate_stage2(value: u64);
    fn riscv64_invalidate_stage2_vmid(vmid: usize);
}
