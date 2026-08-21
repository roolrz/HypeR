//! Architecture-independent monotonic timekeeping.

mod timers;

use core::hint::spin_loop;

pub use timers::{
    QueueStats as TimerQueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode, cancel,
    local_statistics as timer_statistics, reschedule, schedule_after, schedule_at,
    schedule_periodic,
};

use hyper::hal::timer::{
    ConversionError, MonotonicCounter, deadline_reached, nanoseconds_to_ticks, ticks_to_nanoseconds,
};
use hyper::sync::atomic::{AtomicU64, Ordering};

static COUNTER_FREQUENCY_HZ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Architecture(crate::arch::time::Error),
    Conversion(ConversionError),
    DeadlineTooFar,
    InvalidCpuIndex,
    NotInitialized,
    TimerQueue(hyper::time::TimerQueueError),
    TimerQueueAlreadyInitialized,
    TimerQueueNotInitialized,
}

impl From<crate::arch::time::Error> for Error {
    fn from(error: crate::arch::time::Error) -> Self {
        Self::Architecture(error)
    }
}

impl From<ConversionError> for Error {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub counter_frequency_hz: u64,
}

pub fn initialize() -> Result<Capabilities, Error> {
    let frequency = crate::arch::time::Counter::frequency_hz()?;
    COUNTER_FREQUENCY_HZ
        .compare_exchange(0, frequency, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| Error::AlreadyInitialized)?;
    Ok(Capabilities {
        counter_frequency_hz: frequency,
    })
}

/// Initializes and reports the kernel monotonic clock source.
pub(crate) fn initialize_timekeeping(boot: &super::boot::Initialization) -> Result<(), Error> {
    crate::arch::time::prepare(boot.essential())?;
    let capabilities = initialize()?;
    crate::println!(
        "HypeR: monotonic clocksource active at {} Hz",
        capabilities.counter_frequency_hz
    );
    Ok(())
}

pub(crate) fn initialize_local_timer_queue() -> Result<(), Error> {
    timers::initialize_local()
}

pub(crate) fn handle_timer_interrupt() -> Result<usize, Error> {
    timers::handle_interrupt()
}

pub(crate) fn request_hardware_wakeup(deadline: u64) -> Result<(), Error> {
    timers::request_hardware_wakeup(deadline)
}

pub fn counter_frequency_hz() -> Result<u64, Error> {
    let frequency = COUNTER_FREQUENCY_HZ.load(Ordering::Acquire);
    if frequency == 0 {
        Err(Error::NotInitialized)
    } else {
        Ok(frequency)
    }
}

pub fn monotonic_ticks() -> u64 {
    crate::arch::time::Counter::read()
}

pub fn monotonic_nanoseconds() -> Result<u64, Error> {
    Ok(ticks_to_nanoseconds(
        monotonic_ticks(),
        counter_frequency_hz()?,
    )?)
}

/// Returns an absolute counter deadline no earlier than `nanoseconds` ahead.
pub fn deadline_after(nanoseconds: u64) -> Result<u64, Error> {
    let ticks = nanoseconds_to_ticks(nanoseconds, counter_frequency_hz()?)?;
    if ticks > i64::MAX as u64 {
        return Err(Error::DeadlineTooFar);
    }
    Ok(monotonic_ticks().wrapping_add(ticks))
}

/// Polls an allocation-free condition until it succeeds or monotonic time
/// reaches the requested duration.
///
/// This is intended for boot and fail-stop handshakes where scheduling or
/// interrupts may be unavailable. Normal runtime waits should block a Thread.
pub(crate) fn spin_wait_until(
    timeout_nanoseconds: u64,
    mut condition: impl FnMut() -> bool,
) -> Result<bool, Error> {
    let deadline = deadline_after(timeout_nanoseconds)?;
    loop {
        if condition() {
            return Ok(true);
        }
        if deadline_reached(monotonic_ticks(), deadline) {
            return Ok(false);
        }
        spin_loop();
    }
}
