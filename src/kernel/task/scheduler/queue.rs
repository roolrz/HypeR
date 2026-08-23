// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Intrusive primitives and class-aware per-CPU ready queues.
//!
//! Class ordering is resolved here, while each class owns its queue policy.
//! The scheduler state machine never compares parameters from unrelated
//! scheduling classes.

use alloc::boxed::Box;
use hyper::cpu::CpuIndex;

use super::Error;
use crate::kernel::task::policy::{PRIORITY_LEVELS, SchedulingPolicy, ThreadPriority};
use crate::kernel::task::thread::{QueueLinks, QueueMembership, Thread, ThreadId, ThreadState};
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

    /// Validates and returns the next runnable thread in class order.
    pub fn peek_next(
        &self,
        threads: &[Option<Box<Thread>>],
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
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        let thread = thread_ref(threads, id)?;
        match thread.scheduling_policy() {
            SchedulingPolicy::Fifo { priority } => {
                self.realtime.enqueue(threads, id, cpu, priority.get())
            }
            SchedulingPolicy::Fair => self.fair.enqueue(threads, id, cpu),
            SchedulingPolicy::Idle => Err(Error::InvalidThreadState),
        }
    }

    pub fn enqueue_front(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        let thread = thread_ref(threads, id)?;
        match thread.scheduling_policy() {
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
        threads: &mut [Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        if let Some(id) = self.realtime.dequeue(threads, cpu)? {
            return Ok(Some(id));
        }
        self.fair.dequeue(threads, cpu)
    }

    pub fn remove(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
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
        threads: &[Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        let Some(id) = self.queue.head else {
            if self.queue.len == 0 && self.queue.tail.is_none() {
                return Ok(None);
            }
            return Err(Error::QueueCorrupted);
        };
        let links = thread_ref(threads, id)?.queue_links();
        if self.queue.len == 0 || links.membership != (QueueMembership::ReadyFair { cpu }) {
            return Err(Error::QueueCorrupted);
        }
        validate_neighbors(threads, &self.queue, links, id)?;
        Ok(Some(id))
    }

    fn enqueue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        push(
            threads,
            &mut self.queue,
            id,
            QueueMembership::ReadyFair { cpu },
        )?;
        thread_mut(threads, id)?.set_state(ThreadState::Ready);
        Ok(())
    }

    fn enqueue_front(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        push_front(
            threads,
            &mut self.queue,
            id,
            QueueMembership::ReadyFair { cpu },
        )?;
        thread_mut(threads, id)?.set_state(ThreadState::Ready);
        Ok(())
    }

    fn dequeue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        pop(threads, &mut self.queue, QueueMembership::ReadyFair { cpu })
    }

    fn remove(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
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
        threads: &[Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<(ThreadId, ThreadPriority)>, Error> {
        let Some(priority) = self.highest_ready_priority() else {
            return Ok(None);
        };
        let queue = &self.queues[priority];
        let id = queue.head.ok_or(Error::QueueCorrupted)?;
        let links = thread_ref(threads, id)?.queue_links();
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
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        let membership = QueueMembership::ReadyRealTime { cpu, priority };
        push(
            threads,
            &mut self.queues[usize::from(priority)],
            id,
            membership,
        )?;
        self.mark_non_empty(priority);
        self.len += 1;
        thread_mut(threads, id)?.set_state(ThreadState::Ready);
        Ok(())
    }

    fn enqueue_front(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        let membership = QueueMembership::ReadyRealTime { cpu, priority };
        push_front(
            threads,
            &mut self.queues[usize::from(priority)],
            id,
            membership,
        )?;
        self.mark_non_empty(priority);
        self.len += 1;
        thread_mut(threads, id)?.set_state(ThreadState::Ready);
        Ok(())
    }

    pub fn dequeue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        let Some(priority) = self.highest_ready_priority() else {
            return Ok(None);
        };
        let membership = QueueMembership::ReadyRealTime {
            cpu,
            priority: priority as u8,
        };
        let id =
            pop(threads, &mut self.queues[priority], membership)?.ok_or(Error::QueueCorrupted)?;
        self.len -= 1;
        if self.queues[priority].len == 0 {
            self.mark_empty(priority as u8);
        }
        Ok(Some(id))
    }

    pub fn remove(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        let queue = &mut self.queues[usize::from(priority)];
        remove(
            threads,
            queue,
            id,
            QueueMembership::ReadyRealTime { cpu, priority },
        )?;
        self.len -= 1;
        if queue.len == 0 {
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
    threads: &mut [Option<Box<Thread>>],
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    if thread_ref(threads, id)?.queue_links().membership != QueueMembership::None {
        return Err(Error::ThreadAlreadyQueued);
    }
    if let Some(head) = queue.head {
        let mut links = thread_ref(threads, head)?.queue_links();
        links.previous = Some(id);
        thread_mut(threads, head)?.set_queue_links(links);
    } else {
        queue.tail = Some(id);
    }
    thread_mut(threads, id)?.set_queue_links(QueueLinks {
        previous: None,
        next: queue.head,
        membership,
    });
    queue.head = Some(id);
    queue.len += 1;
    Ok(())
}

pub(super) fn push(
    threads: &mut [Option<Box<Thread>>],
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    if thread_ref(threads, id)?.queue_links().membership != QueueMembership::None {
        return Err(Error::ThreadAlreadyQueued);
    }
    if let Some(tail) = queue.tail {
        let mut links = thread_ref(threads, tail)?.queue_links();
        links.next = Some(id);
        thread_mut(threads, tail)?.set_queue_links(links);
    } else {
        queue.head = Some(id);
    }
    thread_mut(threads, id)?.set_queue_links(QueueLinks {
        previous: queue.tail,
        next: None,
        membership,
    });
    queue.tail = Some(id);
    queue.len += 1;
    Ok(())
}

pub(super) fn pop(
    threads: &mut [Option<Box<Thread>>],
    queue: &mut ThreadQueue,
    membership: QueueMembership,
) -> Result<Option<ThreadId>, Error> {
    let Some(id) = queue.head else {
        return Ok(None);
    };
    remove(threads, queue, id, membership)?;
    Ok(Some(id))
}

pub(super) fn remove(
    threads: &mut [Option<Box<Thread>>],
    queue: &mut ThreadQueue,
    id: ThreadId,
    membership: QueueMembership,
) -> Result<(), Error> {
    let links = thread_ref(threads, id)?.queue_links();
    if links.membership != membership || queue.len == 0 {
        return Err(Error::QueueCorrupted);
    }
    validate_neighbors(threads, queue, links, id)?;
    update_previous_link(threads, queue, links)?;
    update_next_link(threads, queue, links)?;
    queue.len -= 1;
    thread_mut(threads, id)?.set_queue_links(QueueLinks::EMPTY);
    Ok(())
}

fn update_previous_link(
    threads: &mut [Option<Box<Thread>>],
    queue: &mut ThreadQueue,
    links: QueueLinks,
) -> Result<(), Error> {
    match links.previous {
        Some(previous) => {
            let mut previous_links = thread_ref(threads, previous)?.queue_links();
            previous_links.next = links.next;
            thread_mut(threads, previous)?.set_queue_links(previous_links);
        }
        None => queue.head = links.next,
    }
    Ok(())
}

fn update_next_link(
    threads: &mut [Option<Box<Thread>>],
    queue: &mut ThreadQueue,
    links: QueueLinks,
) -> Result<(), Error> {
    match links.next {
        Some(next) => {
            let mut next_links = thread_ref(threads, next)?.queue_links();
            next_links.previous = links.previous;
            thread_mut(threads, next)?.set_queue_links(next_links);
        }
        None => queue.tail = links.previous,
    }
    Ok(())
}

fn validate_neighbors(
    threads: &[Option<Box<Thread>>],
    queue: &ThreadQueue,
    links: QueueLinks,
    id: ThreadId,
) -> Result<(), Error> {
    let previous_valid = match links.previous {
        Some(previous) => thread_ref(threads, previous)?.queue_links().next == Some(id),
        None => queue.head == Some(id),
    };
    let next_valid = match links.next {
        Some(next) => thread_ref(threads, next)?.queue_links().previous == Some(id),
        None => queue.tail == Some(id),
    };
    if previous_valid && next_valid {
        Ok(())
    } else {
        Err(Error::QueueCorrupted)
    }
}

pub(super) fn thread_ref(threads: &[Option<Box<Thread>>], id: ThreadId) -> Result<&Thread, Error> {
    let index = usize::try_from(id.get()).map_err(|_| Error::ThreadNotFound)?;
    threads
        .get(index)
        .and_then(Option::as_deref)
        .ok_or(Error::ThreadNotFound)
}

pub(super) fn thread_mut(
    threads: &mut [Option<Box<Thread>>],
    id: ThreadId,
) -> Result<&mut Thread, Error> {
    let index = usize::try_from(id.get()).map_err(|_| Error::ThreadNotFound)?;
    threads
        .get_mut(index)
        .and_then(Option::as_deref_mut)
        .ok_or(Error::ThreadNotFound)
}
