// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deadline-based blocking for schedulable host threads.
//!
//! Each sleep owns a private wait queue and one local-CPU timer. The sleep
//! record lock linearizes timer expiry with parking, while a completion flag
//! keeps the timer callback's raw context alive until the callback has made its
//! final access. This module chooses blocking policy; the timer and scheduler
//! remain independent mechanisms.

use core::hint::spin_loop;

use hyper::hal::timer::deadline_reached;
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicBool, Ordering};

use super::scheduler;
use super::thread::ThreadId;
use super::wait::WaitQueue;

type SleepLock = InterruptSpinLock<SleepRecord, crate::arch::irq::LocalMask>;

const NANOSECONDS_PER_MICROSECOND: u64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u64 = 1_000_000;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

struct SleepRecord {
    expired: bool,
    sleeper: ThreadId,
    waiters: WaitQueue,
}

struct Sleep {
    record: SleepLock,
    callback_complete: AtomicBool,
}

impl Sleep {
    const fn new(sleeper: ThreadId) -> Self {
        Self {
            record: SleepLock::new(SleepRecord {
                expired: false,
                sleeper,
                waiters: WaitQueue::new(),
            }),
            callback_complete: AtomicBool::new(false),
        }
    }

    fn context(&self) -> usize {
        core::ptr::from_ref(self).expose_provenance()
    }

    fn wait_for_callback(&self) {
        while !self.callback_complete.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SleepError {
    Allocation,
    DurationOverflow,
    Scheduler(scheduler::Error),
    Time(crate::kernel::time::Error),
    TimerCleanup {
        park: scheduler::Error,
        timer: crate::kernel::time::Error,
    },
}

impl From<scheduler::Error> for SleepError {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<crate::kernel::time::Error> for SleepError {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Time(error)
    }
}

/// Blocks the calling thread for at least `nanoseconds` of monotonic time.
///
/// A zero duration returns immediately. A nonzero sleep requires a schedulable
/// thread context with local interrupts and preemption enabled. The thread is
/// bound to its current CPU's timer queue while blocked.
pub fn sleep_ns(nanoseconds: u64) -> Result<(), SleepError> {
    sleep_for(nanoseconds)
}

/// Blocks the calling thread for at least `microseconds` of monotonic time.
///
/// The context requirements are the same as for [`sleep_ns`]. Returns
/// [`SleepError::DurationOverflow`] when the duration cannot be represented in
/// nanoseconds.
pub fn sleep_us(microseconds: u64) -> Result<(), SleepError> {
    sleep_for_scaled(microseconds, NANOSECONDS_PER_MICROSECOND)
}

/// Blocks the calling thread for at least `milliseconds` of monotonic time.
///
/// The context requirements are the same as for [`sleep_ns`]. Returns
/// [`SleepError::DurationOverflow`] when the duration cannot be represented in
/// nanoseconds.
pub fn sleep_ms(milliseconds: u64) -> Result<(), SleepError> {
    sleep_for_scaled(milliseconds, NANOSECONDS_PER_MILLISECOND)
}

/// Blocks the calling thread for at least `seconds` of monotonic time.
///
/// The context requirements are the same as for [`sleep_ns`]. Returns
/// [`SleepError::DurationOverflow`] when the duration cannot be represented in
/// nanoseconds.
pub fn sleep_s(seconds: u64) -> Result<(), SleepError> {
    sleep_for_scaled(seconds, NANOSECONDS_PER_SECOND)
}

fn sleep_for_scaled(duration: u64, nanoseconds_per_unit: u64) -> Result<(), SleepError> {
    let nanoseconds = duration
        .checked_mul(nanoseconds_per_unit)
        .ok_or(SleepError::DurationOverflow)?;
    sleep_for(nanoseconds)
}

/// Blocks the calling thread for a duration expressed in nanoseconds.
///
/// A zero duration returns immediately. The sleep is bound to the calling
/// CPU's timer queue; the current scheduler does not migrate blocked threads.
/// Recoverable allocation, timer setup, and scheduler rejection errors are
/// reported without leaving a live timer or a blocked thread behind.
fn sleep_for(nanoseconds: u64) -> Result<(), SleepError> {
    if nanoseconds == 0 {
        return Ok(());
    }
    scheduler::ensure_sleepable()?;
    let deadline = crate::kernel::time::deadline_after(nanoseconds)?;
    sleep_until_future(deadline)
}

/// Blocks the calling thread until the monotonic counter reaches `deadline`.
///
/// Deadlines at or before the current counter value return immediately. A
/// caller must supply a deadline within the counter's unambiguous half-range,
/// matching the kernel timer queue contract.
pub fn sleep_until(deadline: u64) -> Result<(), SleepError> {
    if deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
        return Ok(());
    }
    scheduler::ensure_sleepable()?;
    sleep_until_future(deadline)
}

fn sleep_until_future(deadline: u64) -> Result<(), SleepError> {
    if deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
        return Ok(());
    }

    let sleeper = scheduler::current_thread_id()?;
    let sleep = hyper::mm::try_box(Sleep::new(sleeper)).map_err(|_| SleepError::Allocation)?;
    let timer = crate::kernel::time::schedule_at(
        deadline,
        crate::kernel::time::TimerMode::OneShot,
        expire_sleep,
        sleep.context(),
    )?;

    let park = sleep.record.with(|record| {
        if record.expired {
            Ok(None)
        } else {
            scheduler::prepare_park_locked(&record.waiters).map(Some)
        }
    });
    let park = match park {
        Ok(park) => park,
        Err(error) => {
            return match crate::kernel::time::cancel(timer) {
                Ok(()) => Err(SleepError::Scheduler(error)),
                Err(
                    timer @ crate::kernel::time::Error::TimerQueue(
                        hyper::time::TimerQueueError::InvalidHandle,
                    ),
                ) => {
                    // Expiry won the race. Its callback must release the raw
                    // context before the sleep allocation can be dropped.
                    sleep.wait_for_callback();
                    Err(SleepError::TimerCleanup { park: error, timer })
                }
                Err(timer @ crate::kernel::time::Error::Architecture(_)) => {
                    // The timer queue detaches the node before reprogramming
                    // the hardware comparator, so this error cannot retain
                    // the callback context.
                    Err(SleepError::TimerCleanup { park: error, timer })
                }
                Err(timer) => {
                    // Invalid CPU/queue lifecycle errors occur before timer
                    // removal. Returning would free a callback context which
                    // may still be queued, so this is a soundness boundary.
                    if sleep.callback_complete.load(Ordering::Acquire) {
                        return Err(SleepError::TimerCleanup { park: error, timer });
                    }
                    crate::pr_crit!(
                        "HypeR: failed to retire thread sleep timer after park error: \
                         park={error:?} timer={timer:?}"
                    );
                    crate::arch::cpu::halt()
                }
            };
        }
    };

    if let Some(token) = park {
        scheduler::complete_park(token);
    }
    // A remote CPU may run the awakened thread before the callback returns.
    // Its release store is the lifetime boundary for the raw timer context.
    sleep.wait_for_callback();
    Ok(())
}

fn expire_sleep(_event: crate::kernel::time::TimerEvent, context: usize) {
    // SAFETY: `context` points to the boxed Sleep retained by sleep_until_future.
    // That owner waits for callback_complete before returning or dropping it.
    let sleep = unsafe { &*core::ptr::with_exposed_provenance::<Sleep>(context) };
    let result = sleep.record.with(|record| {
        record.expired = true;
        let awakened = scheduler::wake_one(&record.waiters)?;
        if awakened.is_some_and(|thread| thread != record.sleeper) {
            return Err(scheduler::Error::QueueCorrupted);
        }
        Ok(())
    });
    sleep.callback_complete.store(true, Ordering::Release);
    if let Err(error) = result {
        // The sleeping thread may otherwise remain blocked forever, and the
        // callback context could never be reclaimed safely.
        crate::pr_crit!("HypeR: thread sleep wakeup failed: {error:?}");
        crate::arch::cpu::halt()
    }
}
