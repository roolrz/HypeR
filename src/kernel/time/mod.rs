// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host clocksource, timer queues, and architectural tick lifecycle.
//!
//! This subsystem owns time conversion and per-CPU tick policy. It consumes an
//! IRQ domain for physical delivery but does not own interrupt-controller
//! discovery or routing policy.

mod tick;
mod timers;

pub(crate) use tick::Error as TickError;

use core::hint::spin_loop;

pub use timers::{
    QueueStats as TimerQueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode, cancel,
    local_statistics as timer_statistics, schedule_after, schedule_at, schedule_periodic,
};

use hyper::hal::interrupt::InterruptId;
use hyper::hal::timer::{
    ConversionError, deadline_reached, nanoseconds_to_ticks, ticks_to_nanoseconds,
};
use hyper::sync::atomic::{AtomicU64, Ordering};

use crate::kernel::irq::interrupt::VirtualInterrupt;

static COUNTER_FREQUENCY_HZ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Architecture(crate::hal::time::Error),
    Conversion(ConversionError),
    DeadlineTooFar,
    InvalidCpuIndex,
    NotInitialized,
    TimerQueue(hyper::time::TimerQueueError),
    TimerQueueAlreadyInitialized,
    TimerQueueNotInitialized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Clock(Error),
    MissingTimer,
    Tick(tick::Error),
}

impl From<crate::hal::time::Error> for Error {
    fn from(error: crate::hal::time::Error) -> Self {
        Self::Architecture(error)
    }
}

impl From<ConversionError> for Error {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GuestTimerSource {
    pub(crate) interrupt: InterruptId,
    pub(crate) requires_host_mapping: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Capabilities {
    pub(crate) ticks_per_second: u32,
    pub(crate) counter_frequency_hz: u64,
    pub(crate) hardware_interrupt: InterruptId,
    pub(crate) virtual_interrupt: VirtualInterrupt,
    pub(crate) guest_timer: GuestTimerSource,
}

fn initialize_clock() -> Result<u64, Error> {
    let frequency = crate::hal::time::counter_frequency_hz()?;
    COUNTER_FREQUENCY_HZ
        .compare_exchange(0, frequency, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| Error::AlreadyInitialized)?;
    Ok(frequency)
}

/// Initializes the monotonic clock, architectural tick, and boot CPU queue.
pub(crate) fn initialize(
    boot: &mut super::boot::Initialization,
) -> Result<(), InitializationError> {
    crate::hal::time::prepare(boot.essential())
        .map_err(Error::Architecture)
        .map_err(InitializationError::Clock)?;
    let counter_frequency_hz = initialize_clock().map_err(InitializationError::Clock)?;
    crate::println!(
        "HypeR: monotonic clocksource active at {} Hz",
        counter_frequency_hz
    );

    let info = boot
        .essential()
        .timer()
        .ok_or(InitializationError::MissingTimer)?;
    let capabilities =
        tick::initialize(info, boot.interrupts().root_domain).map_err(InitializationError::Tick)?;
    crate::println!(
        "HypeR: architectural timer: host INTID {}, guest INTID {}, {} Hz tick from a {} Hz counter",
        capabilities.hardware_interrupt.get(),
        capabilities.guest_timer.interrupt.get(),
        capabilities.ticks_per_second,
        capabilities.counter_frequency_hz
    );
    crate::println!(
        "HypeR: timer mapped to dynamic VIRQ {}",
        capabilities.virtual_interrupt.get()
    );
    crate::println!("HypeR: dynamically owned per-CPU software timer queues active");
    boot.set_timer(capabilities);
    Ok(())
}

pub(crate) fn initialize_local_cpu() -> Result<(), tick::Error> {
    tick::initialize_local_cpu()
}

pub(crate) fn initialize_local_timer_queue() -> Result<(), Error> {
    timers::initialize_local()
}

pub(crate) fn handle_timer_interrupt() -> Result<usize, Error> {
    timers::handle_interrupt()
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
    crate::hal::time::read_counter()
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

/// Converts an absolute monotonic nanosecond value into a counter deadline.
///
/// The counter and its nanosecond representation are sampled once. Deriving
/// the delta and final deadline from that same snapshot prevents syscall
/// preparation time from silently extending an absolute deadline.
pub(crate) fn deadline_from_monotonic_nanoseconds(nanoseconds: u64) -> Result<u64, Error> {
    let frequency = counter_frequency_hz()?;
    let now_ticks = monotonic_ticks();
    let now_nanoseconds = ticks_to_nanoseconds(now_ticks, frequency)?;
    if nanoseconds <= now_nanoseconds {
        return Ok(now_ticks);
    }
    let delta_ticks = nanoseconds_to_ticks(nanoseconds - now_nanoseconds, frequency)?;
    if delta_ticks > i64::MAX as u64 {
        return Err(Error::DeadlineTooFar);
    }
    Ok(now_ticks.wrapping_add(delta_ticks))
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
