// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Intrusive FIFO primitives and per-CPU priority ready queues.

use alloc::boxed::Box;
use hyper::cpu::CpuIndex;

use super::Error;
use crate::kernel::task::policy::{PRIORITY_LEVELS, SchedulingClass, ThreadPriority};
use crate::kernel::task::thread::{QueueLinks, QueueMembership, Thread, ThreadId, ThreadState};
use crate::kernel::task::wait::ThreadQueue;

const PRIORITIES_PER_BITMAP_WORD: usize = u64::BITS as usize;
const PRIORITY_BITMAP_WORDS: usize = PRIORITY_LEVELS.div_ceil(PRIORITIES_PER_BITMAP_WORD);

/// Runnable classes owned by one CPU.
///
/// Idle is intentionally absent: its Thread is selected only when every
/// runnable class is empty.
pub(super) struct CpuRunQueue {
    fixed_priority: FixedPriorityFifo,
}

impl CpuRunQueue {
    pub const fn new() -> Self {
        Self {
            fixed_priority: FixedPriorityFifo::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.fixed_priority.len()
    }

    pub fn highest_priority(&self) -> Option<ThreadPriority> {
        self.fixed_priority
            .highest_ready_priority()
            .map(|priority| ThreadPriority::new(priority as u8))
    }

    /// Validates and returns the next runnable thread without mutating queues.
    pub fn peek_highest(
        &self,
        threads: &[Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<(ThreadId, ThreadPriority)>, Error> {
        self.fixed_priority.peek_highest(threads, cpu)
    }

    pub fn enqueue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        let thread = thread_ref(threads, id)?;
        let Some(priority) = thread.priority() else {
            return Err(Error::InvalidThreadState);
        };
        if thread.scheduling_class() != SchedulingClass::FixedPriority {
            return Err(Error::InvalidThreadState);
        }
        self.fixed_priority
            .enqueue(threads, id, cpu, priority.get())
    }

    pub fn enqueue_front(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
    ) -> Result<(), Error> {
        let thread = thread_ref(threads, id)?;
        let Some(priority) = thread.priority() else {
            return Err(Error::InvalidThreadState);
        };
        if thread.scheduling_class() != SchedulingClass::FixedPriority {
            return Err(Error::InvalidThreadState);
        }
        self.fixed_priority
            .enqueue_front(threads, id, cpu, priority.get())
    }

    pub fn dequeue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        cpu: CpuIndex,
    ) -> Result<Option<ThreadId>, Error> {
        self.fixed_priority.dequeue(threads, cpu)
    }

    pub fn remove(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: CpuIndex,
        priority: u8,
    ) -> Result<(), Error> {
        self.fixed_priority.remove(threads, id, cpu, priority)
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
        let membership = QueueMembership::Ready {
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
        let membership = QueueMembership::Ready { cpu, priority };
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
        let membership = QueueMembership::Ready { cpu, priority };
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
        let membership = QueueMembership::Ready {
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
        remove(threads, queue, id, QueueMembership::Ready { cpu, priority })?;
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
