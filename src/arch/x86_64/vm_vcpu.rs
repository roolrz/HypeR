// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! x86 vCPU hardware-state mechanisms.

use super::{GuestSyncAction, GuestSyncFrame, VcpuContext, VmInterruptController};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedSoftwareInterrupt,
}

/// Admits a stopped vCPU for selected-backend entry.
///
/// x86 VMX/SVM loads the guest machine state in the final entry backend, so
/// this phase has no local register bank to load.
///
/// # Safety
///
/// The caller must exclusively own the pinned stopped context and keep local
/// interrupts masked until guest entry.
pub const unsafe fn activate(
    _context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    _physical_count: u64,
) -> Result<bool, Error> {
    Ok(false)
}

/// Completes selected-backend detachment after an x86 VM exit.
///
/// # Safety
///
/// The caller must exclusively own the stopped context and keep local
/// interrupts masked through the surrounding scheduler transaction.
pub const unsafe fn deactivate(
    _context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    _physical_count: u64,
) -> Result<(), Error> {
    Ok(())
}

pub(crate) fn deliver_software_interrupt(
    _context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    _request: u64,
) -> Result<(), Error> {
    Err(Error::UnsupportedSoftwareInterrupt)
}

pub(crate) fn handle_guest_device_access(
    _context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    _frame: &mut GuestSyncFrame<'_>,
    fallback: GuestSyncAction,
) -> GuestSyncAction {
    fallback
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
