// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` vCPU timer and virtual-interrupt hardware-state mechanisms.

use hyper::vm::interrupt::{VirtualCpuId, VirtualInterruptId};

use super::{GuestSyncAction, GuestSyncFrame, VcpuContext, VmInterruptController, vm_timer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Architecture(super::VgicError),
    Bridge(vm_timer::Error),
    Controller(hyper::vm::interrupt::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationError {
    MissingHostTimerInterrupt,
}

impl From<super::VgicError> for Error {
    fn from(error: super::VgicError) -> Self {
        Self::Architecture(error)
    }
}

impl From<hyper::vm::interrupt::Error> for Error {
    fn from(error: hyper::vm::interrupt::Error) -> Self {
        Self::Controller(error)
    }
}

impl From<vm_timer::Error> for Error {
    fn from(error: vm_timer::Error) -> Self {
        Self::Bridge(error)
    }
}

/// Loads the timer, system-register, and virtual-interrupt state of a stopped
/// vCPU into the current PE.
///
/// # Safety
///
/// `context` must be pinned and exclusively owned by the stopped vCPU. No
/// virtual hardware may already be active locally, stage-2 must already select
/// this VM, and local interrupts must remain masked.
pub unsafe fn activate(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    physical_count: u64,
) -> Result<bool, Error> {
    vm_timer::reconcile_saved(context, vcpu_id, interrupts, physical_count)?;
    let timer_asserted = context.virtual_timer_interrupt_asserted_at(physical_count);

    // SAFETY: The caller owns this stopped vCPU and IRQs remain masked.
    unsafe { context.activate_system_registers() };
    // SAFETY: The same exclusive stopped-vCPU contract covers its timer.
    unsafe { context.activate_timer() };
    let result = interrupts.with(|controller| {
        let vcpu = VirtualCpuId::new(vcpu_id);
        controller.synchronize(vcpu, context.vgic.slots())?;
        let _ = controller.refill(vcpu, context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        // SAFETY: Activation above made this the live local timer bank.
        unsafe { context.deactivate_timer() };
        // SAFETY: Guest execution never began and this context still owns the
        // live local system-register bank.
        unsafe { context.deactivate_system_registers() };
        return Err(error.into());
    }
    // SAFETY: Model synchronization completed while this stopped vCPU remained
    // exclusively owned with local IRQs masked.
    if let Err(error) = unsafe { context.activate_vgic() } {
        super::disable_vgic();
        // SAFETY: The timer was activated above and remains local.
        unsafe { context.deactivate_timer() };
        // SAFETY: Guest execution never began and this context owns the live
        // system-register bank.
        unsafe { context.deactivate_system_registers() };
        return Err(error.into());
    }
    Ok(timer_asserted)
}

/// Saves and detaches the active local timer, vGIC, and system-register state.
///
/// # Safety
///
/// `context` must exclusively own the active local vCPU, guest execution must
/// have stopped, and local interrupts must remain masked.
pub unsafe fn deactivate(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    physical_count: u64,
) -> Result<(), Error> {
    // SAFETY: This context owns the active local timer bank.
    unsafe { context.deactivate_timer() };
    // SAFETY: Guest execution is stopped and local interrupts are masked.
    let vgic_result = unsafe { context.deactivate_vgic() };
    if vgic_result.is_err() {
        super::disable_vgic();
    }
    // SAFETY: This context owns the live guest system-register bank.
    unsafe { context.deactivate_system_registers() };
    vgic_result?;
    interrupts.with(|controller| {
        controller.synchronize(VirtualCpuId::new(vcpu_id), context.vgic.slots())
    })?;
    vm_timer::reconcile_saved(context, vcpu_id, interrupts, physical_count)?;
    Ok(())
}

pub(crate) fn deliver_software_interrupt(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    request: u64,
) -> Result<(), Error> {
    const TARGET_LIST_MASK: u64 = 0xffff;
    const AFFINITY_1_SHIFT: u32 = 16;
    const INTERRUPT_SHIFT: u32 = 24;
    const AFFINITY_2_SHIFT: u32 = 32;
    const BROADCAST: u64 = 1 << 40;
    const RANGE_SHIFT: u32 = 44;
    const AFFINITY_3_SHIFT: u32 = 48;

    let interrupt = VirtualInterruptId::new(((request >> INTERRUPT_SHIFT) & 0xf) as u32).ok_or(
        Error::Controller(hyper::vm::interrupt::Error::NotConfigured),
    )?;
    let target_list = request & TARGET_LIST_MASK;
    let affinity_1 = (request >> AFFINITY_1_SHIFT) & 0xff;
    let affinity_2 = (request >> AFFINITY_2_SHIFT) & 0xff;
    let affinity_3 = (request >> AFFINITY_3_SHIFT) & 0xff;
    let range = (request >> RANGE_SHIFT) & 0xf;
    let broadcast = request & BROADCAST != 0;

    // SAFETY: Guest synchronous entry masked local IRQs and this is the active vCPU.
    if let Err(error) = unsafe { context.deactivate_vgic() } {
        super::disable_vgic();
        return Err(error.into());
    }
    let result = interrupts.with(|controller| {
        let current = VirtualCpuId::new(vcpu_id);
        controller.synchronize(current, context.vgic.slots())?;
        for index in 0..interrupts.vcpu_count() {
            let aff0 = u64::from(index & 0xff);
            let aff1 = u64::from((index >> 8) & 0xff);
            let aff2 = u64::from((index >> 16) & 0xff);
            let aff3 = u64::from((index >> 24) & 0xff);
            let selected = if broadcast {
                index != vcpu_id
            } else {
                aff1 == affinity_1
                    && aff2 == affinity_2
                    && aff3 == affinity_3
                    && aff0 >= range * 16
                    && aff0 < range * 16 + 16
                    && target_list & (1 << (aff0 - range * 16)) != 0
            };
            if selected {
                controller.inject(interrupt, VirtualCpuId::new(index))?;
            }
        }
        let _ = controller.refill(current, context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The complete model was reconciled into this still-active vCPU.
    unsafe { context.activate_vgic()? };
    Ok(())
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

pub fn handle_virtual_timer_interrupt(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
) -> Result<bool, Error> {
    let asserted = context.virtual_timer_interrupt_asserted_hardware();
    vm_timer::refresh_running_vgic(context, vcpu_id, interrupts, asserted)?;
    Ok(asserted)
}

pub fn handle_maintenance_interrupt(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
) -> Result<bool, Error> {
    let asserted = context.virtual_timer_interrupt_asserted_hardware();
    vm_timer::refresh_running_vgic(context, vcpu_id, interrupts, asserted)?;
    Ok(!asserted)
}

pub fn maintenance_interrupt_pending() -> bool {
    super::vgic_maintenance_state().status != 0
}

pub fn quiesce_virtual_interrupt_delivery() {
    super::disable_vgic();
}

pub fn inject_timer_for_validation(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
) -> Result<(), Error> {
    vm_timer::refresh_running_vgic(context, vcpu_id, interrupts, true).map_err(Into::into)
}

pub(crate) fn update_guest_device_interrupt(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    interrupt: VirtualInterruptId,
    asserted: bool,
) -> Result<(), Error> {
    // SAFETY: Guest-exit dispatch supplied the active local vCPU with IRQs masked.
    if let Err(error) = unsafe { context.deactivate_vgic() } {
        super::disable_vgic();
        return Err(error.into());
    }
    let result = interrupts.with(|controller| {
        let vcpu = VirtualCpuId::new(vcpu_id);
        controller.synchronize(vcpu, context.vgic.slots())?;
        if asserted {
            controller.inject(interrupt, vcpu)?;
        } else {
            controller.clear_pending(interrupt, vcpu)?;
        }
        let _ = controller.refill(vcpu, context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: Refill completed for the same active local vCPU.
    unsafe { context.activate_vgic()? };
    Ok(())
}
