// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native EL0 machine capability selection for `AArch64`.
//!
//! Native EL0 uses the host stage-1 translation regime. This layer does
//! not own process mappings, syscall policy, or a runnable entry operation.

use core::arch::asm;

use super::user_contract::UserExecutionCapabilities;
pub use super::user_contract::UserMachineContractError;

/// Reports the native-user host-stage contract. Eight-bit ASIDs are selected
/// conservatively until discovery publishes optional wider identifier fields.
pub(super) fn execution_capabilities() -> Result<UserExecutionCapabilities, UserMachineContractError>
{
    let address = super::address::capabilities();
    if !supports_pan() {
        return Err(UserMachineContractError::UnsupportedPrivilegedAccessProtection);
    }
    UserExecutionCapabilities::new(
        address.virtual_address_bits,
        address.physical_address_bits,
        8,
    )
}

fn supports_pan() -> bool {
    let features: u64;
    // SAFETY: The feature register is read-only and this function changes no
    // machine state. EL2 may read the architected field directly.
    unsafe {
        asm!(
            "mrs {features}, ID_AA64MMFR1_EL1",
            features = out(reg) features,
            options(nomem, nostack, preserves_flags),
        );
    }
    (features >> super::registers::ID_AA64MMFR1_PAN_SHIFT) & super::registers::ID_AA64MMFR1_PAN_MASK
        != 0
}

/// Checks native-user protection features required on every admitted PE.
pub(crate) fn current_cpu_is_compatible() -> bool {
    supports_pan()
}

/// Returns the exclusive virtual-address limit available to native userspace.
///
/// VHE assigns native userspace the complete lower `TTBR0_EL2` range while the
/// host remains in the upper `TTBR1_EL2` range.
pub fn user_address_limit() -> Result<u64, UserMachineContractError> {
    execution_capabilities()?;
    Ok(super::address::STAGE1_VA_LIMIT)
}

/// Returns the selected ARM64 layout's exclusive application mapping limit.
pub fn application_address_limit() -> u64 {
    super::address::STAGE1_LAYOUT.application_address_limit()
}

/// Establishes the privileged-access invariant before a VHE user root is
/// installed on this CPU.
pub(crate) fn assert_kernel_pan() -> Result<(), UserMachineContractError> {
    if !supports_pan() {
        return Err(UserMachineContractError::UnsupportedPrivilegedAccessProtection);
    }
    // SAFETY: Capability discovery proves FEAT_PAN. Asserting PAN only removes
    // privileged access to EL0 mappings and is intentionally retained by all
    // subsequent kernel contexts on this CPU. Use the architectural generic
    // system-register encoding so the minimal LLVM target need not advertise
    // PAN at compile time; runtime admission above remains authoritative.
    unsafe {
        asm!(
            "msr S3_0_C4_C2_3, {enabled}",
            enabled = in(reg) 1_u64,
            options(nomem, nostack, preserves_flags)
        );
    }
    Ok(())
}

/// Copies bytes from machine-visible normal memory into private kernel memory.
///
/// # Safety
///
/// Both ranges must be valid for `length`, resident, non-faulting, and
/// non-overlapping. The source may be concurrently accessed by another PE.
/// If an asynchronous machine failure interrupts the loop, earlier bytes may
/// already have been copied.
pub(crate) unsafe fn copy_from_exposed(source: *const u8, destination: *mut u8, length: usize) {
    // SAFETY: The caller establishes residency, validity, and non-overlap. A
    // single asm block prevents LLVM from replacing the operation with memcpy
    // or assuming exclusive Rust access. Omitting `nomem` supplies the compiler
    // memory clobber required for externally mutable memory.
    unsafe {
        asm!(
            "cbz {length}, 3f",
            "2:",
            "ldrb {byte:w}, [{source}], #1",
            "strb {byte:w}, [{destination}], #1",
            "subs {length}, {length}, #1",
            "b.ne 2b",
            "3:",
            source = inout(reg) source => _,
            destination = inout(reg) destination => _,
            length = inout(reg) length => _,
            byte = out(reg) _,
            options(nostack),
        );
    }
}

/// Copies bytes from private kernel memory into machine-visible normal memory.
///
/// # Safety
///
/// Both ranges must be valid for `length`, resident, non-faulting, and
/// non-overlapping. The destination may be concurrently accessed by another
/// PE. Earlier bytes are observable if the operation is interrupted.
pub(crate) unsafe fn copy_to_exposed(source: *const u8, destination: *mut u8, length: usize) {
    // SAFETY: The proof and compiler-memory contract are identical to
    // copy_from_exposed; direction does not change the byte-loop mechanics.
    unsafe {
        asm!(
            "cbz {length}, 3f",
            "2:",
            "ldrb {byte:w}, [{source}], #1",
            "strb {byte:w}, [{destination}], #1",
            "subs {length}, {length}, #1",
            "b.ne 2b",
            "3:",
            source = inout(reg) source => _,
            destination = inout(reg) destination => _,
            length = inout(reg) length => _,
            byte = out(reg) _,
            options(nostack),
        );
    }
}
