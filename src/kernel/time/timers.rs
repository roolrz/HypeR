//! Per-CPU software timers multiplexed onto one architectural comparator.

use hyper::hal::timer::DeadlineTimer;
use hyper::sync::InterruptSpinLock;
use hyper::time::{DeadlineQueue, TimerQueueError};

pub use hyper::time::{QueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const CAPACITY: usize = hyper::config::TIMER_QUEUE_CAPACITY as usize;
const MAX_CALLBACKS_PER_INTERRUPT: usize = CAPACITY;

type TimerLock = InterruptSpinLock<ProcessorTimers, crate::arch::LocalInterruptMask>;

static TIMERS: [TimerLock; MAX_CPUS] = [const { TimerLock::new(ProcessorTimers::new()) }; MAX_CPUS];

struct ProcessorTimers {
    initialized: bool,
    queue: DeadlineQueue<CAPACITY>,
}

impl ProcessorTimers {
    const fn new() -> Self {
        Self {
            initialized: false,
            queue: DeadlineQueue::new(),
        }
    }
}

pub(super) fn initialize_local() -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        if timers.initialized {
            return Err(super::Error::TimerQueueAlreadyInitialized);
        }
        crate::arch::ArchitectureTimer::disable();
        timers.queue.initialize_id(cpu)?;
        timers.initialized = true;
        Ok(())
    })
}

pub fn schedule_at(
    deadline: u64,
    mode: TimerMode,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        let previous = timers.queue.next_deadline();
        let handle = timers.queue.schedule(deadline, mode, callback, context)?;
        if timers.queue.next_deadline() != previous
            && let Err(error) = program_next(&timers.queue)
        {
            let _ = timers.queue.cancel(handle);
            return Err(error);
        }
        Ok(handle)
    })
}

pub fn schedule_after(
    nanoseconds: u64,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    let deadline = super::deadline_after(nanoseconds)?;
    schedule_at(deadline, TimerMode::OneShot, callback, context)
}

pub fn schedule_periodic(
    first_deadline: u64,
    interval_ticks: u64,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    schedule_at(
        first_deadline,
        TimerMode::Periodic {
            interval: interval_ticks,
        },
        callback,
        context,
    )
}

/// Cancels a timer owned by the calling CPU.
pub fn cancel(handle: TimerHandle) -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        let previous = timers.queue.next_deadline();
        timers.queue.cancel(handle)?;
        if timers.queue.next_deadline() != previous {
            program_next(&timers.queue)?;
        }
        Ok(())
    })
}

/// Changes a timer deadline without changing its callback or periodic mode.
pub fn reschedule(handle: TimerHandle, deadline: u64) -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        timers.queue.reschedule(handle, deadline)?;
        program_next(&timers.queue)
    })
}

pub fn local_statistics() -> Option<QueueStats> {
    let cpu = current_cpu().ok()?;
    TIMERS[cpu].with(|timers| timers.initialized.then(|| timers.queue.stats()))
}

pub(super) fn handle_interrupt() -> Result<usize, super::Error> {
    let cpu = current_cpu()?;
    crate::arch::ArchitectureTimer::mask();
    let mut callbacks = 0;
    while callbacks < MAX_CALLBACKS_PER_INTERRUPT {
        let expired = TIMERS[cpu].with(|timers| {
            ensure_initialized(timers)?;
            Ok::<_, super::Error>(timers.queue.pop_expired(super::monotonic_ticks()))
        })?;
        let Some((event, callback, context)) = expired else {
            break;
        };
        callback(event, context);
        callbacks += 1;
    }
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        program_next(&timers.queue)
    })?;
    Ok(callbacks)
}

fn current_cpu() -> Result<usize, super::Error> {
    let cpu = crate::arch::current_cpu_index();
    (cpu < MAX_CPUS)
        .then_some(cpu)
        .ok_or(super::Error::InvalidCpuIndex)
}

fn ensure_initialized(timers: &ProcessorTimers) -> Result<(), super::Error> {
    if timers.initialized {
        Ok(())
    } else {
        Err(super::Error::TimerQueueNotInitialized)
    }
}

fn program_next(queue: &DeadlineQueue<CAPACITY>) -> Result<(), super::Error> {
    match queue.next_deadline() {
        Some(deadline) => crate::arch::ArchitectureTimer::set_deadline(deadline)?,
        None => crate::arch::ArchitectureTimer::disable(),
    }
    Ok(())
}

impl From<TimerQueueError> for super::Error {
    fn from(error: TimerQueueError) -> Self {
        Self::TimerQueue(error)
    }
}
