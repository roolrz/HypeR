//! AArch64 stage-2 translation tables and activation.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use hyper::mm::{PAGE_SIZE, PhysicalAddress};

use super::registers;

// A 39-bit IPA started at level 1 uses exactly one 4 KiB root table. A
// 40-bit IPA would require two concatenated, 8 KiB-aligned root tables.
const _: () = {
    assert!(
        registers::TRANSLATION_TABLE_ENTRY_COUNT_4K as u64 * registers::STAGE2_LEVEL_SIZES_4K[0]
            == registers::STAGE2_IPA_LIMIT
    );
    assert!(
        registers::VTCR_EL2_GUEST_VALUE & registers::VTCR_EL2_T0SZ_MASK
            == 64 - registers::STAGE2_IPA_BITS as u64
    );
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation,
    Conflict,
    InvalidAddress,
    InvalidRange,
    InvalidVmid,
}

#[derive(Clone, Copy)]
enum MemoryType {
    Normal,
    // Retained for future device passthrough/driver-domain mappings. The
    // built-in guest console is intentionally trapped and emulated instead.
    #[allow(dead_code)]
    Device,
}

pub struct Stage2AddressSpace {
    root: PhysicalAddress,
    vmid: u16,
}

impl Stage2AddressSpace {
    pub fn new(
        vmid: u16,
        allocator: &mut impl FnMut() -> Option<PhysicalAddress>,
    ) -> Result<Self, Error> {
        if vmid == 0 {
            return Err(Error::InvalidVmid);
        }
        let root = allocator().ok_or(Error::Allocation)?;
        validate_table(root)?;
        Ok(Self { root, vmid })
    }

    pub const fn root_address(&self) -> u64 {
        self.root.get()
    }

    pub fn map_normal(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        allocator: &mut impl FnMut() -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        self.map_range(ipa, physical, size, MemoryType::Normal, allocator)
    }

    #[allow(dead_code)]
    pub fn map_device(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        allocator: &mut impl FnMut() -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        self.map_range(ipa, physical, size, MemoryType::Device, allocator)
    }

    /// Installs this VM's stage-2 hierarchy on the current CPU.
    ///
    /// # Safety
    ///
    /// The caller must exclusively own the local guest execution context and
    /// must not switch VMIDs without first stopping lower-EL execution.
    pub unsafe fn activate(&self) {
        let vttbr = (u64::from(self.vmid) << registers::VTTBR_EL2_VMID_SHIFT) | self.root.get();
        // SAFETY: The hierarchy is complete and owned by this address space.
        unsafe {
            asm!(
                "dsb ishst",
                "msr VTCR_EL2, {vtcr}",
                "msr VTTBR_EL2, {vttbr}",
                "isb",
                "tlbi VMALLS12E1IS",
                "dsb ish",
                "isb",
                "mrs {hcr}, HCR_EL2",
                "orr {hcr}, {hcr}, {vm}",
                "msr HCR_EL2, {hcr}",
                "isb",
                vtcr = in(reg) registers::VTCR_EL2_GUEST_VALUE,
                vttbr = in(reg) vttbr,
                vm = in(reg) registers::HCR_EL2_VM,
                hcr = out(reg) _,
                options(nostack, preserves_flags)
            );
        }
    }

    fn map_range(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        memory: MemoryType,
        allocator: &mut impl FnMut() -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        if size == 0
            || ipa & (PAGE_SIZE - 1) != 0
            || physical & (PAGE_SIZE - 1) != 0
            || size & (PAGE_SIZE - 1) != 0
        {
            return Err(Error::InvalidRange);
        }
        let end = ipa.checked_add(size).ok_or(Error::AddressOverflow)?;
        let physical_end = physical.checked_add(size).ok_or(Error::AddressOverflow)?;
        if end > registers::STAGE2_IPA_LIMIT || physical_end > registers::PHYSICAL_ADDRESS_LIMIT {
            return Err(Error::InvalidAddress);
        }

        let mut offset = 0;
        while offset < size {
            let current_ipa = ipa + offset;
            let current_physical = physical + offset;
            let remaining = size - offset;
            let level = best_level(current_ipa, current_physical, remaining);
            self.map_leaf(current_ipa, current_physical, level, memory, allocator)?;
            offset += registers::STAGE2_LEVEL_SIZES_4K[level];
        }
        Ok(())
    }

    fn map_leaf(
        &mut self,
        ipa: u64,
        physical: u64,
        leaf_level: usize,
        memory: MemoryType,
        allocator: &mut impl FnMut() -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        let mut table = self.root;
        for level in 0..leaf_level {
            let index = index(ipa, level);
            let entry = read_entry(table, index)?;
            table = if entry & registers::TRANSLATION_DESC_TYPE_MASK
                == registers::STAGE2_DESC_TABLE_OR_PAGE
            {
                PhysicalAddress::new(entry & registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT)
            } else if entry == 0 {
                let child = allocator().ok_or(Error::Allocation)?;
                validate_table(child)?;
                write_entry(
                    table,
                    index,
                    child.get() | registers::STAGE2_DESC_TABLE_OR_PAGE,
                )?;
                child
            } else {
                return Err(Error::Conflict);
            };
        }

        let kind = if leaf_level == 2 {
            registers::STAGE2_DESC_TABLE_OR_PAGE
        } else {
            registers::STAGE2_DESC_BLOCK
        };
        let attributes = registers::STAGE2_DESC_ACCESS_FLAG
            | registers::STAGE2_DESC_READ_WRITE
            | match memory {
                MemoryType::Normal => {
                    registers::STAGE2_DESC_INNER_SHAREABLE
                        | registers::STAGE2_DESC_MEMATTR_NORMAL_WB
                }
                MemoryType::Device => registers::STAGE2_DESC_MEMATTR_DEVICE_NGNRE,
            };
        let descriptor =
            (physical & registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT) | attributes | kind;
        let slot = index(ipa, leaf_level);
        let existing = read_entry(table, slot)?;
        if existing != 0 && existing != descriptor {
            return Err(Error::Conflict);
        }
        write_entry(table, slot, descriptor)
    }
}

fn best_level(ipa: u64, physical: u64, remaining: u64) -> usize {
    for (level, &size) in registers::STAGE2_LEVEL_SIZES_4K.iter().enumerate() {
        if ipa & (size - 1) == 0 && physical & (size - 1) == 0 && remaining >= size {
            return level;
        }
    }
    2
}

fn index(ipa: u64, level: usize) -> usize {
    ((ipa >> registers::STAGE2_LEVEL_SHIFTS_4K[level])
        & (registers::TRANSLATION_TABLE_ENTRY_COUNT_4K as u64 - 1)) as usize
}

fn validate_table(table: PhysicalAddress) -> Result<(), Error> {
    if table.get() & (PAGE_SIZE - 1) != 0 || table.get() >= registers::PHYSICAL_ADDRESS_LIMIT {
        Err(Error::InvalidAddress)
    } else {
        Ok(())
    }
}

fn read_entry(table: PhysicalAddress, slot: usize) -> Result<u64, Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: The table is a live page owned by this stage-2 hierarchy.
    Ok(unsafe { read_volatile(pointer.add(slot)) })
}

fn write_entry(table: PhysicalAddress, slot: usize, value: u64) -> Result<(), Error> {
    let pointer = table_pointer(table)? as *mut u64;
    // SAFETY: Hierarchy construction is serialized before guest execution.
    unsafe { write_volatile(pointer.add(slot), value) };
    Ok(())
}

fn table_pointer(table: PhysicalAddress) -> Result<*const u64, Error> {
    registers::LINEAR_VIRTUAL_BASE
        .checked_add(table.get())
        .and_then(|address| usize::try_from(address).ok())
        .map(|address| address as *const u64)
        .ok_or(Error::InvalidAddress)
}
