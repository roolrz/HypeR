// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deadline arbitration for scheduler wait queues.
//!
//! Timer setup and allocation happen before queue publication. The actual
//! park, notification, timeout, and cancellation transitions remain
//! allocation-free and are serialized by the scheduler's embedded wait
//! record. A timer stays on its source CPU even when its Thread migrates;
//! handle-directed cancellation retires it from that original queue.

use core::hint::spin_loop;

use hyper::hal::timer::deadline_reached;
use hyper::sync::atomic::{AtomicBool, Ordering};

use super::scheduler;
use super::wait::{WaitMobility, WaitOutcome, WaitQueue, WaitTicket};

struct TimeoutContext {
    ticket: Option<WaitTicket>,
    callback_complete: AtomicBool,
}

impl TimeoutContext {
    const fn new() -> Self {
        Self {
            ticket: None,
            callback_complete: AtomicBool::new(false),
        }
    }

    fn pointer(&self) -> usize {
        core::ptr::from_ref(self).expose_provenance()
    }

    fn wait_for_callback(&self) {
        while !self.callback_complete.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimedWaitError {
    Allocation,
    Scheduler(scheduler::Error),
    Time(crate::kernel::time::Error),
    TimerCleanup(crate::kernel::time::Error),
}

impl From<scheduler::Error> for TimedWaitError {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<crate::kernel::time::Error> for TimedWaitError {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Time(error)
    }
}

impl WaitQueue {
    /// Blocks until notification or until `deadline` is reached.
    ///
    /// A deadline at or before the current monotonic counter completes without
    /// blocking. Notifications are not retained: callers which need a durable
    /// condition should use a synchronization primitive that owns both state
    /// and a wait queue. `deadline` must be within the counter's unambiguous
    /// wrapping half-range, matching the software timer queue contract.
    pub fn wait_until(&self, deadline: u64) -> Result<WaitOutcome, TimedWaitError> {
        if deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
            return Ok(WaitOutcome::TimedOut);
        }
        scheduler::ensure_sleepable()?;

        // Allocate the callback owner before arming the Thread. Allocation is
        // deliberately outside the scheduler wait transaction.
        let mut timeout =
            hyper::mm::try_box(TimeoutContext::new()).map_err(|_| TimedWaitError::Allocation)?;
        let registration = scheduler::begin_wait(WaitMobility::Migratable)?;
        // Publish the immutable ticket before exposing the context pointer to
        // the timer queue. schedule_at masks local IRQ delivery through queue
        // insertion, and no write to `ticket` occurs after this point.
        timeout.ticket = Some(registration.ticket());
        let timer = match crate::kernel::time::schedule_at(
            deadline,
            crate::kernel::time::TimerMode::OneShot,
            expire_wait,
            timeout.pointer(),
        ) {
            Ok(timer) => timer,
            Err(error) => {
                finish_unscheduled(registration);
                return Err(error.into());
            }
        };

        let outcome = match scheduler::prepare_registered_park(self, registration)? {
            scheduler::PreparedWait::Park(token) => scheduler::complete_park(token),
            scheduler::PreparedWait::Completed(outcome) => outcome,
        };
        retire_timeout(timer, &timeout, outcome)?;
        Ok(outcome)
    }

    /// Blocks until notified or until `nanoseconds` of monotonic time elapse.
    pub fn wait_for(&self, nanoseconds: u64) -> Result<WaitOutcome, TimedWaitError> {
        let deadline = crate::kernel::time::deadline_after(nanoseconds)?;
        self.wait_until(deadline)
    }
}

fn finish_unscheduled(registration: scheduler::WaitRegistration) {
    match scheduler::finish_wait(registration) {
        Ok(None) => {}
        Ok(Some(outcome)) => wait_invariant("unscheduled wait was resolved", Some(outcome)),
        Err(error) => {
            crate::pr_crit!("HypeR: unscheduled wait cleanup failed: {error:?}");
            crate::hal::cpu::halt()
        }
    }
}

fn retire_timeout(
    timer: crate::kernel::time::TimerHandle,
    timeout: &TimeoutContext,
    outcome: WaitOutcome,
) -> Result<(), TimedWaitError> {
    if outcome == WaitOutcome::TimedOut {
        timeout.wait_for_callback();
        return Ok(());
    }

    match crate::kernel::time::cancel(timer) {
        Ok(()) => Ok(()),
        Err(crate::kernel::time::Error::TimerQueue(
            hyper::time::TimerQueueError::InvalidHandle,
        )) => {
            // Expiry detached the node before cancellation acquired its source
            // queue. The callback is the remaining owner of the raw context.
            timeout.wait_for_callback();
            Ok(())
        }
        Err(error @ crate::kernel::time::Error::Architecture(_)) => {
            // Cancellation detached the node before local comparator
            // reprogramming failed, so the callback cannot access the context.
            Err(TimedWaitError::TimerCleanup(error))
        }
        Err(error) => {
            // These lifecycle/identity failures occur before proven removal.
            // Returning could free a callback context which is still queued.
            if timeout.callback_complete.load(Ordering::Acquire) {
                return Err(TimedWaitError::TimerCleanup(error));
            }
            crate::pr_crit!("HypeR: failed to retire timed wait callback: {error:?}");
            crate::hal::cpu::halt()
        }
    }
}

fn expire_wait(_event: crate::kernel::time::TimerEvent, context: usize) {
    // SAFETY: wait_until owns this boxed context until it either cancels the
    // timer or observes callback_complete with Acquire ordering.
    let timeout = unsafe { &*core::ptr::with_exposed_provenance::<TimeoutContext>(context) };
    let result = match timeout.ticket {
        Some(ticket) => scheduler::resolve_wait(ticket, WaitOutcome::TimedOut),
        None => Err(scheduler::Error::InvalidWaitRegistration),
    };
    timeout.callback_complete.store(true, Ordering::Release);
    match result {
        Ok(resolution) if !resolution.made_ready || resolution.won => {}
        Ok(_) => wait_invariant("timeout published Ready without winning", None),
        Err(error) => {
            crate::pr_crit!("HypeR: timed wait resolution failed: {error:?}");
            crate::hal::cpu::halt()
        }
    }
}

fn wait_invariant(message: &str, outcome: Option<WaitOutcome>) -> ! {
    crate::pr_crit!("HypeR: timed wait invariant failed: {message}; outcome={outcome:?}");
    crate::hal::cpu::halt()
}
