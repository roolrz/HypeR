// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Real-context scheduler, wait-queue, Mutex, and Semaphore tests.

use hyper::cpu::CpuIndex;
use hyper::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::sync::{Mutex, Semaphore};
use crate::kernel::task::WaitQueue;
use crate::kernel::task::scheduler::{self, CpuMask, ThreadPriority};

static FIFO_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static FIFO_FAILURE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Affinity(usize),
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
    exercise_affinity_creation()?;
    exercise_fair_rotation()?;
    exercise_policy_transitions()?;
    exercise_fifo_preemption_points()?;
    exercise_mutex_and_semaphore_handoff()?;
    exercise_wait_queue_wake_all()?;
    let stats = scheduler::statistics()?;
    if stats.ready != 0
        || stats.blocked != 0
        || stats.real_time_class_threads + stats.fair_class_threads + stats.idle_class_threads
            != stats.threads
        || stats.idle_class_threads != stats.idle
    {
        return Err(Error::StateMismatch);
    }
    Ok(())
}

fn exercise_policy_transitions() -> Result<(), Error> {
    FIFO_SEQUENCE.store(0, Ordering::Release);

    let dormant =
        scheduler::kthread_create_fifo("policy-dormant", fifo_peer, 1, ThreadPriority::HIGHEST)?;
    scheduler::set_thread_fair_policy(dormant)?;
    scheduler::thread_ready(dormant)?;
    if scheduler::cond_resched()? {
        return Err(Error::StateMismatch);
    }
    scheduler::yield_now()?;

    let ready = scheduler::kthread_create("policy-ready", fifo_peer, 2)?;
    scheduler::thread_ready(ready)?;
    scheduler::set_thread_fifo_policy(ready, ThreadPriority::HIGHEST)?;
    scheduler::set_thread_fair_policy(ready)?;
    if scheduler::cond_resched()? {
        return Err(Error::StateMismatch);
    }
    scheduler::yield_now()?;

    let current = scheduler::current_thread_id()?;
    scheduler::set_thread_fifo_policy(current, ThreadPriority::HIGHEST)?;
    let peer =
        scheduler::kthread_create_fifo("policy-running", fifo_peer, 3, ThreadPriority::NORMAL)?;
    scheduler::thread_ready(peer)?;
    scheduler::set_thread_fair_policy(current)?;
    if !scheduler::cond_resched()? {
        return Err(Error::StateMismatch);
    }

    if FIFO_SEQUENCE.load(Ordering::Acquire) != 3 {
        return Err(Error::StateMismatch);
    }
    Ok(())
}

fn exercise_fair_rotation() -> Result<(), Error> {
    FIFO_SEQUENCE.store(0, Ordering::Release);
    let first = scheduler::kthread_create("fair-peer/0", fifo_peer, 1)?;
    let second = scheduler::kthread_create("fair-peer/1", fifo_peer, 2)?;
    scheduler::thread_ready(first)?;
    scheduler::thread_ready(second)?;

    // A Fair wakeup does not immediately displace an equal-class thread. One
    // deliberately oversized charge expires the private backend quantum; the
    // safe point must then rotate both peers in FIFO order.
    if scheduler::cond_resched()? {
        return Err(Error::StateMismatch);
    }
    scheduler::account_tick(u64::MAX)?;
    if !scheduler::cond_resched()? || FIFO_SEQUENCE.load(Ordering::Acquire) != 2 {
        return Err(Error::StateMismatch);
    }
    Ok(())
}

fn exercise_affinity_creation() -> Result<(), Error> {
    let current = crate::kernel::cpu::current_index().ok_or(Error::Affinity(1))?;
    if current != CpuIndex::BOOT {
        return Err(Error::Affinity(2));
    }
    let online_cpus = crate::kernel::cpu::online_cpu_count();
    let remote = if online_cpus > 1 {
        CpuIndex::new(1)
    } else {
        None
    };
    let local_affinity = remote.map_or(CpuMask::single(current), |remote| {
        CpuMask::EMPTY.with_cpu(current).with_cpu(remote)
    });
    let local = scheduler::kthread_create_with_affinity(
        "affinity-local",
        affinity_local_worker,
        0,
        local_affinity,
    )?;
    if scheduler::thread_placement(local)? != (current, local_affinity) {
        return Err(Error::Affinity(4));
    }
    scheduler::thread_ready(local)?;
    scheduler::yield_now()?;

    if let Some(remote) = remote {
        let remote_only = CpuMask::single(remote);
        let remote_thread = scheduler::kthread_create_with_affinity(
            "affinity-remote",
            affinity_local_worker,
            0,
            remote_only,
        )?;
        if scheduler::thread_placement(remote_thread)? != (remote, remote_only) {
            return Err(Error::Affinity(5));
        }
        scheduler::discard_dormant_kernel_thread(remote_thread)?;
    }

    if !matches!(
        scheduler::kthread_create_with_affinity("affinity-empty", fifo_peer, 0, CpuMask::EMPTY),
        Err(scheduler::Error::EmptyCpuAffinity)
    ) {
        return Err(Error::Affinity(7));
    }

    if let Some(unregistered) = CpuIndex::new(online_cpus)
        && !matches!(
            scheduler::kthread_create_with_affinity(
                "affinity-offline",
                fifo_peer,
                0,
                CpuMask::single(unregistered)
            ),
            Err(scheduler::Error::NoRegisteredCpuInAffinity)
        )
    {
        return Err(Error::Affinity(8));
    }
    Ok(())
}

extern "C" fn affinity_local_worker(_argument: usize) {}

fn exercise_fifo_preemption_points() -> Result<(), Error> {
    FIFO_SEQUENCE.store(0, Ordering::Release);

    // The bootstrap Thread is Fair by default. Move it explicitly into RT FIFO
    // so this test isolates equal-priority FIFO head preservation.
    scheduler::set_thread_fifo_policy(scheduler::current_thread_id()?, ThreadPriority::NORMAL)?;

    let guard = scheduler::preempt_disable()?;
    let nested = scheduler::preempt_disable()?;
    if !matches!(
        scheduler::yield_now(),
        Err(scheduler::Error::CannotSleepWithPreemptionDisabled)
    ) || scheduler::preempt_enable_and_reschedule(nested)?
    {
        return Err(Error::StateMismatch);
    }
    if scheduler::preempt_enable_and_reschedule(guard)? {
        return Err(Error::StateMismatch);
    }

    let first =
        scheduler::kthread_create_fifo("fifo-peer/0", fifo_peer, 2, ThreadPriority::NORMAL)?;
    let second =
        scheduler::kthread_create_fifo("fifo-peer/1", fifo_peer, 3, ThreadPriority::NORMAL)?;
    let higher =
        scheduler::kthread_create_fifo("fifo-higher", fifo_peer, 1, ThreadPriority::HIGHEST)?;
    scheduler::thread_ready(first)?;
    scheduler::thread_ready(second)?;
    scheduler::set_thread_fifo_policy(first, ThreadPriority::NORMAL)?;
    scheduler::thread_ready(higher)?;

    if !scheduler::cond_resched()? || FIFO_SEQUENCE.load(Ordering::Acquire) != 1 {
        return Err(Error::StateMismatch);
    }
    scheduler::yield_now()?;
    if FIFO_SEQUENCE.load(Ordering::Acquire) != 3 {
        return Err(Error::StateMismatch);
    }
    scheduler::yield_now()?;

    scheduler::set_thread_fifo_policy(scheduler::current_thread_id()?, ThreadPriority::LOWEST)?;
    exercise_running_priority_changes(0, 3)?;
    exercise_running_priority_changes(1, 2)?;
    exercise_running_priority_changes(2, 3)?;
    scheduler::set_thread_fair_policy(scheduler::current_thread_id()?)?;
    Ok(())
}

fn exercise_running_priority_changes(mode: usize, expected: usize) -> Result<(), Error> {
    FIFO_SEQUENCE.store(0, Ordering::Release);
    FIFO_FAILURE.store(0, Ordering::Release);
    let worker = scheduler::kthread_create_fifo(
        "fifo-priority-change",
        fifo_priority_change,
        mode,
        ThreadPriority::NORMAL,
    )?;
    scheduler::thread_ready(worker)?;
    scheduler::yield_now()?;
    if FIFO_SEQUENCE.load(Ordering::Acquire) != expected
        || FIFO_FAILURE.load(Ordering::Acquire) != 0
    {
        return Err(Error::StateMismatch);
    }
    Ok(())
}

extern "C" fn fifo_priority_change(mode: usize) {
    let current = match scheduler::current_thread_id() {
        Ok(id) => id,
        Err(_) => return record_fifo_failure(1),
    };
    let equal_expected = if mode == 2 { 3 } else { 1 };
    let equal = match scheduler::kthread_create_fifo(
        "fifo-priority-peer",
        fifo_peer,
        equal_expected,
        ThreadPriority::new(64),
    ) {
        Ok(id) => id,
        Err(_) => return record_fifo_failure(2),
    };
    if scheduler::thread_ready(equal).is_err() {
        return record_fifo_failure(3);
    }

    if mode == 0 {
        let lower = match scheduler::kthread_create_fifo(
            "fifo-lower-peer",
            fifo_peer,
            3,
            ThreadPriority::new(200),
        ) {
            Ok(id) => id,
            Err(_) => return record_fifo_failure(4),
        };
        if scheduler::thread_ready(lower).is_err()
            || scheduler::set_thread_fifo_policy(current, ThreadPriority::new(64)).is_err()
            || scheduler::set_thread_fifo_policy(current, ThreadPriority::new(200)).is_err()
            || !matches!(scheduler::cond_resched(), Ok(true))
        {
            return record_fifo_failure(5);
        }
    } else if mode == 1 {
        if scheduler::set_thread_fifo_policy(current, ThreadPriority::new(200)).is_err()
            || scheduler::set_thread_fifo_policy(current, ThreadPriority::new(64)).is_err()
            || !matches!(scheduler::cond_resched(), Ok(true))
        {
            return record_fifo_failure(6);
        }
    } else {
        if scheduler::set_thread_fifo_policy(current, ThreadPriority::new(64)).is_err()
            || scheduler::set_thread_fifo_policy(equal, ThreadPriority::new(200)).is_err()
            || !matches!(scheduler::cond_resched(), Ok(false))
        {
            return record_fifo_failure(7);
        }
        let future_equal = match scheduler::kthread_create_fifo(
            "fifo-future-peer",
            fifo_peer,
            1,
            ThreadPriority::new(64),
        ) {
            Ok(id) => id,
            Err(_) => return record_fifo_failure(8),
        };
        if scheduler::thread_ready(future_equal).is_err()
            || !matches!(scheduler::cond_resched(), Ok(false))
            || scheduler::yield_now().is_err()
        {
            return record_fifo_failure(9);
        }
    }
    fifo_peer(2);
}

fn record_fifo_failure(code: usize) {
    FIFO_FAILURE.store(code, Ordering::Release);
}

extern "C" fn fifo_peer(expected: usize) {
    let _ =
        FIFO_SEQUENCE.compare_exchange(expected - 1, expected, Ordering::AcqRel, Ordering::Acquire);
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
    scheduler::set_thread_fifo_policy(first, ThreadPriority::HIGHEST)?;
    scheduler::set_thread_fifo_policy(second, ThreadPriority::HIGHEST)?;
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
    // SAFETY: ConsumerArgument points to the parent's SyncState, which remains
    // pinned until every consumer signals completion.
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
    // SAFETY: WaitArgument retains the pinned parent WaitState until this
    // worker publishes completion.
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
