// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

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
    claim: Option<TimerClaim>,
    claim_context: usize,
    recycle: Option<TimerRecycle>,
    recycle_context: usize,
    next: Option<Box<Self>>,
}

pub type TimerRecycle = fn(ReservedTimerNode, usize);
pub type TimerClaim = fn(TimerEvent, usize);

pub struct ReservedTimerCallbacks {
    pub callback: TimerCallback,
    pub context: usize,
    pub claim: TimerClaim,
    pub claim_context: usize,
    pub recycle: TimerRecycle,
    pub recycle_context: usize,
}

/// An allocated timer node that is not linked into a queue.
///
/// Construct this before acquiring a queue lock and drop cancelled timers
/// after releasing it. This keeps the global allocator outside the timer
/// queue's lock ordering.
pub struct PendingTimer(Box<TimerNode>);

impl PendingTimer {
    pub const fn allocation_size() -> usize {
        core::mem::size_of::<TimerNode>()
    }

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
            claim: None,
            claim_context: 0,
            recycle: None,
            recycle_context: 0,
            next: None,
        })
        .map(Self)
        .map_err(|_| Error::Allocation)
    }
}

/// One preallocated one-shot node reserved for allocation-free rearming.
pub struct ReservedTimerNode(Box<TimerNode>);

impl ReservedTimerNode {
    pub fn try_new() -> Result<Self, Error> {
        crate::mm::try_box(TimerNode {
            identity: 0,
            deadline: 0,
            sequence: 0,
            mode: TimerMode::OneShot,
            callback: reserved_unarmed,
            context: 0,
            claim: None,
            claim_context: 0,
            recycle: None,
            recycle_context: 0,
            next: None,
        })
        .map(Self)
        .map_err(|_| Error::Allocation)
    }

    pub fn prepare(
        mut self,
        deadline: u64,
        callbacks: ReservedTimerCallbacks,
    ) -> PendingReservedTimer {
        self.0.deadline = deadline;
        self.0.mode = TimerMode::OneShot;
        self.0.callback = callbacks.callback;
        self.0.context = callbacks.context;
        self.0.claim = Some(callbacks.claim);
        self.0.claim_context = callbacks.claim_context;
        self.0.recycle = Some(callbacks.recycle);
        self.0.recycle_context = callbacks.recycle_context;
        PendingReservedTimer(self.0)
    }
}

/// Configured reserved node not yet linked into a deadline queue.
pub struct PendingReservedTimer(Box<TimerNode>);

/// A callback delivery whose one-shot node, if any, remains owned until the
/// delivery is invoked or dropped outside the queue lock.
#[must_use = "expired timers must be invoked; reserved deliveries recover on drop"]
pub struct ExpiredTimer {
    event: TimerEvent,
    callback: TimerCallback,
    context: usize,
    retired: Option<RetiredTimer>,
}

enum RetiredTimer {
    Ordinary(PendingTimer),
    Reserved(ReservedTimerNode, TimerRecycle, usize),
}

impl ExpiredTimer {
    pub const fn event(&self) -> TimerEvent {
        self.event
    }

    pub const fn context(&self) -> usize {
        self.context
    }

    pub fn invoke(mut self) {
        (self.callback)(self.event, self.context);
        self.recycle();
    }

    fn recycle(&mut self) {
        match self.retired.take() {
            Some(RetiredTimer::Ordinary(timer)) => drop(timer),
            Some(RetiredTimer::Reserved(node, recycle, recycle_context)) => {
                recycle(node, recycle_context)
            }
            None => {}
        }
    }
}

impl Drop for ExpiredTimer {
    fn drop(&mut self) {
        if matches!(self.retired, Some(RetiredTimer::Reserved(..))) {
            // A reserved expiry was already claimed under its queue lock. Its
            // notification is therefore mandatory even when a safe caller
            // abandons the delivery value.
            (self.callback)(self.event, self.context);
        }
        self.recycle();
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

    /// Links a preallocated reserved node without invoking the allocator.
    pub fn insert_reserved(&mut self, pending: PendingReservedTimer) -> TimerHandle {
        let mut node = pending.0;
        let identity = self.take_identity();
        node.identity = identity;
        node.sequence = self.take_sequence();
        self.insert_node(node);
        self.stats.schedules = self.stats.schedules.saturating_add(1);
        self.refresh_active_stats();
        TimerHandle::owned(self.queue_id, identity)
    }

    /// Unlinks a timer and returns its ownership to the caller.
    ///
    /// The returned allocation should be dropped after releasing the queue
    /// lock.
    pub fn cancel(&mut self, handle: TimerHandle) -> Result<PendingTimer, Error> {
        let node = self.detach(handle, false)?;
        self.stats.cancellations = self.stats.cancellations.saturating_add(1);
        self.refresh_active_stats();
        Ok(PendingTimer(node))
    }

    pub fn cancel_reserved(&mut self, handle: TimerHandle) -> Result<ReservedTimerNode, Error> {
        let mut node = self.detach(handle, true)?;
        node.claim = None;
        node.claim_context = 0;
        node.recycle = None;
        node.recycle_context = 0;
        self.stats.cancellations = self.stats.cancellations.saturating_add(1);
        self.refresh_active_stats();
        Ok(ReservedTimerNode(node))
    }

    pub fn reschedule(&mut self, handle: TimerHandle, deadline: u64) -> Result<(), Error> {
        let mut node = self.detach(handle, false)?;
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
        let event = TimerEvent {
            handle: TimerHandle::owned(self.queue_id, identity),
            deadline,
            observed_at: now,
            overruns: 0,
        };
        if let Some(claim) = node.claim.take() {
            claim(event, node.claim_context);
        }
        let (overruns, retired) = match mode {
            TimerMode::OneShot => {
                let retired = match (node.recycle.take(), node.recycle_context) {
                    (Some(recycle), recycle_context) => {
                        RetiredTimer::Reserved(ReservedTimerNode(node), recycle, recycle_context)
                    }
                    (None, _) => RetiredTimer::Ordinary(PendingTimer(node)),
                };
                (0, Some(retired))
            }
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
            event: TimerEvent { overruns, ..event },
            callback,
            context,
            retired,
        })
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.head.as_ref().map(|node| node.deadline)
    }

    pub const fn stats(&self) -> QueueStats {
        self.stats
    }

    fn detach(&mut self, handle: TimerHandle, reserved: bool) -> Result<Box<TimerNode>, Error> {
        let identity = handle
            .owned_identity(self.queue_id)
            .ok_or(Error::InvalidHandle)?;
        let mut link = &mut self.head;
        loop {
            let matches = link.as_ref().is_some_and(|node| node.identity == identity);
            if matches {
                if link
                    .as_ref()
                    .is_some_and(|node| node.recycle.is_some() != reserved)
                {
                    return Err(Error::InvalidHandle);
                }
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

fn reserved_unarmed(_event: TimerEvent, _context: usize) {}

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
