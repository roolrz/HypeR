// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V vCPU hardware activation.

use super::VmInterruptController;
use crate::kernel::task::thread::VcpuExecution;
use crate::kernel::vm::active_vcpu;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Active(active_vcpu::Error),
    UnsupportedSoftwareInterrupt,
}

pub(crate) fn deliver_software_interrupt(
    _execution: &mut VcpuExecution,
    _interrupts: &VmInterruptController,
    _request: u64,
) -> Result<(), Error> {
    Err(Error::UnsupportedSoftwareInterrupt)
}

pub(crate) fn handle_guest_device_access(
    _execution: &mut VcpuExecution,
    _interrupts: &VmInterruptController,
    _frame: &mut super::GuestSyncFrame<'_>,
    fallback: super::GuestSyncAction,
) -> super::GuestSyncAction {
    fallback
}

impl From<active_vcpu::Error> for Error {
    fn from(error: active_vcpu::Error) -> Self {
        Self::Active(error)
    }
}

impl VcpuExecution {
    /// Loads the stopped vCPU into the current hart's virtualization state.
    ///
    /// # Safety
    ///
    /// `execution` must be non-null, aligned, pinned, and exclusively owned by
    /// the caller for the guest-run lifetime. Local interrupts must be masked.
    pub unsafe fn activate_virtual_hardware(execution: *mut Self) -> Result<(), Error> {
        // SAFETY: The caller guarantees `execution` is valid and exclusive.
        // This temporary context reference ends before the raw pointer is published.
        unsafe { (*execution).context.activate_system_registers() };
        // SAFETY: The scheduler-origin pointer remains pinned for the active run.
        unsafe { active_vcpu::set_raw(execution)? };
        Ok(())
    }

    /// Saves the current hart's virtualization state into the active vCPU.
    ///
    /// # Safety
    ///
    /// The caller must own the active vCPU and keep interrupts masked.
    pub unsafe fn deactivate_virtual_hardware(&mut self) -> Result<(), Error> {
        active_vcpu::clear(self)?;
        // SAFETY: This method's contract identifies `self` as the active owned vCPU.
        unsafe { self.context.deactivate_system_registers() };
        Ok(())
    }
}
