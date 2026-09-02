// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest architectural-timer host binding.
//!
//! VM policy owns the physical PPI handler used to inject a guest timer. Host
//! timekeeping supplies only the firmware-derived source description.

use hyper::cpu::PerCpu;
use hyper::hal::interrupt::{
    HostInterruptBinding, InterruptId, InterruptPriority, InterruptTrigger,
};
use hyper::platform::{PlatformInterrupt, PlatformInterruptTrigger};
use hyper::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::kernel::irq::interrupt::{
    HandlerResult, IrqDomainId, PreparedRegistration, VirtualInterrupt,
};

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
static TIMER_SOURCE_MASKED: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);

pub(super) struct PreparedBinding {
    mapping: Option<PreparedRegistration>,
    maintenance: Option<PreparedRegistration>,
    recovery: Option<crate::kernel::irq::interrupt::Registration>,
}

impl PreparedBinding {
    pub(super) const fn host_interrupt(&self) -> Option<HostInterruptBinding> {
        match self.mapping.as_ref() {
            Some(prepared) => Some(HostInterruptBinding::new(prepared.interrupt().get())),
            None => None,
        }
    }

    pub(super) const fn maintenance_interrupt(&self) -> Option<VirtualInterrupt> {
        match self.maintenance.as_ref() {
            Some(prepared) => Some(prepared.interrupt()),
            None => None,
        }
    }

    pub(super) fn activate(self) -> Result<(), Error> {
        let host_timer = self.host_interrupt().map(HostInterruptBinding::get);
        // Maintenance and recovery refer to the prepared timer VIRQ. Recovery
        // is already attached to the active host tick, but its masked-source
        // fast path remains false until guest execution can begin. Keep every
        // capability until the complete group commits.
        let PreparedBinding {
            mapping: prepared_mapping,
            maintenance: prepared_maintenance,
            recovery,
        } = self;
        let mapping = match activate_one(prepared_mapping) {
            Ok(registration) => registration,
            Err(error) => {
                rollback_mapping(prepared_maintenance);
                rollback_active_handler(recovery);
                return Err(error);
            }
        };
        let maintenance = match activate_one(prepared_maintenance) {
            Ok(registration) => registration,
            Err(error) => {
                rollback_active_mapping(mapping);
                rollback_active_handler(recovery);
                return Err(error);
            }
        };
        if let Some(registration) = mapping {
            registration.retain_permanently();
        }
        if let Some(registration) = maintenance {
            registration.retain_permanently();
        }
        if let Some(registration) = recovery {
            registration.retain_permanently();
        }
        HOST_TIMER_INTERRUPT.store(host_timer.unwrap_or(NO_HOST_TIMER), Ordering::Release);
        Ok(())
    }

    pub(super) fn rollback(self) {
        rollback_active_handler(self.recovery);
        rollback_mapping(self.mapping);
        rollback_mapping(self.maintenance);
    }
}

fn activate_one(
    prepared: Option<PreparedRegistration>,
) -> Result<Option<crate::kernel::irq::interrupt::Registration>, Error> {
    let Some(prepared) = prepared else {
        return Ok(None);
    };
    match crate::kernel::irq::interrupt::activate(prepared) {
        Ok(registration) => Ok(Some(registration)),
        Err(failure) => {
            let (error, prepared) = failure.into_parts();
            if let Err(discard) = crate::kernel::irq::interrupt::discard_prepared(prepared) {
                let (discard_error, _prepared) = discard.into_parts();
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: guest timer IRQ activation rollback failed: {discard_error:?}"
                ));
            }
            Err(Error::Interrupt(error))
        }
    }
}

fn rollback_active_mapping(registration: Option<crate::kernel::irq::interrupt::Registration>) {
    let Some(registration) = registration else {
        return;
    };
    crate::hal::vm::quiesce_virtual_interrupt_delivery();
    let interrupt = registration.interrupt();
    match crate::kernel::irq::interrupt::unregister(registration) {
        Ok(()) => {
            if let Err(error) = crate::kernel::irq::interrupt::unmap(interrupt) {
                crate::pr_warn!("HypeR: active guest timer IRQ unmap failed: {error:?}");
            }
        }
        Err(failure) => {
            let (error, registration) = failure.into_parts();
            crate::pr_warn!(
                "HypeR: retaining active guest timer IRQ after rollback failed: {error:?}"
            );
            registration.retain_permanently();
        }
    }
}

fn rollback_active_handler(registration: Option<crate::kernel::irq::interrupt::Registration>) {
    let Some(registration) = registration else {
        return;
    };
    if let Err(failure) = crate::kernel::irq::interrupt::unregister(registration) {
        let (error, registration) = failure.into_parts();
        crate::pr_warn!("HypeR: retaining active IRQ handler after rollback failed: {error:?}");
        registration.retain_permanently();
    }
}

pub(crate) fn set_host_timer_enabled(enabled: bool) -> Result<(), Error> {
    let interrupt = HOST_TIMER_INTERRUPT.load(Ordering::Acquire);
    if interrupt == NO_HOST_TIMER {
        return Ok(());
    }
    let interrupt = VirtualInterrupt::from_raw(interrupt);
    let result = if enabled {
        crate::kernel::irq::interrupt::enable_local(interrupt)
    } else {
        crate::kernel::irq::interrupt::disable_local(interrupt)
    };
    result.map_err(Error::Interrupt)?;
    set_local_source_masked(!enabled);
    Ok(())
}

pub(super) fn validate_hardware(
    timer_interrupt: hyper::hal::interrupt::InterruptId,
    services: &crate::hal::vm::ExitServicesReady,
    prepared_interrupts: &crate::hal::vm::PreparedInterruptVirtualization,
) -> Result<bool, ValidationError> {
    let now = crate::kernel::time::monotonic_ticks();
    let Some(prepared) = crate::hal::vm::prepare_timer_validation(
        timer_interrupt,
        now,
        services,
        prepared_interrupts,
    )
    .map_err(ValidationError::Machine)?
    else {
        return Ok(false);
    };
    let (capability, interrupts, hardware) = prepared.into_parts();
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
        super::active_vcpu::with(|execution, active_interrupts| {
            crate::hal::vm::inject_timer_for_validation(
                &capability,
                &mut execution.hardware,
                execution.vcpu_id,
                active_interrupts,
            )
        })
        .map_err(ValidationError::Active)?
        .ok_or(ValidationError::StateMismatch)?
        .map_err(|_| ValidationError::StateMismatch)?;
        if !crate::hal::vm::timer_validation_succeeded(&capability, &interrupts)
            .map_err(ValidationError::Interrupts)?
        {
            return Err(ValidationError::StateMismatch);
        }
        Ok(())
    })();
    // SAFETY: This remains the active validation vCPU with IRQs masked.
    match unsafe { super::vcpu::deactivate(&mut execution) } {
        Ok(()) => {}
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: timer validation cannot retire active vCPU hardware: {error:?}"
        )),
    }
    validation.map(|()| true)
}

fn rollback_mapping(mapping: Option<PreparedRegistration>) {
    let Some(prepared) = mapping else {
        return;
    };
    if let Err(failure) = crate::kernel::irq::interrupt::discard_prepared(prepared) {
        let (error, _prepared) = failure.into_parts();
        crate::kernel::crash::fatal(format_args!(
            "HypeR: guest timer IRQ preparation rollback failed: {error:?}"
        ));
    }
}

pub(super) fn prepare(
    source: crate::kernel::time::GuestTimerSource,
    host_tick_interrupt: VirtualInterrupt,
    domain: IrqDomainId,
    maintenance: Option<PlatformInterrupt>,
) -> Result<PreparedBinding, Error> {
    let mapping = if source.requires_host_mapping {
        Some(
            domain
                .prepare_shared_mapping(
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
        Some(prepared) => usize::try_from(prepared.interrupt().get())
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
    let recovery = if mapping.is_some() {
        match crate::kernel::irq::interrupt::register_shared(
            host_tick_interrupt,
            maintenance_context,
            handle_source_recovery,
        ) {
            Ok(mapping) => Some(mapping),
            Err(error) => {
                rollback_mapping(mapping);
                return Err(Error::Interrupt(error));
            }
        }
    } else {
        None
    };
    let maintenance = match maintenance {
        Some(interrupt) => match domain.prepare_shared_mapping(
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
                rollback_active_handler(recovery);
                return Err(Error::Interrupt(error));
            }
        },
        None => None,
    };
    Ok(PreparedBinding {
        mapping,
        maintenance,
        recovery,
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
        Ok(Some(Ok(true))) => {
            set_local_source_masked(true);
            HandlerResult::HandledAndMaskLocal
        }
        Ok(Some(Ok(false))) => HandlerResult::Handled,
        Ok(None) => {
            set_local_source_masked(true);
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
            set_local_source_masked(false);
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

/// Rechecks a timer PPI retained masked across an early EOI-maintenance edge.
///
/// Linux may retire the virtual LR before its timer programming write has
/// deasserted CNTV. Maintenance must keep the level source masked in that
/// interval, but deassertion itself produces no later interrupt. The existing
/// host tick is therefore used as an allocation-free recovery edge. Its normal
/// path is one CPU-local relaxed load; vGIC reconciliation occurs only while
/// this source is known to be masked.
fn handle_source_recovery(_interrupt: VirtualInterrupt, context: usize) -> HandlerResult {
    if !local_source_masked() {
        return HandlerResult::NotHandled;
    }
    let Some(timer_interrupt) = decode_bound_interrupt(context) else {
        crate::hal::vm::quiesce_virtual_interrupt_delivery();
        crate::pr_err!("HypeR: invalid virtual-timer recovery binding");
        return HandlerResult::Handled;
    };
    let can_unmask = match super::active_vcpu::with(|execution, interrupts| {
        crate::hal::vm::handle_maintenance_interrupt(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
        )
    }) {
        Ok(Some(Ok(can_unmask))) => can_unmask,
        Ok(None) => true,
        Ok(Some(Err(error))) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: virtual-timer recovery failed: {error:?}");
            false
        }
        Err(error) => {
            crate::hal::vm::quiesce_virtual_interrupt_delivery();
            crate::pr_err!("HypeR: invalid active vCPU during timer recovery: {error:?}");
            false
        }
    };
    if can_unmask {
        set_local_source_masked(false);
        HandlerResult::HandledAndUnmaskLocal(timer_interrupt)
    } else {
        HandlerResult::NotHandled
    }
}

fn decode_bound_interrupt(context: usize) -> Option<VirtualInterrupt> {
    context
        .checked_sub(1)
        .and_then(|raw| u32::try_from(raw).ok())
        .map(VirtualInterrupt::from_raw)
}

fn local_source_masked() -> bool {
    crate::kernel::cpu::current_index()
        .is_some_and(|cpu| TIMER_SOURCE_MASKED[cpu].load(Ordering::Relaxed))
}

fn set_local_source_masked(masked: bool) {
    if let Some(cpu) = crate::kernel::cpu::current_index() {
        TIMER_SOURCE_MASKED[cpu].store(masked, Ordering::Relaxed);
    }
}
