// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Intrusive primitives and class-aware per-CPU ready queues.
//!
//! Class ordering is resolved here, while each class owns its queue policy.
//! The scheduler state machine never compares parameters from unrelated
//! scheduling classes.

use hyper::cpu::CpuIndex;

use super::Error;
use super::registry::{CpuThreadTableAuthority, ThreadControlAuthority, ThreadTableWriteAuthority};
use crate::kernel::task::policy::{PRIORITY_LEVELS, SchedulingPolicy, ThreadPriority};
use crate::kernel::task::thread::{QueueLinks, QueueMembership, ThreadId};
use crate::kernel::task::wait::ThreadQueue;

const PRIORITIES_PER_BITMAP_WORD: usize = u64::BITS as usize;
const PRIORITY_BITMAP_WORDS: usize = PRIORITY_LEVELS.div_ceil(PRIORITIES_PER_BITMAP_WORD);

/// Runnable classes owned by one CPU.
///
/// Idle is intentionally absent: its Thread is selected only when every
/// runnable class is empty.
pub(super) struct CpuRunQueue {
    realtime: FixedPriorityFifo,
    fair: FairQueue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadyThread {
    pub id: ThreadId,
    pub policy: SchedulingPolicy,
}

/// Combined transition-lock and CPU-lock authority for one ready queue.
///
/// Coordinator-owned candidates may enter the CPU domain, while existing
/// ready/current entities are accessed only through the matching CPU token.
pub(super) struct ReadyQueueAuthority<'table, 'cpu> {
    coordinator: ThreadTableWriteAuthority<'table>,
    cpu: CpuThreadTableAuthority<'cpu>,
}

/// Pure CPU-local authority for already CPU-owned current/ready entities.
///
/// It cannot publish coordinator-owned Threads and cannot observe global
/// control links. This is the only authority admitted by local scheduling.
pub(super) struct LocalReadyQueueAuthority<'cpu> {
    cpu: CpuThreadTableAuthority<'cpu>,
}

impl<'cpu> LocalReadyQueueAuthority<'cpu> {
    pub(super) const fn new(cpu: CpuThreadTableAuthority<'cpu>) -> Self {
        Self { cpu }
    }
}

/// TransitionLock-owned authority for global waiting/terminated queues.
///
/// It deliberately has no CPU scheduling token: control links are a separate
/// Thread cell, so adjacent nodes may have schedules owned by different CPUs.
pub(super) struct ControlQueueAuthority<'table> {
    control: ThreadControlAuthority<'table>,
}

impl<'table> ControlQueueAuthority<'table> {
    pub(super) const fn new(control: ThreadControlAuthority<'table>) -> Self {
        Self { control }
    }
}

impl<'table, 'cpu> ReadyQueueAuthority<'table, 'cpu> {
    pub(super) const fn new(
        coordinator: ThreadTableWriteAuthority<'table>,
        cpu: CpuThreadTableAuthority<'cpu>,
    ) -> Self {
        Self { coordinator, cpu }
    }

    /// Mutates schedule state through the authority selected by its owner locator.
    fn with_schedule_mut<R>(
        &mut self,
        id: ThreadId,
        operation: impl for<'schedule> FnOnce(
            &'schedule mut crate::kernel::task::thread::ThreadScheduleState,
        ) -> R,
    ) -> Result<R, Error> {
        let owner = self
            .coordinator
            .with_thread(id, |thread| thread.schedule_owner_cpu())?;
        match owner {
            None => self
                .coordinator
                .with_thread_mut(id, |thread| thread.with_coordinator_schedule_mut(operation)),
            Some(cpu) if cpu == self.cpu.cpu() => self
                .cpu
                .with_thread_mut(id, |_thread, schedule| operation(schedule)),
            Some(_) => Err(Error::InvalidThreadState),
        }
    }

    fn scheduling_policy(&self, id: ThreadId) -> Result<SchedulingPolicy, Error> {
        let owner = self
            .coordinator
            .with_thread(id, |thread| thread.schedule_owner_cpu())?;
        match owner {
            None => self
                .coordinator
                .with_thread(id, |thread| thread.scheduling_policy()),
            Some(cpu) if cpu == self.cpu.cpu() => self
                .cpu
                .with_thread(id, |_thread, schedule| schedule.scheduling),
            Some(_) => Err(Error::InvalidThreadState),
        }
    }
}

impl CpuRunQueue {
    pub const fn new() -> Self {
        Self {
            realtime: FixedPriorityFifo::new(),
            fair: FairQueue::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.realtime.len() + self.fair.len()
    }

    pub const fn has_fair_threads(&self) -> bool {
        self.fair.len() != 0
    }

    pub const fn real_time_len(&self) -> usize {
        self.realtime.len()
    }

    pub const fn fair_len(&self) -> usize {
        self.fair.len()
    }

    /// Validates and returns the next runnable thread in class order.
    pub fn peek_next(
        &self,
        threads: &impl ReadyQueueAccess,
        cpu: CpuIndex,
    ) -> Result<Option<ReadyThread>, Error> {
        if let Some((id, priority)) = self.realtime.peek_highest(threads, cpu)? {
            return Ok(Some(ReadyThread {
                id,
                policy: SchedulingPolicy::fifo(priority),
            }));
        }
        self.fair.peek(threads, cpu).map(|candidate| {
            candidate.map(|id| ReadyThread {
                id,
                policy: SchedulingPolicy::fair(),
            })
        })
    }

    pub fn enqueue(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        match threads.scheduling_policy(id)? {
            SchedulingPolicy::Fifo { priority } => {
                self.realtime.enqueue(threads, id, cpu, priority.get())
            }
            SchedulingPolicy::Fair => self.fair.enqueue(threads, id, cpu),
            SchedulingPolicy::Idle => Err(Error::InvalidThreadState),
        }
    }

    pub fn enqueue_front(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        match threads.scheduling_policy(id)? {
            SchedulingPolicy::Fifo { priority } => {
                self.realtime
                    .enqueue_front(threads, id, cpu, priority.get())
            }
            SchedulingPolicy::Fair => self.fair.enqueue_front(threads, id, cpu),
            SchedulingPolicy::Idle => Err(Error::InvalidThreadState),
        }
    }

    pub fn dequeue(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        if let Some(id) = self.realtime.dequeue(threads, cpu)? {
            return Ok(Some(id));
        }
        self.fair.dequeue(threads, cpu)
    }

    pub fn remove(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
        membership: QueueMembership,
    ) -> Result<(), Error> {
        match membership {
            QueueMembership::ReadyRealTime {
                cpu: member_cpu,
                priority,
            } if member_cpu == cpu => self.realtime.remove(threads, id, cpu, priority),
            QueueMembership::ReadyFair { cpu: member_cpu } if member_cpu == cpu => {
                self.fair.remove(threads, id, cpu)
            }
            _ => Err(Error::QueueCorrupted),
        }
    }
}

/// Initial backend for the fair scheduling class.
///
/// This FIFO contains only ready threads. Running-thread slice accounting is
/// owned by scheduler state, so replacing this backend does not affect generic
/// intrusive queue or context-switch machinery.
struct FairQueue {
    queue: ThreadQueue,
}

impl FairQueue {
    const fn new() -> Self {
        Self {
            queue: ThreadQueue::new(),
        }
    }

    const fn len(&self) -> usize {
        self.queue.len
    }

    fn peek(
        &self,
        threads: &impl ReadyQueueAccess,
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        let Some(id) = self.queue.head else {
            if self.queue.len == 0 && self.queue.tail.is_none() {
                return Ok(None);
            }
            return Err(Error::QueueCorrupted);
        };
        let links = queue_links(threads, id)?;
        if self.queue.len == 0 || links.membership != (QueueMembership::ReadyFair { cpu }) {
            return Err(Error::QueueCorrupted);
        }
        validate_neighbors(threads, &self.queue, links, id)?;
        Ok(Some(id))
    }

    fn enqueue(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        push(
            threads,
            &mut self.queue,
            id,
            QueueMembership::ReadyFair { cpu },
        )?;
        Ok(())
    }

    fn enqueue_front(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        push_front(
            threads,
            &mut self.queue,
            id,
            QueueMembership::ReadyFair { cpu },
        )?;
        Ok(())
    }

    fn dequeue(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        pop(threads, &mut self.queue, QueueMembership::ReadyFair { cpu })
    }

    fn remove(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        remove(
            threads,
            &mut self.queue,
            id,
            QueueMembership::ReadyFair { cpu },
        )
    }
}

struct FixedPriorityFifo {
    queues: [ThreadQueue; PRIORITY_LEVELS],
    bitmap: [u64; PRIORITY_BITMAP_WORDS],
    len: usize,
}

impl FixedPriorityFifo {
    pub const fn new() -> Self {
        Self {
            queues: [ThreadQueue::new(); PRIORITY_LEVELS],
            bitmap: [0; PRIORITY_BITMAP_WORDS],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    fn peek_highest(
        &self,
        threads: &impl ReadyQueueAccess,
        cpu: CpuIndex,
    ) -> Result<Option<(ThreadId, ThreadPriority)>, Error> {
        let Some(priority) = self.highest_ready_priority() else {
            return Ok(None);
        };
        let queue = self.queues.get(priority).ok_or(Error::QueueCorrupted)?;
        let id = queue.head.ok_or(Error::QueueCorrupted)?;
        let links = queue_links(threads, id)?;
        let membership = QueueMembership::ReadyRealTime {
            cpu,
            priority: priority as u8,
        };
        if links.membership != membership || queue.len == 0 {
            return Err(Error::QueueCorrupted);
        }
        validate_neighbors(threads, queue, links, id)?;
        Ok(Some((id, ThreadPriority::new(priority as u8))))
    }

    pub fn enqueue(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        let membership = QueueMembership::ReadyRealTime { cpu, priority };
        let new_len = self.len.checked_add(1).ok_or(Error::QueueCorrupted)?;
        let queue = self
            .queues
            .get_mut(usize::from(priority))
            .ok_or(Error::QueueCorrupted)?;
        push(threads, queue, id, membership)?;
        self.mark_non_empty(priority);
        self.len = new_len;
        Ok(())
    }

    fn enqueue_front(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        let membership = QueueMembership::ReadyRealTime { cpu, priority };
        let new_len = self.len.checked_add(1).ok_or(Error::QueueCorrupted)?;
        let queue = self
            .queues
            .get_mut(usize::from(priority))
            .ok_or(Error::QueueCorrupted)?;
        push_front(threads, queue, id, membership)?;
        self.mark_non_empty(priority);
        self.len = new_len;
        Ok(())
    }

    pub fn dequeue(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        let Some(priority) = self.highest_ready_priority() else {
            return Ok(None);
        };
        let membership = QueueMembership::ReadyRealTime {
            cpu,
            priority: priority as u8,
        };
        let new_len = self.len.checked_sub(1).ok_or(Error::QueueCorrupted)?;
        let queue = self.queues.get_mut(priority).ok_or(Error::QueueCorrupted)?;
        let id = pop(threads, queue, membership)?.ok_or(Error::QueueCorrupted)?;
        self.len = new_len;
        if self.queues[priority].len == 0 {
            self.mark_empty(priority as u8);
        }
        Ok(Some(id))
    }

    pub fn remove(
        &mut self,
        threads: &mut impl ReadyQueueAccess,
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        let new_len = self.len.checked_sub(1).ok_or(Error::QueueCorrupted)?;
        let queue = self
            .queues
            .get_mut(usize::from(priority))
            .ok_or(Error::QueueCorrupted)?;
        remove(
            threads,
            queue,
            id,
            QueueMembership::ReadyRealTime { cpu, priority },
        )?;
        self.len = new_len;
        let queue_empty = queue.len == 0;
        if queue_empty {
            self.mark_empty(priority);
        }
        Ok(())
    }

    fn mark_non_empty(&mut self, priority: u8) {
        let priority = usize::from(priority);
        let word = priority / PRIORITIES_PER_BITMAP_WORD;
        let bit = priority % PRIORITIES_PER_BITMAP_WORD;
        self.bitmap[word] |= 1u64 << bit;
    }

    fn mark_empty(&mut self, priority: u8) {
        let priority = usize::from(priority);
        let word = priority / PRIORITIES_PER_BITMAP_WORD;
        let bit = priority % PRIORITIES_PER_BITMAP_WORD;
        self.bitmap[word] &= !(1u64 << bit);
    }

    fn highest_ready_priority(&self) -> Option<usize> {
        self.bitmap.iter().position(|word| *word != 0).map(|word| {
            word * PRIORITIES_PER_BITMAP_WORD + self.bitmap[word].trailing_zeros() as usize
        })
    }
}

fn push_front(
    threads: &mut impl ThreadQueueAuthority,
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    let new_len = preflight_insert(threads, queue, id, membership)?;
    let old_head = queue.head;
    if let Some(head) = queue.head {
        let mut links = queue_links(threads, head).unwrap_or_else(|_| queue_invariant());
        links.previous = Some(id);
        if !threads.set_queue_links(head, links) {
            queue_invariant();
        }
    } else {
        queue.tail = Some(id);
    }
    commit_insert(
        threads,
        id,
        QueueLinks {
            previous: None,
            next: old_head,
            membership,
        },
    );
    queue.head = Some(id);
    queue.len = new_len;
    Ok(())
}

fn push(
    threads: &mut impl ThreadQueueAuthority,
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    let new_len = preflight_insert(threads, queue, id, membership)?;
    let old_tail = queue.tail;
    if let Some(tail) = queue.tail {
        let mut links = queue_links(threads, tail).unwrap_or_else(|_| queue_invariant());
        links.next = Some(id);
        if !threads.set_queue_links(tail, links) {
            queue_invariant();
        }
    } else {
        queue.head = Some(id);
    }
    commit_insert(
        threads,
        id,
        QueueLinks {
            previous: old_tail,
            next: None,
            membership,
        },
    );
    queue.tail = Some(id);
    queue.len = new_len;
    Ok(())
}

fn pop(
    threads: &mut impl ThreadQueueAuthority,
    queue: &mut ThreadQueue,
    membership: QueueMembership,
) -> Result<Option<ThreadId>, Error> {
    let Some(id) = queue.head else {
        return Ok(None);
    };
    remove(threads, queue, id, membership)?;
    Ok(Some(id))
}

fn remove(
    threads: &mut impl ThreadQueueAuthority,
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    let links = queue_links(threads, id)?;
    if links.membership != membership || queue.len == 0 {
        return Err(Error::QueueCorrupted);
    }
    let new_len = queue.len.checked_sub(1).ok_or(Error::QueueCorrupted)?;
    validate_neighbors(threads, queue, links, id)?;
    preflight_residence(threads, id, membership)?;

    // Commit contains no fallible operation. Every involved identity and
    // reciprocal link was validated above while the transition lock retains
    // the sole table authority.
    update_previous_link(threads, queue, links);
    update_next_link(threads, queue, links);
    queue.len = new_len;
    commit_remove(threads, id, membership);
    Ok(())
}

fn update_previous_link(
    threads: &mut impl ThreadQueueAuthority,
    queue: &mut ThreadQueue,
    links: QueueLinks,
) {
    match links.previous {
        Some(previous) => {
            let mut previous_links =
                queue_links(threads, previous).unwrap_or_else(|_| queue_invariant());
            previous_links.next = links.next;
            if !threads.set_queue_links(previous, previous_links) {
                queue_invariant();
            }
        }
        None => queue.head = links.next,
    }
}

fn update_next_link(
    threads: &mut impl ThreadQueueAuthority,
    queue: &mut ThreadQueue,
    links: QueueLinks,
) {
    match links.next {
        Some(next) => {
            let mut next_links = queue_links(threads, next).unwrap_or_else(|_| queue_invariant());
            next_links.previous = links.previous;
            if !threads.set_queue_links(next, next_links) {
                queue_invariant();
            }
        }
        None => queue.tail = links.previous,
    }
}

fn validate_neighbors(
    threads: &impl ThreadReadAuthority,
    queue: &ThreadQueue,
    links: QueueLinks,
    id: ThreadId,
) -> Result<(), Error> {
    let previous_valid = match links.previous {
        Some(previous) => queue_links(threads, previous)?.next == Some(id),
        None => queue.head == Some(id),
    };
    let next_valid = match links.next {
        Some(next) => queue_links(threads, next)?.previous == Some(id),
        None => queue.tail == Some(id),
    };
    if previous_valid && next_valid {
        Ok(())
    } else {
        Err(Error::QueueCorrupted)
    }
}

fn preflight_insert(
    threads: &impl ThreadQueueAuthority,
    queue: &ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<usize, Error> {
    let (links, assigned_cpu) = threads.insertion_state(id, membership)?;
    if links.membership != QueueMembership::None {
        return Err(Error::ThreadAlreadyQueued);
    }
    if let Some(cpu) = ready_cpu(membership)
        && assigned_cpu != Some(cpu)
    {
        return Err(Error::InvalidThreadState);
    }
    let new_len = queue.len.checked_add(1).ok_or(Error::QueueCorrupted)?;
    match (queue.head, queue.tail, queue.len) {
        (None, None, 0) => Ok(new_len),
        (Some(head), Some(tail), len) if len != 0 => {
            let head_links = queue_links(threads, head)?;
            let tail_links = queue_links(threads, tail)?;
            if head_links.membership == membership
                && head_links.previous.is_none()
                && tail_links.membership == membership
                && tail_links.next.is_none()
            {
                Ok(new_len)
            } else {
                Err(Error::QueueCorrupted)
            }
        }
        _ => Err(Error::QueueCorrupted),
    }
}

fn preflight_residence(
    threads: &impl ThreadQueueAuthority,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    let valid = threads.residence_matches(id, ready_cpu(membership))?;
    if valid {
        Ok(())
    } else {
        Err(Error::QueueCorrupted)
    }
}

fn commit_insert(threads: &mut impl ThreadQueueAuthority, id: ThreadId, links: QueueLinks) {
    if !threads.commit_insert(id, links) {
        queue_invariant();
    }
}

fn commit_remove(
    threads: &mut impl ThreadQueueAuthority,
    id: ThreadId,
    membership: QueueMembership,
) {
    if !threads.commit_remove(id, membership) {
        queue_invariant();
    }
}

const fn ready_cpu(membership: QueueMembership) -> Option<CpuIndex> {
    match membership {
        QueueMembership::ReadyRealTime { cpu, .. } | QueueMembership::ReadyFair { cpu } => {
            Some(cpu)
        }
        QueueMembership::None | QueueMembership::Waiting { .. } | QueueMembership::Terminated => {
            None
        }
    }
}

fn queue_links(threads: &impl ThreadReadAuthority, id: ThreadId) -> Result<QueueLinks, Error> {
    threads.queue_links(id)
}

pub(super) trait ThreadReadAuthority {
    fn queue_links(&self, id: ThreadId) -> Result<QueueLinks, Error>;
}

pub(super) trait ThreadQueueAuthority: ThreadReadAuthority {
    fn insertion_state(
        &self,
        id: ThreadId,
        membership: QueueMembership,
    ) -> Result<(QueueLinks, Option<CpuIndex>), Error>;
    fn residence_matches(&self, id: ThreadId, cpu: Option<CpuIndex>) -> Result<bool, Error>;
    fn set_queue_links(&mut self, id: ThreadId, links: QueueLinks) -> bool;
    fn commit_insert(&mut self, id: ThreadId, links: QueueLinks) -> bool;
    fn commit_remove(&mut self, id: ThreadId, membership: QueueMembership) -> bool;
}

pub(super) trait ReadyQueueAccess: ThreadQueueAuthority {
    fn scheduling_policy(&self, id: ThreadId) -> Result<SchedulingPolicy, Error>;
}

impl ThreadReadAuthority for ControlQueueAuthority<'_> {
    fn queue_links(&self, id: ThreadId) -> Result<QueueLinks, Error> {
        self.control.links(id)
    }
}

impl ThreadQueueAuthority for ControlQueueAuthority<'_> {
    fn insertion_state(
        &self,
        id: ThreadId,
        membership: QueueMembership,
    ) -> Result<(QueueLinks, Option<CpuIndex>), Error> {
        if !matches!(
            membership,
            QueueMembership::Waiting { .. } | QueueMembership::Terminated
        ) {
            return Err(Error::InvalidThreadState);
        }
        self.control.links(id).map(|links| (links, None))
    }

    fn residence_matches(&self, id: ThreadId, cpu: Option<CpuIndex>) -> Result<bool, Error> {
        if cpu.is_some() {
            return Ok(false);
        }
        self.control.links(id).map(|_links| true)
    }

    fn set_queue_links(&mut self, id: ThreadId, links: QueueLinks) -> bool {
        self.control
            .with_links_mut(id, |stored| *stored = links)
            .is_ok()
    }

    fn commit_insert(&mut self, id: ThreadId, links: QueueLinks) -> bool {
        self.control
            .with_links_mut(id, |stored| {
                if ready_cpu(links.membership).is_some()
                    || stored.membership != QueueMembership::None
                {
                    return false;
                }
                *stored = links;
                true
            })
            .unwrap_or(false)
    }

    fn commit_remove(&mut self, id: ThreadId, membership: QueueMembership) -> bool {
        self.control
            .with_links_mut(id, |stored| {
                if ready_cpu(membership).is_some() || stored.membership != membership {
                    return false;
                }
                *stored = QueueLinks::EMPTY;
                true
            })
            .unwrap_or(false)
    }
}

pub(super) fn control_push(
    threads: &mut ControlQueueAuthority<'_>,
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    if ready_cpu(membership).is_some() || membership == QueueMembership::None {
        return Err(Error::InvalidThreadState);
    }
    push(threads, queue, id, membership)
}

pub(super) fn control_pop(
    threads: &mut ControlQueueAuthority<'_>,
    queue: &mut ThreadQueue,
    membership: QueueMembership,
) -> Result<Option<ThreadId>, Error> {
    if ready_cpu(membership).is_some() || membership == QueueMembership::None {
        return Err(Error::InvalidThreadState);
    }
    pop(threads, queue, membership)
}

pub(super) fn control_remove(
    threads: &mut ControlQueueAuthority<'_>,
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    if ready_cpu(membership).is_some() || membership == QueueMembership::None {
        return Err(Error::InvalidThreadState);
    }
    remove(threads, queue, id, membership)
}

impl ThreadReadAuthority for ReadyQueueAuthority<'_, '_> {
    fn queue_links(&self, id: ThreadId) -> Result<QueueLinks, Error> {
        let owner = self
            .coordinator
            .with_thread(id, |thread| thread.schedule_owner_cpu())?;
        match owner {
            None => self.coordinator.with_thread(id, |thread| {
                thread
                    .table_owned_ready_queue_links()
                    .unwrap_or_else(|| queue_invariant())
            }),
            Some(cpu) if cpu == self.cpu.cpu() => self
                .cpu
                .with_thread(id, |_thread, schedule| schedule.ready_queue_links),
            Some(_) => Err(Error::InvalidThreadState),
        }
    }
}

impl ThreadQueueAuthority for ReadyQueueAuthority<'_, '_> {
    fn insertion_state(
        &self,
        id: ThreadId,
        membership: QueueMembership,
    ) -> Result<(QueueLinks, Option<CpuIndex>), Error> {
        let target = ready_cpu(membership).ok_or(Error::InvalidThreadState)?;
        if self.coordinator.control_links(id)?.membership != QueueMembership::None {
            return Err(Error::ThreadAlreadyQueued);
        }
        let owner = self
            .coordinator
            .with_thread(id, |thread| thread.schedule_owner_cpu())?;
        match owner {
            None => self.coordinator.with_thread(id, |thread| {
                (
                    thread
                        .table_owned_ready_queue_links()
                        .unwrap_or_else(|| queue_invariant()),
                    Some(thread.cpu_index()),
                )
            }),
            Some(cpu) if cpu == self.cpu.cpu() && target == cpu => {
                self.cpu.with_thread(id, |_thread, schedule| {
                    (
                        schedule.ready_queue_links,
                        Some(schedule.placement.assigned_cpu()),
                    )
                })
            }
            Some(_) => Err(Error::InvalidThreadState),
        }
    }

    fn residence_matches(&self, id: ThreadId, cpu: Option<CpuIndex>) -> Result<bool, Error> {
        let Some(expected) = cpu else {
            return Ok(false);
        };
        self.coordinator
            .with_thread(id, |thread| thread.schedule_owner_cpu() == Some(expected))
    }

    fn set_queue_links(&mut self, id: ThreadId, links: QueueLinks) -> bool {
        if ready_cpu(links.membership).is_none() && links != QueueLinks::EMPTY {
            return false;
        }
        self.with_schedule_mut(id, |schedule| schedule.ready_queue_links = links)
            .is_ok()
    }

    fn commit_insert(&mut self, id: ThreadId, links: QueueLinks) -> bool {
        let Some(target_cpu) = ready_cpu(links.membership) else {
            return false;
        };
        let owner = self
            .coordinator
            .with_thread(id, |thread| thread.schedule_owner_cpu());
        match owner {
            Ok(None) => self
                .coordinator
                .with_thread_mut(id, |thread| {
                    thread.publish_ready_ownership(target_cpu, links)
                })
                .unwrap_or(false),
            Ok(Some(owner)) if owner == self.cpu.cpu() && owner == target_cpu => self
                .with_schedule_mut(id, |schedule| {
                    if schedule.ready_queue_links.membership != QueueMembership::None {
                        return false;
                    }
                    schedule.ready_queue_links = links;
                    schedule.state = crate::kernel::task::thread::ThreadState::Ready;
                    true
                })
                .unwrap_or(false),
            Ok(Some(_)) | Err(_) => false,
        }
    }

    fn commit_remove(&mut self, id: ThreadId, membership: QueueMembership) -> bool {
        if ready_cpu(membership) != Some(self.cpu.cpu()) {
            return false;
        }
        self.with_schedule_mut(id, |schedule| {
            if schedule.ready_queue_links.membership != membership {
                false
            } else {
                schedule.ready_queue_links = QueueLinks::EMPTY;
                true
            }
        })
        .unwrap_or(false)
    }
}

impl ReadyQueueAccess for ReadyQueueAuthority<'_, '_> {
    fn scheduling_policy(&self, id: ThreadId) -> Result<SchedulingPolicy, Error> {
        ReadyQueueAuthority::scheduling_policy(self, id)
    }
}

impl ThreadReadAuthority for LocalReadyQueueAuthority<'_> {
    fn queue_links(&self, id: ThreadId) -> Result<QueueLinks, Error> {
        self.cpu
            .with_thread(id, |_thread, schedule| schedule.ready_queue_links)
    }
}

impl ThreadQueueAuthority for LocalReadyQueueAuthority<'_> {
    fn insertion_state(
        &self,
        id: ThreadId,
        membership: QueueMembership,
    ) -> Result<(QueueLinks, Option<CpuIndex>), Error> {
        let target = ready_cpu(membership).ok_or(Error::InvalidThreadState)?;
        if target != self.cpu.cpu() {
            return Err(Error::InvalidThreadState);
        }
        self.cpu.with_thread(id, |_thread, schedule| {
            (
                schedule.ready_queue_links,
                Some(schedule.placement.assigned_cpu()),
            )
        })
    }

    fn residence_matches(&self, id: ThreadId, cpu: Option<CpuIndex>) -> Result<bool, Error> {
        if cpu != Some(self.cpu.cpu()) {
            return Ok(false);
        }
        self.cpu.with_thread(id, |_thread, _schedule| true)
    }

    fn set_queue_links(&mut self, id: ThreadId, links: QueueLinks) -> bool {
        if ready_cpu(links.membership).is_none() && links != QueueLinks::EMPTY {
            return false;
        }
        self.cpu
            .with_thread_mut(id, |_thread, schedule| schedule.ready_queue_links = links)
            .is_ok()
    }

    fn commit_insert(&mut self, id: ThreadId, links: QueueLinks) -> bool {
        if ready_cpu(links.membership) != Some(self.cpu.cpu()) {
            return false;
        }
        self.cpu
            .with_thread_mut(id, |_thread, schedule| {
                if schedule.ready_queue_links.membership != QueueMembership::None {
                    return false;
                }
                schedule.ready_queue_links = links;
                schedule.state = crate::kernel::task::thread::ThreadState::Ready;
                true
            })
            .unwrap_or(false)
    }

    fn commit_remove(&mut self, id: ThreadId, membership: QueueMembership) -> bool {
        if ready_cpu(membership) != Some(self.cpu.cpu()) {
            return false;
        }
        self.cpu
            .with_thread_mut(id, |_thread, schedule| {
                if schedule.ready_queue_links.membership != membership {
                    return false;
                }
                schedule.ready_queue_links = QueueLinks::EMPTY;
                true
            })
            .unwrap_or(false)
    }
}

impl ReadyQueueAccess for LocalReadyQueueAuthority<'_> {
    fn scheduling_policy(&self, id: ThreadId) -> Result<SchedulingPolicy, Error> {
        self.cpu
            .with_thread(id, |_thread, schedule| schedule.scheduling)
    }
}

fn queue_invariant() -> ! {
    crate::hal::cpu::halt()
}
