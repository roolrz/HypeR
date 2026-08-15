//! Kernel tick registration and interrupt handling.

use hyper::hal::interrupt::{InterruptId, InterruptTrigger};
use hyper::hal::timer::{PeriodicTimer, PeriodicTimerProperties};
use hyper::platform::{PlatformInterruptTrigger, TimerInfo, TimerKind};
use hyper::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use super::interrupt::{HandlerResult, IrqDomainId, VirtualInterrupt};

const TICKS_PER_SECOND: u32 = hyper::config::TIMER_HZ as u32;
const TIMER_PRIORITY: u8 = 0x80;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;

static TICK_COUNT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static RECURRING_IRQ_OBSERVED: [AtomicBool; MAX_CPUS] =
    [const { AtomicBool::new(false) }; MAX_CPUS];
static ONLINE_CPU_COUNT: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_REPORT_PRINTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Interrupt(super::interrupt::Error),
    Timer(crate::arch::TimerError),
    InvalidCpuIndex,
}

impl From<super::interrupt::Error> for Error {
    fn from(error: super::interrupt::Error) -> Self {
        Self::Interrupt(error)
    }
}

impl From<crate::arch::TimerError> for Error {
    fn from(error: crate::arch::TimerError) -> Self {
        Self::Timer(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub ticks_per_second: u32,
    pub counter_frequency_hz: u64,
    pub hardware_interrupt: InterruptId,
    pub virtual_interrupt: VirtualInterrupt,
}

pub fn initialize(info: TimerInfo, domain: IrqDomainId) -> Result<Capabilities, Error> {
    let TimerKind::ArmGenericHypervisorPhysical = info.kind;
    let hardware_interrupt = InterruptId::new(info.interrupt);
    let trigger = match info.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let virtual_interrupt =
        super::interrupt::map(domain, hardware_interrupt, TIMER_PRIORITY, trigger)?;
    let lifecycle_probe =
        match super::interrupt::register_shared(virtual_interrupt, 1, shared_probe) {
            Ok(registration) => registration,
            Err(error) => {
                let _ = super::interrupt::unmap(virtual_interrupt);
                return Err(error.into());
            }
        };
    if let Err(error) = super::interrupt::unregister(lifecycle_probe) {
        let _ = super::interrupt::unmap(virtual_interrupt);
        return Err(error.into());
    }
    let registration = match super::interrupt::register_shared(virtual_interrupt, 0, handle_tick) {
        Ok(registration) => registration,
        Err(error) => {
            let _ = super::interrupt::unmap(virtual_interrupt);
            return Err(error.into());
        }
    };
    let shared_registration =
        match super::interrupt::register_shared(virtual_interrupt, 0, shared_probe) {
            Ok(registration) => registration,
            Err(error) => {
                let _ = super::interrupt::unregister(registration);
                let _ = super::interrupt::unmap(virtual_interrupt);
                return Err(error.into());
            }
        };
    let PeriodicTimerProperties {
        counter_frequency_hz,
        ..
    } = match crate::arch::ArchitectureTimer::start(TICKS_PER_SECOND) {
        Ok(properties) => properties,
        Err(error) => {
            let _ = super::interrupt::unregister(shared_registration);
            let _ = super::interrupt::unregister(registration);
            let _ = super::interrupt::unmap(virtual_interrupt);
            return Err(error.into());
        }
    };
    Ok(Capabilities {
        ticks_per_second: TICKS_PER_SECOND,
        counter_frequency_hz,
        hardware_interrupt,
        virtual_interrupt,
    })
}

/// Starts the already-mapped architectural PPI on a secondary CPU.
pub fn initialize_local_cpu() -> Result<(), Error> {
    if crate::arch::current_cpu_index() >= MAX_CPUS {
        return Err(Error::InvalidCpuIndex);
    }
    let _properties = crate::arch::ArchitectureTimer::start(TICKS_PER_SECOND)?;
    Ok(())
}

/// Publishes the stable online CPU count used for timer health observation.
pub fn set_online_cpu_count(count: usize) -> Result<(), Error> {
    if count == 0 || count > MAX_CPUS {
        return Err(Error::InvalidCpuIndex);
    }
    ONLINE_CPU_COUNT.store(count, Ordering::Release);
    Ok(())
}

fn shared_probe(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    HandlerResult::NotHandled
}

fn handle_tick(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    if let Err(error) = crate::arch::ArchitectureTimer::handle_interrupt() {
        super::exception::fatal_timer(error);
    }
    let cpu = crate::arch::current_cpu_index();
    if cpu >= MAX_CPUS {
        super::exception::fatal_timer(crate::arch::TimerError::InvalidCpuIndex);
    }
    let tick = TICK_COUNT[cpu].fetch_add(1, Ordering::Relaxed) + 1;
    if tick == 3 {
        RECURRING_IRQ_OBSERVED[cpu].store(true, Ordering::Release);
        report_timer_health_once();
    }
    HandlerResult::Handled
}

fn report_timer_health_once() {
    let online = ONLINE_CPU_COUNT.load(Ordering::Acquire);
    if online == 0
        || !RECURRING_IRQ_OBSERVED[..online]
            .iter()
            .all(|observed| observed.load(Ordering::Acquire))
        || ACTIVE_REPORT_PRINTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    crate::println!("HypeR: periodic timer IRQs active on {online} CPUs");
}
