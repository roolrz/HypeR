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
    UserMachineContractError, UserPagePermissions, UserTranslationRegisters,
};
use super::{address, memory, registers};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    Conflict,
    Contract(UserMachineContractError),
    InvalidAddress,
    InvalidRange,
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
}

#[derive(Clone, Copy)]
pub(crate) enum LocalOperation {
    Replace,
    Invalidate,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalRequest {
    root_register: u64,
    generation: u64,
    operation: LocalOperation,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalIdentity {
    root_register: u64,
}

pub(crate) struct LocalActivation {
    installed_identifier: u16,
    previous_root: u64,
    previous_hcr: u64,
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

    pub(crate) const fn local_request(&self, operation: LocalOperation) -> LocalRequest {
        LocalRequest {
            root_register: self.root_register(),
            generation: self.generation(),
            operation,
        }
    }

    pub(crate) const fn local_identity(&self) -> LocalIdentity {
        LocalIdentity {
            root_register: self.root_register(),
        }
    }
}

pub(crate) fn local_identity_is_active(identity: LocalIdentity) -> bool {
    let current: u64;
    // SAFETY: TTBR0_EL2 is readable at EL2 without side effects.
    unsafe {
        asm!("mrs {current}, TTBR0_EL2", current=out(reg) current, options(nomem,nostack,preserves_flags));
    }
    current == identity.root_register
}

/// Builds an immutable VHE EL2&0 stage-1 root.
///
/// # Safety
///
/// Each allocator result must be a uniquely owned, zeroed, linearly mapped
/// table block with the requested order and natural alignment. The caller
/// retains every block until acknowledged retirement of the returned root.
pub(crate) unsafe fn prepare_address_space(
    asid: u16,
    generation: u64,
    mut enumerate: impl FnMut(&mut dyn FnMut(MappingPage)),
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<PreparedAddressSpace, Error> {
    let capabilities = super::user::execution_capabilities()?;
    let root = allocator(0).ok_or(Error::Allocation)?;
    validate_table(root)?;
    let mut builder = Builder { root, allocator };
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
        registers: UserTranslationRegisters::new(capabilities, root.get(), asid, generation)?,
    })
}

struct Builder<'allocator, Allocator> {
    root: PhysicalAddress,
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
        self.map_stage1(page, permissions)
    }

    fn map_stage1(
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
            permissions.stage1_descriptor(page.physical.get()),
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
        validate_table(child)?;
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
            operand=in(reg) operand,
            root=in(reg) root.root_register(),
            previous_root=out(reg) previous_root,
            previous_hcr=out(reg) previous_hcr,
            options(nostack,preserves_flags)
        );
    }
    LocalActivation {
        installed_identifier: root.identifier(),
        previous_root,
        previous_hcr,
        not_send_or_sync: PhantomData,
    }
}

/// Leaves one local native translation interval and restores its predecessor.
///
/// # Safety
///
/// This must run on the same pinned CPU which created `activation`, after
/// lower-EL execution has stopped and while update admission remains excluded.
pub(crate) unsafe fn deactivate_local(activation: LocalActivation) {
    let operand = u64::from(activation.installed_identifier) << registers::TTBR_ASID_SHIFT;
    // SAFETY: The consumed CPU-affine token supplies same-PE ownership.
    unsafe {
        asm!(
            "tlbi aside1, {operand}",
            "dsb ish",
            "msr TTBR0_EL2, {root}",
            "msr HCR_EL2, {hcr}",
            "isb",
            operand=in(reg) operand,
            root=in(reg) activation.previous_root,
            hcr=in(reg) activation.previous_hcr,
            options(nostack,preserves_flags)
        );
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
            root_address,
            identifier,
            request.generation,
        )?,
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
    let operand = u64::from(root.identifier()) << registers::TTBR_ASID_SHIFT;
    // SAFETY: The caller supplies identifier retention and excludes lower-EL
    // use until the local tagged invalidation and context synchronization finish.
    unsafe {
        asm!(
            "tlbi aside1, {operand}",
            "dsb ish",
            "isb",
            operand=in(reg) operand,
            options(nostack,preserves_flags)
        );
    }
}

fn stage1_index(virtual_address: u64, level: usize) -> usize {
    registers::stage1_table_index(virtual_address, level, address::STAGE1_VA_BITS)
}

fn validate_table(table: PhysicalAddress) -> Result<(), Error> {
    memory::linear_page_address(table)
        .map(|_| ())
        .ok_or(Error::InvalidAddress)
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
    memory::linear_page_address(table)
        .map(core::ptr::with_exposed_provenance_mut::<u64>)
        .ok_or(Error::InvalidAddress)
}
