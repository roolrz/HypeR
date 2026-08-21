//! Kernel tick registration and interrupt handling.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::hal::interrupt::{InterruptId, InterruptPriority, InterruptTrigger};
use hyper::hal::timer::MonotonicCounter;
use hyper::platform::{PlatformInterruptTrigger, TimerInfo};
use hyper::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use super::interrupt::{HandlerResult, IrqDomainId, VirtualInterrupt};

const TICKS_PER_SECOND: u32 = hyper::config::TIMER_HZ as u32;

static TICK_COUNT: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static RECURRING_IRQ_OBSERVED: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);
static ONLINE_CPU_COUNT: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_REPORT_PRINTED: AtomicBool = AtomicBool::new(false);
static VIRTUAL_TIMER_VIRQ: AtomicU32 = AtomicU32::new(u32::MAX);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Interrupt(super::interrupt::Error),
    Description(crate::arch::time::DescriptionError),
    Timer(crate::arch::time::Error),
    Time(crate::kernel::time::Error),
    InvalidCpuIndex,
    InvalidTickInterval,
    InconsistentCounterFrequency,
}

impl From<super::interrupt::Error> for Error {
    fn from(error: super::interrupt::Error) -> Self {
        Self::Interrupt(error)
    }
}

impl From<crate::arch::time::Error> for Error {
    fn from(error: crate::arch::time::Error) -> Self {
        Self::Timer(error)
    }
}

impl From<crate::arch::time::DescriptionError> for Error {
    fn from(error: crate::arch::time::DescriptionError) -> Self {
        Self::Description(error)
    }
}

impl From<crate::kernel::time::Error> for Error {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Time(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub ticks_per_second: u32,
    pub counter_frequency_hz: u64,
    pub hardware_interrupt: InterruptId,
    pub virtual_interrupt: VirtualInterrupt,
    pub guest_virtual_interrupt: InterruptId,
    pub guest_virtual_host_interrupt: VirtualInterrupt,
}

pub fn initialize(info: TimerInfo, domain: IrqDomainId) -> Result<Capabilities, Error> {
    let description = crate::arch::time::describe(info)?;
    let hardware_interrupt = InterruptId::new(description.hardware.interrupt);
    let trigger = match description.hardware.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let guest_virtual_interrupt = description.guest_virtual_interrupt;
    let guest_virtual_mapping = if description.map_guest_virtual_interrupt {
        let (interrupt, registration) = domain.register_shared_mapping(
            guest_virtual_interrupt,
            InterruptPriority::Normal,
            InterruptTrigger::Level,
            0,
            handle_guest_virtual_timer,
        )?;
        Some((interrupt, registration))
    } else {
        None
    };
    let (virtual_interrupt, registration) = match domain.register_shared_mapping(
        hardware_interrupt,
        InterruptPriority::Normal,
        trigger,
        0,
        handle_host_timer,
    ) {
        Ok(ownership) => ownership,
        Err(error) => {
            rollback_mapping(guest_virtual_mapping);
            return Err(error.into());
        }
    };
    let counter_frequency_hz = match crate::kernel::time::counter_frequency_hz() {
        Ok(frequency) => frequency,
        Err(error) => {
            rollback_registered_mapping(registration, virtual_interrupt);
            rollback_mapping(guest_virtual_mapping);
            return Err(error.into());
        }
    };
    if let Err(error) = start_local_tick(counter_frequency_hz) {
        rollback_registered_mapping(registration, virtual_interrupt);
        rollback_mapping(guest_virtual_mapping);
        return Err(error);
    }

    // The architectural timer is part of the kernel runtime and has no
    // teardown phase. Publish its permanent ownership only after every
    // fallible initialization step has completed.
    registration.retain_permanently();
    let guest_virtual_host_interrupt =
        if let Some((interrupt, registration)) = guest_virtual_mapping {
            registration.retain_permanently();
            VIRTUAL_TIMER_VIRQ.store(interrupt.get(), Ordering::Release);
            Some(interrupt)
        } else {
            None
        };
    Ok(Capabilities {
        ticks_per_second: TICKS_PER_SECOND,
        counter_frequency_hz,
        hardware_interrupt,
        virtual_interrupt,
        guest_virtual_interrupt,
        guest_virtual_host_interrupt: guest_virtual_host_interrupt.unwrap_or(virtual_interrupt),
    })
}

fn rollback_mapping(mapping: Option<(VirtualInterrupt, super::interrupt::Registration)>) {
    if let Some((interrupt, registration)) = mapping {
        rollback_registered_mapping(registration, interrupt);
    }
}

fn rollback_registered_mapping(
    registration: super::interrupt::Registration,
    interrupt: VirtualInterrupt,
) {
    match super::interrupt::unregister(registration) {
        Ok(()) => rollback_unused_mapping(interrupt),
        Err(failure) => retain_failed_registration(failure),
    }
}

fn rollback_unused_mapping(interrupt: VirtualInterrupt) {
    if let Err(error) = super::interrupt::unmap(interrupt) {
        crate::pr_warn!("HypeR: IRQ mapping rollback failed: {error:?}");
    }
}

fn retain_failed_registration(failure: super::interrupt::UnregisterFailure) {
    let (error, registration) = failure.into_parts();
    crate::pr_warn!("HypeR: retaining IRQ handler after rollback failed: {error:?}");
    registration.retain_permanently();
}

pub fn guest_virtual_host_interrupt() -> Option<VirtualInterrupt> {
    let interrupt = VIRTUAL_TIMER_VIRQ.load(Ordering::Acquire);
    (interrupt != u32::MAX).then_some(VirtualInterrupt::from_raw(interrupt))
}

/// Starts the already-mapped architectural PPI on a secondary CPU.
pub fn initialize_local_cpu() -> Result<(), Error> {
    current_cpu()?;
    start_local_tick(crate::kernel::time::counter_frequency_hz()?)
}

/// Publishes the stable online CPU count used for timer health observation.
pub fn set_online_cpu_count(count: usize) -> Result<(), Error> {
    if count == 0 || count > hyper::cpu::MAX_CPUS {
        return Err(Error::InvalidCpuIndex);
    }
    ONLINE_CPU_COUNT.store(count, Ordering::Release);
    report_timer_health_once();
    Ok(())
}

fn handle_guest_virtual_timer(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    crate::arch::vm::handle_virtual_timer_interrupt()
}

fn handle_host_timer(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    if let Err(error) = crate::kernel::time::handle_timer_interrupt() {
        super::exception::fatal_timer(error);
    }
    crate::arch::vm::poll_timer(crate::kernel::time::monotonic_ticks());
    HandlerResult::Handled
}

fn start_local_tick(counter_frequency_hz: u64) -> Result<(), Error> {
    if crate::arch::time::Counter::frequency_hz()? != counter_frequency_hz {
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
        Err(error) => super::exception::fatal_timer(error),
    };
    let periods = event.overruns.saturating_add(1);
    let previous = TICK_COUNT[cpu].fetch_add(periods, Ordering::Relaxed);
    if previous < 3 && previous.saturating_add(periods) >= 3 {
        RECURRING_IRQ_OBSERVED[cpu].store(true, Ordering::Release);
        report_timer_health_once();
    }
}

fn report_timer_health_once() {
    let online = ONLINE_CPU_COUNT.load(Ordering::Acquire);
    if online == 0
        || !RECURRING_IRQ_OBSERVED
            .iter()
            .take(online)
            .all(|observed| observed.load(Ordering::Acquire))
        || ACTIVE_REPORT_PRINTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    crate::println!("HypeR: periodic timer IRQs active on {online} CPUs");
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpuIndex)
}
