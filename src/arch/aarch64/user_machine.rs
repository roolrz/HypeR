// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Owned native-user translation hierarchies and local Arm transitions.
//!
//! Kernel policy supplies and retains every table page. This module only
//! encodes descriptors and performs bounded local register/TLBI sequences.

use core::arch::asm;
use core::marker::PhantomData;
use core::ptr::{read_volatile, write_volatile};

use hyper::mm::{PAGE_SIZE, PhysicalAddress};

use super::user_contract::{
    UserMachineContractError, UserPagePermissions, UserTranslationRegime, UserTranslationRegisters,
};
use super::{address, memory, registers};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AddressOverflow,
    Allocation,
    Conflict,
    Contract(UserMachineContractError),
    InvalidAddress,
    InvalidRange,
    WrongHostMode,
}

impl From<UserMachineContractError> for Error {
    fn from(error: UserMachineContractError) -> Self {
        Self::Contract(error)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MappingPage {
    pub(crate) address: u64,
    pub(crate) physical: PhysicalAddress,
    pub(crate) readable: bool,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

pub(crate) struct PreparedAddressSpace {
    registers: UserTranslationRegisters,
    control: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum LocalOperation {
    Replace,
    Invalidate,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalRequest {
    root_register: u64,
    control: u64,
    generation: u64,
    regime: UserTranslationRegime,
    operation: LocalOperation,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalIdentity {
    root_register: u64,
    regime: UserTranslationRegime,
}

pub(crate) struct LocalActivation {
    regime: UserTranslationRegime,
    installed_identifier: u16,
    previous_root: u64,
    previous_control: u64,
    previous_hcr: u64,
    previous_sctlr_el1: u64,
    previous_cpacr_el1: u64,
    previous_cntkctl_el1: u64,
    not_send_or_sync: PhantomData<*mut ()>,
}

impl PreparedAddressSpace {
    pub(crate) const fn root_register(&self) -> u64 {
        self.registers.root_register()
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.registers.generation()
    }

    pub(crate) const fn identifier(&self) -> u16 {
        (self.registers.root_register() >> 48) as u16
    }

    pub(crate) const fn is_vhe(&self) -> bool {
        matches!(
            self.registers.regime(),
            UserTranslationRegime::VheHostStage1
        )
    }

    pub(crate) const fn local_request(&self, operation: LocalOperation) -> LocalRequest {
        LocalRequest {
            root_register: self.root_register(),
            control: self.control,
            generation: self.generation(),
            regime: self.registers.regime(),
            operation,
        }
    }

    pub(crate) const fn local_identity(&self) -> LocalIdentity {
        LocalIdentity {
            root_register: self.root_register(),
            regime: self.registers.regime(),
        }
    }
}

pub(crate) fn local_identity_is_active(identity: LocalIdentity) -> bool {
    let current: u64;
    // SAFETY: Both translation-base registers are readable at EL2 without
    // side effects; only the register for the identity's regime is selected.
    unsafe {
        match identity.regime {
            UserTranslationRegime::VheHostStage1 => asm!(
                "mrs {current}, TTBR0_EL2",
                current = out(reg) current,
                options(nomem, nostack, preserves_flags)
            ),
            UserTranslationRegime::NvheStage2Only => asm!(
                "mrs {current}, VTTBR_EL2",
                current = out(reg) current,
                options(nomem, nostack, preserves_flags)
            ),
        }
    };
    current == identity.root_register
}

/// Builds an immutable VHE EL2&0 stage-1 root.
///
/// # Safety
///
/// Each allocator result must be a uniquely owned, zeroed, linearly mapped
/// table block with the requested order and natural alignment. The caller
/// retains every block until acknowledged retirement of the returned root.
pub(crate) unsafe fn prepare_vhe(
    asid: u16,
    generation: u64,
    mut enumerate: impl FnMut(&mut dyn FnMut(MappingPage)),
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<PreparedAddressSpace, Error> {
    let capabilities = super::user::execution_capabilities()?;
    if !super::host::is_vhe() {
        return Err(Error::WrongHostMode);
    }
    let root = allocator(0).ok_or(Error::Allocation)?;
    validate_table(root, 0)?;
    let mut builder = Builder {
        root,
        regime: UserTranslationRegime::VheHostStage1,
        allocator,
    };
    let mut result = Ok(());
    enumerate(&mut |page| {
        if result.is_ok() {
            result = builder.map(page);
        }
    });
    result?;
    // Complete descriptor publication before a Release-published root token.
    // SAFETY: DSB has no pointer operand and orders builder-owned table stores.
    unsafe { asm!("dsb ishst", options(nostack, preserves_flags)) };
    Ok(PreparedAddressSpace {
        registers: UserTranslationRegisters::new(
            capabilities,
            UserTranslationRegime::VheHostStage1,
            root.get(),
            asid,
            generation,
        )?,
        control: address::capabilities().stage1_tcr_el2(),
    })
}

/// Builds an immutable nVHE private stage-2-only root.
///
/// # Safety
///
/// The allocator and retention requirements are identical to [`prepare_vhe`].
pub(crate) unsafe fn prepare_nvhe(
    vmid: u16,
    generation: u64,
    mut enumerate: impl FnMut(&mut dyn FnMut(MappingPage)),
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<PreparedAddressSpace, Error> {
    let capabilities = super::user::execution_capabilities()?;
    if super::host::is_vhe() {
        return Err(Error::WrongHostMode);
    }
    // CONFIG_ARM64_IPA_BITS is restricted to 39 until concatenated stage-2
    // roots are implemented, matching the existing guest geometry.
    if capabilities.address_bits() > 39 {
        return Err(Error::InvalidAddress);
    }
    let root = allocator(0).ok_or(Error::Allocation)?;
    validate_table(root, 0)?;
    let mut builder = Builder {
        root,
        regime: UserTranslationRegime::NvheStage2Only,
        allocator,
    };
    let mut result = Ok(());
    enumerate(&mut |page| {
        if result.is_ok() {
            result = builder.map(page);
        }
    });
    result?;
    // SAFETY: See prepare_vhe.
    unsafe { asm!("dsb ishst", options(nostack, preserves_flags)) };
    Ok(PreparedAddressSpace {
        registers: UserTranslationRegisters::new(
            capabilities,
            UserTranslationRegime::NvheStage2Only,
            root.get(),
            vmid,
            generation,
        )?,
        control: address::capabilities().stage2_vtcr_el2(),
    })
}

struct Builder<'allocator, Allocator> {
    root: PhysicalAddress,
    regime: UserTranslationRegime,
    allocator: &'allocator mut Allocator,
}

impl<Allocator: FnMut(usize) -> Option<PhysicalAddress>> Builder<'_, Allocator> {
    fn map(&mut self, page: MappingPage) -> Result<(), Error> {
        if !page.address.is_multiple_of(PAGE_SIZE) || !page.physical.get().is_multiple_of(PAGE_SIZE)
        {
            return Err(Error::InvalidRange);
        }
        let capabilities = super::user::execution_capabilities()?;
        let limit = capabilities.user_address_limit();
        if page.address >= limit || page.physical.get() >= address::physical_address_limit() {
            return Err(Error::InvalidAddress);
        }
        let permissions = UserPagePermissions::new(page.readable, page.writable, page.executable)?;
        if !page.readable {
            return Ok(());
        }
        match self.regime {
            UserTranslationRegime::VheHostStage1 => self.map_vhe(page, permissions),
            UserTranslationRegime::NvheStage2Only => self.map_nvhe(page, permissions),
        }
    }

    fn map_vhe(
        &mut self,
        page: MappingPage,
        permissions: UserPagePermissions,
    ) -> Result<(), Error> {
        let mut table = self.root;
        for level in 0..3 {
            let index = stage1_index(page.address, level);
            table = self.descend(table, index, registers::STAGE1_DESC_TABLE_OR_PAGE)?;
        }
        write_leaf(
            table,
            stage1_index(page.address, 3),
            permissions.vhe_stage1_descriptor(page.physical.get()),
        )
    }

    fn map_nvhe(
        &mut self,
        page: MappingPage,
        permissions: UserPagePermissions,
    ) -> Result<(), Error> {
        let mut table = self.root;
        for level in 0..2 {
            let index = stage2_index(page.address, level);
            table = self.descend(table, index, registers::STAGE2_DESC_TABLE_OR_PAGE)?;
        }
        write_leaf(
            table,
            stage2_index(page.address, 2),
            permissions.nvhe_stage2_descriptor(page.physical.get()),
        )
    }

    fn descend(
        &mut self,
        table: PhysicalAddress,
        index: usize,
        table_kind: u64,
    ) -> Result<PhysicalAddress, Error> {
        let entry = read_entry(table, index)?;
        if entry & registers::TRANSLATION_DESC_TYPE_MASK == table_kind {
            return Ok(PhysicalAddress::new(
                entry & registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT,
            ));
        }
        if entry != 0 {
            return Err(Error::Conflict);
        }
        let child = (self.allocator)(0).ok_or(Error::Allocation)?;
        validate_table(child, 0)?;
        write_entry(table, index, child.get() | table_kind)?;
        Ok(child)
    }
}

fn write_leaf(table: PhysicalAddress, index: usize, descriptor: u64) -> Result<(), Error> {
    let current = read_entry(table, index)?;
    if current != 0 && current != descriptor {
        return Err(Error::Conflict);
    }
    write_entry(table, index, descriptor)
}

/// Installs a root locally after kernel admission has excluded an update cut.
///
/// # Safety
///
/// The complete hierarchy and identifier must remain retained, execution must
/// be pinned to this CPU, and kernel policy must serialize this transition
/// against address-space update admission on the same CPU.
pub(crate) unsafe fn activate_local(root: &PreparedAddressSpace) -> LocalActivation {
    if root.is_vhe() {
        let previous_root: u64;
        let previous_hcr: u64;
        // Invalidate only this ASID on this PE. The privileged upper range is
        // owned independently by TTBR1_EL2 and cannot refill an EL0 entry.
        let operand = u64::from(root.identifier()) << registers::TTBR_ASID_SHIFT;
        // SAFETY: The caller supplies root/ID retention and local exclusion.
        unsafe {
            asm!(
                "mrs {previous_root}, TTBR0_EL2",
                "mrs {previous_hcr}, HCR_EL2",
                "tlbi aside1, {operand}",
                "dsb ish",
                "msr TTBR0_EL2, {root}",
                "isb",
                operand = in(reg) operand,
                root = in(reg) root.root_register(),
                previous_root = out(reg) previous_root,
                previous_hcr = out(reg) previous_hcr,
                options(nostack, preserves_flags)
            )
        };
        LocalActivation {
            regime: UserTranslationRegime::VheHostStage1,
            installed_identifier: root.identifier(),
            previous_root,
            previous_control: 0,
            previous_hcr,
            previous_sctlr_el1: 0,
            previous_cpacr_el1: 0,
            previous_cntkctl_el1: 0,
            not_send_or_sync: PhantomData,
        }
    } else {
        let previous_hcr = read_hcr_el2();
        let native_hcr = match super::user_contract::LowerElReturnRegime::Native(
            UserTranslationRegime::NvheStage2Only,
        )
        .transition_hcr(previous_hcr)
        {
            Ok(value) => value,
            Err(_) => invalid_machine_state(),
        };
        let previous_root: u64;
        let previous_control: u64;
        let previous_sctlr_el1: u64;
        let previous_cpacr_el1: u64;
        let previous_cntkctl_el1: u64;
        // SAFETY: The caller supplies the stopped lower-EL and retained-root
        // proof. Direct nVHE userspace uses stage-2 only, so the inherited EL1
        // stage-1 regime is replaced before HCR enables lower-EL execution.
        // VMALLS12E1 is local and targets the selected VMID.
        unsafe {
            asm!(
                "mrs {previous_root}, VTTBR_EL2",
                "mrs {previous_control}, VTCR_EL2",
                "mrs {previous_sctlr_el1}, SCTLR_EL1",
                "mrs {previous_cpacr_el1}, CPACR_EL1",
                "mrs {previous_cntkctl_el1}, CNTKCTL_EL1",
                "msr SCTLR_EL1, {native_sctlr_el1}",
                "msr CPACR_EL1, {native_cpacr_el1}",
                "msr CNTKCTL_EL1, xzr",
                "msr VTCR_EL2, {control}",
                "msr VTTBR_EL2, {root}",
                "msr HCR_EL2, {hcr}",
                "isb",
                control = in(reg) root.control,
                root = in(reg) root.root_register(),
                hcr = in(reg) native_hcr,
                native_sctlr_el1 = in(reg) registers::SCTLR_EL1_GUEST_RESET_VALUE,
                native_cpacr_el1 = in(reg) registers::CPACR_EL1_FPEN_ALL,
                previous_root = out(reg) previous_root,
                previous_control = out(reg) previous_control,
                previous_sctlr_el1 = out(reg) previous_sctlr_el1,
                previous_cpacr_el1 = out(reg) previous_cpacr_el1,
                previous_cntkctl_el1 = out(reg) previous_cntkctl_el1,
                options(nostack, preserves_flags)
            )
        };
        // SAFETY: The selected VTTBR is retained and no lower-EL execution is
        // possible in this EL2 transition interval.
        unsafe { invalidate_selected_stage2() };
        LocalActivation {
            regime: UserTranslationRegime::NvheStage2Only,
            installed_identifier: root.identifier(),
            previous_root,
            previous_control,
            previous_hcr,
            previous_sctlr_el1,
            previous_cpacr_el1,
            previous_cntkctl_el1,
            not_send_or_sync: PhantomData,
        }
    }
}

/// Leaves one local native translation interval and restores its predecessor.
///
/// # Safety
///
/// This must run on the same pinned CPU which created `activation`, after
/// lower-EL execution has stopped and while update admission remains excluded.
pub(crate) unsafe fn deactivate_local(activation: LocalActivation) {
    match activation.regime {
        UserTranslationRegime::VheHostStage1 => {
            let operand = u64::from(activation.installed_identifier) << registers::TTBR_ASID_SHIFT;
            // SAFETY: The consumed CPU-affine token supplies same-PE ownership.
            unsafe {
                asm!(
                    "tlbi aside1, {operand}",
                    "dsb ish",
                    "msr TTBR0_EL2, {root}",
                    "msr HCR_EL2, {hcr}",
                    "isb",
                    operand = in(reg) operand,
                    root = in(reg) activation.previous_root,
                    hcr = in(reg) activation.previous_hcr,
                    options(nostack, preserves_flags)
                )
            };
        }
        UserTranslationRegime::NvheStage2Only => {
            // The installed VMID is selected until the local invalidation
            // completes, then all predecessor registers are restored.
            // SAFETY: The consumed token proves the required local interval.
            unsafe { invalidate_selected_stage2() };
            // SAFETY: Invalidation completed before restoring the predecessor
            // translation and exception-routing registers.
            unsafe {
                asm!(
                    "msr VTTBR_EL2, {root}",
                    "msr VTCR_EL2, {control}",
                    "msr SCTLR_EL1, {sctlr_el1}",
                    "msr CPACR_EL1, {cpacr_el1}",
                    "msr CNTKCTL_EL1, {cntkctl_el1}",
                    "msr HCR_EL2, {hcr}",
                    "isb",
                    root = in(reg) activation.previous_root,
                    control = in(reg) activation.previous_control,
                    sctlr_el1 = in(reg) activation.previous_sctlr_el1,
                    cpacr_el1 = in(reg) activation.previous_cpacr_el1,
                    cntkctl_el1 = in(reg) activation.previous_cntkctl_el1,
                    hcr = in(reg) activation.previous_hcr,
                    options(nostack, preserves_flags)
                )
            };
        }
    }
}

/// Replaces the currently admitted native root while preserving its original
/// predecessor for the higher-level activation token.
///
/// # Safety
///
/// Kernel policy must prove the current CPU is an active target, keep both
/// roots retained, and hold the address-space admission gate closed.
pub(crate) unsafe fn replace_local(root: &PreparedAddressSpace) {
    // SAFETY: The caller supplies the full activation contract. The transient
    // predecessor token is deliberately discarded because the original local
    // activation token remains the sole leave authority.
    let _ = unsafe { activate_local(root) };
}

/// Applies one fixed scalar request published by the acknowledged kernel RPC.
///
/// # Safety
///
/// The publisher must retain the identifier and hierarchy named by `request`
/// through this CPU's acknowledgement and must exclude concurrent lower-EL
/// use outside the operation selected by the request.
pub(crate) unsafe fn service_local_request(request: LocalRequest) -> Result<(), Error> {
    let capabilities = super::user::execution_capabilities()?;
    let root_address = request.root_register & registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT;
    let identifier = (request.root_register >> 48) as u16;
    let root = PreparedAddressSpace {
        registers: UserTranslationRegisters::new(
            capabilities,
            request.regime,
            root_address,
            identifier,
            request.generation,
        )?,
        control: request.control,
    };
    match request.operation {
        LocalOperation::Replace => {
            // SAFETY: The request publisher provides the active-target proof.
            unsafe { replace_local(&root) };
        }
        LocalOperation::Invalidate => {
            // SAFETY: The publisher provides tag retention and exclusion.
            unsafe { invalidate_local(&root) };
        }
    }
    Ok(())
}

/// Invalidates this root's tag locally without installing it.
///
/// # Safety
///
/// The identifier must remain retained and the caller must prevent lower-EL
/// execution from using the local tag until completion.
pub(crate) unsafe fn invalidate_local(root: &PreparedAddressSpace) {
    if root.is_vhe() {
        let operand = u64::from(root.identifier()) << registers::TTBR_ASID_SHIFT;
        // SAFETY: The caller supplies the identifier retention contract.
        unsafe {
            asm!(
                "tlbi aside1, {operand}",
                "dsb ish",
                "isb",
                operand = in(reg) operand,
                options(nostack, preserves_flags)
            )
        };
    } else {
        let saved_vttbr: u64;
        // SAFETY: The temporary register interval executes only at EL2 and is
        // restored after the local tagged stage-2 invalidation completes.
        unsafe {
            asm!(
                "mrs {saved_vttbr}, VTTBR_EL2",
                "msr VTTBR_EL2, {root}",
                "isb",
                root = in(reg) root.root_register(),
                saved_vttbr = out(reg) saved_vttbr,
                options(nostack, preserves_flags)
            )
        };
        // SAFETY: The request retains the selected VMID and excludes lower EL.
        unsafe { invalidate_selected_stage2() };
        // SAFETY: The tagged invalidation is complete, so restoring the saved
        // VTTBR cannot expose references to the request-owned hierarchy.
        unsafe {
            asm!(
                "msr VTTBR_EL2, {saved_vttbr}",
                "isb",
                saved_vttbr = in(reg) saved_vttbr,
                options(nostack, preserves_flags)
            )
        };
    }
}

/// Invalidates the stage-2/combined regime selected by the current VTTBR.
///
/// HCR.TGE can retarget combined-regime TLB maintenance. Every native stage-2
/// path therefore selects an explicit guest-translation regime for the TLBI
/// and restores the exact preceding HCR before returning.
///
/// # Safety
///
/// The current VTTBR identifier must remain retained, and lower-EL execution
/// must remain excluded until this function returns.
unsafe fn invalidate_selected_stage2() {
    let _saved_hcr: u64;
    let _tlbi_hcr: u64;
    // SAFETY: The caller owns the selected VTTBR interval. DSB completes the
    // invalidation before HCR restoration, and the final ISB synchronizes the
    // restored exception and translation regime.
    unsafe {
        asm!(
            "mrs {_saved_hcr}, HCR_EL2",
            "bic {_tlbi_hcr}, {_saved_hcr}, {tge}",
            "orr {_tlbi_hcr}, {_tlbi_hcr}, {vm}",
            "msr HCR_EL2, {_tlbi_hcr}",
            "isb",
            "tlbi vmalls12e1",
            "dsb ish",
            "isb",
            "msr HCR_EL2, {_saved_hcr}",
            "isb",
            _saved_hcr = out(reg) _saved_hcr,
            _tlbi_hcr = out(reg) _tlbi_hcr,
            tge = in(reg) registers::HCR_EL2_TGE,
            vm = in(reg) registers::HCR_EL2_VM,
            options(nostack, preserves_flags)
        )
    };
}

fn stage1_index(virtual_address: u64, level: usize) -> usize {
    registers::stage1_table_index(virtual_address, level, address::STAGE1_VA_BITS)
}

fn stage2_index(ipa: u64, level: usize) -> usize {
    ((ipa >> registers::STAGE2_LEVEL_SHIFTS_4K[level]) & 0x1ff) as usize
}

fn validate_table(table: PhysicalAddress, order: usize) -> Result<(), Error> {
    let alignment = PAGE_SIZE
        .checked_shl(order as u32)
        .ok_or(Error::AddressOverflow)?;
    if !table.get().is_multiple_of(alignment) || table.get() >= address::physical_address_limit() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

fn read_entry(table: PhysicalAddress, index: usize) -> Result<u64, Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: The table hierarchy is retained and immutable except for the
    // current exclusive builder before publication.
    Ok(unsafe { read_volatile(pointer.add(index)) })
}

fn write_entry(table: PhysicalAddress, index: usize, value: u64) -> Result<(), Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: Only the unpublished builder writes this retained table page.
    unsafe { write_volatile(pointer.add(index), value) };
    Ok(())
}

fn table_pointer(table: PhysicalAddress) -> Result<*mut u64, Error> {
    memory::linear_mapping_base()
        .checked_add(table.get())
        .and_then(|address| usize::try_from(address).ok())
        .map(core::ptr::with_exposed_provenance_mut::<u64>)
        .ok_or(Error::AddressOverflow)
}

fn read_hcr_el2() -> u64 {
    let value: u64;
    // SAFETY: HCR_EL2 is readable at EL2 without side effects.
    unsafe {
        asm!(
            "mrs {value}, HCR_EL2",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags)
        )
    };
    value
}

fn invalid_machine_state() -> ! {
    loop {
        // SAFETY: A contract mismatch is unrecoverable before lower-EL entry.
        unsafe { asm!("wfe", options(nomem, nostack, preserves_flags)) };
    }
}
