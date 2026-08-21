// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler wait queues for thread-context blocking and IRQ-safe wakeups.

use core::cell::UnsafeCell;

use super::scheduler;
use super::thread::ThreadId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreadQueue {
    pub head: Option<ThreadId>,
    pub tail: Option<ThreadId>,
    pub len: usize,
}

impl ThreadQueue {
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }
}

/// FIFO queue of blocked threads.
///
/// Queue links live inside each scheduler-owned `Thread`, so waiting and
/// waking never allocate. The queue must outlive every thread waiting on it;
/// safe users normally satisfy this by embedding it in a static object or in
/// an object retained by the blocked thread's stack/owner.
pub struct WaitQueue {
    state: UnsafeCell<ThreadQueue>,
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            state: UnsafeCell::new(ThreadQueue::new()),
        }
    }

    /// Blocks the current thread until a matching wake operation dequeues it.
    ///
    /// A wait queue does not retain wakeups, so this is an unconditional park,
    /// not an event counter. Condition-based primitives should combine their
    /// state lock with the scheduler's locked-park path as Mutex/Semaphore do.
    pub fn wait(&self) -> Result<(), scheduler::Error> {
        let token = scheduler::prepare_park(self)?;
        scheduler::complete_park(token);
        Ok(())
    }

    pub fn wake_one(&self) -> Result<Option<ThreadId>, scheduler::Error> {
        scheduler::wake_one(self)
    }

    pub fn wake_all(&self) -> Result<usize, scheduler::Error> {
        scheduler::wake_all(self)
    }

    pub fn len(&self) -> Result<usize, scheduler::Error> {
        scheduler::waiter_count(self)
    }

    pub fn is_empty(&self) -> Result<bool, scheduler::Error> {
        self.len().map(|len| len == 0)
    }

    pub(super) fn identity(&self) -> usize {
        self as *const Self as usize
    }

    /// Returns the internal pointer accessed only under the scheduler lock.
    pub(super) const fn state_pointer(&self) -> *mut ThreadQueue {
        self.state.get()
    }
}

impl Default for WaitQueue {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: WaitQueue state is accessed only while the global scheduler lock is
// held. The UnsafeCell exists so embedded queues remain const-constructible.
unsafe impl Sync for WaitQueue {}
