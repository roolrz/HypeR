//! Intrusive FIFO primitives and per-CPU priority ready queues.

use alloc::boxed::Box;

use super::Error;
use crate::kernel::task::thread::{
    QueueLinks, QueueMembership, THREAD_PRIORITY_LEVELS, Thread, ThreadId, ThreadState,
};
use crate::kernel::task::wait::ThreadQueue;

pub(super) struct ReadyQueues {
    queues: [ThreadQueue; THREAD_PRIORITY_LEVELS],
    bitmap: u32,
    len: usize,
}

impl ReadyQueues {
    pub const fn new() -> Self {
        Self {
            queues: [ThreadQueue::new(); THREAD_PRIORITY_LEVELS],
            bitmap: 0,
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn enqueue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: usize,
        priority: u8,
    ) -> Result<(), Error> {
        let membership = QueueMembership::Ready { cpu, priority };
        push(
            threads,
            &mut self.queues[usize::from(priority)],
            id,
            membership,
        )?;
        self.bitmap |= 1u32 << priority;
        self.len += 1;
        thread_mut(threads, id)?.set_state(ThreadState::Ready);
        Ok(())
    }

    pub fn dequeue(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        cpu: usize,
    ) -> Result<Option<ThreadId>, Error> {
        if self.bitmap == 0 {
            return Ok(None);
        }
        let priority = self.bitmap.trailing_zeros() as usize;
        let membership = QueueMembership::Ready {
            cpu,
            priority: priority as u8,
        };
        let id =
            pop(threads, &mut self.queues[priority], membership)?.ok_or(Error::QueueCorrupted)?;
        self.len -= 1;
        if self.queues[priority].len == 0 {
            self.bitmap &= !(1u32 << priority);
        }
        Ok(Some(id))
    }

    pub fn remove(
        &mut self,
        threads: &mut [Option<Box<Thread>>],
        id: ThreadId,
        cpu: usize,
        priority: u8,
    ) -> Result<(), Error> {
        let queue = &mut self.queues[usize::from(priority)];
        remove(threads, queue, id, QueueMembership::Ready { cpu, priority })?;
        self.len -= 1;
        if queue.len == 0 {
            self.bitmap &= !(1u32 << priority);
        }
        Ok(())
    }
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
