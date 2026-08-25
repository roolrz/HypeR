// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest architectural-timer host binding.
//!
//! VM policy owns the physical PPI handler used to inject a guest timer. Host
//! timekeeping supplies only the firmware-derived source description.

use hyper::hal::interrupt::{
    HostInterruptBinding, InterruptId, InterruptPriority, InterruptTrigger,
};
use hyper::platform::{PlatformInterrupt, PlatformInterruptTrigger};
use hyper::sync::atomic::{AtomicU32, Ordering};

use crate::kernel::irq::interrupt::{HandlerResult, IrqDomainId, Registration, VirtualInterrupt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Interrupt(crate::kernel::irq::interrupt::Error),
    InvalidHandlerContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValidationError {
    Active(super::active_vcpu::Error),
    Hardware(super::vcpu::HardwareTransitionError),
    Interrupts(crate::hal::vm::InterruptError),
    Machine(crate::hal::vm::TimerValidationError),
    StateMismatch,
}

const NO_HOST_TIMER: u32 = u32::MAX;
static HOST_TIMER_INTERRUPT: AtomicU32 = AtomicU32::new(NO_HOST_TIMER);

pub(super) struct PreparedBinding {
    mapping: Option<(VirtualInterrupt, Registration)>,
    maintenance: Option<(VirtualInterrupt, Registration)>,
}

impl PreparedBinding {
    pub(super) const fn host_interrupt(&self) -> Option<HostInterruptBinding> {
        match self.mapping.as_ref() {
            Some((interrupt, _)) => Some(HostInterruptBinding::new(interrupt.get())),
            None => None,
        }
    }

    pub(super) const fn maintenance_interrupt(&self) -> Option<VirtualInterrupt> {
        match self.maintenance.as_ref() {
            Some((interrupt, _)) => Some(*interrupt),
            None => None,
        }
    }

    pub(super) fn retain_permanently(self) {
        let host_timer = self.host_interrupt().map(HostInterruptBinding::get);
        if let Some((_, registration)) = self.mapping {
            registration.retain_permanently();
        }
        if let Some((_, registration)) = self.maintenance {
            registration.retain_permanently();
        }
        HOST_TIMER_INTERRUPT.store(host_timer.unwrap_or(NO_HOST_TIMER), Ordering::Release);
    }

    pub(super) fn rollback(self) {
        rollback_mapping(self.mapping);
        rollback_mapping(self.maintenance);
    }
}

pub(crate) fn set_host_timer_enabled(enabled: bool) -> Result<(), Error> {
    let interrupt = HOST_TIMER_INTERRUPT.load(Ordering::Acquire);
    if interrupt == NO_HOST_TIMER {
        return Ok(());
    }
    let interrupt = VirtualInterrupt::from_raw(interrupt);
    if enabled {
        crate::kernel::irq::interrupt::enable_local(interrupt)
    } else {
        crate::kernel::irq::interrupt::disable_local(interrupt)
    }
    .map_err(Error::Interrupt)
}

pub(super) fn validate_hardware(
    timer_interrupt: hyper::hal::interrupt::InterruptId,
) -> Result<bool, ValidationError> {
    let now = crate::kernel::time::monotonic_ticks();
    let Some((interrupts, hardware)) =
        crate::hal::vm::prepare_timer_validation(timer_interrupt, now)
            .map_err(ValidationError::Machine)?
    else {
        return Ok(false);
    };
    // SAFETY: `interrupts` remains fixed and outlives execution activation,
    // publication, deactivation, and drop.
    let mut execution = unsafe {
        crate::kernel::task::thread::VcpuExecution::for_timer_validation(hardware, &interrupts)
    };
    let execution_pointer = core::ptr::addr_of_mut!(execution);
    // SAFETY: This boot-local validation object is pinned on the stack, is
    // exclusively owned, and local interrupts remain masked.
    unsafe { super::vcpu::activate(execution_pointer) }.map_err(ValidationError::Hardware)?;
    let validation = (|| {
        let injected = super::active_vcpu::with(|execution, active_interrupts| {
            crate::hal::vm::inject_timer_for_validation(
                &mut execution.hardware,
                execution.vcpu_id,
                active_interrupts,
            )
        })
        .map_err(ValidationError::Active)?
        .ok_or(ValidationError::StateMismatch)?
        .map_err(|_| ValidationError::StateMismatch)?;
        if !injected
            || !crate::hal::vm::timer_validation_succeeded(&interrupts)
                .map_err(ValidationError::Interrupts)?
        {
            return Err(ValidationError::StateMismatch);
        }
        Ok(())
    })();
    // SAFETY: This remains the active validation vCPU with IRQs masked.
    match unsafe { super::vcpu::deactivate(&mut execution) } {
        Ok(()) => {}
        Err(super::vcpu::HardwareTransitionError::Active(error)) => {
            crate::pr_crit!(
                "HypeR: timer validation cannot clear active vCPU publication: {error:?}"
            );
            crate::hal::cpu::halt()
        }
        Err(error) => return Err(ValidationError::Hardware(error)),
    }
    validation.map(|()| true)
}

fn rollback_mapping(mapping: Option<(VirtualInterrupt, Registration)>) {
    let Some((interrupt, registration)) = mapping else {
        return;
    };
    match crate::kernel::irq::interrupt::unregister(registration) {
        Ok(()) => {
            if let Err(error) = crate::kernel::irq::interrupt::unmap(interrupt) {
                crate::pr_warn!("HypeR: guest timer IRQ mapping rollback failed: {error:?}");
            }
        }
        Err(failure) => {
            let (error, registration) = failure.into_parts();
            crate::pr_warn!("HypeR: retaining guest timer IRQ after rollback failed: {error:?}");
            registration.retain_permanently();
        }
    }
}

pub(super) fn prepare(
    source: crate::kernel::time::GuestTimerSource,
    domain: IrqDomainId,
    maintenance: Option<PlatformInterrupt>,
) -> Result<PreparedBinding, Error> {
    let mapping = if source.requires_host_mapping {
        Some(
            domain
                .register_shared_mapping(
                    source.interrupt,
                    InterruptPriority::Normal,
                    InterruptTrigger::Level,
                    0,
                    handle_interrupt,
                )
                .map_err(Error::Interrupt)?,
        )
    } else {
        None
    };
    // Zero represents an absent timer mapping, so encode an allocated VIRQ as
    // raw + 1. VIRQ zero is valid and must not collide with the sentinel.
    let maintenance_context = match mapping.as_ref() {
        Some((interrupt, _)) => usize::try_from(interrupt.get())
            .ok()
            .and_then(|raw| raw.checked_add(1))
            .ok_or(Error::InvalidHandlerContext),
        None => Ok(0),
    };
    let maintenance_context = match maintenance_context {
        Ok(context) => context,
        Err(error) => {
            rollback_mapping(mapping);
            return Err(error);
        }
    };
    let maintenance = match maintenance {
        Some(interrupt) => match domain.register_shared_mapping(
            InterruptId::new(interrupt.interrupt),
            InterruptPriority::High,
            match interrupt.trigger {
                PlatformInterruptTrigger::Level => InterruptTrigger::Level,
                PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
            },
            maintenance_context,
            handle_maintenance_interrupt,
        ) {
            Ok(mapping) => Some(mapping),
            Err(error) => {
                rollback_mapping(mapping);
                return Err(Error::Interrupt(error));
            }
        },
        None => None,
    };
    Ok(PreparedBinding {
        mapping,
        maintenance,
    })
}

fn handle_interrupt(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    match super::active_vcpu::with(|execution, interrupts| {
        crate::hal::vm::handle_virtual_timer_interrupt(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
        )
    }) {
        Ok(Some(Ok(true))) => HandlerResult::HandledAndMaskLocal,
        Ok(Some(Ok(false))) => HandlerResult::Handled,
        Ok(None) => {
            crate::pr_warn!("HypeR: masked virtual timer PPI without an active vCPU");
            HandlerResult::HandledAndMaskLocal
        }
        Ok(Some(Err(error))) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: virtual timer injection failed: {error:?}");
            HandlerResult::HandledAndMaskLocal
        }
        Err(error) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: invalid active vCPU during virtual timer IRQ: {error:?}");
            HandlerResult::HandledAndMaskLocal
        }
    }
}

fn handle_maintenance_interrupt(_interrupt: VirtualInterrupt, context: usize) -> HandlerResult {
    if !crate::hal::vm::maintenance_interrupt_pending() {
        return HandlerResult::NotHandled;
    }
    match super::active_vcpu::with(|execution, interrupts| {
        crate::hal::vm::handle_maintenance_interrupt(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
        )
    }) {
        Ok(Some(Ok(true))) => {
            let Some(raw) = context
                .checked_sub(1)
                .and_then(|raw| u32::try_from(raw).ok())
            else {
                crate::hal::vm::quiesce_virtual_interrupt_delivery();
                crate::pr_err!("HypeR: invalid virtual-timer maintenance binding");
                return HandlerResult::Handled;
            };
            HandlerResult::HandledAndUnmaskLocal(VirtualInterrupt::from_raw(raw))
        }
        Ok(Some(Ok(false))) => HandlerResult::Handled,
        Ok(None) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: virtual-interrupt maintenance without an active vCPU");
            HandlerResult::Handled
        }
        Ok(Some(Err(error))) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: virtual-interrupt maintenance failed: {error:?}");
            HandlerResult::Handled
        }
        Err(error) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: invalid active vCPU during maintenance IRQ: {error:?}");
            HandlerResult::Handled
        }
    }
}
