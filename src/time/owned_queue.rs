//! Dynamically owned deadline queue implemented as a safe intrusive list.
//!
//! Every timer node owns the next node through `Option<Box<TimerNode>>`.
//! Capacity is therefore bounded by available memory rather than a central
//! slot array. Allocation and destruction remain separate from queue mutation,
//! allowing callers to keep allocator activity outside their timer lock.

#![forbid(unsafe_code)]

use alloc::boxed::Box;

use super::queue::{
    Error, QueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode, next_generation,
};
use crate::hal::timer::deadline_reached;

struct TimerNode {
    identity: u64,
    deadline: u64,
    sequence: u64,
    mode: TimerMode,
    callback: TimerCallback,
    context: usize,
    next: Option<Box<Self>>,
}

/// An allocated timer node that is not linked into a queue.
///
/// Construct this before acquiring a queue lock and drop cancelled timers
/// after releasing it. This keeps the global allocator outside the timer
/// queue's lock ordering.
pub struct PendingTimer(Box<TimerNode>);

impl PendingTimer {
    pub fn try_new(
        deadline: u64,
        mode: TimerMode,
        callback: TimerCallback,
        context: usize,
    ) -> Result<Self, Error> {
        if matches!(
            mode,
            TimerMode::Periodic {
                interval: 0 | 0x8000_0000_0000_0000..
            }
        ) {
            return Err(Error::InvalidInterval);
        }
        crate::mm::try_box(TimerNode {
            identity: 0,
            deadline,
            sequence: 0,
            mode,
            callback,
            context,
            next: None,
        })
        .map(Self)
        .map_err(|_| Error::Allocation)
    }
}

/// A callback delivery whose one-shot node, if any, remains owned until the
/// delivery is invoked or dropped outside the queue lock.
pub struct ExpiredTimer {
    event: TimerEvent,
    callback: TimerCallback,
    context: usize,
    _retired: Option<PendingTimer>,
}

impl ExpiredTimer {
    pub const fn event(&self) -> TimerEvent {
        self.event
    }

    pub const fn context(&self) -> usize {
        self.context
    }

    pub fn invoke(self) {
        (self.callback)(self.event, self.context);
    }
}

/// A deadline-ordered, dynamically owned intrusive list.
///
/// Insertions, cancellation, and rescheduling are O(n); expiry of the earliest
/// timer is O(1). The queue uses only safe Rust: ownership links replace raw
/// pointers, and all traversal requires an exclusive borrow of the queue.
/// The two-phase ownership API permits a future intrusive tree or timer wheel
/// without changing kernel allocation and lock boundaries.
pub struct OwnedDeadlineQueue {
    queue_id: usize,
    head: Option<Box<TimerNode>>,
    len: usize,
    next_identity: u64,
    next_sequence: u64,
    stats: QueueStats,
}

impl OwnedDeadlineQueue {
    pub const fn new() -> Self {
        Self {
            queue_id: 0,
            head: None,
            len: 0,
            next_identity: 0,
            next_sequence: 0,
            stats: QueueStats {
                active_timers: 0,
                peak_timers: 0,
                schedules: 0,
                schedule_failures: 0,
                cancellations: 0,
                reschedules: 0,
                callbacks: 0,
                overruns: 0,
            },
        }
    }

    pub fn initialize_id(&mut self, queue_id: usize) -> Result<(), Error> {
        if self.stats.schedules != 0 || self.len != 0 {
            return Err(Error::QueueAlreadyUsed);
        }
        self.queue_id = queue_id;
        Ok(())
    }

    /// Links a preallocated timer without invoking the allocator.
    pub fn insert(&mut self, mut pending: PendingTimer) -> TimerHandle {
        let identity = self.take_identity();
        pending.0.identity = identity;
        pending.0.sequence = self.take_sequence();
        self.insert_node(pending.0);
        self.stats.schedules = self.stats.schedules.saturating_add(1);
        self.refresh_active_stats();
        TimerHandle::owned(self.queue_id, identity)
    }

    /// Unlinks a timer and returns its ownership to the caller.
    ///
    /// The returned allocation should be dropped after releasing the queue
    /// lock.
    pub fn cancel(&mut self, handle: TimerHandle) -> Result<PendingTimer, Error> {
        let node = self.detach(handle)?;
        self.stats.cancellations = self.stats.cancellations.saturating_add(1);
        self.refresh_active_stats();
        Ok(PendingTimer(node))
    }

    pub fn reschedule(&mut self, handle: TimerHandle, deadline: u64) -> Result<(), Error> {
        let mut node = self.detach(handle)?;
        node.deadline = deadline;
        node.sequence = self.take_sequence();
        self.insert_node(node);
        self.stats.reschedules = self.stats.reschedules.saturating_add(1);
        Ok(())
    }

    pub fn pop_expired(&mut self, now: u64) -> Option<ExpiredTimer> {
        let deadline = self.head.as_ref()?.deadline;
        if !deadline_reached(now, deadline) {
            return None;
        }
        let mut node = self.head.take()?;
        self.head = node.next.take();
        self.len -= 1;
        let identity = node.identity;
        let mode = node.mode;
        let callback = node.callback;
        let context = node.context;
        let (overruns, retired) = match mode {
            TimerMode::OneShot => (0, Some(PendingTimer(node))),
            TimerMode::Periodic { interval } => {
                let periods = now.wrapping_sub(deadline) / interval + 1;
                node.deadline = deadline.wrapping_add(interval.wrapping_mul(periods));
                node.sequence = self.take_sequence();
                self.insert_node(node);
                (periods - 1, None)
            }
        };
        self.stats.callbacks = self.stats.callbacks.saturating_add(1);
        self.stats.overruns = self.stats.overruns.saturating_add(overruns);
        self.refresh_active_stats();
        Some(ExpiredTimer {
            event: TimerEvent {
                handle: TimerHandle::owned(self.queue_id, identity),
                deadline,
                observed_at: now,
                overruns,
            },
            callback,
            context,
            _retired: retired,
        })
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.head.as_ref().map(|node| node.deadline)
    }

    pub const fn stats(&self) -> QueueStats {
        self.stats
    }

    fn detach(&mut self, handle: TimerHandle) -> Result<Box<TimerNode>, Error> {
        let identity = handle
            .owned_identity(self.queue_id)
            .ok_or(Error::InvalidHandle)?;
        let mut link = &mut self.head;
        loop {
            let matches = link.as_ref().is_some_and(|node| node.identity == identity);
            if matches {
                let Some(mut node) = link.take() else {
                    return Err(Error::InvalidHandle);
                };
                *link = node.next.take();
                self.len -= 1;
                return Ok(node);
            }
            let Some(node) = link.as_mut() else {
                return Err(Error::InvalidHandle);
            };
            link = &mut node.next;
        }
    }

    fn insert_node(&mut self, mut node: Box<TimerNode>) {
        // Own the traversed prefix temporarily. This is a safe-Rust cursor:
        // it avoids both raw links and recursion proportional to queue length.
        let mut remainder = self.head.take();
        let mut reversed_prefix = None;
        loop {
            match remainder.take() {
                Some(mut current) if !node_precedes(&node, &current) => {
                    remainder = current.next.take();
                    current.next = reversed_prefix;
                    reversed_prefix = Some(current);
                }
                Some(current) => {
                    node.next = Some(current);
                    remainder = Some(node);
                    break;
                }
                None => {
                    remainder = Some(node);
                    break;
                }
            }
        }
        while let Some(mut current) = reversed_prefix.take() {
            reversed_prefix = current.next.take();
            current.next = remainder;
            remainder = Some(current);
        }
        self.head = remainder;
        self.len += 1;
    }

    fn take_identity(&mut self) -> u64 {
        self.next_identity = next_generation(self.next_identity);
        self.next_identity
    }

    fn take_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        sequence
    }

    fn refresh_active_stats(&mut self) {
        self.stats.active_timers = self.len;
        self.stats.peak_timers = self.stats.peak_timers.max(self.len);
    }
}

impl Default for OwnedDeadlineQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OwnedDeadlineQueue {
    fn drop(&mut self) {
        // Iterative destruction avoids recursive Box drops on a long list.
        while let Some(mut node) = self.head.take() {
            self.head = node.next.take();
        }
        self.len = 0;
    }
}

fn node_precedes(left: &TimerNode, right: &TimerNode) -> bool {
    if left.deadline == right.deadline {
        left.sequence < right.sequence
    } else {
        (left.deadline.wrapping_sub(right.deadline) as i64) < 0
    }
}
