// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-private atomic waits, keyed by non-reused mapping identity.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicU32, Ordering};
use hyper::sync::InterruptSpinLock;
#[derive(Debug)]
pub(crate) enum Error {
    Process(ProcessError),
    Wait(crate::kernel::object::ObjectWaitError),
    InvalidInput,
}
impl From<ProcessError> for Error {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}
impl From<crate::kernel::object::ObjectWaitError> for Error {
    fn from(error: crate::kernel::object::ObjectWaitError) -> Self {
        Self::Wait(error)
    }
}
use super::{Process, ProcessError};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceKind};
use crate::kernel::mm::user_space::{MappingToken, UserAddress};
use crate::kernel::object::{TimedWaitPreparation, prepare_timed_wait};
use crate::kernel::task::{WaitOutcome, WaitQueue, WaitTicket, scheduler};

type Bucket = InterruptSpinLock<State, crate::hal::irq::LocalMask>;
struct State {
    queue: WaitQueue,
    waiters: Option<Box<Waiter>>,
}
struct Waiter {
    key: MappingToken,
    address: u64,
    ticket: WaitTicket,
    next: Option<Box<Waiter>>,
    _charge: CommittedCharge,
}
const BUCKET_COUNT: usize = 64;
static BUCKETS: [Bucket; BUCKET_COUNT] = [const {
    Bucket::new(State {
        queue: WaitQueue::new(),
        waiters: None,
    })
}; BUCKET_COUNT];
fn bucket(address: u64) -> &'static Bucket {
    &BUCKETS[(address as usize >> 2) % BUCKET_COUNT]
}

pub(crate) fn wait(
    process: &Process,
    address: u64,
    expected: u32,
    deadline: u64,
    cancelled: impl Fn() -> bool,
) -> Result<WaitOutcome, Error> {
    if !address.is_multiple_of(4) {
        return Err(Error::InvalidInput);
    }
    let word =
        process.retry_user_memory(|space| space.pin_atomic_u32(UserAddress::new(address)))?;
    let linear =
        crate::kernel::mm::memory::linear_address(word.physical).ok_or(Error::InvalidInput)?;
    // SAFETY: aligned u32 is wholly within a resident writable page. `word`
    // retains its mapping lease and page until all accesses finish, even if
    // userspace unmaps it. All accesses here are atomic, never user copies.
    let atomic = unsafe { &*core::ptr::with_exposed_provenance::<AtomicU32>(linear) };
    if atomic.load(Ordering::Relaxed) != expected {
        return Ok(WaitOutcome::Notified);
    }
    let domain = process.resource_domain();
    let charge = domain
        .reserve(ResourceAmount::ZERO.with(
            ResourceKind::KernelMemoryBytes,
            core::mem::size_of::<Waiter>() as u64,
        ))
        .map_err(ProcessError::from)?
        .commit();
    // Allocate before arming a scheduler generation; no fallible allocation is
    // allowed once a PreparedTimedWait must be explicitly resolved.
    let mut storage = hyper::mm::try_box(core::mem::MaybeUninit::<Waiter>::uninit())
        .map_err(|_| ProcessError::Allocation)?;
    let prepared = match prepare_timed_wait(&domain, deadline)? {
        TimedWaitPreparation::Completed(outcome) => return Ok(outcome),
        TimedWaitPreparation::Armed(prepared) => prepared,
    };
    let ticket = prepared.ticket();
    storage.write(Waiter {
        key: word.token,
        address,
        ticket,
        next: None,
        _charge: charge,
    });
    // SAFETY: the entire Waiter was initialized immediately above.
    let mut waiter = unsafe { storage.assume_init() };
    let bucket = bucket(address);
    enum Publication {
        Mismatch(crate::kernel::object::PreparedTimedWait),
        Waiting(crate::kernel::object::PublishedTimedWait),
    }
    // SAFETY: condition check, registration and wake all use this bucket lock.
    // The retained IRQ mask goes directly to finish or is dropped on mismatch.
    let (published, mask) = unsafe {
        bucket.with_mask_retained(|state| {
            if atomic.load(Ordering::Relaxed) != expected {
                return Publication::Mismatch(prepared);
            }
            if cancelled() {
                prepared.request_cancellation();
            }
            waiter.next = state.waiters.take();
            state.waiters = Some(waiter);
            Publication::Waiting(prepared.publish_locked(&state.queue))
        })
    };
    match published {
        Publication::Mismatch(prepared) => {
            drop(mask);
            prepared.abort()?;
            Ok(WaitOutcome::Notified)
        }
        Publication::Waiting(published) => {
            let (outcome, retirement) = published.finish(mask);
            let removed = bucket.with(|state| unlink(state, ticket));
            drop(removed);
            retirement?;
            Ok(outcome)
        }
    }
}

pub(crate) fn wake(process: &Process, address: u64, count: u32) -> Result<u64, Error> {
    if !address.is_multiple_of(4) {
        return Err(Error::InvalidInput);
    }
    let word =
        process.retry_user_memory(|space| space.pin_atomic_u32(UserAddress::new(address)))?;
    bucket(address).with(|state| {
        let mut woke = 0;
        let mut current = state.waiters.as_deref();
        while let Some(waiter) = current {
            if woke == u64::from(count) {
                break;
            }
            if waiter.key == word.token
                && waiter.address == address
                && scheduler::notify_registered_with(waiter.ticket, || {})
                    .map_err(ProcessError::from)?
                    .won
            {
                woke += 1;
            }
            current = waiter.next.as_deref();
        }
        Ok(woke)
    })
}

fn unlink(state: &mut State, ticket: WaitTicket) -> Box<Waiter> {
    let mut link = &mut state.waiters;
    loop {
        if link.as_ref().is_some_and(|node| node.ticket == ticket) {
            let Some(mut node) = link.take() else {
                invariant()
            };
            *link = node.next.take();
            return node;
        }
        link = match link.as_mut() {
            Some(node) => &mut node.next,
            None => invariant(),
        };
    }
}
fn invariant() -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: atomic wait lost registration"))
}

pub(crate) fn sleep(
    process: &Process,
    deadline: u64,
    cancelled: impl Fn() -> bool,
) -> Result<WaitOutcome, Error> {
    let prepared = match prepare_timed_wait(&process.resource_domain(), deadline)? {
        TimedWaitPreparation::Completed(outcome) => return Ok(outcome),
        TimedWaitPreparation::Armed(prepared) => prepared,
    };
    if cancelled() {
        prepared.request_cancellation();
    }
    // SAFETY: the bucket is permanent; its queue is only used for exact-ticket
    // resolution. No ordinary wake operation targets sleepers on this queue.
    let (published, mask) =
        unsafe { BUCKETS[0].with_mask_retained(|state| prepared.publish_locked(&state.queue)) };
    let (outcome, retirement) = published.finish(mask);
    retirement?;
    Ok(outcome)
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn waiter_count(process: &Process, address: u64) -> Result<usize, Error> {
    let word =
        process.retry_user_memory(|space| space.pin_atomic_u32(UserAddress::new(address)))?;
    Ok(bucket(address).with(|state| {
        let mut count = 0;
        let mut current = state.waiters.as_deref();
        while let Some(waiter) = current {
            if waiter.key == word.token && waiter.address == address {
                count += 1;
            }
            current = waiter.next.as_deref();
        }
        count
    }))
}
