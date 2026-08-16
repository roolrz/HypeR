//! Kernel tick registration and interrupt handling.

use hyper::hal::interrupt::{InterruptId, InterruptTrigger};
use hyper::hal::timer::MonotonicCounter;
#[cfg(target_arch = "aarch64")]
use hyper::platform::PlatformInterrupt;
use hyper::platform::{PlatformInterruptTrigger, TimerInfo, TimerKind};
#[cfg(target_arch = "aarch64")]
use hyper::sync::atomic::AtomicU32;
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
#[cfg(target_arch = "aarch64")]
static VIRTUAL_TIMER_VIRQ: AtomicU32 = AtomicU32::new(u32::MAX);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Interrupt(super::interrupt::Error),
    Timer(crate::arch::TimerError),
    Time(crate::kernel::time::Error),
    InvalidCpuIndex,
    InconsistentCounterFrequency,
    #[cfg(target_arch = "aarch64")]
    InvalidInterruptTrigger,
    UnsupportedTimer,
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

#[cfg(target_arch = "aarch64")]
pub fn initialize(info: TimerInfo, domain: IrqDomainId) -> Result<Capabilities, Error> {
    if info.kind != TimerKind::ArmGeneric {
        return Err(Error::UnsupportedTimer);
    }
    if info.hypervisor_physical.trigger != PlatformInterruptTrigger::Level
        || info.virtual_timer.trigger != PlatformInterruptTrigger::Level
    {
        return Err(Error::InvalidInterruptTrigger);
    }
    let PlatformInterrupt { interrupt, trigger } = info.hypervisor_physical;
    let hardware_interrupt = InterruptId::new(interrupt);
    let trigger = match trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let guest_virtual_interrupt = InterruptId::new(info.virtual_timer.interrupt);
    let guest_virtual_host_interrupt = super::interrupt::map(
        domain,
        guest_virtual_interrupt,
        TIMER_PRIORITY,
        InterruptTrigger::Level,
    )?;
    if let Err(error) = super::interrupt::register_shared(
        guest_virtual_host_interrupt,
        0,
        handle_guest_virtual_timer,
    ) {
        let _ = super::interrupt::unmap(guest_virtual_host_interrupt);
        return Err(error.into());
    }
    VIRTUAL_TIMER_VIRQ.store(guest_virtual_host_interrupt.get(), Ordering::Release);
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
    let registration =
        match super::interrupt::register_shared(virtual_interrupt, 0, handle_host_timer) {
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
    let counter_frequency_hz = crate::kernel::time::counter_frequency_hz()?;
    if let Err(error) = start_local_tick(counter_frequency_hz) {
        let _ = super::interrupt::unregister(shared_registration);
        let _ = super::interrupt::unregister(registration);
        let _ = super::interrupt::unmap(virtual_interrupt);
        return Err(error);
    }
    Ok(Capabilities {
        ticks_per_second: TICKS_PER_SECOND,
        counter_frequency_hz,
        hardware_interrupt,
        virtual_interrupt,
        guest_virtual_interrupt,
        guest_virtual_host_interrupt,
    })
}

#[cfg(target_arch = "riscv64")]
pub fn initialize(info: TimerInfo, domain: IrqDomainId) -> Result<Capabilities, Error> {
    if info.kind != TimerKind::RiscvSupervisor
        || info.hypervisor_physical.trigger != PlatformInterruptTrigger::Level
    {
        return Err(Error::UnsupportedTimer);
    }
    let hardware_interrupt = InterruptId::new(info.hypervisor_physical.interrupt);
    let virtual_interrupt = super::interrupt::map(
        domain,
        hardware_interrupt,
        TIMER_PRIORITY,
        InterruptTrigger::Level,
    )?;
    let registration = super::interrupt::register_shared(virtual_interrupt, 0, handle_host_timer)?;
    if let Err(error) = start_local_tick(crate::kernel::time::counter_frequency_hz()?) {
        let _ = super::interrupt::unregister(registration);
        let _ = super::interrupt::unmap(virtual_interrupt);
        return Err(error);
    }
    Ok(Capabilities {
        ticks_per_second: TICKS_PER_SECOND,
        counter_frequency_hz: crate::kernel::time::counter_frequency_hz()?,
        hardware_interrupt,
        virtual_interrupt,
        guest_virtual_interrupt: InterruptId::new(5),
        guest_virtual_host_interrupt: virtual_interrupt,
    })
}

#[cfg(target_arch = "x86_64")]
pub fn initialize(info: TimerInfo, domain: IrqDomainId) -> Result<Capabilities, Error> {
    if info.kind != TimerKind::X86TscDeadline
        || info.hypervisor_physical.trigger != PlatformInterruptTrigger::Edge
    {
        return Err(Error::UnsupportedTimer);
    }
    let hardware_interrupt = InterruptId::new(info.hypervisor_physical.interrupt);
    let virtual_interrupt = super::interrupt::map(
        domain,
        hardware_interrupt,
        TIMER_PRIORITY,
        InterruptTrigger::Edge,
    )?;
    let registration = super::interrupt::register_shared(virtual_interrupt, 0, handle_host_timer)?;
    let counter_frequency_hz = crate::kernel::time::counter_frequency_hz()?;
    if let Err(error) = start_local_tick(counter_frequency_hz) {
        let _ = super::interrupt::unregister(registration);
        let _ = super::interrupt::unmap(virtual_interrupt);
        return Err(error);
    }
    Ok(Capabilities {
        ticks_per_second: TICKS_PER_SECOND,
        counter_frequency_hz,
        hardware_interrupt,
        virtual_interrupt,
        guest_virtual_interrupt: InterruptId::new(info.virtual_timer.interrupt),
        guest_virtual_host_interrupt: virtual_interrupt,
    })
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn guest_virtual_host_interrupt() -> Option<VirtualInterrupt> {
    let interrupt = VIRTUAL_TIMER_VIRQ.load(Ordering::Acquire);
    (interrupt != u32::MAX).then_some(VirtualInterrupt::from_raw(interrupt))
}

/// Starts the already-mapped architectural PPI on a secondary CPU.
pub fn initialize_local_cpu() -> Result<(), Error> {
    if crate::arch::current_cpu_index() >= MAX_CPUS {
        return Err(Error::InvalidCpuIndex);
    }
    start_local_tick(crate::kernel::time::counter_frequency_hz()?)
}

/// Publishes the stable online CPU count used for timer health observation.
pub fn set_online_cpu_count(count: usize) -> Result<(), Error> {
    if count == 0 || count > MAX_CPUS {
        return Err(Error::InvalidCpuIndex);
    }
    ONLINE_CPU_COUNT.store(count, Ordering::Release);
    report_timer_health_once();
    Ok(())
}

#[cfg(target_arch = "aarch64")]
fn shared_probe(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    HandlerResult::NotHandled
}

#[cfg(target_arch = "aarch64")]
fn handle_guest_virtual_timer(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    match crate::kernel::vm::handle_arch_timer_interrupt() {
        Ok(outcome) if outcome.active && outcome.asserted => HandlerResult::HandledAndMaskLocal,
        Ok(outcome) if outcome.active => HandlerResult::Handled,
        Ok(_) => {
            crate::pr_warn!("HypeR: masked virtual timer PPI without an active vCPU");
            HandlerResult::HandledAndMaskLocal
        }
        Err(error) => {
            crate::arch::disable_vgic();
            crate::pr_err!("HypeR: virtual timer injection failed: {error:?}");
            HandlerResult::HandledAndMaskLocal
        }
    }
}

fn handle_host_timer(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    if let Err(error) = crate::kernel::time::handle_timer_interrupt() {
        super::exception::fatal_timer(error);
    }
    crate::arch::poll_guest_timer(crate::kernel::time::monotonic_ticks());
    HandlerResult::Handled
}

fn start_local_tick(counter_frequency_hz: u64) -> Result<(), Error> {
    if crate::arch::ArchitectureCounter::frequency_hz()? != counter_frequency_hz {
        return Err(Error::InconsistentCounterFrequency);
    }
    let interval = counter_frequency_hz / u64::from(TICKS_PER_SECOND);
    if interval == 0 {
        return Err(crate::arch::TimerError::InvalidFrequency.into());
    }
    crate::kernel::time::initialize_local_timer_queue()?;
    let first_deadline = crate::kernel::time::monotonic_ticks().wrapping_add(interval);
    let _ =
        crate::kernel::time::schedule_periodic(first_deadline, interval, handle_periodic_tick, 0)?;
    Ok(())
}

fn handle_periodic_tick(event: crate::kernel::time::TimerEvent, _context: usize) {
    let cpu = crate::arch::current_cpu_index();
    if cpu >= MAX_CPUS {
        super::exception::fatal_timer(crate::arch::TimerError::InvalidCpuIndex);
    }
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
