// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V vCPU hardware-state mechanisms.

use core::arch::asm;

use super::{VcpuContext, VmInterruptController};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    SupervisorTimerCompareUnavailable,
}

/// Loads one exclusively owned stopped vCPU into the current hart.
///
/// # Safety
///
/// `context` must be pinned and exclusively owned, no vCPU may already be
/// active on this hart, and local interrupts must be masked.
pub unsafe fn activate(
    context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    _physical_count: u64,
) -> Result<bool, Error> {
    if !enable_supervisor_timer_compare() {
        return Err(Error::SupervisorTimerCompareUnavailable);
    }
    // SAFETY: The caller grants exclusive ownership of the stopped context.
    unsafe { context.activate_system_registers() };
    Ok(false)
}

fn enable_supervisor_timer_compare() -> bool {
    const HENVCFG_STCE: u64 = 1 << 63;
    let environment: u64;
    // SAFETY: HENVCFG is available because the H extension is a platform
    // requirement. STCE is writable only when firmware enabled MENVCFG.STCE;
    // reading it back validates this hart before any VSTIMECMP access.
    unsafe {
        asm!(
            "csrs henvcfg, {stce}",
            "csrr {environment}, henvcfg",
            stce = in(reg) HENVCFG_STCE,
            environment = out(reg) environment,
            options(nomem, nostack)
        )
    };
    environment & HENVCFG_STCE != 0
}

/// Saves the current hart's guest state into its exclusively owned context.
///
/// # Safety
///
/// `context` must own the active local vCPU and local interrupts must remain
/// masked until the save completes.
pub unsafe fn deactivate(
    context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    _physical_count: u64,
) -> Result<(), Error> {
    // SAFETY: The caller identifies this context as the active local vCPU.
    unsafe { context.deactivate_system_registers() };
    Ok(())
}

pub const fn handle_virtual_timer_interrupt(
    _context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
) -> Result<bool, Error> {
    Ok(false)
}

pub const fn handle_maintenance_interrupt(
    _context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
) -> Result<bool, Error> {
    Ok(false)
}

pub const fn maintenance_interrupt_pending() -> bool {
    false
}

pub const fn quiesce_virtual_interrupt_delivery() {}
