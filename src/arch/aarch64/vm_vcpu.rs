// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` vCPU timer and virtual-interrupt hardware-state mechanisms.

use hyper::vm::arm::gic::GicInterruptId;
use hyper::vm::interrupt::VirtualCpuId;

use super::{VcpuContext, VmInterruptController, vm_timer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Architecture(super::VgicError),
    Bridge(vm_timer::Error),
    Controller(hyper::vm::arm::gic::RuntimeError),
    GuestRun(super::context::GuestRunError),
    ReturnWorld(super::lower_el::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GicAccessError {
    Architecture(super::VgicError),
    Transaction(super::vm_interrupt::AccessError),
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

impl From<hyper::vm::arm::gic::RuntimeError> for Error {
    fn from(error: hyper::vm::arm::gic::RuntimeError) -> Self {
        Self::Controller(error)
    }
}

impl From<vm_timer::Error> for Error {
    fn from(error: vm_timer::Error) -> Self {
        Self::Bridge(error)
    }
}

impl From<super::lower_el::Error> for Error {
    fn from(error: super::lower_el::Error) -> Self {
        Self::ReturnWorld(error)
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
        Ok::<(), hyper::vm::arm::gic::RuntimeError>(())
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
    if let Err(error) = super::lower_el::publish_guest(context) {
        super::disable_vgic();
        // SAFETY: Guest execution did not begin and this context still owns
        // the local timer and system-register banks activated above.
        unsafe { context.deactivate_timer() };
        // SAFETY: The failed publication left no lower-EL frame consumer.
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
    // Retire return-world ownership before changing any live guest register
    // bank. A lower-EL vector can therefore never observe partially detached
    // hardware under a still-published guest identity.
    super::lower_el::retire_guest(context)?;
    // SAFETY: The caller's active-vCPU contract remains in force after
    // return-world retirement.
    unsafe { deactivate_banks(context, vcpu_id, interrupts, physical_count) }
}

/// Detaches a terminal run whose vector already closed lower-EL ownership.
///
/// # Safety
///
/// `context` and `stopped` must identify the same active local vCPU. Local
/// interrupts remain masked and callback publication must already be cleared.
pub unsafe fn deactivate_stopped(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    physical_count: u64,
    mut stopped: super::context::StoppedGuestRun,
) -> Result<(), StoppedDeactivationFailure> {
    // SAFETY: The caller owns this terminal stopped vCPU and its live banks.
    if let Err(error) = unsafe { deactivate_banks(context, vcpu_id, interrupts, physical_count) } {
        return Err(StoppedDeactivationFailure {
            error,
            _stopped: stopped,
        });
    }
    if let Err(error) = context.consume_stopped(&mut stopped) {
        return Err(StoppedDeactivationFailure {
            error: Error::GuestRun(error),
            _stopped: stopped,
        });
    }
    Ok(())
}

unsafe fn deactivate_banks(
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

pub struct StoppedDeactivationFailure {
    error: Error,
    _stopped: super::context::StoppedGuestRun,
}

impl StoppedDeactivationFailure {
    pub const fn error(&self) -> Error {
        self.error
    }
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

    let interrupt = GicInterruptId::new(((request >> INTERRUPT_SHIFT) & 0xf) as u32).ok_or(
        Error::Controller(hyper::vm::arm::gic::RuntimeError::NotConfigured),
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
        Ok::<(), hyper::vm::arm::gic::RuntimeError>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The complete model was reconciled into this still-active vCPU.
    unsafe { context.activate_vgic()? };
    Ok(())
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
    interrupt: GicInterruptId,
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
        Ok::<(), hyper::vm::arm::gic::RuntimeError>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: Refill completed for the same active local vCPU.
    unsafe { context.activate_vgic()? };
    Ok(())
}

/// Updates only the VM-owned interrupt model for a stopped or remotely
/// running vCPU. The caller publishes durable reconcile work after this lock
/// transaction completes; this function never accesses another CPU's live
/// virtual-interface bank.
pub(crate) fn update_saved_guest_device_interrupt(
    interrupts: &VmInterruptController,
    vcpu_id: u32,
    interrupt: GicInterruptId,
    asserted: bool,
) -> Result<(), Error> {
    interrupts
        .with(|controller| {
            let vcpu = VirtualCpuId::new(vcpu_id);
            if asserted {
                controller.inject(interrupt, vcpu)
            } else {
                controller.clear_pending(interrupt, vcpu)
            }
        })
        .map_err(Into::into)
}

/// Reconciles saved interrupt-model work into the active local vGIC bank.
pub(crate) fn reconcile_active_interrupts(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
) -> Result<(), Error> {
    // SAFETY: The active-vCPU owner calls with local IRQs masked and retains
    // exclusive ownership of this CPU's live virtual interface.
    if let Err(error) = unsafe { context.deactivate_vgic() } {
        super::disable_vgic();
        return Err(error.into());
    }
    let result = interrupts.with(|controller| {
        let vcpu = VirtualCpuId::new(vcpu_id);
        controller.synchronize(vcpu, context.vgic.slots())?;
        let _ = controller.refill(vcpu, context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::arm::gic::RuntimeError>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: Synchronization and refill completed for this same local bank.
    unsafe { context.activate_vgic()? };
    Ok(())
}

/// Requests a prompt exit from a guest currently running on `cpu`.
///
/// The VM-owned reconcile bit is the durable condition. The targeted SGI is
/// only a hardware prompt and may race harmlessly with migration.
pub(crate) fn request_guest_exit(cpu: hyper::cpu::CpuIndex) -> bool {
    super::gic_cpu_interface::notify_guest_exit(cpu)
}

pub(crate) fn access_guest_gic(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    access: hyper::vm::aarch64::device::gicv3::DecodedAccess,
    operation: hyper::vm::exit::MmioOperation,
) -> Result<Option<u64>, GicAccessError> {
    // SAFETY: Guest synchronous entry masked local IRQs and owns this vCPU.
    if let Err(error) = unsafe { context.deactivate_vgic() } {
        super::disable_vgic();
        return Err(GicAccessError::Architecture(error));
    }
    let result = interrupts.access_saved_bank(
        VirtualCpuId::new(vcpu_id),
        context.vgic.slots_mut(),
        access.register(),
        operation,
    );
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            super::disable_vgic();
            return Err(GicAccessError::Transaction(error));
        }
    };
    // SAFETY: The transaction reconciled and refilled the complete saved bank.
    if let Err(error) = unsafe { context.activate_vgic() } {
        super::disable_vgic();
        return Err(GicAccessError::Architecture(error));
    }
    Ok(value)
}
