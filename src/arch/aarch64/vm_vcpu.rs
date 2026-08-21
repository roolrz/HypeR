// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` vCPU timer and virtual-interrupt activation.

use hyper::vm::interrupt::{VirtualCpuId, VirtualInterruptId};

use super::{VmInterruptController, vm_timer};
use crate::kernel::task::thread::VcpuExecution;
use crate::kernel::vm::active_vcpu;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Active(active_vcpu::Error),
    Architecture(super::VgicError),
    Bridge(vm_timer::Error),
    Controller(hyper::vm::interrupt::Error),
    HostInterrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerValidationError {
    ActiveBridge(vm_timer::Error),
    Context(super::VgicError),
    Interrupts(super::VmInterruptError),
    Model(hyper::vm::interrupt::Error),
    StateMismatch,
    Vcpu(Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationError {
    ArchitectedTimer(TimerValidationError),
}

impl From<TimerValidationError> for InitializationError {
    fn from(error: TimerValidationError) -> Self {
        Self::ArchitectedTimer(error)
    }
}

impl From<active_vcpu::Error> for Error {
    fn from(error: active_vcpu::Error) -> Self {
        Self::Active(error)
    }
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

impl VcpuExecution {
    /// Activates the timer and interrupt hardware used by a guest vCPU.
    ///
    /// # Safety
    ///
    /// `execution` must be non-null, aligned, pinned, and exclusively owned by
    /// the stopped vCPU. No Rust reference to it may span this call: successful
    /// activation publishes the pointer for exception reentry. The caller must
    /// keep local IRQs masked until guest entry.
    pub unsafe fn activate_virtual_hardware(execution: *mut Self) -> Result<(), Error> {
        // SAFETY: The caller's valid pinned execution contains either an
        // installed VmBinding or the timer-validation binding whose lifetime
        // is part of its construction contract. The raw pointer targets the
        // VM allocation rather than the execution itself.
        let interrupts = unsafe { core::ptr::from_ref((&*execution).interrupts()) };
        {
            // SAFETY: The caller supplies the non-null, aligned, exclusively
            // owned pointer. This reference ends before raw publication below.
            let execution = unsafe { &mut *execution };
            // SAFETY: The binding contract above keeps the model live, and
            // this reference ends before active execution publication.
            let interrupts = unsafe { &*interrupts };
            let now = crate::kernel::time::monotonic_ticks();
            vm_timer::reconcile_saved(execution, interrupts, now)?;
            let timer_asserted = execution.context.virtual_timer_interrupt_asserted_at(now);
            set_host_timer_enabled(!timer_asserted)?;

            // SAFETY: The caller owns this stopped vCPU and IRQs remain masked.
            unsafe { execution.context.activate_system_registers() };
            // SAFETY: The same exclusive stopped-vCPU contract covers its timer.
            unsafe { execution.context.activate_timer() };
            let result = interrupts.with(|controller| {
                let vcpu = VirtualCpuId::new(execution.vcpu_id);
                controller.synchronize(vcpu, execution.context.vgic.slots())?;
                let _ = controller.refill(vcpu, execution.context.vgic.slots_mut())?;
                Ok::<(), hyper::vm::interrupt::Error>(())
            });
            if let Err(error) = result {
                // SAFETY: Activation above made this the live local timer bank.
                unsafe { execution.context.deactivate_timer() };
                // SAFETY: Guest execution never began; this context still owns
                // the live local system-register bank.
                unsafe { execution.context.deactivate_system_registers() };
                set_host_timer_enabled(true)?;
                return Err(error.into());
            }
            // SAFETY: Model synchronization completed while this stopped vCPU
            // remained exclusively owned with local IRQs masked.
            if let Err(error) = unsafe { execution.context.activate_vgic() } {
                // SAFETY: The timer was activated above and remains local.
                unsafe { execution.context.deactivate_timer() };
                // SAFETY: Guest execution never began and the context owns the
                // live system-register bank.
                unsafe { execution.context.deactivate_system_registers() };
                set_host_timer_enabled(true)?;
                return Err(error.into());
            }
        }

        // SAFETY: The caller guarantees the pointer is pinned and exclusive;
        // all temporary references ended before publication.
        if let Err(error) = unsafe { active_vcpu::set_raw(execution) } {
            // SAFETY: Publication failed, so the caller's exclusive ownership
            // permits a fresh temporary reference for rollback.
            let execution = unsafe { &mut *execution };
            super::disable_vgic();
            // SAFETY: This context still owns the activated local timer bank.
            unsafe { execution.context.deactivate_timer() };
            // SAFETY: Guest execution never began and this context still owns
            // the live system-register bank.
            unsafe { execution.context.deactivate_system_registers() };
            set_host_timer_enabled(true)?;
            return Err(error.into());
        }
        Ok(())
    }

    /// Saves guest timer/vGIC state and removes the local active binding.
    ///
    /// # Safety
    ///
    /// This must be the active local vCPU with local IRQs masked.
    pub unsafe fn deactivate_virtual_hardware(&mut self) -> Result<(), Error> {
        let interrupts = core::ptr::from_ref(self.interrupts());
        active_vcpu::clear(self)?;
        // SAFETY: Clearing succeeded for this active vCPU with IRQs masked, so
        // its local timer bank is exclusively owned here.
        unsafe { self.context.deactivate_timer() };
        // SAFETY: This is the active local vCPU and delivery is quiesced by the
        // masked exception context.
        let vgic_result = unsafe { self.context.deactivate_vgic() };
        // SAFETY: Guest execution has stopped and this context owns the live
        // system-register bank.
        unsafe { self.context.deactivate_system_registers() };
        let state_result = (|| {
            // SAFETY: Clearing the active binding does not release the
            // execution-owned VM binding; it remains live through this call.
            let interrupts = unsafe { &*interrupts };
            vgic_result?;
            interrupts.with(|controller| {
                controller.synchronize(VirtualCpuId::new(self.vcpu_id), self.context.vgic.slots())
            })?;
            vm_timer::reconcile_saved(self, interrupts, crate::kernel::time::monotonic_ticks())?;
            Ok::<(), Error>(())
        })();
        let host_timer_result = set_host_timer_enabled(true);
        state_result?;
        host_timer_result
    }
}

pub(crate) fn deliver_software_interrupt(
    execution: &mut VcpuExecution,
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
    let source = execution.vcpu_id;

    // SAFETY: Guest synchronous entry masked local IRQs and this is the active vCPU.
    unsafe { execution.context.deactivate_vgic()? };
    let result = interrupts.with(|controller| {
        let current = VirtualCpuId::new(source);
        controller.synchronize(current, execution.context.vgic.slots())?;
        for index in 0..interrupts.vcpu_count() {
            let aff0 = u64::from(index & 0xff);
            let aff1 = u64::from((index >> 8) & 0xff);
            let aff2 = u64::from((index >> 16) & 0xff);
            let aff3 = u64::from((index >> 24) & 0xff);
            let selected = if broadcast {
                index != source
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
        let _ = controller.refill(current, execution.context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The complete model was reconciled into this still-active vCPU;
    // synchronous guest entry keeps local IRQs masked.
    unsafe { execution.context.activate_vgic()? };
    Ok(())
}

pub(crate) fn handle_guest_device_access(
    _execution: &mut VcpuExecution,
    _interrupts: &VmInterruptController,
    _frame: &mut super::GuestSyncFrame<'_>,
    fallback: super::GuestSyncAction,
) -> super::GuestSyncAction {
    fallback
}

pub fn validate_arch_timer(
    timer_interrupt: hyper::hal::interrupt::InterruptId,
) -> Result<(), TimerValidationError> {
    let timer_interrupt = VirtualInterruptId::new(timer_interrupt.get()).ok_or(
        TimerValidationError::Interrupts(super::VmInterruptError::InvalidInterrupt),
    )?;
    let interrupts =
        VmInterruptController::new(1, timer_interrupt).map_err(TimerValidationError::Interrupts)?;
    let mut context = super::VcpuContext::new(0);
    let _ = context
        .initialize_virtual_interrupts()
        .map_err(TimerValidationError::Context)?;
    let now = crate::kernel::time::monotonic_ticks();
    context.set_virtual_count(now, now);
    context.set_virtual_timer_deadline(now.wrapping_add(1_000_000));
    context.set_virtual_timer_enabled(true);
    // SAFETY: `interrupts` is declared before `execution`, remains fixed on
    // this stack, and outlives activation, deactivation, and execution drop.
    let mut execution = unsafe { VcpuExecution::for_timer_validation(context, &interrupts) };
    // SAFETY: Boot validation owns these pinned objects and keeps IRQs masked.
    unsafe {
        VcpuExecution::activate_virtual_hardware(core::ptr::addr_of_mut!(execution))
            .map_err(TimerValidationError::Vcpu)?;
    }
    // Once activation publishes `execution`, every validation outcome must
    // clear that publication before this stack frame can be destroyed.
    let validation = (|| {
        if !vm_timer::inject_active_for_validation().map_err(TimerValidationError::ActiveBridge)? {
            return Err(TimerValidationError::StateMismatch);
        }
        let snapshot = interrupts
            .timer_snapshot(VirtualCpuId::new(0))
            .map_err(TimerValidationError::Model)?;
        if !snapshot.pending || !snapshot.listed {
            return Err(TimerValidationError::StateMismatch);
        }
        Ok(())
    })();
    // SAFETY: This remains the active validation vCPU with IRQs masked.
    let deactivation = unsafe {
        execution
            .deactivate_virtual_hardware()
            .map_err(TimerValidationError::Vcpu)
    };
    // An active-slot cleanup failure cannot be returned: doing so would drop
    // `execution` while the per-CPU slot still reaches it. Later deactivation
    // failures occur after `active_vcpu::clear` and are safe to report.
    match deactivation {
        Ok(()) => validation,
        Err(TimerValidationError::Vcpu(Error::Active(error))) => {
            crate::pr_crit!("HypeR: timer-validation vCPU cleanup failed: {error:?}");
            super::halt()
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn update_guest_device_interrupt(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    interrupt: VirtualInterruptId,
    asserted: bool,
) -> Result<(), Error> {
    // SAFETY: `active_vcpu::with` supplied the active local vCPU while IRQs are
    // masked, so its virtual interface may be temporarily quiesced.
    unsafe { execution.context.deactivate_vgic()? };
    let result = interrupts.with(|controller| {
        let vcpu = VirtualCpuId::new(execution.vcpu_id);
        controller.synchronize(vcpu, execution.context.vgic.slots())?;
        if asserted {
            controller.inject(interrupt, vcpu)?;
        } else {
            controller.clear_pending(interrupt, vcpu)?;
        }
        let _ = controller.refill(vcpu, execution.context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        super::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: Refill completed for the same active local vCPU while IRQs remain
    // masked, so virtual delivery may resume.
    unsafe { execution.context.activate_vgic()? };
    Ok(())
}

fn set_host_timer_enabled(enabled: bool) -> Result<(), Error> {
    let interrupt =
        crate::kernel::irq::timer::guest_virtual_host_interrupt().ok_or(Error::HostInterrupt)?;
    let result = if enabled {
        crate::kernel::irq::interrupt::enable_local(interrupt)
    } else {
        crate::kernel::irq::interrupt::disable_local(interrupt)
    };
    result.map_err(|_| Error::HostInterrupt)
}
