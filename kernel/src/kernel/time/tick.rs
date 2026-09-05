// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel tick registration and interrupt handling.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::hal::interrupt::{InterruptId, InterruptPriority, InterruptTrigger};
use hyper::platform::{PlatformInterruptTrigger, TimerInfo};
use hyper::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::kernel::irq::interrupt::{HandlerResult, IrqDomainId, VirtualInterrupt};

const TICKS_PER_SECOND: u32 = hyper::config::TIMER_HZ as u32;

static TICK_COUNT: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static RECURRING_IRQ_OBSERVED: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_REPORT_PRINTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Interrupt(crate::kernel::irq::interrupt::Error),
    Description(crate::hal::time::DescriptionError),
    Timer(crate::hal::time::Error),
    Time(crate::kernel::time::Error),
    InvalidCpuIndex,
    InvalidTickInterval,
    InconsistentCounterFrequency,
}

impl From<crate::kernel::irq::interrupt::Error> for Error {
    fn from(error: crate::kernel::irq::interrupt::Error) -> Self {
        Self::Interrupt(error)
    }
}

impl From<crate::hal::time::Error> for Error {
    fn from(error: crate::hal::time::Error) -> Self {
        Self::Timer(error)
    }
}

impl From<crate::hal::time::DescriptionError> for Error {
    fn from(error: crate::hal::time::DescriptionError) -> Self {
        Self::Description(error)
    }
}

impl From<crate::kernel::time::Error> for Error {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Time(error)
    }
}

pub(super) fn initialize(
    info: TimerInfo,
    domain: IrqDomainId,
) -> Result<super::Capabilities, Error> {
    let description = crate::hal::time::describe(info)?;
    let hardware_interrupt = InterruptId::new(description.hardware.interrupt);
    let trigger = match description.hardware.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let (virtual_interrupt, registration) = domain.register_shared_mapping(
        hardware_interrupt,
        InterruptPriority::Normal,
        trigger,
        0,
        handle_host_timer,
    )?;
    let counter_frequency_hz = match crate::kernel::time::counter_frequency_hz() {
        Ok(frequency) => frequency,
        Err(error) => {
            rollback_registered_mapping(registration, virtual_interrupt);
            return Err(error.into());
        }
    };
    if let Err(error) = start_local_tick(counter_frequency_hz) {
        rollback_registered_mapping(registration, virtual_interrupt);
        return Err(error);
    }

    // The architectural timer is part of the kernel runtime and has no
    // teardown phase. Publish its permanent ownership only after every
    // fallible initialization step has completed.
    registration.retain_permanently();
    Ok(super::Capabilities {
        ticks_per_second: TICKS_PER_SECOND,
        counter_frequency_hz,
        hardware_interrupt,
        virtual_interrupt,
        guest_timer: super::GuestTimerSource {
            interrupt: description.guest_virtual_interrupt,
            requires_host_mapping: description.map_guest_virtual_interrupt,
        },
    })
}

fn rollback_registered_mapping(
    registration: crate::kernel::irq::interrupt::Registration,
    interrupt: VirtualInterrupt,
) {
    match crate::kernel::irq::interrupt::unregister(registration) {
        Ok(()) => rollback_unused_mapping(interrupt),
        Err(failure) => retain_failed_registration(failure),
    }
}

fn rollback_unused_mapping(interrupt: VirtualInterrupt) {
    if let Err(error) = crate::kernel::irq::interrupt::unmap(interrupt) {
        crate::pr_warn!("HypeR: IRQ mapping rollback failed: {error:?}");
    }
}

fn retain_failed_registration(failure: crate::kernel::irq::interrupt::UnregisterFailure) {
    let (error, registration) = failure.into_parts();
    crate::pr_warn!("HypeR: retaining IRQ handler after rollback failed: {error:?}");
    registration.retain_permanently();
}

/// Starts the already-mapped architectural PPI on a secondary CPU.
pub(super) fn initialize_local_cpu() -> Result<(), Error> {
    current_cpu()?;
    start_local_tick(crate::kernel::time::counter_frequency_hz()?)
}

fn handle_host_timer(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    if let Err(error) = crate::kernel::time::handle_timer_interrupt() {
        crate::kernel::irq::exception::fatal_timer(error);
    }
    HandlerResult::Handled
}

fn start_local_tick(counter_frequency_hz: u64) -> Result<(), Error> {
    if crate::hal::time::counter_frequency_hz()? != counter_frequency_hz {
        return Err(Error::InconsistentCounterFrequency);
    }
    let interval = counter_frequency_hz / u64::from(TICKS_PER_SECOND);
    if interval == 0 {
        return Err(Error::InvalidTickInterval);
    }
    crate::kernel::time::initialize_local_timer_queue()?;
    let first_deadline = crate::kernel::time::monotonic_ticks().wrapping_add(interval);
    let _ =
        crate::kernel::time::schedule_periodic(first_deadline, interval, handle_periodic_tick, 0)?;
    Ok(())
}

fn handle_periodic_tick(event: crate::kernel::time::TimerEvent, _context: usize) {
    let cpu = match current_cpu() {
        Ok(cpu) => cpu,
        Err(error) => crate::kernel::irq::exception::fatal_timer(error),
    };
    let periods = event.overruns.saturating_add(1);
    let previous = TICK_COUNT[cpu].fetch_add(periods, Ordering::Relaxed);
    if let Err(error) = crate::kernel::task::scheduler::account_tick(periods) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: scheduler tick accounting failed: {error:?}"
        ));
    }
    if previous.saturating_add(periods) >= 3 {
        if previous < 3 {
            RECURRING_IRQ_OBSERVED[cpu].store(true, Ordering::Release);
        }
        // CPU participation is published only after every secondary reports
        // online. Retry during that bounded startup window so a CPU which
        // reached its third tick earlier cannot permanently lose the report.
        // The relaxed fast check avoids scanning per-CPU state after success.
        if !ACTIVE_REPORT_PRINTED.load(Ordering::Relaxed) {
            report_timer_health_once();
        }
    }
}

fn report_timer_health_once() {
    let Some(participating) = crate::kernel::cpu::participating_cpu_count() else {
        return;
    };
    if participating == 0
        || !RECURRING_IRQ_OBSERVED
            .iter()
            .take(participating)
            .all(|observed| observed.load(Ordering::Acquire))
        || ACTIVE_REPORT_PRINTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    crate::pr_info!("HypeR: periodic timer IRQs active on {participating} CPUs");
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpuIndex)
}
