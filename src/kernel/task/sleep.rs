// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deadline-based blocking for schedulable host threads.
//!
//! Sleep is policy layered on the common timed-wait transaction. Timer expiry,
//! notification, and cancellation therefore share the same generation-tagged
//! arbitration used by every other scheduler wait.

use hyper::hal::timer::deadline_reached;

use super::{TimedWaitError, WaitOutcome, WaitQueue};
use crate::kernel::task::scheduler;

const NANOSECONDS_PER_MICROSECOND: u64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u64 = 1_000_000;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SleepError {
    Allocation,
    DurationOverflow,
    Scheduler(scheduler::Error),
    Time(crate::kernel::time::Error),
    TimerCleanup(crate::kernel::time::Error),
    UnexpectedWake(WaitOutcome),
}

impl From<TimedWaitError> for SleepError {
    fn from(error: TimedWaitError) -> Self {
        match error {
            TimedWaitError::Allocation => Self::Allocation,
            TimedWaitError::Scheduler(error) => Self::Scheduler(error),
            TimedWaitError::Time(error) => Self::Time(error),
            TimedWaitError::TimerCleanup(error) => Self::TimerCleanup(error),
        }
    }
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

/// Blocks the calling Thread for at least `nanoseconds` of monotonic time.
///
/// A zero duration returns immediately. A nonzero sleep requires a schedulable
/// Thread context with local interrupts and preemption enabled. The timer
/// remains on its source CPU while the blocked Thread may migrate; expiry
/// wakes it on its then-current scheduler assignment.
pub fn sleep_ns(nanoseconds: u64) -> Result<(), SleepError> {
    sleep_for(nanoseconds)
}

/// Blocks the calling Thread for at least `microseconds` of monotonic time.
pub fn sleep_us(microseconds: u64) -> Result<(), SleepError> {
    sleep_for_scaled(microseconds, NANOSECONDS_PER_MICROSECOND)
}

/// Blocks the calling Thread for at least `milliseconds` of monotonic time.
pub fn sleep_ms(milliseconds: u64) -> Result<(), SleepError> {
    sleep_for_scaled(milliseconds, NANOSECONDS_PER_MILLISECOND)
}

/// Blocks the calling Thread for at least `seconds` of monotonic time.
pub fn sleep_s(seconds: u64) -> Result<(), SleepError> {
    sleep_for_scaled(seconds, NANOSECONDS_PER_SECOND)
}

fn sleep_for_scaled(duration: u64, nanoseconds_per_unit: u64) -> Result<(), SleepError> {
    let nanoseconds = duration
        .checked_mul(nanoseconds_per_unit)
        .ok_or(SleepError::DurationOverflow)?;
    sleep_for(nanoseconds)
}

fn sleep_for(nanoseconds: u64) -> Result<(), SleepError> {
    if nanoseconds == 0 {
        return Ok(());
    }
    let deadline = crate::kernel::time::deadline_after(nanoseconds)?;
    sleep_until_future(deadline)
}

/// Blocks the calling Thread until the monotonic counter reaches `deadline`.
///
/// Deadlines at or before the current counter value return immediately. A
/// caller must supply a deadline within the counter's unambiguous half-range,
/// matching the kernel timer queue contract.
pub fn sleep_until(deadline: u64) -> Result<(), SleepError> {
    if deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
        return Ok(());
    }
    sleep_until_future(deadline)
}

fn sleep_until_future(deadline: u64) -> Result<(), SleepError> {
    let waiters = WaitQueue::new();
    match waiters.wait_until(deadline)? {
        WaitOutcome::TimedOut => Ok(()),
        outcome => Err(SleepError::UnexpectedWake(outcome)),
    }
}
