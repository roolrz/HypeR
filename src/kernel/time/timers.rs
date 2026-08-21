//! Per-CPU software timers multiplexed onto one architectural comparator.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::hal::timer::DeadlineTimer;
use hyper::sync::InterruptSpinLock;
use hyper::time::{OwnedDeadlineQueue, PendingTimer, TimerQueueError};

pub use hyper::time::{QueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode};

/// Bound one interrupt's callback work without imposing a timer storage cap.
const MAX_CALLBACKS_PER_INTERRUPT: usize = 64;

type TimerLock = InterruptSpinLock<ProcessorTimers, crate::arch::irq::LocalMask>;

static TIMERS: PerCpu<TimerLock> =
    PerCpu::new([const { TimerLock::new(ProcessorTimers::new()) }; hyper::cpu::MAX_CPUS]);

struct ProcessorTimers {
    initialized: bool,
    queue: OwnedDeadlineQueue,
}

impl ProcessorTimers {
    const fn new() -> Self {
        Self {
            initialized: false,
            queue: OwnedDeadlineQueue::new(),
        }
    }
}

pub(super) fn initialize_local() -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        if timers.initialized {
            return Err(super::Error::TimerQueueAlreadyInitialized);
        }
        crate::arch::time::Timer::disable();
        timers.queue.initialize_id(cpu.get())?;
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
    TIMERS[cpu].with(|timers| ensure_initialized(timers))?;
    let pending = PendingTimer::try_new(deadline, mode, callback, context)?;
    let (result, retired) = TIMERS[cpu].with(|timers| {
        let previous = timers.queue.next_deadline();
        let handle = timers.queue.insert(pending);
        if timers.queue.next_deadline() != previous
            && let Err(error) = program_next(&timers.queue)
        {
            let retired = timers.queue.cancel(handle).ok();
            return (Err(error), retired);
        }
        (Ok(handle), None)
    });
    drop(retired);
    result
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
    let (result, retired) = TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        let previous = timers.queue.next_deadline();
        let retired = timers.queue.cancel(handle)?;
        let result = if timers.queue.next_deadline() != previous {
            program_next(&timers.queue)
        } else {
            Ok(())
        };
        Ok::<_, super::Error>((result, retired))
    })?;
    drop(retired);
    result
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
    crate::arch::time::Timer::mask();
    let mut callbacks = 0;
    while callbacks < MAX_CALLBACKS_PER_INTERRUPT {
        let expired = TIMERS[cpu].with(|timers| {
            ensure_initialized(timers)?;
            Ok::<_, super::Error>(timers.queue.pop_expired(super::monotonic_ticks()))
        })?;
        let Some(expired) = expired else {
            break;
        };
        expired.invoke();
        callbacks += 1;
    }
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        program_next(&timers.queue)
    })?;
    Ok(callbacks)
}

pub(super) fn request_hardware_wakeup(deadline: u64) -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        let earlier = timers
            .queue
            .next_deadline()
            .is_none_or(|next| (deadline.wrapping_sub(next) as i64) < 0);
        if earlier {
            crate::arch::time::Timer::set_deadline(deadline)?;
        }
        Ok(())
    })
}

fn current_cpu() -> Result<CpuIndex, super::Error> {
    crate::kernel::cpu::current_index().ok_or(super::Error::InvalidCpuIndex)
}

fn ensure_initialized(timers: &ProcessorTimers) -> Result<(), super::Error> {
    if timers.initialized {
        Ok(())
    } else {
        Err(super::Error::TimerQueueNotInitialized)
    }
}

fn program_next(queue: &OwnedDeadlineQueue) -> Result<(), super::Error> {
    match queue.next_deadline() {
        Some(deadline) => crate::arch::time::Timer::set_deadline(deadline)?,
        None => crate::arch::time::Timer::disable(),
    }
    Ok(())
}

impl From<TimerQueueError> for super::Error {
    fn from(error: TimerQueueError) -> Self {
        Self::TimerQueue(error)
    }
}
