//! Real-context scheduler, wait-queue, Mutex, and Semaphore tests.

use hyper::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::sync::{Mutex, Semaphore};
use crate::kernel::task::WaitQueue;
use crate::kernel::task::scheduler;
use crate::kernel::task::thread::ThreadPriority;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Scheduler(scheduler::Error),
    Synchronization(crate::kernel::sync::Error),
    Worker(usize),
    StateMismatch,
}

impl From<scheduler::Error> for Error {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<crate::kernel::sync::Error> for Error {
    fn from(error: crate::kernel::sync::Error) -> Self {
        Self::Synchronization(error)
    }
}

struct SyncState {
    value: Mutex<usize>,
    gate: Semaphore,
    done: Semaphore,
    next_order: AtomicUsize,
    consumer_order: [AtomicUsize; 2],
    worker_error: AtomicUsize,
}

impl SyncState {
    const fn new() -> Self {
        Self {
            value: Mutex::new(0),
            gate: Semaphore::new(0),
            done: Semaphore::new(0),
            next_order: AtomicUsize::new(1),
            consumer_order: [AtomicUsize::new(0), AtomicUsize::new(0)],
            worker_error: AtomicUsize::new(0),
        }
    }

    fn fail(&self, code: usize) {
        let _ = self
            .worker_error
            .compare_exchange(0, code, Ordering::AcqRel, Ordering::Acquire);
    }
}

struct ConsumerArgument {
    state: *const SyncState,
    index: usize,
}

struct WaitState {
    queue: WaitQueue,
    done: Semaphore,
    order: AtomicUsize,
    observed: [AtomicUsize; 2],
    worker_error: AtomicUsize,
}

impl WaitState {
    const fn new() -> Self {
        Self {
            queue: WaitQueue::new(),
            done: Semaphore::new(0),
            order: AtomicUsize::new(1),
            observed: [AtomicUsize::new(0), AtomicUsize::new(0)],
            worker_error: AtomicUsize::new(0),
        }
    }
}

struct WaitArgument {
    state: *const WaitState,
    index: usize,
}

pub(super) fn run() -> Result<(), Error> {
    exercise_nonblocking_paths()?;
    exercise_mutex_and_semaphore_handoff()?;
    exercise_wait_queue_wake_all()?;
    let stats = scheduler::statistics()?;
    if stats.ready != 0 || stats.blocked != 0 {
        return Err(Error::StateMismatch);
    }
    Ok(())
}

fn exercise_nonblocking_paths() -> Result<(), Error> {
    let semaphore = Semaphore::new(2);
    if semaphore.available_permits() != 2
        || !semaphore.try_acquire()
        || !semaphore.try_acquire()
        || semaphore.try_acquire()
    {
        return Err(Error::StateMismatch);
    }
    semaphore.release()?;
    if semaphore.available_permits() != 1 {
        return Err(Error::StateMismatch);
    }
    let mutex = Mutex::new(7usize);
    let guard = mutex.try_lock()?.ok_or(Error::StateMismatch)?;
    if *guard != 7 || mutex.waiter_count()? != 0 {
        return Err(Error::StateMismatch);
    }
    if !matches!(
        mutex.try_lock(),
        Err(crate::kernel::sync::Error::WouldDeadlock)
    ) {
        return Err(Error::StateMismatch);
    }
    drop(guard);
    Ok(())
}

fn exercise_mutex_and_semaphore_handoff() -> Result<(), Error> {
    let state = SyncState::new();
    let arguments = [
        ConsumerArgument {
            state: &state,
            index: 0,
        },
        ConsumerArgument {
            state: &state,
            index: 1,
        },
    ];
    let producer = scheduler::kthread_create("sync-producer", producer, (&state as *const _) as _)?;
    let first = scheduler::kthread_create(
        "sync-consumer/0",
        consumer,
        (&arguments[0] as *const _) as _,
    )?;
    let second = scheduler::kthread_create(
        "sync-consumer/1",
        consumer,
        (&arguments[1] as *const _) as _,
    )?;
    scheduler::thread_ready(producer)?;
    if !scheduler::thread_ready(first)? || scheduler::thread_ready(first)? {
        return Err(Error::StateMismatch);
    }
    scheduler::thread_ready(second)?;
    scheduler::set_thread_priority(first, ThreadPriority::HIGHEST)?;
    scheduler::set_thread_priority(second, ThreadPriority::HIGHEST)?;
    scheduler::yield_now()?;
    for _ in 0..3 {
        state.done.acquire()?;
    }
    scheduler::yield_now()?;
    verify_sync_state(&state)
}

fn verify_sync_state(state: &SyncState) -> Result<(), Error> {
    let error = state.worker_error.load(Ordering::Acquire);
    let value = *state.value.lock()?;
    let order = [
        state.consumer_order[0].load(Ordering::Acquire),
        state.consumer_order[1].load(Ordering::Acquire),
    ];
    if error == 0 && value == 3 && order == [1, 2] {
        Ok(())
    } else if error != 0 {
        Err(Error::Worker(error))
    } else {
        Err(Error::StateMismatch)
    }
}

extern "C" fn producer(argument: usize) {
    // SAFETY: The parent waits for all three completion handoffs.
    let state = unsafe { &*(argument as *const SyncState) };
    let Ok(mut value) = state.value.lock() else {
        state.fail(1);
        let _ = state.done.release();
        return;
    };
    *value = 1;
    if !matches!(
        state.value.try_lock(),
        Err(crate::kernel::sync::Error::WouldDeadlock)
    ) {
        state.fail(2);
    }
    if state.gate.release().is_err() || state.gate.release().is_err() {
        state.fail(3);
    }
    if scheduler::yield_now().is_err() || state.value.waiter_count() != Ok(2) {
        state.fail(4);
    }
    drop(value);
    if state.done.release().is_err() {
        state.fail(5);
    }
}

extern "C" fn consumer(argument: usize) {
    // SAFETY: The parent retains both argument records until completion.
    let argument = unsafe { &*(argument as *const ConsumerArgument) };
    let state = unsafe { &*argument.state };
    if state.gate.acquire().is_err() {
        state.fail(6 + argument.index);
    } else {
        if !matches!(state.value.try_lock(), Ok(None)) {
            state.fail(8 + argument.index);
        }
        match state.value.lock() {
            Ok(mut value) => {
                *value += 1;
                let order = state.next_order.fetch_add(1, Ordering::AcqRel);
                state.consumer_order[argument.index].store(order, Ordering::Release);
            }
            Err(_) => state.fail(10 + argument.index),
        }
    }
    if state.done.release().is_err() {
        state.fail(12 + argument.index);
    }
}

fn exercise_wait_queue_wake_all() -> Result<(), Error> {
    let state = WaitState::new();
    let arguments = [
        WaitArgument {
            state: &state,
            index: 0,
        },
        WaitArgument {
            state: &state,
            index: 1,
        },
    ];
    for (index, argument) in arguments.iter().enumerate() {
        let id = scheduler::kthread_create(
            if index == 0 { "waiter/0" } else { "waiter/1" },
            waiter,
            (argument as *const WaitArgument) as usize,
        )?;
        scheduler::thread_ready(id)?;
    }
    scheduler::yield_now()?;
    if state.queue.len()? != 2 || state.queue.wake_all()? != 2 {
        return Err(Error::StateMismatch);
    }
    scheduler::yield_now()?;
    state.done.acquire()?;
    state.done.acquire()?;
    scheduler::yield_now()?;
    let observed = [
        state.observed[0].load(Ordering::Acquire),
        state.observed[1].load(Ordering::Acquire),
    ];
    if observed != [1, 2] || state.worker_error.load(Ordering::Acquire) != 0 {
        return Err(Error::StateMismatch);
    }
    Ok(())
}

extern "C" fn waiter(argument: usize) {
    // SAFETY: The parent waits for both waiter completions.
    let argument = unsafe { &*(argument as *const WaitArgument) };
    let state = unsafe { &*argument.state };
    if state.queue.wait().is_err() {
        state.worker_error.store(1, Ordering::Release);
    } else {
        let order = state.order.fetch_add(1, Ordering::AcqRel);
        state.observed[argument.index].store(order, Ordering::Release);
    }
    if state.done.release().is_err() {
        state.worker_error.store(2, Ordering::Release);
    }
}
