// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` stage-2 translation tables and activation.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use hyper::mm::{PAGE_SIZE, PhysicalAddress};
use hyper::vm::translation::{ActiveMappingError, publish_active_mapping};

use super::{address, memory, registers};

// An up-to-39-bit IPA started at level 1 fits in one 4 KiB root table. A
// 40-bit IPA would require two concatenated, 8 KiB-aligned root tables.
const _: () = {
    assert!(
        registers::TRANSLATION_TABLE_ENTRY_COUNT_4K as u64 * registers::STAGE2_LEVEL_SIZES_4K[0]
            >= address::STAGE2_IPA_LIMIT
    );
    assert!(registers::VTCR_EL2_GUEST_BASE & registers::VTCR_EL2_T0SZ_MASK == 0);
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
    pub fn required_table_pages(ipa: u64, size: u64) -> Result<usize, Error> {
        validate_ipa_range(ipa, size)?;
        let end = ipa.checked_add(size).ok_or(Error::AddressOverflow)?;
        let level1 = covering_regions(ipa, end, registers::STAGE2_LEVEL_SIZES_4K[0])?;
        let level2 = covering_regions(ipa, end, registers::STAGE2_LEVEL_SIZES_4K[1])?;
        1usize
            .checked_add(level1)
            .and_then(|pages| pages.checked_add(level2))
            .ok_or(Error::AddressOverflow)
    }

    /// Creates an empty stage-2 hierarchy.
    ///
    /// # Safety
    ///
    /// Every page returned by `allocator` must be uniquely owned by this
    /// hierarchy, zero-initialized, aligned as requested, accessible through
    /// the permanent host linear map, and kept alive until this address space
    /// can no longer be active or accessed.
    pub unsafe fn new(
        vmid: u16,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<Self, Error> {
        if vmid == 0 {
            return Err(Error::InvalidVmid);
        }
        let root = allocator(1, 1).ok_or(Error::Allocation)?;
        validate_table(root)?;
        Ok(Self { root, vmid })
    }

    pub const fn root_address(&self) -> u64 {
        self.root.get()
    }

    pub(crate) fn retirement_request(&self) -> super::GuestStage2RetirementRequest {
        let vttbr = (u64::from(self.vmid) << registers::VTTBR_EL2_VMID_SHIFT) | self.root.get();
        super::GuestStage2RetirementRequest::new(vttbr, address::capabilities().stage2_vtcr_el2())
    }

    #[allow(dead_code)]
    /// Maps normal memory using page-table pages supplied by `allocator`.
    ///
    /// # Safety
    ///
    /// Newly returned table pages must satisfy the ownership, initialization,
    /// mapping, alignment, and lifetime contract of [`Self::new`]. The caller
    /// must also serialize all updates to this hierarchy.
    pub unsafe fn map_normal(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        self.map_range(ipa, physical, size, MemoryType::Normal, allocator)
    }

    /// Adds one normal-memory page mapping.
    ///
    /// # Safety
    ///
    /// Newly returned table pages must satisfy the ownership, initialization,
    /// mapping, alignment, and lifetime contract of [`Self::new`]. The caller
    /// must also serialize all updates to this hierarchy.
    pub unsafe fn map_normal_page(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        validate_page(ipa, physical)?;
        self.map_leaf(ipa, physical, 2, MemoryType::Normal, allocator)
    }

    /// Adds a 4 KiB invalid-to-valid mapping while this VMID is active, then
    /// publishes that new leaf to the guest translation regime.
    ///
    /// # Safety
    ///
    /// This address space must be active on the current CPU, and the caller
    /// must serialize page-table updates for this VM. Newly returned table
    /// pages must also satisfy the ownership, mapping, and lifetime contract
    /// of [`Self::new`].
    pub unsafe fn map_normal_page_active(
        &mut self,
        ipa: u64,
        physical: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), ActiveMappingError<Error>> {
        publish_active_mapping(
            self,
            |stage2| {
                // SAFETY: This method inherits the allocator and serialization
                // requirements in addition to requiring the hierarchy active.
                unsafe { stage2.map_normal_page(ipa, physical, allocator) }
            },
            |_| {
                // SAFETY: The caller guarantees this VMID remains active while
                // the new descriptor is published to the guest regime.
                unsafe { publish_new_leaf() };
                Ok(())
            },
        )
    }

    /// Reissues new-leaf publication for one unchanged active guest page.
    ///
    /// # Safety
    ///
    /// This address space must be active on the current CPU.
    pub unsafe fn invalidate_page_active(&self, ipa: u64) -> Result<(), Error> {
        if ipa & (PAGE_SIZE - 1) != 0 || ipa >= address::STAGE2_IPA_LIMIT {
            return Err(Error::InvalidAddress);
        }
        // SAFETY: The method contract guarantees that this address space is
        // selected by the current CPU's VTTBR_EL2. Current callers use this
        // only as recovery after a fault on an unchanged valid leaf.
        unsafe { invalidate_existing_ipa(ipa) };
        Ok(())
    }

    #[allow(dead_code)]
    /// Maps device memory using page-table pages supplied by `allocator`.
    ///
    /// # Safety
    ///
    /// Newly returned table pages must satisfy the ownership, initialization,
    /// mapping, alignment, and lifetime contract of [`Self::new`]. The caller
    /// must also serialize all updates to this hierarchy.
    pub unsafe fn map_device(
        &mut self,
        ipa: u64,
        physical: u64,
        size: u64,
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
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
        let vtcr = address::capabilities().stage2_vtcr_el2();
        // VHE makes TGE select whether guest or host TLBs are targeted. Keep
        // the temporary guest-regime interval entirely inside this register-
        // only sequence so host memory accesses cannot observe it.
        // SAFETY: The hierarchy is complete and owned by this address space.
        unsafe {
            asm!(
                "dsb ishst",
                "mrs {host_hcr}, HCR_EL2",
                "orr {host_hcr}, {host_hcr}, {vm}",
                "msr VTCR_EL2, {vtcr}",
                "msr VTTBR_EL2, {vttbr}",
                "msr HCR_EL2, {host_hcr}",
                "isb",
                "bic {guest_hcr}, {host_hcr}, {tge}",
                "msr HCR_EL2, {guest_hcr}",
                "isb",
                "tlbi VMALLS12E1IS",
                "dsb ish",
                "isb",
                "msr HCR_EL2, {host_hcr}",
                "isb",
                vtcr = in(reg) vtcr,
                vttbr = in(reg) vttbr,
                vm = in(reg) registers::HCR_EL2_VM,
                tge = in(reg) registers::HCR_EL2_TGE,
                host_hcr = out(reg) _,
                guest_hcr = out(reg) _,
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
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
    ) -> Result<(), Error> {
        validate_range(ipa, physical, size)?;

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
        allocator: &mut impl FnMut(usize, usize) -> Option<PhysicalAddress>,
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
                let child = allocator(1, 1).ok_or(Error::Allocation)?;
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
                MemoryType::Device => {
                    registers::STAGE2_DESC_MEMATTR_DEVICE_NGNRE | registers::STAGE2_DESC_XN
                }
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

/// Invalidates one retained guest translation identity on the current CPU.
///
/// The request is opaque outside the architecture. Its publisher retains the
/// exact root and VMID until every targeted CPU acknowledges this operation.
pub(crate) fn retire_local(request: super::GuestStage2RetirementRequest) {
    let retiring_vttbr = request.retiring_vttbr();
    let guest_vtcr = request.guest_vtcr();
    // SAFETY: The caller has stopped every execution of the retiring VM and
    // retains its root and VMID. This register-only interval selects that
    // exact guest regime, performs a local combined stage-1/stage-2
    // invalidation, and restores unrelated local state before returning.
    unsafe {
        asm!(
            "mrs {saved_hcr}, HCR_EL2",
            "mrs {saved_vttbr}, VTTBR_EL2",
            "mrs {saved_vtcr}, VTCR_EL2",
            "orr {guest_hcr}, {saved_hcr}, {vm}",
            "bic {guest_hcr}, {guest_hcr}, {tge}",
            "msr VTCR_EL2, {guest_vtcr}",
            "msr VTTBR_EL2, {retiring_vttbr}",
            "msr HCR_EL2, {guest_hcr}",
            "isb",
            "dsb ishst",
            "tlbi VMALLS12E1",
            "dsb ish",
            "isb",
            "cmp {saved_vttbr}, {retiring_vttbr}",
            "csel {restore_vttbr}, xzr, {saved_vttbr}, eq",
            "msr VTTBR_EL2, {restore_vttbr}",
            "msr VTCR_EL2, {saved_vtcr}",
            "msr HCR_EL2, {saved_hcr}",
            "isb",
            saved_hcr = out(reg) _,
            saved_vttbr = out(reg) _,
            saved_vtcr = out(reg) _,
            guest_hcr = out(reg) _,
            restore_vttbr = out(reg) _,
            guest_vtcr = in(reg) guest_vtcr,
            retiring_vttbr = in(reg) retiring_vttbr,
            vm = in(reg) registers::HCR_EL2_VM,
            tge = in(reg) registers::HCR_EL2_TGE,
            options(nostack)
        );
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
    if table.get() & (PAGE_SIZE - 1) != 0 || table.get() >= address::physical_address_limit() {
        Err(Error::InvalidAddress)
    } else {
        Ok(())
    }
}

fn validate_page(ipa: u64, physical: u64) -> Result<(), Error> {
    if ipa & (PAGE_SIZE - 1) != 0 || physical & (PAGE_SIZE - 1) != 0 {
        return Err(Error::InvalidRange);
    }
    if ipa >= address::STAGE2_IPA_LIMIT
        || physical >= address::physical_address_limit()
        || ipa.checked_add(PAGE_SIZE).is_none()
        || physical.checked_add(PAGE_SIZE).is_none()
    {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

fn validate_range(ipa: u64, physical: u64, size: u64) -> Result<(), Error> {
    validate_ipa_range(ipa, size)?;
    if !physical.is_multiple_of(PAGE_SIZE) {
        return Err(Error::InvalidRange);
    }
    let physical_end = physical.checked_add(size).ok_or(Error::AddressOverflow)?;
    if physical_end > address::physical_address_limit() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

fn validate_ipa_range(ipa: u64, size: u64) -> Result<(), Error> {
    if size == 0 || !ipa.is_multiple_of(PAGE_SIZE) || !size.is_multiple_of(PAGE_SIZE) {
        return Err(Error::InvalidRange);
    }
    let end = ipa.checked_add(size).ok_or(Error::AddressOverflow)?;
    if end > address::STAGE2_IPA_LIMIT {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

fn covering_regions(start: u64, end: u64, span: u64) -> Result<usize, Error> {
    let first = start / span;
    let last = end.checked_sub(1).ok_or(Error::InvalidRange)? / span;
    usize::try_from(last - first + 1).map_err(|_| Error::AddressOverflow)
}

/// Publishes one invalid-to-valid leaf for the active VMID.
///
/// Translation faults are not cached, so this path does not invalidate the
/// guest's complete stage-1 regime. Descriptor replacement and unmapping must
/// use [`invalidate_replaced_ipa`] instead.
unsafe fn publish_new_leaf() {
    // Translation faults are not cached, so invalid-to-valid publication only
    // has to make the descriptor store visible before guest retry. ERET is the
    // context-synchronization event for the faulting processing element.
    // SAFETY: DSB has no pointer operand and orders the serialized table write.
    unsafe { asm!("dsb ishst", options(nostack, preserves_flags)) };
}

/// Invalidates one unchanged valid leaf after an unexpected repeated fault.
unsafe fn invalidate_existing_ipa(ipa: u64) {
    let operand = ipa >> registers::TLBI_IPAS2E1_IPA_SHIFT;
    // SAFETY: The caller guarantees the current VTTBR_EL2 selects this address
    // space and keeps it active until the page-address invalidation completes.
    unsafe {
        asm!(
            "dsb ishst",
            "mrs {host_hcr}, HCR_EL2",
            "bic {guest_hcr}, {host_hcr}, {tge}",
            "msr HCR_EL2, {guest_hcr}",
            "isb",
            "tlbi ipas2e1is, {operand}",
            "dsb ish",
            "isb",
            "msr HCR_EL2, {host_hcr}",
            "isb",
            operand = in(reg) operand,
            tge = in(reg) registers::HCR_EL2_TGE,
            host_hcr = out(reg) _,
            guest_hcr = out(reg) _,
            options(nostack, preserves_flags)
        );
    }
}

/// Invalidates an existing stage-2 mapping and any guest stage-1 translations
/// which could have walked through its previous physical page.
///
/// This is intentionally separate from new-leaf publication: flushing the
/// complete guest stage-1 regime on every demand-zero fault would add needless
/// hot-path cost. Future replacement/unmap operations must call this helper
/// after publishing their descriptor update.
#[allow(dead_code)]
unsafe fn invalidate_replaced_ipa(ipa: u64) {
    let operand = ipa >> registers::TLBI_IPAS2E1_IPA_SHIFT;
    // SAFETY: The caller guarantees that VTTBR_EL2 selects the updated address
    // space. HCR.TGE is cleared while guest-regime TLBIs execute, then restored
    // only after both invalidations complete in the inner-shareable domain.
    unsafe {
        asm!(
            "dsb ishst",
            "mrs {host_hcr}, HCR_EL2",
            "bic {guest_hcr}, {host_hcr}, {tge}",
            "msr HCR_EL2, {guest_hcr}",
            "isb",
            "tlbi ipas2e1is, {operand}",
            "dsb ish",
            "tlbi vmalle1is",
            "dsb ish",
            "isb",
            "msr HCR_EL2, {host_hcr}",
            "isb",
            operand = in(reg) operand,
            tge = in(reg) registers::HCR_EL2_TGE,
            host_hcr = out(reg) _,
            guest_hcr = out(reg) _,
            options(nostack, preserves_flags)
        );
    }
}

fn read_entry(table: PhysicalAddress, slot: usize) -> Result<u64, Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: The table is a live page owned by this stage-2 hierarchy.
    Ok(unsafe { read_volatile(pointer.add(slot)) })
}

fn write_entry(table: PhysicalAddress, slot: usize, value: u64) -> Result<(), Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: Hierarchy construction is serialized before guest execution.
    unsafe { write_volatile(pointer.add(slot), value) };
    Ok(())
}

fn table_pointer(table: PhysicalAddress) -> Result<*mut u64, Error> {
    memory::linear_mapping_base()
        .checked_add(table.get())
        .and_then(|address| usize::try_from(address).ok())
        .map(core::ptr::with_exposed_provenance_mut::<u64>)
        .ok_or(Error::InvalidAddress)
}
