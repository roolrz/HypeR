// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-owned thread registry, CPU state, and lifecycle transitions.

use alloc::boxed::Box;
use hyper::cpu::{CpuIndex, PerCpu};
use hyper::sync::InterruptSpinLock;

use super::queue::{self, CpuRunQueue};
use super::registry::{
    CpuScheduleAuthorityToken, CpuThreadTableAuthority, ThreadRegistry, ThreadRegistryStatus,
    ThreadReservation, ThreadTableCapability,
};
use super::{
    CrashTaskSnapshot, CurrentUser, CurrentVcpu, Error, MigrationStatus, SecondaryStack, Statistics,
};
use crate::kernel::task::policy::{
    CpuMask, PlacementPolicy, SchedulingClass, SchedulingPolicy, ThreadPriority,
};
use crate::kernel::task::thread::{
    DeferredFifoPlacement, ExecutionKind, MigrationRequest, QueueMembership, Thread, ThreadId,
    ThreadScheduleState, ThreadState,
};
use crate::kernel::task::wait::{
    PendingResolution, ThreadQueue, WaitMobility, WaitOutcome, WaitQueue, WaitRecord,
    WaitRecordError, WaitTicket,
};

/// Owned observation produced by one closure-bounded table access.
///
/// It contains only copyable state and stable machine-resource addresses; no
/// Rust reference into a Thread slot can escape the authority closure.
#[derive(Clone, Copy)]
pub(super) struct ThreadObservation {
    schedule_owner: Option<CpuIndex>,
    schedule: Option<ThreadScheduleObservation>,
    execution: ExecutionKind,
    context: *mut crate::hal::context::ThreadContext,
    vcpu: Option<*mut crate::kernel::task::thread::VcpuExecution>,
    user: Option<core::ptr::NonNull<crate::kernel::process::UserExecution>>,
    stack_bounds: Option<(usize, usize)>,
}

#[derive(Clone, Copy)]
struct ThreadScheduleObservation {
    cpu: CpuIndex,
    affinity: CpuMask,
    placement: PlacementPolicy,
    state: ThreadState,
    scheduling: SchedulingPolicy,
    fair_slice_expired: bool,
    deferred_fifo_placement: Option<DeferredFifoPlacement>,
    queue_links: crate::kernel::task::thread::QueueLinks,
    wait: WaitRecord,
    pending_migration: Option<MigrationRequest>,
}

impl ThreadObservation {
    fn capture(thread: &Thread) -> Self {
        let schedule_owner = thread.schedule_owner_cpu();
        Self {
            schedule_owner,
            schedule: schedule_owner.is_none().then(|| ThreadScheduleObservation {
                cpu: thread.cpu_index(),
                affinity: thread.affinity(),
                placement: thread.placement_policy(),
                state: thread.state(),
                scheduling: thread.scheduling_policy(),
                fair_slice_expired: thread.fair_slice_expired(),
                deferred_fifo_placement: thread.deferred_fifo_placement(),
                queue_links: thread.queue_links(),
                wait: *thread.wait_record(),
                pending_migration: thread.pending_migration(),
            }),
            execution: thread.execution_kind(),
            context: thread.context_pointer(),
            vcpu: thread.vcpu_execution_pointer(),
            user: thread.user_execution_pointer(),
            stack_bounds: thread.kernel_stack_bounds(),
        }
    }

    fn capture_cpu(thread: &Thread, cpu: CpuIndex, schedule: &ThreadScheduleState) -> Self {
        Self {
            schedule_owner: Some(cpu),
            schedule: Some(ThreadScheduleObservation {
                cpu: schedule.placement.assigned_cpu(),
                affinity: schedule.placement.affinity(),
                placement: schedule.placement.policy(),
                state: schedule.state,
                scheduling: schedule.scheduling,
                fair_slice_expired: schedule.fair_slice_expired(),
                deferred_fifo_placement: schedule.deferred_fifo_placement,
                // SAFETY: every Scheduler operation is serialized by the
                // TransitionLock in addition to this matching CPU lock.
                queue_links: unsafe { thread.combined_queue_links(schedule.ready_queue_links) },
                wait: schedule.wait,
                pending_migration: schedule.pending_migration,
            }),
            execution: thread.execution_kind(),
            context: thread.context_pointer(),
            vcpu: thread.vcpu_execution_pointer(),
            user: thread.user_execution_pointer(),
            stack_bounds: thread.kernel_stack_bounds(),
        }
    }

    fn schedule(self) -> ThreadScheduleObservation {
        self.schedule.unwrap_or_else(|| crate::hal::cpu::halt())
    }

    pub(super) const fn schedule_owner_cpu(self) -> Option<CpuIndex> {
        self.schedule_owner
    }
    fn cpu_index(self) -> CpuIndex {
        self.schedule().cpu
    }
    fn affinity(self) -> CpuMask {
        self.schedule().affinity
    }
    fn placement_policy(self) -> PlacementPolicy {
        self.schedule().placement
    }
    pub(super) fn state(self) -> ThreadState {
        self.schedule().state
    }
    fn scheduling_policy(self) -> SchedulingPolicy {
        self.schedule().scheduling
    }
    fn scheduling_class(self) -> SchedulingClass {
        self.schedule().scheduling.class()
    }
    fn fair_slice_expired(self) -> bool {
        self.schedule().fair_slice_expired
    }
    fn deferred_fifo_placement(self) -> Option<DeferredFifoPlacement> {
        self.schedule().deferred_fifo_placement
    }
    fn queue_links(self) -> crate::kernel::task::thread::QueueLinks {
        self.schedule().queue_links
    }
    fn wait_record(&self) -> &WaitRecord {
        match &self.schedule {
            Some(schedule) => &schedule.wait,
            None => crate::hal::cpu::halt(),
        }
    }
    fn pending_migration(self) -> Option<MigrationRequest> {
        self.schedule().pending_migration
    }
    pub(super) const fn execution_kind(self) -> ExecutionKind {
        self.execution
    }
    const fn context_pointer(self) -> *mut crate::hal::context::ThreadContext {
        self.context
    }
    const fn vcpu_execution_pointer(
        self,
    ) -> Option<*mut crate::kernel::task::thread::VcpuExecution> {
        self.vcpu
    }
    const fn user_execution_pointer(
        self,
    ) -> Option<core::ptr::NonNull<crate::kernel::process::UserExecution>> {
        self.user
    }
    pub(super) const fn kernel_stack_bounds(self) -> Option<(usize, usize)> {
        self.stack_bounds
    }
    fn can_run_on(self, cpu: CpuIndex) -> bool {
        self.schedule().affinity.contains(cpu)
    }
}

/// Exclusive table authority scoped to one Thread identity without exposing a
/// reference to the underlying slot.
struct ThreadMutation<'registry> {
    registry: &'registry mut ThreadRegistry,
    id: ThreadId,
    cpu: Option<CpuIndex>,
}

impl ThreadMutation<'_> {
    fn apply<R>(&mut self, operation: impl for<'thread> FnOnce(&'thread mut Thread) -> R) -> R {
        match self.registry.with_thread_mut(self.id, operation) {
            Ok(value) => value,
            Err(_) => crate::hal::cpu::halt(),
        }
    }

    fn apply_schedule<R>(
        &mut self,
        operation: impl for<'schedule> FnOnce(&'schedule mut ThreadScheduleState) -> R,
    ) -> R {
        match self.cpu {
            None => self.apply(|thread| thread.with_coordinator_schedule_mut(operation)),
            Some(cpu) => match self.registry.with_thread(self.id, |thread| {
                // SAFETY: a CPU mutation is created only by `with_cpu_domain`,
                // after it has locked and revalidated this exact owner.
                unsafe { thread.with_cpu_schedule_mut(cpu, operation) }
            }) {
                Ok(Some(value)) => value,
                Ok(None) | Err(_) => crate::hal::cpu::halt(),
            },
        }
    }

    fn arm_user_execution(
        &mut self,
        ownership: crate::kernel::process::UserExecutionOwnership,
    ) -> bool {
        self.apply(|thread| thread.arm_user_execution(ownership))
    }
    fn replace_affinity(&mut self, affinity: CpuMask) -> bool {
        self.apply_schedule(|schedule| {
            let Some(placement) = schedule.placement.with_affinity(affinity) else {
                return false;
            };
            schedule.placement = placement;
            true
        })
    }
    fn request_migration(&mut self, request: MigrationRequest) -> bool {
        self.apply_schedule(|schedule| match schedule.pending_migration {
            Some(existing) => existing == request,
            None => {
                schedule.pending_migration = Some(request);
                true
            }
        })
    }
    fn reassign_stopped_with_affinity(&mut self, cpu: CpuIndex, affinity: CpuMask) -> bool {
        self.apply_schedule(|schedule| {
            let Some(placement) = schedule.placement.reassign_with_affinity(cpu, affinity) else {
                return false;
            };
            schedule.placement = placement;
            true
        })
    }
    fn replenish_fair_slice(&mut self, quantum: u64) {
        self.apply_schedule(|schedule| schedule.replenish_fair_slice(quantum));
    }
    fn set_deferred_fifo_placement(&mut self, placement: Option<DeferredFifoPlacement>) {
        self.apply_schedule(|schedule| schedule.deferred_fifo_placement = placement);
    }
    fn set_scheduling_policy(&mut self, policy: SchedulingPolicy) -> bool {
        self.apply_schedule(|schedule| {
            if schedule.scheduling_class() == SchedulingClass::Idle {
                return false;
            }
            schedule.scheduling = policy;
            schedule.replenish_fair_slice(0);
            schedule.deferred_fifo_placement = None;
            true
        })
    }
    fn set_state(&mut self, state: ThreadState) {
        self.apply_schedule(|schedule| schedule.state = state);
    }
    fn mark_running_on(&mut self, cpu: CpuIndex) -> bool {
        self.apply_schedule(|schedule| {
            let Some(placement) = schedule.placement.mark_running(cpu) else {
                return false;
            };
            schedule.placement = placement;
            schedule.deferred_fifo_placement = None;
            schedule.state = ThreadState::Running;
            true
        })
    }
    fn take_migration_request(&mut self) -> Option<MigrationRequest> {
        self.apply_schedule(|schedule| schedule.pending_migration.take())
    }
    fn ensure_kernel_stack(
        &mut self,
    ) -> Result<(usize, usize), crate::kernel::task::thread::Error> {
        self.apply(Thread::ensure_kernel_stack)
    }
    fn become_idle(&mut self) {
        self.apply_schedule(|schedule| {
            schedule.scheduling = SchedulingPolicy::Idle;
            schedule.state = ThreadState::Idle;
        });
    }
    fn with_wait_record<R>(
        &mut self,
        operation: impl for<'wait> FnOnce(&'wait mut WaitRecord) -> R,
    ) -> R {
        self.apply_schedule(|schedule| operation(&mut schedule.wait))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadyOutcome {
    pub changed: bool,
    pub target_cpu: CpuIndex,
    pub should_preempt: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MigrationOutcome {
    pub status: MigrationStatus,
    pub source_reschedule: Option<CpuIndex>,
    pub target_ready: Option<ReadyOutcome>,
}

pub(super) enum PreparedWait {
    Park {
        switch: PreparedContextSwitch,
        ticket: WaitTicket,
    },
    Completed(WaitOutcome),
}

pub(super) struct ResolvedWait {
    pub won: bool,
    pub ready: Option<ReadyOutcome>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WakePreemption {
    Policy,
    FairBoundary,
}

pub(super) enum VcpuStopTarget {
    Running(CpuIndex),
    Runnable {
        cpu: CpuIndex,
        ready: Option<ReadyOutcome>,
    },
    Migrating(CpuIndex),
    Terminated,
}

/// Charges one CPU's running Fair entity without taking the transition lock.
pub(super) fn account_tick(cpu: CpuIndex, elapsed: u64) -> Result<bool, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let current = local.current;
        let has_fair_ready = local.run_queue.has_fair_threads();
        let mut threads = local.thread_authority();
        threads.with_thread_mut(current, |_thread, schedule| match schedule.state {
            ThreadState::Idle => Ok(false),
            ThreadState::Running => {
                if !schedule.account_fair_ticks(elapsed, super::FAIR_QUANTUM_TICKS) {
                    return Ok(false);
                }
                if has_fair_ready {
                    Ok(true)
                } else {
                    schedule.replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                    Ok(false)
                }
            }
            _ => Err(Error::InvalidThreadState),
        })?
    })
}

/// Attempts a cooperative scheduling decision using only the current CPU lock.
pub(super) fn prepare_local_yield(cpu: CpuIndex) -> Result<LocalScheduleAttempt, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        if local.switching_from.is_some() {
            return Err(Error::ThreadTransitionInProgress);
        }
        let current = local.current;
        let (state, policy, pending_migration) = {
            let threads = local.thread_authority();
            threads.with_thread(current, |_thread, schedule| {
                (
                    schedule.state,
                    schedule.scheduling,
                    schedule.pending_migration,
                )
            })?
        };
        if pending_migration.is_some() {
            return Ok(LocalScheduleAttempt::NeedsCoordinator);
        }
        let _ = super::super::preempt::take_pending_locked(cpu)?;
        let Some(candidate) = local.local_peek_ready()? else {
            return Ok(LocalScheduleAttempt::Complete(None));
        };
        let enqueue_current = match state {
            ThreadState::Running => {
                let can_yield_to = match (policy, candidate.policy) {
                    (SchedulingPolicy::Fair, _) => true,
                    (
                        SchedulingPolicy::Fifo { priority: current },
                        SchedulingPolicy::Fifo { priority: ready },
                    ) => ready <= current,
                    (SchedulingPolicy::Fifo { .. }, SchedulingPolicy::Fair) => false,
                    (SchedulingPolicy::Idle, _) | (_, SchedulingPolicy::Idle) => false,
                };
                if !can_yield_to {
                    return Ok(LocalScheduleAttempt::Complete(None));
                }
                if policy == SchedulingPolicy::Fair {
                    let mut threads = local.thread_authority();
                    threads.with_thread_mut(current, |_thread, schedule| {
                        schedule.replenish_fair_slice(super::FAIR_QUANTUM_TICKS)
                    })?;
                }
                true
            }
            ThreadState::Idle => false,
            _ => return Err(Error::InvalidThreadState),
        };
        if enqueue_current {
            local.local_enqueue_ready(current, false)?;
        }
        let next = match local.local_dequeue_ready() {
            Ok(Some(next)) => next,
            Ok(None) => scheduler_invariant(Error::CurrentThreadMissing),
            Err(error) => scheduler_invariant(error),
        };
        if next != candidate.id {
            scheduler_invariant(Error::QueueCorrupted);
        }
        match local.prepare_local_switch(current, next) {
            Ok(switch) => Ok(LocalScheduleAttempt::Complete(Some(switch))),
            Err(error) => scheduler_invariant(error),
        }
    })
}

/// Attempts an IRQ-tail/conditional preemption using only the current CPU lock.
pub(super) fn prepare_local_preemption(cpu: CpuIndex) -> Result<LocalScheduleAttempt, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        if local.switching_from.is_some() {
            return Err(Error::ThreadTransitionInProgress);
        }
        if !super::super::preempt::pending(cpu)? {
            return Ok(LocalScheduleAttempt::Complete(None));
        }
        let current = local.current;
        let (state, policy, fair_expired, deferred, pending_migration) = {
            let threads = local.thread_authority();
            threads.with_thread(current, |_thread, schedule| {
                (
                    schedule.state,
                    schedule.scheduling,
                    schedule.fair_slice_expired(),
                    schedule.deferred_fifo_placement,
                    schedule.pending_migration,
                )
            })?
        };
        if pending_migration.is_some() {
            return Ok(LocalScheduleAttempt::NeedsCoordinator);
        }
        let Some(candidate) = local.local_peek_ready()? else {
            let _ = super::super::preempt::take_pending_locked(cpu)?;
            if state == ThreadState::Running {
                let mut threads = local.thread_authority();
                threads.with_thread_mut(current, |_thread, schedule| {
                    if schedule.fair_slice_expired() {
                        schedule.replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                    }
                    schedule.deferred_fifo_placement = None;
                })?;
            }
            return Ok(LocalScheduleAttempt::Complete(None));
        };
        let enqueue_front = match state {
            ThreadState::Idle => None,
            ThreadState::Running => {
                let fair_rotation = policy == SchedulingPolicy::Fair
                    && candidate.policy == SchedulingPolicy::Fair
                    && fair_expired;
                let fifo_deferred_rotation = matches!(
                    (policy, candidate.policy, deferred),
                    (
                        SchedulingPolicy::Fifo { priority: current },
                        SchedulingPolicy::Fifo { priority: ready },
                        Some(DeferredFifoPlacement::Tail),
                    ) if current == ready
                );
                if !policy.is_preempted_by(candidate.policy)
                    && !fair_rotation
                    && !fifo_deferred_rotation
                {
                    let _ = super::super::preempt::take_pending_locked(cpu)?;
                    let mut threads = local.thread_authority();
                    threads.with_thread_mut(current, |_thread, schedule| {
                        schedule.deferred_fifo_placement = None
                    })?;
                    return Ok(LocalScheduleAttempt::Complete(None));
                }
                if fair_rotation {
                    let mut threads = local.thread_authority();
                    threads.with_thread_mut(current, |_thread, schedule| {
                        schedule.replenish_fair_slice(super::FAIR_QUANTUM_TICKS)
                    })?;
                    Some(false)
                } else {
                    Some(deferred != Some(DeferredFifoPlacement::Tail))
                }
            }
            _ => return Err(Error::InvalidThreadState),
        };
        if !super::super::preempt::take_pending_locked(cpu)? {
            return Ok(LocalScheduleAttempt::Complete(None));
        }
        if let Some(front) = enqueue_front {
            let mut threads = local.thread_authority();
            threads.with_thread_mut(current, |_thread, schedule| {
                schedule.deferred_fifo_placement = None
            })?;
            local.local_enqueue_ready(current, front)?;
        }
        let next = match local.local_dequeue_ready() {
            Ok(Some(next)) => next,
            Ok(None) => scheduler_invariant(Error::CurrentThreadMissing),
            Err(error) => scheduler_invariant(error),
        };
        if next != candidate.id {
            scheduler_invariant(Error::QueueCorrupted);
        }
        match local.prepare_local_switch(current, next) {
            Ok(switch) => Ok(LocalScheduleAttempt::Complete(Some(switch))),
            Err(error) => scheduler_invariant(error),
        }
    })
}

/// Completes an ordinary switch without entering the transition coordinator.
pub(super) fn complete_local_switch_tail(
    cpu: CpuIndex,
    ticket: u64,
) -> Result<LocalTailCompletion, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let switching = local.switching_from.ok_or(Error::PreemptionInvariant)?;
        if switching.generation != ticket {
            return Err(Error::PreemptionInvariant);
        }
        if switching.disposition != SwitchDisposition::Local {
            return Ok(LocalTailCompletion::NeedsCoordinator);
        }
        let (state, pending) = {
            let threads = local.thread_authority();
            threads.with_thread(switching.thread, |_thread, schedule| {
                (schedule.state, schedule.pending_migration)
            })?
        };
        if pending.is_some() || !matches!(state, ThreadState::Ready | ThreadState::Idle) {
            return Ok(LocalTailCompletion::NeedsCoordinator);
        }
        if local.switching_from != Some(switching) {
            return Err(Error::PreemptionInvariant);
        }
        local.switching_from = None;
        Ok(LocalTailCompletion::Complete)
    })
}

/// Best-effort crash observation taken under one non-blocking CPU lock.
///
/// Current identity, schedule state, and immutable resource metadata come
/// from one coherent CPU-domain snapshot. Failure to acquire that lock is
/// reported as absence; crash handling must never wait for scheduler state.
pub(super) fn try_cpu_snapshot(cpu: CpuIndex) -> Option<CrashTaskSnapshot> {
    CPU_SCHEDULERS[cpu]
        .try_with(|slot| {
            let local = slot.as_mut()?;
            let current = local.current;
            let threads = local.thread_authority();
            threads
                .with_thread(current, |thread, schedule| CrashTaskSnapshot {
                    id: thread.id(),
                    state: schedule.state,
                    execution: thread.execution_kind(),
                    stack: thread.kernel_stack_bounds(),
                    // Current stack memory is live and cannot be scanned.
                    stack_statistics: None,
                })
                .ok()
        })
        .flatten()
}

pub(super) fn local_current_thread(cpu: CpuIndex) -> Result<ThreadId, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        slot.as_ref()
            .map(|local| local.current)
            .ok_or(Error::CpuNotRegistered)
    })
}

pub(super) fn local_current_vcpu(cpu: CpuIndex) -> Result<Option<CurrentVcpu>, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let current = local.current;
        let threads = local.thread_authority();
        threads.with_thread(current, |thread, schedule| {
            if !matches!(schedule.state, ThreadState::Running | ThreadState::Idle) {
                return Err(Error::InvalidThreadState);
            }
            let Some(execution) = thread.vcpu_execution_pointer() else {
                return Ok(None);
            };
            let stack = thread.kernel_stack_bounds().ok_or(Error::Allocation)?;
            Ok(Some(CurrentVcpu {
                thread: current,
                execution,
                stack,
            }))
        })?
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SwitchingContext {
    thread: ThreadId,
    generation: u64,
    disposition: SwitchDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SwitchDisposition {
    Local,
    Coordinated,
}

pub(super) enum LocalScheduleAttempt {
    Complete(Option<PreparedContextSwitch>),
    NeedsCoordinator,
}

pub(super) enum LocalTailCompletion {
    Complete,
    NeedsCoordinator,
}

pub(super) struct CoordinatedTailCompletion {
    pub ready: Option<ReadyOutcome>,
    pub retirement_published: bool,
}

/// Detached resource owner consumed only by the dedicated reaper Thread.
pub(super) struct RetiredThread {
    pub id: ThreadId,
    pub thread: Box<Thread>,
    pub retirement: super::ResourceRetirement,
}

struct RetirementTicket {
    id: ThreadId,
    retirement: super::ResourceRetirement,
}

/// Fixed-capacity FIFO for teardown which may block or acquire subsystem locks.
///
/// A queued element retains its registry slot in `Retiring`, so the number of
/// elements can never exceed the scheduler's fixed Thread capacity even when
/// creators race a stalled reaper.
struct RetirementQueue {
    entries: [Option<RetirementTicket>; super::THREAD_CAPACITY],
    head: usize,
    length: usize,
}

impl RetirementQueue {
    const fn new() -> Self {
        Self {
            entries: [const { None }; super::THREAD_CAPACITY],
            head: 0,
            length: 0,
        }
    }

    fn push(&mut self, retired: RetirementTicket) -> Result<(), Error> {
        if self.entries.is_empty()
            || self.head >= self.entries.len()
            || self.length >= self.entries.len()
        {
            return Err(Error::ThreadLimit);
        }
        let tail = self
            .head
            .checked_add(self.length)
            .ok_or(Error::QueueCorrupted)?
            % self.entries.len();
        let next_length = self.length.checked_add(1).ok_or(Error::QueueCorrupted)?;
        if self.entries[tail].is_some() {
            return Err(Error::QueueCorrupted);
        }
        self.entries[tail] = Some(retired);
        self.length = next_length;
        Ok(())
    }

    fn pop(&mut self) -> Result<Option<RetirementTicket>, Error> {
        if self.length == 0 {
            return Ok(None);
        }
        if self.entries.is_empty() || self.head >= self.entries.len() {
            return Err(Error::QueueCorrupted);
        }
        let retired = self.entries[self.head]
            .take()
            .ok_or(Error::QueueCorrupted)?;
        self.head = self.head.checked_add(1).ok_or(Error::QueueCorrupted)? % self.entries.len();
        self.length = self.length.checked_sub(1).ok_or(Error::QueueCorrupted)?;
        Ok(Some(retired))
    }

    const fn is_empty(&self) -> bool {
        self.length == 0
    }
}

pub(super) struct Scheduler {
    // Thread identities never repeat, while empty registry slots do. The
    // complete bounded backing is allocated during scheduler initialization;
    // user-created Threads cannot grow persistent kernel infrastructure.
    registry: ThreadRegistry,
    cpu_reservations: PerCpu<bool>,
    /// TransitionLock-owned admission truth for CPUs with an installed idle.
    ///
    /// This is policy metadata, not a mirror of CPU-local execution state.
    /// Cross-CPU placement may read it without nesting a target CPU lock.
    schedulable_cpus: CpuMask,
    /// Stack-scoped authority installed only while one `CpuScheduler` lock is held.
    active_domain: Option<ActiveCpuDomain>,
    /// Coordinator-owned entity awaiting publication after a source CPU lock
    /// is released. `TransitionLock` keeps this handoff invisible externally.
    deferred_ready_handoff: Option<(ThreadId, CpuIndex)>,
    terminated: ThreadQueue,
    retirements: RetirementQueue,
}

/// CPU-local scheduling state that may be touched without the transition lock.
///
/// `current` is a linear token: it is either here or temporarily returned to
/// its registry Thread while a transition holds this CPU lock.
struct CpuScheduler {
    index: CpuIndex,
    table: ThreadTableCapability,
    authority: CpuScheduleAuthorityToken,
    current: ThreadId,
    idle: Option<ThreadId>,
    run_queue: CpuRunQueue,
    switching_from: Option<SwitchingContext>,
    next_switch_generation: u64,
    context_switches: u64,
}

#[derive(Clone, Copy)]
struct ActiveCpuDomain {
    cpu: CpuIndex,
    local: core::ptr::NonNull<CpuScheduler>,
}

// SAFETY: this pointer is never transferred as an independent capability. It
// is installed and cleared while SCHEDULER and the pointed-to CPU lock are
// both held, and every dereference revalidates the recorded CPU.
unsafe impl Send for ActiveCpuDomain {}

impl CpuScheduler {
    const fn new(index: CpuIndex, table: ThreadTableCapability, current: ThreadId) -> Self {
        Self {
            index,
            table,
            authority: CpuScheduleAuthorityToken::new(),
            current,
            idle: None,
            run_queue: CpuRunQueue::new(),
            switching_from: None,
            next_switch_generation: 1,
            context_switches: 0,
        }
    }

    fn thread_authority(&mut self) -> CpuThreadTableAuthority<'_> {
        self.table.cpu_authority(self.index, &mut self.authority)
    }

    fn local_peek_ready(&mut self) -> Result<Option<queue::ReadyThread>, Error> {
        let cpu = self.index;
        let threads = queue::LocalReadyQueueAuthority::new(
            self.table.cpu_authority(cpu, &mut self.authority),
        );
        self.run_queue.peek_next(&threads, cpu)
    }

    fn local_enqueue_ready(&mut self, id: ThreadId, front: bool) -> Result<(), Error> {
        let cpu = self.index;
        let mut threads = queue::LocalReadyQueueAuthority::new(
            self.table.cpu_authority(cpu, &mut self.authority),
        );
        if front {
            self.run_queue.enqueue_front(&mut threads, id, cpu)
        } else {
            self.run_queue.enqueue(&mut threads, id, cpu)
        }
    }

    fn local_dequeue_ready(&mut self) -> Result<Option<ThreadId>, Error> {
        let cpu = self.index;
        let mut threads = queue::LocalReadyQueueAuthority::new(
            self.table.cpu_authority(cpu, &mut self.authority),
        );
        self.run_queue.dequeue(&mut threads, cpu)
    }

    fn prepare_local_switch(
        &mut self,
        current: ThreadId,
        next: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        if self.switching_from.is_some() || self.current != current || current == next {
            return Err(Error::ThreadTransitionInProgress);
        }
        let cpu = self.index;
        let (previous, next_context) = {
            let mut threads = self.thread_authority();
            let previous =
                threads.with_thread(current, |thread, _schedule| thread.context_pointer())?;
            let next_context = threads.with_thread_mut(next, |thread, schedule| {
                if schedule.state != ThreadState::Ready {
                    return Err(Error::InvalidThreadState);
                }
                let Some(placement) = schedule.placement.mark_running(cpu) else {
                    return Err(Error::InvalidThreadState);
                };
                schedule.placement = placement;
                schedule.state = ThreadState::Running;
                if schedule.fair_slice_expired() {
                    schedule.replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                }
                Ok::<_, Error>(thread.context_pointer().cast_const())
            })??;
            (previous, next_context)
        };
        let generation = self.next_switch_generation;
        self.next_switch_generation = generation
            .checked_add(1)
            .unwrap_or_else(|| crate::hal::cpu::halt());
        self.switching_from = Some(SwitchingContext {
            thread: current,
            generation,
            disposition: SwitchDisposition::Local,
        });
        self.current = next;
        self.context_switches = self.context_switches.saturating_add(1);
        Ok(PreparedContextSwitch {
            previous,
            next: next_context,
            ticket: generation,
            armed: true,
        })
    }
}

type CpuSchedulerLock = InterruptSpinLock<Option<CpuScheduler>, crate::hal::irq::LocalMask>;

static CPU_SCHEDULERS: PerCpu<CpuSchedulerLock> =
    PerCpu::new([const { CpuSchedulerLock::new(None) }; hyper::cpu::MAX_CPUS]);

#[must_use = "a prepared context switch must be consumed by the architecture boundary"]
pub(super) struct PreparedContextSwitch {
    previous: *mut crate::hal::context::ThreadContext,
    next: *const crate::hal::context::ThreadContext,
    ticket: u64,
    armed: bool,
}

impl PreparedContextSwitch {
    /// Activates the only context switch represented by this transition.
    ///
    /// The scheduler transaction is already committed, so this is the sole
    /// operation which can disarm the fail-stop Drop path.
    pub(super) fn activate(mut self, previous_interrupt_state: super::TransitionMaskState) {
        self.armed = false;
        // SAFETY: Scheduler queues contain only pinned, scheduler-owned
        // Threads. The assembly path saves `previous_interrupt_state`, changes
        // to the incoming stack, and completes switching_from ownership before
        // it restores the incoming interrupt state.
        unsafe {
            crate::hal::context::switch_thread_context(
                self.previous,
                self.next,
                previous_interrupt_state,
                super::finish_context_switch_tail,
                self.ticket as usize,
            )
        };
    }
}

impl Drop for PreparedContextSwitch {
    fn drop(&mut self) {
        if self.armed {
            // Drop may run while scheduler or architecture transition locks
            // are held. Diagnostics could deadlock before preserving the
            // committed context-switch state.
            crate::hal::cpu::halt()
        }
    }
}

impl Scheduler {
    pub fn thread_registry_status(&self, id: ThreadId) -> ThreadRegistryStatus {
        self.registry.status(id)
    }

    #[cfg(feature = "kernel-self-test")]
    pub fn thread_object_snapshot(
        &self,
        id: ThreadId,
    ) -> Result<crate::kernel::task::ThreadObjectSnapshot, Error> {
        self.registry.object_snapshot(id)
    }

    pub fn scan_thread_objects(
        &self,
        cursor: crate::kernel::task::ThreadObjectScanCursor,
    ) -> crate::kernel::task::ThreadObjectSnapshotPage {
        self.registry.scan_objects(cursor)
    }

    /// Routes schedule access without recursively reacquiring an active CPU lock.
    ///
    /// `Some(cpu)` means the caller must enter that CPU domain. `None` means
    /// either coordinator ownership or that the matching CPU domain is
    /// already active and the function body may proceed directly.
    fn cpu_lock_required_for(&self, id: ThreadId) -> Result<Option<CpuIndex>, Error> {
        let owner = self.registry.with_thread(id, Thread::schedule_owner_cpu)?;
        match (owner, self.active_domain) {
            (None, _) => Ok(None),
            (Some(cpu), None) => Ok(Some(cpu)),
            (Some(cpu), Some(active)) if cpu == active.cpu => Ok(None),
            (Some(_), Some(_)) => Err(Error::InvalidThreadState),
        }
    }

    pub fn new(boot_cpu: CpuIndex) -> Result<Self, Error> {
        let bootstrap =
            hyper::mm::try_box(Thread::bootstrap(boot_cpu)?).map_err(|_| Error::Allocation)?;
        let boot_idle_id = match ThreadId::from_scheduler_parts(1, 1) {
            Some(id) => id,
            None => return Err(Error::IdentifierExhausted),
        };
        let boot_idle = Thread::idle(boot_idle_id, boot_cpu, "idle", super::idle_thread_entry)?;
        let boot_idle = hyper::mm::try_box(boot_idle).map_err(|_| Error::Allocation)?;
        let registry = ThreadRegistry::new(bootstrap, boot_idle)?;
        Ok(Self {
            registry,
            cpu_reservations: PerCpu::new([false; hyper::cpu::MAX_CPUS]),
            schedulable_cpus: CpuMask::EMPTY,
            active_domain: None,
            deferred_ready_handoff: None,
            terminated: ThreadQueue::new(),
            retirements: RetirementQueue::new(),
        })
    }

    /// Publishes the boot CPU's linear running token during global commit.
    pub fn activate_boot_cpu(&mut self, cpu: CpuIndex) -> Result<(), Error> {
        if !CPU_SCHEDULERS[cpu].with(|slot| slot.is_none()) {
            return Err(Error::AlreadyInitialized);
        }
        let claimed = self
            .registry
            .with_thread_mut(ThreadId::BOOTSTRAP, |thread| thread.claim_schedule(cpu))?;
        let boot_idle = ThreadId::from_scheduler_parts(1, 1).ok_or(Error::IdentifierExhausted)?;
        let idle_claimed = self
            .registry
            .with_thread_mut(boot_idle, |thread| thread.claim_schedule(cpu))?;
        if !claimed || !idle_claimed {
            return Err(Error::InvalidThreadState);
        }
        // All fallible scheduler construction is complete. From this point the
        // fixed table has kernel lifetime and can later be addressed through
        // transferred CPU authority tokens without borrowing `Scheduler`.
        self.registry.publish_table();
        let table = self.registry.published_table();
        let installed = CPU_SCHEDULERS[cpu].with(|slot| {
            if slot.is_some() {
                return false;
            }
            let mut scheduler = CpuScheduler::new(cpu, table, ThreadId::BOOTSTRAP);
            scheduler.idle = Some(boot_idle);
            *slot = Some(scheduler);
            true
        });
        if !installed {
            crate::hal::cpu::halt();
        }
        self.schedulable_cpus = self.schedulable_cpus.with_cpu(cpu);
        Ok(())
    }

    pub fn current_thread(&self, cpu: CpuIndex) -> Result<ThreadId, Error> {
        if let Some(active) = self.active_domain {
            if active.cpu != cpu {
                return Err(Error::CpuNotRegistered);
            }
            // SAFETY: active_domain exists only inside the matching lock's
            // closure and is cleared before that closure returns.
            return Ok(unsafe { active.local.as_ref() }.current);
        }
        CPU_SCHEDULERS[cpu].with(|slot| {
            slot.as_ref()
                .map(|scheduler| scheduler.current)
                .ok_or(Error::CpuNotRegistered)
        })
    }

    #[cfg(feature = "kernel-self-test")]
    pub fn thread_placement(&mut self, id: ThreadId) -> Result<(CpuIndex, CpuMask), Error> {
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| scheduler.thread_placement(id));
        }
        let thread = self.thread(id)?;
        Ok((thread.cpu_index(), thread.affinity()))
    }

    /// Reports whether no CPU owns or may still be saving this context.
    pub fn context_is_stopped(&self, id: ThreadId) -> Result<bool, Error> {
        let _ = self.thread(id)?;
        if self.active_domain.is_some() {
            // Never acquire a second CPU lock from an active CPU domain.
            // Conservatively defer reclamation until the outer transition
            // releases its owner lock.
            return Ok(false);
        }
        for index in 0..hyper::cpu::MAX_CPUS {
            let Some(cpu) = CpuIndex::new(index) else {
                crate::hal::cpu::halt();
            };
            let in_use = CPU_SCHEDULERS[cpu].with(|slot| {
                slot.as_ref().is_some_and(|local| {
                    local.current == id
                        || local
                            .switching_from
                            .is_some_and(|switching| switching.thread == id)
                })
            });
            if in_use {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn reserve_secondary(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        if CPU_SCHEDULERS[cpu].with(|slot| slot.is_some()) || self.cpu_reservations[cpu] {
            return Err(Error::CpuAlreadyRegistered);
        }
        let reservation = self.reserve_thread_slot(cpu)?;
        self.cpu_reservations[cpu] = true;
        Ok(reservation)
    }

    pub fn publish_secondary(
        &mut self,
        reservation: &ThreadReservation,
        thread: Box<Thread>,
    ) -> Result<SecondaryStack, (Error, Box<Thread>)> {
        if !self.cpu_reservations[reservation.cpu()]
            || CPU_SCHEDULERS[reservation.cpu()].with(|slot| slot.is_some())
        {
            return Err((Error::InvalidThreadState, thread));
        }
        if !CPU_SCHEDULERS[reservation.cpu()].with(|slot| slot.is_none()) {
            return Err((Error::CpuAlreadyRegistered, thread));
        }
        let Some(virtual_top) = thread.kernel_stack_top() else {
            return Err((Error::Allocation, thread));
        };
        let Some(physical_top) = thread.kernel_stack_physical_top() else {
            return Err((Error::Allocation, thread));
        };
        self.publish_thread(reservation, thread)?;
        let claimed = match self.registry.with_thread_mut(reservation.id(), |thread| {
            thread.claim_schedule(reservation.cpu())
        }) {
            Ok(claimed) => claimed,
            Err(error) => scheduler_invariant(error),
        };
        if !claimed {
            scheduler_invariant(Error::InvalidThreadState);
        }
        let table = self.registry.published_table();
        let installed = CPU_SCHEDULERS[reservation.cpu()].with(|slot| {
            if slot.is_some() {
                return false;
            }
            *slot = Some(CpuScheduler::new(
                reservation.cpu(),
                table,
                reservation.id(),
            ));
            true
        });
        if !installed {
            crate::hal::cpu::halt();
        }
        self.cpu_reservations[reservation.cpu()] = false;
        Ok(SecondaryStack {
            physical_top,
            virtual_top,
        })
    }

    pub fn abandon_reservation(&mut self, reservation: &ThreadReservation) -> Result<(), Error> {
        // Validate and release the exact registry capability before changing
        // secondary-CPU admission state. A stale reservation must be entirely
        // failure-atomic.
        self.registry.abandon(reservation)?;
        if self.cpu_reservations[reservation.cpu()] {
            self.cpu_reservations[reservation.cpu()] = false;
        }
        Ok(())
    }

    pub fn reserve_kernel_thread(
        &mut self,
        preferred_cpu: CpuIndex,
        affinity: CpuMask,
    ) -> Result<ThreadReservation, Error> {
        let cpu = self.select_cpu(preferred_cpu, affinity)?;
        self.reserve_thread_slot(cpu)
    }

    /// Selects an admitted registered CPU, retaining creator locality where possible.
    ///
    /// If the preferred CPU is unavailable or excluded, the lowest-numbered
    /// registered CPU in the mask provides a stable fallback. This is an
    /// initial assignment only; load balancing may replace the policy later.
    fn select_cpu(&self, preferred_cpu: CpuIndex, affinity: CpuMask) -> Result<CpuIndex, Error> {
        if affinity.is_empty() {
            return Err(Error::EmptyCpuAffinity);
        }
        if affinity.contains(preferred_cpu) && self.cpu_is_schedulable(preferred_cpu) {
            return Ok(preferred_cpu);
        }
        for index in 0..hyper::cpu::MAX_CPUS {
            let Some(cpu) = CpuIndex::new(index) else {
                crate::hal::cpu::halt();
            };
            if affinity.contains(cpu) && self.cpu_is_schedulable(cpu) {
                return Ok(cpu);
            }
        }
        Err(Error::NoRegisteredCpuInAffinity)
    }

    pub fn reserve_vcpu_thread(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        self.schedulable_cpu_slot(cpu)?;
        self.reserve_thread_slot(cpu)
    }

    pub fn publish_thread(
        &mut self,
        reservation: &ThreadReservation,
        thread: Box<Thread>,
    ) -> Result<(), (Error, Box<Thread>)> {
        self.registry.publish(reservation, thread)
    }

    pub fn take_dormant_vcpu(&mut self, id: ThreadId) -> Result<Box<Thread>, Error> {
        self.take_dormant_thread(id, crate::kernel::task::thread::ExecutionKind::Vcpu)
    }

    pub fn take_dormant_user(&mut self, id: ThreadId) -> Result<Box<Thread>, Error> {
        self.take_dormant_thread(id, crate::kernel::task::thread::ExecutionKind::User)
    }

    pub fn arm_dormant_user(
        &mut self,
        id: ThreadId,
        ownership: crate::kernel::process::UserExecutionOwnership,
    ) -> Result<(), Error> {
        if self.thread(id)?.state() != ThreadState::Dormant {
            return Err(Error::InvalidThreadState);
        }
        let mut thread = self.thread_mut(id)?;
        if !thread.arm_user_execution(ownership) {
            return Err(Error::InvalidThreadState);
        }
        Ok(())
    }

    #[cfg(feature = "kernel-self-test")]
    pub fn take_dormant_kernel_thread(&mut self, id: ThreadId) -> Result<Box<Thread>, Error> {
        self.take_dormant_thread(id, crate::kernel::task::thread::ExecutionKind::Kernel)
    }

    fn take_dormant_thread(
        &mut self,
        id: ThreadId,
        execution: crate::kernel::task::thread::ExecutionKind,
    ) -> Result<Box<Thread>, Error> {
        let valid = self.registry.with_thread(id, |thread| {
            thread.id() == id
                && thread.state() == ThreadState::Dormant
                && thread.queue_links().membership == QueueMembership::None
                && thread.execution_kind() == execution
        })?;
        if !valid {
            return Err(Error::InvalidThreadState);
        }
        self.registry.take(id)
    }

    pub fn current_vcpu(&mut self, cpu: CpuIndex) -> Result<CurrentVcpu, Error> {
        let id = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| scheduler.current_vcpu(cpu));
        }
        let thread = self.thread(id)?;
        if thread.state() != ThreadState::Running {
            return Err(Error::InvalidThreadState);
        }
        let stack = thread.kernel_stack_bounds().ok_or(Error::Allocation)?;
        let execution = thread
            .vcpu_execution_pointer()
            .ok_or(Error::InvalidThreadState)?;
        Ok(CurrentVcpu {
            thread: id,
            execution,
            stack,
        })
    }

    /// Returns the CPU currently executing the exact vCPU Thread.
    ///
    /// Ready, dormant, blocked, and migrating Threads deliberately have no
    /// prompt target. Their next activation consumes durable VM work instead
    /// of guessing from a mutable placement assignment.
    pub fn running_vcpu_cpu(&self, id: ThreadId) -> Result<Option<CpuIndex>, Error> {
        let thread = self.thread(id)?;
        if thread.execution_kind() != crate::kernel::task::thread::ExecutionKind::Vcpu {
            return Err(Error::InvalidThreadState);
        }
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            // This read-only method is called under TransitionLock by its
            // public wrapper; acquire the owner CPU and re-enter once.
            return CPU_SCHEDULERS[cpu].with(|slot| {
                let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
                let current = local.current;
                let threads = local.thread_authority();
                threads.with_thread(id, |_thread, schedule| match schedule.state {
                    ThreadState::Running if current == id => Ok(Some(cpu)),
                    ThreadState::Running => Err(Error::InvalidThreadState),
                    ThreadState::Ready
                    | ThreadState::Dormant
                    | ThreadState::Blocked
                    | ThreadState::Migrating
                    | ThreadState::Terminated => Ok(None),
                    ThreadState::Idle => Err(Error::InvalidThreadState),
                })?
            });
        }
        match thread.state() {
            ThreadState::Running => {
                let cpu = thread.cpu_index();
                (self.current_thread(cpu)? == id)
                    .then_some(Some(cpu))
                    .ok_or(Error::InvalidThreadState)
            }
            ThreadState::Dormant
            | ThreadState::Ready
            | ThreadState::Blocked
            | ThreadState::Migrating
            | ThreadState::Terminated => Ok(None),
            ThreadState::Idle => Err(Error::InvalidThreadState),
        }
    }

    /// Classifies the exact vCPU continuation after durable stop publication.
    ///
    /// A blocked WFI continuation must already have been resolved through its
    /// endpoint ticket before this transaction. Dormant vCPUs become runnable
    /// so the fixed runner can publish hardware-detached and reap ownership.
    pub fn request_vcpu_stop(&mut self, id: ThreadId) -> Result<VcpuStopTarget, Error> {
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| scheduler.request_vcpu_stop(id));
        }
        let thread = self.thread(id)?;
        if thread.execution_kind() != ExecutionKind::Vcpu {
            return Err(Error::InvalidThreadState);
        }
        let cpu = thread.cpu_index();
        match thread.state() {
            ThreadState::Dormant => {
                let ready = self.make_ready(id)?;
                Ok(VcpuStopTarget::Runnable {
                    cpu,
                    ready: Some(ready),
                })
            }
            ThreadState::Ready => Ok(VcpuStopTarget::Runnable { cpu, ready: None }),
            ThreadState::Running => {
                if self.current_thread(cpu)? != id {
                    return Err(Error::InvalidThreadState);
                }
                Ok(VcpuStopTarget::Running(cpu))
            }
            ThreadState::Migrating => Ok(VcpuStopTarget::Migrating(cpu)),
            ThreadState::Terminated => Ok(VcpuStopTarget::Terminated),
            ThreadState::Blocked | ThreadState::Idle => Err(Error::InvalidThreadState),
        }
    }

    pub fn current_user(&mut self, cpu: CpuIndex) -> Result<CurrentUser, Error> {
        let id = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| scheduler.current_user(cpu));
        }
        let thread = self.thread(id)?;
        if thread.state() != ThreadState::Running {
            return Err(Error::InvalidThreadState);
        }
        let stack = thread.kernel_stack_bounds().ok_or(Error::Allocation)?;
        let execution = thread
            .user_execution_pointer()
            .ok_or(Error::InvalidThreadState)?;
        let object = self.registry.with_thread(id, |thread| {
            thread
                .user_thread()
                .cloned()
                .ok_or(Error::InvalidThreadState)
        })??;
        Ok(CurrentUser {
            thread: id,
            object,
            execution,
            stack,
        })
    }

    pub fn make_ready(&mut self, id: ThreadId) -> Result<ReadyOutcome, Error> {
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| scheduler.make_ready(id));
        }
        let cpu = self.thread(id)?.cpu_index();
        match self.thread(id)?.state() {
            ThreadState::Dormant => {
                self.enqueue_ready(id)?;
                Ok(ReadyOutcome {
                    changed: true,
                    target_cpu: cpu,
                    should_preempt: self.ready_thread_preempts(id)?,
                })
            }
            ThreadState::Blocked => Err(Error::ThreadBlocked),
            ThreadState::Ready | ThreadState::Running | ThreadState::Idle => Ok(ReadyOutcome {
                changed: false,
                target_cpu: cpu,
                should_preempt: false,
            }),
            ThreadState::Migrating => Err(Error::MigrationInProgress),
            ThreadState::Terminated => Err(Error::TerminatedThread),
        }
    }

    pub fn make_ready_from_wait(&mut self, id: ThreadId) -> Result<ReadyOutcome, Error> {
        let thread = self.thread(id)?;
        if thread.state() != ThreadState::Blocked
            || thread.queue_links().membership != QueueMembership::None
        {
            return Err(Error::QueueCorrupted);
        }
        let cpu = thread.cpu_index();
        self.enqueue_ready(id)?;
        Ok(ReadyOutcome {
            changed: true,
            target_cpu: cpu,
            should_preempt: self.ready_thread_preempts(id)?,
        })
    }

    pub fn migrate_thread(
        &mut self,
        id: ThreadId,
        target: CpuIndex,
    ) -> Result<MigrationOutcome, Error> {
        self.schedulable_cpu_slot(target)?;
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self
                .with_cpu_schedule_stored(cpu, |scheduler| scheduler.migrate_thread(id, target));
        }
        let thread = self.thread(id)?;
        if !thread.can_run_on(target) {
            return Err(Error::CpuNotAllowed);
        }
        let plan = MigrationRequest {
            target,
            affinity: thread.affinity(),
        };
        self.request_migration(id, plan)
    }

    pub fn set_thread_affinity(
        &mut self,
        id: ThreadId,
        affinity: CpuMask,
    ) -> Result<MigrationOutcome, Error> {
        if affinity.is_empty() {
            return Err(Error::EmptyCpuAffinity);
        }
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.set_thread_affinity(id, affinity)
            });
        }
        let assigned = self.thread(id)?.cpu_index();
        let target = if affinity.contains(assigned) {
            assigned
        } else {
            self.select_cpu(assigned, affinity)?
        };
        self.request_migration(id, MigrationRequest { target, affinity })
    }

    /// Changes placement only after proving that no CPU can still use stale context.
    fn request_migration(
        &mut self,
        id: ThreadId,
        plan: MigrationRequest,
    ) -> Result<MigrationOutcome, Error> {
        let thread = self.thread(id)?;
        if !matches!(
            thread.execution_kind(),
            ExecutionKind::Kernel | ExecutionKind::User
        ) || thread.placement_policy() != PlacementPolicy::Movable
        {
            return Err(Error::MigrationUnsupported);
        }
        if !plan.affinity.contains(plan.target) || !self.cpu_is_schedulable(plan.target) {
            return Err(Error::NoRegisteredCpuInAffinity);
        }
        if !thread.wait_record().permits_assignment(plan.target) {
            return Err(Error::MigrationBlockedByCpuLocalWait);
        }
        if thread.state() == ThreadState::Terminated {
            return Err(Error::TerminatedThread);
        }

        let state = self.thread(id)?.state();
        let source = self.thread(id)?.cpu_index();
        if let Some(existing) = self.thread(id)?.pending_migration() {
            if existing != plan {
                return Err(Error::MigrationInProgress);
            }
            return Ok(MigrationOutcome {
                status: MigrationStatus::Pending,
                source_reschedule: (state == ThreadState::Running).then_some(source),
                target_ready: None,
            });
        }
        // An affinity-only update retains assignment and cannot expose the
        // outgoing context on another CPU. Applying it in place also preserves
        // the Thread's exact ready-queue position.
        if source == plan.target {
            if !self.thread_mut(id)?.replace_affinity(plan.affinity) {
                return Err(Error::InvalidThreadState);
            }
            return Ok(MigrationOutcome {
                status: MigrationStatus::Completed,
                source_reschedule: None,
                target_ready: None,
            });
        }

        // The scheduler transaction precedes the assembly context save. Any
        // state reached through `switching_from` is therefore still source-CPU
        // owned even if a concurrent wake has already made it Ready. Retain the
        // request on that exact Thread for its incoming tail to consume.
        if self
            .switching_from(source)?
            .is_some_and(|switching| switching.thread == id)
        {
            if !self.thread_mut(id)?.request_migration(plan) {
                return Err(Error::MigrationInProgress);
            }
            return Ok(MigrationOutcome {
                status: MigrationStatus::Pending,
                source_reschedule: None,
                target_ready: None,
            });
        }

        match state {
            ThreadState::Dormant | ThreadState::Blocked => {
                if state == ThreadState::Blocked
                    && !matches!(
                        self.thread(id)?.queue_links().membership,
                        QueueMembership::Waiting { .. }
                    )
                {
                    return Err(Error::QueueCorrupted);
                }
                if !self
                    .thread_mut(id)?
                    .reassign_stopped_with_affinity(plan.target, plan.affinity)
                {
                    return Err(Error::InvalidThreadState);
                }
                Ok(MigrationOutcome {
                    status: MigrationStatus::Completed,
                    source_reschedule: None,
                    target_ready: None,
                })
            }
            ThreadState::Ready => {
                let ready = self.move_ready_thread(id, plan)?;
                Ok(MigrationOutcome {
                    status: MigrationStatus::Completed,
                    source_reschedule: None,
                    target_ready: Some(ready),
                })
            }
            ThreadState::Running => {
                let cpu_slot = self.cpu_slot(source)?;
                if self.current_thread(cpu_slot)? != id {
                    return Err(Error::InvalidThreadState);
                }
                if !self.thread_mut(id)?.request_migration(plan) {
                    return Err(Error::MigrationInProgress);
                }
                Ok(MigrationOutcome {
                    status: MigrationStatus::Pending,
                    source_reschedule: Some(source),
                    target_ready: None,
                })
            }
            ThreadState::Migrating => Err(Error::MigrationInProgress),
            ThreadState::Idle => Err(Error::MigrationUnsupported),
            ThreadState::Terminated => Err(Error::TerminatedThread),
        }
    }

    /// Selects work made eligible by a class preemption or Fair slice expiry.
    ///
    /// A preempted FIFO thread is returned to the head of its own priority
    /// queue. Equal-priority peers therefore cannot overtake it merely because
    /// a higher-priority thread ran temporarily.
    pub fn prepare_preemption(
        &mut self,
        cpu: CpuIndex,
    ) -> Result<Option<PreparedContextSwitch>, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self
                .with_cpu_schedule_stored(owner, |scheduler| scheduler.prepare_preemption(cpu));
        }
        if !super::super::preempt::pending(cpu)? {
            return Ok(None);
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.current_thread(cpu_slot)?;
        if self.thread(current)?.pending_migration().is_some() {
            if !super::super::preempt::take_pending_locked(cpu)? {
                return Err(Error::PreemptionInvariant);
            }
            return self.prepare_running_migration(cpu_slot, current).map(Some);
        }
        let candidate = self.peek_ready(cpu_slot)?;
        let Some(candidate) = candidate else {
            let _ = super::super::preempt::take_pending_locked(cpu)?;
            if self.thread(current)?.state() == ThreadState::Running {
                if self.thread(current)?.fair_slice_expired() {
                    self.thread_mut(current)?
                        .replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                }
                // Deferred placement orders current only against peers present
                // at the priority-change decision. If none remains eligible,
                // future peers must arrive behind the still-running current.
                self.thread_mut(current)?.set_deferred_fifo_placement(None);
            }
            return Ok(None);
        };
        let enqueue_front = match self.thread(current)?.state() {
            ThreadState::Idle => None,
            ThreadState::Running => {
                let current_policy = self.thread(current)?.scheduling_policy();
                let deferred = self.thread(current)?.deferred_fifo_placement();
                let fair_rotation = current_policy == SchedulingPolicy::Fair
                    && candidate.policy == SchedulingPolicy::Fair
                    && self.thread(current)?.fair_slice_expired();
                let fifo_deferred_rotation = matches!(
                    (current_policy, candidate.policy, deferred),
                    (
                        SchedulingPolicy::Fifo { priority: current },
                        SchedulingPolicy::Fifo { priority: ready },
                        Some(DeferredFifoPlacement::Tail),
                    ) if current == ready
                );
                if !current_policy.is_preempted_by(candidate.policy)
                    && !fair_rotation
                    && !fifo_deferred_rotation
                {
                    let _ = super::super::preempt::take_pending_locked(cpu)?;
                    self.thread_mut(current)?.set_deferred_fifo_placement(None);
                    return Ok(None);
                }
                if fair_rotation {
                    self.thread_mut(current)?
                        .replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                    Some(false)
                } else {
                    Some(deferred != Some(DeferredFifoPlacement::Tail))
                }
            }
            _ => return Err(Error::InvalidThreadState),
        };
        if !super::super::preempt::take_pending_locked(cpu)? {
            return Ok(None);
        }
        if let Some(enqueue_front) = enqueue_front {
            self.thread_mut(current)?.set_deferred_fifo_placement(None);
            if enqueue_front {
                self.enqueue_ready_front(current)?;
            } else {
                self.enqueue_ready(current)?;
            }
        }
        let next = self
            .dequeue_ready(cpu_slot)?
            .ok_or(Error::CurrentThreadMissing)?;
        if next != candidate.id {
            return Err(Error::QueueCorrupted);
        }
        self.prepare_switch(cpu_slot, current, next).map(Some)
    }

    pub fn prepare_yield(&mut self, cpu: CpuIndex) -> Result<Option<PreparedContextSwitch>, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| scheduler.prepare_yield(cpu));
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.current_thread(cpu_slot)?;
        // A cooperative yield is a complete scheduling observation. Consume
        // its request epoch while the scheduler lock also serializes queue and
        // migration state. A publisher arriving after this point creates a new
        // epoch and must notify again, closing repeated idle-wakeup cycles on
        // architectures without IRQ-tail preemption.
        let _ = super::super::preempt::take_pending_locked(cpu)?;
        if self.thread(current)?.pending_migration().is_some() {
            return self.prepare_running_migration(cpu_slot, current).map(Some);
        }
        match self.thread(current)?.state() {
            ThreadState::Running => {
                let candidate = self.peek_ready(cpu_slot)?;
                let Some(candidate) = candidate else {
                    return Ok(None);
                };
                let current_policy = self.thread(current)?.scheduling_policy();
                let can_yield_to = match (current_policy, candidate.policy) {
                    (SchedulingPolicy::Fair, _) => true,
                    (
                        SchedulingPolicy::Fifo { priority: current },
                        SchedulingPolicy::Fifo { priority: ready },
                    ) => ready <= current,
                    (SchedulingPolicy::Fifo { .. }, SchedulingPolicy::Fair) => false,
                    (SchedulingPolicy::Idle, _) | (_, SchedulingPolicy::Idle) => false,
                };
                if !can_yield_to {
                    return Ok(None);
                }
                if current_policy == SchedulingPolicy::Fair {
                    self.thread_mut(current)?
                        .replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                }
                self.enqueue_ready(current)?;
                let next = self
                    .dequeue_ready(cpu_slot)?
                    .ok_or(Error::CurrentThreadMissing)?;
                if next != candidate.id {
                    return Err(Error::QueueCorrupted);
                }
                return self.prepare_switch(cpu_slot, current, next).map(Some);
            }
            ThreadState::Idle => {}
            _ => return Err(Error::InvalidThreadState),
        }
        let Some(next) = self.dequeue_ready(cpu_slot)? else {
            return Ok(None);
        };
        self.prepare_switch(cpu_slot, current, next).map(Some)
    }

    pub fn arm_wait(&mut self, cpu: CpuIndex, mobility: WaitMobility) -> Result<WaitTicket, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self
                .with_cpu_schedule_stored(owner, |scheduler| scheduler.arm_wait(cpu, mobility));
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.current_thread(cpu_slot)?;
        let thread = self.thread(current)?;
        if thread.state() != ThreadState::Running {
            return Err(Error::CannotBlockIdle);
        }
        if mobility == WaitMobility::CpuLocal && thread.pending_migration().is_some() {
            return Err(Error::MigrationInProgress);
        }
        self.thread_mut(current)?
            .with_wait_record(|wait| wait.arm(current, mobility, cpu))
            .map_err(Error::from)
    }

    pub fn finish_unqueued_wait(
        &mut self,
        cpu: CpuIndex,
        ticket: WaitTicket,
    ) -> Result<Option<WaitOutcome>, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| {
                scheduler.finish_unqueued_wait(cpu, ticket)
            });
        }
        if current != ticket.thread() {
            return Err(Error::InvalidWaitRegistration);
        }
        self.thread_mut(current)?
            .with_wait_record(|wait| wait.finish_unqueued(ticket))
            .map_err(Error::from)
    }

    pub fn finish_completed_wait(
        &mut self,
        cpu: CpuIndex,
        ticket: WaitTicket,
    ) -> Result<WaitOutcome, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| {
                scheduler.finish_completed_wait(cpu, ticket)
            });
        }
        if current != ticket.thread() {
            return Err(Error::InvalidWaitRegistration);
        }
        self.thread_mut(current)?
            .with_wait_record(|wait| wait.finish_completed(ticket))
            .map_err(Error::from)
    }

    pub fn prepare_registered_park(
        &mut self,
        cpu: CpuIndex,
        wait_queue: &WaitQueue,
        ticket: WaitTicket,
    ) -> Result<PreparedWait, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| {
                scheduler.prepare_registered_park(cpu, wait_queue, ticket)
            });
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.current_thread(cpu_slot)?;
        if current != ticket.thread() {
            return Err(Error::InvalidWaitRegistration);
        }
        if self.thread(current)?.state() != ThreadState::Running {
            return Err(Error::CannotBlockIdle);
        }
        match self
            .thread(current)?
            .wait_record()
            .pending_resolution(ticket)?
        {
            PendingResolution::Armed => {}
            PendingResolution::AlreadyCompleted => {
                let outcome = self
                    .thread_mut(current)?
                    .with_wait_record(|wait| wait.finish_unqueued(ticket))?
                    .ok_or(Error::InvalidWaitRegistration)?;
                return Ok(PreparedWait::Completed(outcome));
            }
            PendingResolution::Queued { .. } | PendingResolution::Stale => {
                return Err(Error::InvalidWaitRegistration);
            }
        }
        if self.switching_from(cpu_slot)?.is_some() {
            return Err(Error::ThreadTransitionInProgress);
        }
        self.enqueue_waiter(wait_queue, current, ticket)?;
        let ready = match self.dequeue_ready(cpu_slot) {
            Ok(ready) => ready,
            Err(error) => scheduler_invariant(error),
        };
        let next = match ready.or(self.idle_thread(cpu_slot)?) {
            Some(id) => id,
            None => return self.rollback_failed_park(wait_queue, current, ticket),
        };
        if next == current {
            return self.rollback_failed_park(wait_queue, current, ticket);
        }
        let switch = match self.prepare_switch(cpu_slot, current, next) {
            Ok(switch) => switch,
            Err(error) => scheduler_invariant(error),
        };
        Ok(PreparedWait::Park { switch, ticket })
    }

    pub fn resolve_wait(
        &mut self,
        ticket: WaitTicket,
        outcome: WaitOutcome,
    ) -> Result<ResolvedWait, Error> {
        self.resolve_wait_with(ticket, outcome, || {})
    }

    /// Resolves one exact wait generation and commits resolver-owned state.
    ///
    /// `on_commit` runs only for the winning resolver, after the wait record
    /// and any queue membership have been completed but before a blocked
    /// Thread is published Ready. The callback must be bounded, infallible,
    /// allocation-free, and must not re-enter the scheduler.
    pub fn resolve_wait_with(
        &mut self,
        ticket: WaitTicket,
        outcome: WaitOutcome,
        on_commit: impl FnOnce(),
    ) -> Result<ResolvedWait, Error> {
        self.resolve_wait_with_preemption(ticket, outcome, WakePreemption::Policy, on_commit)
    }

    /// Resolves one wait with an optional equal-Fair scheduling boundary.
    ///
    /// A Fair boundary expires the running Fair entity only after a blocked
    /// Fair waiter becomes Ready. Real-time class ordering remains entirely
    /// governed by [`SchedulingPolicy`].
    pub fn resolve_wait_with_preemption(
        &mut self,
        ticket: WaitTicket,
        outcome: WaitOutcome,
        preemption: WakePreemption,
        on_commit: impl FnOnce(),
    ) -> Result<ResolvedWait, Error> {
        if let Ok(Some(cpu)) = self.cpu_lock_required_for(ticket.thread()) {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.resolve_wait_with_preemption(ticket, outcome, preemption, on_commit)
            });
        }
        let pending = match self.thread(ticket.thread()) {
            Ok(thread) => thread.wait_record().pending_resolution(ticket)?,
            Err(Error::ThreadNotFound) => {
                return Ok(ResolvedWait {
                    won: false,
                    ready: None,
                });
            }
            Err(error) => return Err(error),
        };
        match pending {
            PendingResolution::Stale | PendingResolution::AlreadyCompleted => Ok(ResolvedWait {
                won: false,
                ready: None,
            }),
            PendingResolution::Armed => {
                let mut thread = match self.thread_mut(ticket.thread()) {
                    Ok(thread) => thread,
                    Err(error) => scheduler_invariant(error),
                };
                let completion = thread.with_wait_record(|wait| wait.complete(ticket, outcome));
                if completion.is_err() {
                    scheduler_invariant(Error::InvalidWaitRegistration);
                }
                on_commit();
                Ok(ResolvedWait {
                    won: true,
                    ready: None,
                })
            }
            PendingResolution::Queued { queue } => {
                // SAFETY: a queued wait owns a live reference to its WaitQueue
                // until the same scheduler-lock transaction unlinks it.
                let wait_queue =
                    unsafe { &*core::ptr::with_exposed_provenance::<WaitQueue>(queue) };
                if let Err(error) = self.remove_waiter(wait_queue, ticket.thread()) {
                    scheduler_invariant(error);
                }
                let mut thread = match self.thread_mut(ticket.thread()) {
                    Ok(thread) => thread,
                    Err(error) => scheduler_invariant(error),
                };
                let completion = thread.with_wait_record(|wait| wait.complete(ticket, outcome));
                if completion.is_err() {
                    scheduler_invariant(Error::InvalidWaitRegistration);
                }
                on_commit();
                let mut ready = match self.make_ready_from_wait(ticket.thread()) {
                    Ok(ready) => ready,
                    Err(error) => scheduler_invariant(error),
                };
                if preemption == WakePreemption::FairBoundary && !ready.should_preempt {
                    ready.should_preempt =
                        match self.expire_current_for_fair_wakeup(ticket.thread()) {
                            Ok(should_preempt) => should_preempt,
                            Err(error) => scheduler_invariant(error),
                        };
                }
                Ok(ResolvedWait {
                    won: true,
                    ready: Some(ready),
                })
            }
        }
    }

    pub fn cancel_waiter(
        &mut self,
        wait_queue: &WaitQueue,
        id: ThreadId,
    ) -> Result<ResolvedWait, Error> {
        if let Ok(Some(cpu)) = self.cpu_lock_required_for(id) {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.cancel_waiter(wait_queue, id)
            });
        }
        let thread = match self.thread(id) {
            Ok(thread) => thread,
            Err(Error::ThreadNotFound) => {
                return Ok(ResolvedWait {
                    won: false,
                    ready: None,
                });
            }
            Err(error) => return Err(error),
        };
        let Some(ticket) = thread
            .wait_record()
            .queued_ticket(id, wait_queue.identity())
        else {
            if thread.queue_links().membership
                == (QueueMembership::Waiting {
                    queue: wait_queue.identity(),
                })
            {
                return Err(Error::QueueCorrupted);
            }
            return Ok(ResolvedWait {
                won: false,
                ready: None,
            });
        };
        self.resolve_wait(ticket, WaitOutcome::Cancelled)
    }

    pub fn notify_one_with(
        &mut self,
        wait_queue: &WaitQueue,
        before_ready: impl FnOnce(ThreadId),
    ) -> Result<Option<(ThreadId, ReadyOutcome)>, Error> {
        // SAFETY: all WaitQueue state is serialized by this scheduler lock.
        let queue = unsafe { &mut *wait_queue.state_pointer() };
        let Some(id) = queue.head else {
            if queue.len == 0 && queue.tail.is_none() {
                return Ok(None);
            }
            return Err(Error::QueueCorrupted);
        };
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.notify_one_with(wait_queue, before_ready)
            });
        }
        let ticket = self
            .thread(id)?
            .wait_record()
            .queued_ticket(id, wait_queue.identity())
            .ok_or(Error::QueueCorrupted)?;
        let popped = Self::queue_pop(
            &mut self.registry,
            queue,
            QueueMembership::Waiting {
                queue: wait_queue.identity(),
            },
        );
        match popped {
            Ok(Some(popped)) if popped == id => {}
            Ok(_) => scheduler_invariant(Error::QueueCorrupted),
            Err(error) => scheduler_invariant(error),
        }
        let mut thread = match self.thread_mut(id) {
            Ok(thread) => thread,
            Err(error) => scheduler_invariant(error),
        };
        let completion =
            thread.with_wait_record(|wait| wait.complete(ticket, WaitOutcome::Notified));
        if completion.is_err() {
            scheduler_invariant(Error::InvalidWaitRegistration);
        }
        before_ready(id);
        let ready = match self.make_ready_from_wait(id) {
            Ok(ready) => ready,
            Err(error) => scheduler_invariant(error),
        };
        Ok(Some((id, ready)))
    }

    pub fn prepare_exit(&mut self, cpu: CpuIndex) -> Result<PreparedContextSwitch, Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| scheduler.prepare_exit(cpu));
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.current_thread(cpu_slot)?;
        if !self.thread(current)?.wait_record().is_idle() {
            return Err(Error::InvalidWaitRegistration);
        }
        // Validate a distinct successor before publishing termination. Once
        // the current Thread enters the terminated queue, every later failure
        // is an internal scheduler invariant and cannot be rolled back.
        let successor = self
            .peek_ready(cpu_slot)?
            .map(|ready| ready.id)
            .or(self.idle_thread(cpu_slot)?)
            .ok_or(Error::CurrentThreadMissing)?;
        if successor == current {
            return Err(Error::CurrentThreadMissing);
        }
        Self::queue_push(
            &mut self.registry,
            &mut self.terminated,
            current,
            QueueMembership::Terminated,
        )?;
        self.thread_mut(current)?.set_state(ThreadState::Terminated);
        let next = self
            .dequeue_ready(cpu_slot)?
            .or(self.idle_thread(cpu_slot)?)
            .ok_or(Error::CurrentThreadMissing)?;
        if next != successor {
            scheduler_invariant(Error::QueueCorrupted);
        }
        self.prepare_switch(cpu_slot, current, next)
    }

    pub fn request_user_stop(
        &mut self,
        id: ThreadId,
        reason: crate::kernel::process::TerminalReason,
    ) -> Result<Option<CpuIndex>, Error> {
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.request_user_stop(id, reason)
            });
        }
        self.with_thread(id, |thread| {
            let object = thread.user_thread().ok_or(Error::InvalidThreadState)?;
            object.request_stop(reason);
            Ok::<_, Error>(())
        })??;
        // `resolve_wait` may publish a blocked Thread directly into a CPU
        // ready domain. Capture every coordinator decision input before that
        // linear ownership transfer; no stored-schedule observation is valid
        // after a queued resolution returns `ready`.
        let (state, cpu, ticket) = {
            let thread = self.thread(id)?;
            (
                thread.state(),
                thread.cpu_index(),
                thread.wait_record().current_ticket(id),
            )
        };
        let wait_resolution = match ticket {
            Some(ticket) => Some(self.resolve_wait(ticket, WaitOutcome::Cancelled)?),
            None => None,
        };
        match state {
            ThreadState::Running => Ok(Some(cpu)),
            ThreadState::Dormant => {
                self.enqueue_terminated(id)?;
                Ok(Some(cpu))
            }
            ThreadState::Ready => {
                // The suspended continuation may own arbitrary RAII state.
                // Keep it queued so the fixed user runner can unwind normally.
                Ok(Some(cpu))
            }
            ThreadState::Blocked => {
                let resolved = wait_resolution.ok_or(Error::InvalidWaitRegistration)?;
                let ready = resolved.ready.ok_or(Error::InvalidWaitRegistration)?;
                Ok(Some(ready.target_cpu))
            }
            ThreadState::Migrating => {
                // Source context is still owned until switch-tail. The durable
                // stop state follows the Thread to its target continuation.
                Ok(Some(cpu))
            }
            ThreadState::Terminated => Ok(None),
            ThreadState::Idle => Err(Error::InvalidThreadState),
        }
    }

    fn enqueue_terminated(&mut self, id: ThreadId) -> Result<(), Error> {
        if self.thread(id)?.queue_links().membership != QueueMembership::None {
            return Err(Error::QueueCorrupted);
        }
        Self::queue_push(
            &mut self.registry,
            &mut self.terminated,
            id,
            QueueMembership::Terminated,
        )?;
        self.thread_mut(id)?.set_state(ThreadState::Terminated);
        Ok(())
    }

    pub fn set_fifo_policy(
        &mut self,
        id: ThreadId,
        priority: ThreadPriority,
    ) -> Result<Option<CpuIndex>, Error> {
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.set_fifo_policy(id, priority)
            });
        }
        let state = self.thread(id)?.state();
        if state == ThreadState::Terminated {
            return Err(Error::TerminatedThread);
        }
        let links = self.thread(id)?.queue_links();
        match links.membership {
            QueueMembership::ReadyRealTime { cpu, priority: old } => {
                if old == priority.get() {
                    return Ok(None);
                }
                self.remove_ready(id, cpu, links.membership)?;
                if !self
                    .thread_mut(id)?
                    .set_scheduling_policy(SchedulingPolicy::fifo(priority))
                {
                    return Err(Error::InvalidThreadState);
                }
                if priority.get() < old {
                    self.enqueue_ready(id)?;
                } else {
                    self.enqueue_ready_front(id)?;
                }
                Ok(self.ready_thread_preempts(id)?.then_some(cpu))
            }
            QueueMembership::ReadyFair { cpu } => {
                self.remove_ready(id, cpu, links.membership)?;
                if !self
                    .thread_mut(id)?
                    .set_scheduling_policy(SchedulingPolicy::fifo(priority))
                {
                    return Err(Error::InvalidThreadState);
                }
                self.enqueue_ready(id)?;
                Ok(self.ready_thread_preempts(id)?.then_some(cpu))
            }
            QueueMembership::None
            | QueueMembership::Waiting { .. }
            | QueueMembership::Terminated => {
                let previous_policy = self.thread(id)?.scheduling_policy();
                let old = previous_policy.priority();
                if previous_policy == SchedulingPolicy::fifo(priority) {
                    return Ok(None);
                }
                if !self
                    .thread_mut(id)?
                    .set_scheduling_policy(SchedulingPolicy::fifo(priority))
                {
                    return Err(Error::InvalidThreadState);
                }
                if state != ThreadState::Running {
                    return Ok(None);
                }
                self.thread_mut(id)?.set_deferred_fifo_placement(None);
                let cpu = self.thread(id)?.cpu_index();
                let ready = self.peek_ready(cpu)?;
                let should_reschedule = ready.is_some_and(|ready| match old {
                    Some(old) => matches!(
                        ready.policy,
                        SchedulingPolicy::Fifo { priority: ready }
                            if ready < priority || (priority < old && ready == priority)
                    ),
                    None => SchedulingPolicy::fifo(priority).is_preempted_by(ready.policy),
                });
                if should_reschedule {
                    if let Some(old) = old {
                        let placement = if priority < old {
                            DeferredFifoPlacement::Tail
                        } else {
                            DeferredFifoPlacement::Head
                        };
                        self.thread_mut(id)?
                            .set_deferred_fifo_placement(Some(placement));
                    }
                    Ok(Some(cpu))
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Moves a non-idle thread into the Fair scheduling class.
    ///
    /// Ready membership is transferred between class queues while the global
    /// scheduler lock is held. A running RT thread lowered to Fair requests a
    /// scheduling decision only when ready RT work now outranks it.
    pub fn set_fair_policy(&mut self, id: ThreadId) -> Result<Option<CpuIndex>, Error> {
        if let Some(cpu) = self.cpu_lock_required_for(id)? {
            return self.with_cpu_schedule_stored(cpu, |scheduler| scheduler.set_fair_policy(id));
        }
        let state = self.thread(id)?.state();
        if state == ThreadState::Terminated {
            return Err(Error::TerminatedThread);
        }
        if self.thread(id)?.scheduling_policy() == SchedulingPolicy::Fair {
            return Ok(None);
        }

        let membership = self.thread(id)?.queue_links().membership;
        if let QueueMembership::ReadyRealTime { cpu, .. } = membership {
            self.remove_ready(id, cpu, membership)?;
            if !self
                .thread_mut(id)?
                .set_scheduling_policy(SchedulingPolicy::fair())
            {
                return Err(Error::InvalidThreadState);
            }
            self.enqueue_ready(id)?;
            return Ok(self.ready_thread_preempts(id)?.then_some(cpu));
        }

        if !self
            .thread_mut(id)?
            .set_scheduling_policy(SchedulingPolicy::fair())
        {
            return Err(Error::InvalidThreadState);
        }
        if state != ThreadState::Running {
            return Ok(None);
        }

        let cpu = self.thread(id)?.cpu_index();
        let candidate = self.peek_ready(cpu)?;
        Ok(candidate
            .is_some_and(|ready| SchedulingPolicy::Fair.is_preempted_by(ready.policy))
            .then_some(cpu))
    }

    pub fn install_current_as_idle(&mut self, cpu: CpuIndex) -> Result<(usize, usize), Error> {
        let current = self.current_thread(cpu)?;
        if let Some(owner) = self.cpu_lock_required_for(current)? {
            return self.with_cpu_schedule_stored(owner, |scheduler| {
                scheduler.install_current_as_idle(cpu)
            });
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        if self.idle_thread(cpu_slot)?.is_some() {
            return Err(Error::IdleThreadAlreadyInstalled);
        }
        let current = self.current_thread(cpu_slot)?;
        if self.thread(current)?.state() != ThreadState::Running {
            return Err(Error::InvalidIdleTransition);
        }
        let mut thread = self.thread_mut(current)?;
        let stack = thread.ensure_kernel_stack()?;
        thread.become_idle();
        self.with_cpu_domain(cpu_slot, |_scheduler, local| {
            if local.idle.is_some() {
                return Err(Error::IdleThreadAlreadyInstalled);
            }
            local.idle = Some(current);
            Ok(())
        })?;
        self.schedulable_cpus = self.schedulable_cpus.with_cpu(cpu);
        Ok(stack)
    }

    /// Retires source-CPU context ownership from the incoming switch tail.
    pub fn complete_incoming_switch(
        &mut self,
        cpu: CpuIndex,
        ticket: u64,
    ) -> Result<CoordinatedTailCompletion, Error> {
        if self.active_domain.is_none() {
            return self.with_cpu_schedule_stored(cpu, |scheduler| {
                scheduler.complete_incoming_switch(cpu, ticket)
            });
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let switching = self.with_cpu_domain(cpu_slot, |_scheduler, local| {
            let switching = local.switching_from.ok_or(Error::PreemptionInvariant)?;
            if switching.generation != ticket {
                return Err(Error::PreemptionInvariant);
            }
            local.switching_from = None;
            Ok(switching)
        })?;
        let plan = self.thread_mut(switching.thread)?.take_migration_request();
        let state = self.thread(switching.thread)?.state();
        if !matches!(state, ThreadState::Ready | ThreadState::Idle) {
            let released = self
                .registry
                .with_thread_mut(switching.thread, |thread| thread.release_schedule(cpu))?;
            if !released {
                scheduler_invariant(Error::InvalidThreadState);
            }
        }
        let ready = match plan {
            Some(plan) => self.complete_migration(switching.thread, plan)?,
            None => None,
        };
        let retirement_published = self.queue_terminated_retirement(switching.thread)?;
        Ok(CoordinatedTailCompletion {
            ready,
            retirement_published,
        })
    }

    pub fn queue_terminated_retirement(&mut self, id: ThreadId) -> Result<bool, Error> {
        if self
            .registry
            .with_thread(id, Thread::schedule_owner_cpu)?
            .is_some()
        {
            return Ok(false);
        }
        let thread = self.thread(id)?;
        if thread.state() != ThreadState::Terminated {
            return Ok(false);
        }
        if thread.queue_links().membership != QueueMembership::Terminated {
            return Err(Error::QueueCorrupted);
        }
        Self::queue_remove(
            &mut self.registry,
            &mut self.terminated,
            id,
            QueueMembership::Terminated,
        )?;
        self.registry.begin_retirement(id)?;
        let retired = RetirementTicket {
            id,
            retirement: super::ResourceRetirement::begin(),
        };
        if self.retirements.push(retired).is_err() {
            // A registry slot remains Retiring for every queued element, so
            // fixed-capacity overflow is an internal accounting violation.
            crate::hal::cpu::halt();
        }
        Ok(true)
    }

    pub fn take_retirement(&mut self) -> Result<Option<RetiredThread>, Error> {
        let Some(ticket) = self.retirements.pop()? else {
            return Ok(None);
        };
        let thread = self.registry.take_retiring(ticket.id)?;
        Ok(Some(RetiredThread {
            id: ticket.id,
            thread,
            retirement: ticket.retirement,
        }))
    }

    pub fn complete_retirement(&mut self, id: ThreadId) -> Result<(), Error> {
        self.registry.complete_retirement(id)
    }

    pub const fn has_retirements(&self) -> bool {
        !self.retirements.is_empty()
    }

    pub fn statistics(&self) -> Statistics {
        let mut stats = Statistics::default();
        self.registry.for_each_thread(|thread| {
            stats.threads += 1;
            if thread.schedule_owner_cpu().is_some() {
                return;
            }
            match thread.scheduling_class() {
                SchedulingClass::RealTime => stats.real_time_class_threads += 1,
                SchedulingClass::Fair => stats.fair_class_threads += 1,
                SchedulingClass::Idle => stats.idle_class_threads += 1,
            }
            match thread.state() {
                ThreadState::Ready => stats.ready += 1,
                ThreadState::Running => stats.running += 1,
                ThreadState::Blocked => stats.blocked += 1,
                ThreadState::Migrating => stats.migrating += 1,
                ThreadState::Idle => stats.idle += 1,
                ThreadState::Dormant | ThreadState::Terminated => {}
            }
        });
        for index in 0..hyper::cpu::MAX_CPUS {
            let Some(cpu) = CpuIndex::new(index) else {
                crate::hal::cpu::halt();
            };
            CPU_SCHEDULERS[cpu].with(|slot| {
                let Some(local) = slot.as_mut() else {
                    return;
                };
                stats.context_switches = stats
                    .context_switches
                    .saturating_add(local.context_switches);
                let topology_ready = local.run_queue.len();
                let topology_real_time_ready = local.run_queue.real_time_len();
                let topology_fair_ready = local.run_queue.fair_len();
                let current = local.current;
                stats.per_cpu_ready[index] = topology_ready;
                let mut observed_ready = 0usize;
                let mut observed_real_time_ready = 0usize;
                let mut observed_fair_ready = 0usize;
                let threads = local.thread_authority();
                self.registry.for_each_thread(|thread| {
                    if thread.schedule_owner_cpu() != Some(cpu) {
                        return;
                    }
                    if threads
                        .with_thread(thread.id(), |_thread, schedule| {
                            let class = schedule.scheduling_class();
                            match class {
                                SchedulingClass::RealTime => stats.real_time_class_threads += 1,
                                SchedulingClass::Fair => stats.fair_class_threads += 1,
                                SchedulingClass::Idle => stats.idle_class_threads += 1,
                            }
                            match schedule.state {
                                ThreadState::Ready => {
                                    stats.ready += 1;
                                    observed_ready += 1;
                                    match class {
                                        SchedulingClass::RealTime => observed_real_time_ready += 1,
                                        SchedulingClass::Fair => observed_fair_ready += 1,
                                        SchedulingClass::Idle => crate::hal::cpu::halt(),
                                    }
                                }
                                ThreadState::Running => stats.running += 1,
                                ThreadState::Blocked => stats.blocked += 1,
                                ThreadState::Migrating => stats.migrating += 1,
                                ThreadState::Idle => stats.idle += 1,
                                ThreadState::Dormant | ThreadState::Terminated => {}
                            }
                        })
                        .is_err()
                    {
                        crate::hal::cpu::halt();
                    }
                });
                if observed_ready != topology_ready
                    || observed_real_time_ready != topology_real_time_ready
                    || observed_fair_ready != topology_fair_ready
                {
                    crate::hal::cpu::halt();
                }
                if threads.with_thread(current, |_thread, schedule| {
                    matches!(schedule.state, ThreadState::Running | ThreadState::Idle)
                }) != Ok(true)
                {
                    crate::hal::cpu::halt();
                }
            });
        }
        stats
    }

    #[cfg(feature = "kernel-self-test")]
    pub const fn registry_slot_count(&self) -> usize {
        self.registry.high_water()
    }

    pub fn with_thread<R>(
        &self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread) -> R,
    ) -> Result<R, Error> {
        self.registry.with_thread(id, operation)
    }

    pub(super) fn thread(&self, id: ThreadId) -> Result<ThreadObservation, Error> {
        self.registry.with_thread(id, |thread| {
            match thread.schedule_owner_cpu() {
                None => ThreadObservation::capture(thread),
                Some(cpu) if self.active_domain.is_some_and(|active| active.cpu == cpu) => {
                    // SAFETY: `active_domain` is installed only while
                    // `with_cpu_schedule_stored` holds this exact CPU lock.
                    unsafe {
                        thread.with_cpu_schedule(cpu, |schedule| {
                            ThreadObservation::capture_cpu(thread, cpu, schedule)
                        })
                    }
                    .unwrap_or_else(|| crate::hal::cpu::halt())
                }
                Some(cpu) => ThreadObservation {
                    schedule_owner: Some(cpu),
                    schedule: None,
                    execution: thread.execution_kind(),
                    context: thread.context_pointer(),
                    vcpu: thread.vcpu_execution_pointer(),
                    user: thread.user_execution_pointer(),
                    stack_bounds: thread.kernel_stack_bounds(),
                },
            }
        })
    }

    fn thread_mut(&mut self, id: ThreadId) -> Result<ThreadMutation<'_>, Error> {
        let owner = self.registry.with_thread(id, Thread::schedule_owner_cpu)?;
        let cpu = match (owner, self.active_domain) {
            (None, _) => None,
            (Some(cpu), Some(active)) if cpu == active.cpu => Some(cpu),
            (Some(_), _) => return Err(Error::InvalidThreadState),
        };
        Ok(ThreadMutation {
            registry: &mut self.registry,
            id,
            cpu,
        })
    }

    /// Runs one coordinator transaction while holding the matching CPU lock.
    ///
    /// Schedule storage remains in its stable Thread cell and its owner
    /// locator remains `Cpu(cpu)`: ready-to-current transitions therefore do
    /// not transfer or recreate schedule state. `active_domain` is private proof
    /// used only to route closure-bounded Thread access after revalidation.
    fn with_cpu_schedule_stored<R>(
        &mut self,
        cpu: CpuIndex,
        operation: impl FnOnce(&mut Self) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let result = self.with_cpu_domain(cpu, |scheduler, local| {
            let current = local.current;
            let current_owner = scheduler
                .registry
                .with_thread(current, Thread::schedule_owner_cpu);
            if current_owner != Ok(Some(cpu)) {
                crate::hal::cpu::halt();
            }
            operation(scheduler)
        });
        if let Some((id, target)) = self.deferred_ready_handoff.take() {
            if self.active_domain.is_some() {
                crate::hal::cpu::halt();
            }
            if let Err(error) = self.enqueue_ready(id) {
                scheduler_invariant(error);
            }
            if self.registry.with_thread(id, Thread::schedule_owner_cpu) != Ok(Some(target)) {
                crate::hal::cpu::halt();
            }
        }
        result
    }

    fn with_cpu_domain<R>(
        &mut self,
        cpu: CpuIndex,
        operation: impl FnOnce(&mut Self, &mut CpuScheduler) -> Result<R, Error>,
    ) -> Result<R, Error> {
        if let Some(active) = self.active_domain {
            if active.cpu != cpu {
                crate::hal::cpu::halt();
            }
            // SAFETY: the matching CPU lock remains held by the outer scope;
            // nested access is serialized by the exclusive Scheduler borrow.
            return operation(self, unsafe { &mut *active.local.as_ptr() });
        }
        CPU_SCHEDULERS[cpu].with(|slot| {
            let local = match slot.as_mut() {
                Some(local) => local,
                None => return Err(Error::CpuNotRegistered),
            };
            if local.index != cpu {
                crate::hal::cpu::halt();
            }
            let local = core::ptr::NonNull::from(&mut *local);
            self.active_domain = Some(ActiveCpuDomain { cpu, local });
            // SAFETY: this is the first and only reborrow after publishing
            // `local`. Nested accesses derive from the same pointer, and the
            // original lock-slot borrow is not used again. Kernel profiles
            // use panic=abort, so unwinding cannot strand active_domain.
            let result = operation(self, unsafe { &mut *local.as_ptr() });
            self.active_domain = None;
            result
        })
    }

    fn prepare_switch(
        &mut self,
        cpu_slot: CpuIndex,
        current: ThreadId,
        next: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        let target_cpu = cpu_slot;
        if self.switching_from(cpu_slot)?.is_some() {
            return Err(Error::ThreadTransitionInProgress);
        }
        if self.thread(next)?.state() == ThreadState::Ready
            && !self.thread_mut(next)?.mark_running_on(target_cpu)
        {
            return Err(Error::InvalidThreadState);
        }
        if self.thread(next)?.fair_slice_expired() {
            self.thread_mut(next)?
                .replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
        }
        let generation = self.with_cpu_domain(cpu_slot, |_scheduler, local| {
            if local.switching_from.is_some() || local.current != current {
                return Err(Error::ThreadTransitionInProgress);
            }
            let generation = local.next_switch_generation;
            local.next_switch_generation = generation
                .checked_add(1)
                .unwrap_or_else(|| crate::hal::cpu::halt());
            local.switching_from = Some(SwitchingContext {
                thread: current,
                generation,
                disposition: SwitchDisposition::Coordinated,
            });
            local.current = next;
            local.context_switches = local.context_switches.saturating_add(1);
            Ok(generation)
        })?;
        let previous = self.thread(current)?.context_pointer();
        let next = self.thread(next)?.context_pointer().cast_const();
        Ok(PreparedContextSwitch {
            previous,
            next,
            ticket: generation,
            armed: true,
        })
    }

    fn prepare_running_migration(
        &mut self,
        cpu_slot: CpuIndex,
        current: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        if self.thread(current)?.state() != ThreadState::Running
            || self.thread(current)?.pending_migration().is_none()
        {
            return Err(Error::InvalidThreadState);
        }
        let next = self
            .dequeue_ready(cpu_slot)?
            .or(self.idle_thread(cpu_slot)?)
            .ok_or(Error::CurrentThreadMissing)?;
        if next == current {
            return Err(Error::InvalidThreadState);
        }
        self.thread_mut(current)?.set_state(ThreadState::Migrating);
        self.prepare_switch(cpu_slot, current, next)
    }

    fn complete_migration(
        &mut self,
        id: ThreadId,
        plan: MigrationRequest,
    ) -> Result<Option<ReadyOutcome>, Error> {
        match self.thread(id)?.state() {
            ThreadState::Ready => self.move_ready_thread(id, plan).map(Some),
            ThreadState::Blocked => {
                if !matches!(
                    self.thread(id)?.queue_links().membership,
                    QueueMembership::Waiting { .. }
                ) {
                    return Err(Error::QueueCorrupted);
                }
                if !self
                    .thread_mut(id)?
                    .reassign_stopped_with_affinity(plan.target, plan.affinity)
                {
                    return Err(Error::InvalidThreadState);
                }
                Ok(None)
            }
            ThreadState::Migrating => {
                if self.thread(id)?.queue_links().membership != QueueMembership::None
                    || !self
                        .thread_mut(id)?
                        .reassign_stopped_with_affinity(plan.target, plan.affinity)
                {
                    return Err(Error::InvalidThreadState);
                }
                let should_preempt = self.enqueue_ready_or_defer(id, plan.target)?;
                Ok(Some(ReadyOutcome {
                    changed: true,
                    target_cpu: plan.target,
                    should_preempt,
                }))
            }
            // Exit won the race with a remote migration publication. The
            // Thread will be reclaimed after this same tail releases source
            // ownership, so no target assignment is published.
            ThreadState::Terminated => Ok(None),
            _ => Err(Error::InvalidThreadState),
        }
    }

    fn move_ready_thread(
        &mut self,
        id: ThreadId,
        plan: MigrationRequest,
    ) -> Result<ReadyOutcome, Error> {
        let source = self.thread(id)?.cpu_index();
        let old_affinity = self.thread(id)?.affinity();
        let membership = self.thread(id)?.queue_links().membership;
        if !matches!(
            membership,
            QueueMembership::ReadyRealTime { cpu, .. } | QueueMembership::ReadyFair { cpu }
                if cpu == source
        ) {
            return Err(Error::QueueCorrupted);
        }
        self.remove_ready(id, source, membership)?;
        if !self
            .registry
            .with_thread_mut(id, |thread| thread.release_schedule(source))?
        {
            scheduler_invariant(Error::InvalidThreadState);
        }
        if !self
            .thread_mut(id)?
            .reassign_stopped_with_affinity(plan.target, plan.affinity)
        {
            self.restore_ready_migration(id, source, old_affinity);
            return Err(Error::InvalidThreadState);
        }
        let should_preempt = self.enqueue_ready_or_defer(id, plan.target)?;
        Ok(ReadyOutcome {
            changed: true,
            target_cpu: plan.target,
            should_preempt,
        })
    }

    fn restore_ready_migration(&mut self, id: ThreadId, cpu: CpuIndex, affinity: CpuMask) {
        let restored = self
            .thread_mut(id)
            .and_then(|mut thread| {
                thread
                    .reassign_stopped_with_affinity(cpu, affinity)
                    .then_some(())
                    .ok_or(Error::InvalidThreadState)
            })
            .and_then(|()| self.enqueue_ready(id));
        if restored.is_err() {
            // The global scheduler lock is held and queue state is no longer
            // recoverable. Diagnostics or coordinated crash entry could
            // reacquire scheduler-adjacent locks, so fail closed in place.
            crate::hal::cpu::halt()
        }
    }

    fn enqueue_ready(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        let cpu = thread.cpu_index();
        if thread.scheduling_class() == SchedulingClass::Idle || !thread.can_run_on(cpu) {
            return Err(Error::InvalidThreadState);
        }
        self.with_cpu_domain(cpu, |scheduler, local| {
            let cpu_threads = local.table.cpu_authority(local.index, &mut local.authority);
            let coordinator = scheduler.registry.write_authority();
            let mut threads = queue::ReadyQueueAuthority::new(coordinator, cpu_threads);
            local.run_queue.enqueue(&mut threads, id, cpu)
        })
    }

    fn enqueue_ready_or_defer(&mut self, id: ThreadId, target: CpuIndex) -> Result<bool, Error> {
        if self
            .active_domain
            .is_some_and(|active| active.cpu != target)
        {
            if self.deferred_ready_handoff.replace((id, target)).is_some() {
                crate::hal::cpu::halt();
            }
            // Conservatively notify the target after the two-phase handoff.
            return Ok(true);
        }
        self.enqueue_ready(id)?;
        self.ready_thread_preempts(id)
    }

    fn enqueue_ready_front(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        let cpu = thread.cpu_index();
        if thread.scheduling_class() == SchedulingClass::Idle || !thread.can_run_on(cpu) {
            return Err(Error::InvalidThreadState);
        }
        self.with_cpu_domain(cpu, |scheduler, local| {
            let cpu_threads = local.table.cpu_authority(local.index, &mut local.authority);
            let coordinator = scheduler.registry.write_authority();
            let mut threads = queue::ReadyQueueAuthority::new(coordinator, cpu_threads);
            local.run_queue.enqueue_front(&mut threads, id, cpu)
        })
    }

    fn ready_thread_preempts(&mut self, id: ThreadId) -> Result<bool, Error> {
        let cpu = self
            .registry
            .with_thread(id, Thread::schedule_owner_cpu)?
            .ok_or(Error::InvalidThreadState)?;
        self.with_cpu_domain(cpu, |_scheduler, local| {
            let candidate_policy = {
                let threads = local.thread_authority();
                threads.with_thread(id, |_thread, schedule| schedule.scheduling)?
            };
            let current = local.current;
            let threads = local.thread_authority();
            threads.with_thread(current, |_thread, schedule| match schedule.state {
                ThreadState::Idle | ThreadState::Running => {
                    Ok(schedule.scheduling.is_preempted_by(candidate_policy))
                }
                _ => Err(Error::InvalidThreadState),
            })?
        })
    }

    /// Turns a latency-sensitive equal-Fair wakeup into a scheduling boundary.
    ///
    /// This is deliberately narrower than priority preemption: it neither
    /// displaces real-time work nor changes the woken entity's queue position.
    fn expire_current_for_fair_wakeup(&mut self, id: ThreadId) -> Result<bool, Error> {
        let cpu = self
            .registry
            .with_thread(id, Thread::schedule_owner_cpu)?
            .ok_or(Error::InvalidThreadState)?;
        self.with_cpu_domain(cpu, |_scheduler, local| {
            let candidate_is_fair = {
                let threads = local.thread_authority();
                threads.with_thread(id, |_thread, schedule| {
                    schedule.scheduling_class() == SchedulingClass::Fair
                })?
            };
            if !candidate_is_fair {
                return Ok(false);
            }
            let current = local.current;
            let mut threads = local.thread_authority();
            threads.with_thread_mut(current, |_thread, schedule| {
                if schedule.state != ThreadState::Running
                    || schedule.scheduling_class() != SchedulingClass::Fair
                {
                    return false;
                }
                schedule.expire_fair_slice();
                true
            })
        })
    }

    fn dequeue_ready(&mut self, cpu: CpuIndex) -> Result<Option<ThreadId>, Error> {
        self.with_cpu_domain(cpu, |scheduler, local| {
            let cpu_threads = local.table.cpu_authority(local.index, &mut local.authority);
            let coordinator = scheduler.registry.write_authority();
            let mut threads = queue::ReadyQueueAuthority::new(coordinator, cpu_threads);
            local.run_queue.dequeue(&mut threads, cpu)
        })
    }

    fn remove_ready(
        &mut self,
        id: ThreadId,
        cpu: CpuIndex,
        membership: QueueMembership,
    ) -> Result<(), Error> {
        self.with_cpu_domain(cpu, |scheduler, local| {
            let cpu_threads = local.table.cpu_authority(local.index, &mut local.authority);
            let coordinator = scheduler.registry.write_authority();
            let mut threads = queue::ReadyQueueAuthority::new(coordinator, cpu_threads);
            local.run_queue.remove(&mut threads, id, cpu, membership)
        })
    }

    fn enqueue_waiter(
        &mut self,
        wait_queue: &WaitQueue,
        id: ThreadId,
        ticket: WaitTicket,
    ) -> Result<(), Error> {
        let membership = QueueMembership::Waiting {
            queue: wait_queue.identity(),
        };
        // SAFETY: Every caller holds the global scheduler lock exclusively.
        let queue = unsafe { &mut *wait_queue.state_pointer() };
        self.thread_mut(id)?
            .with_wait_record(|wait| wait.queue(ticket, wait_queue.identity()))?;
        if let Err(error) = Self::queue_push(&mut self.registry, queue, id, membership) {
            scheduler_invariant(error);
        }
        match self.thread_mut(id) {
            Ok(mut thread) => thread.set_state(ThreadState::Blocked),
            Err(error) => scheduler_invariant(error),
        }
        Ok(())
    }

    fn remove_waiter(&mut self, wait_queue: &WaitQueue, id: ThreadId) -> Result<(), Error> {
        let membership = QueueMembership::Waiting {
            queue: wait_queue.identity(),
        };
        // SAFETY: Every caller holds the global scheduler lock exclusively.
        Self::queue_remove(
            &mut self.registry,
            unsafe { &mut *wait_queue.state_pointer() },
            id,
            membership,
        )
    }

    fn rollback_failed_park(
        &mut self,
        wait_queue: &WaitQueue,
        current: ThreadId,
        ticket: WaitTicket,
    ) -> Result<PreparedWait, Error> {
        if let Err(error) = self.remove_waiter(wait_queue, current) {
            scheduler_invariant(error);
        }
        let mut thread = match self.thread_mut(current) {
            Ok(thread) => thread,
            Err(error) => scheduler_invariant(error),
        };
        if thread
            .with_wait_record(|wait| wait.rollback_queued(ticket))
            .is_err()
        {
            scheduler_invariant(Error::InvalidWaitRegistration);
        }
        thread.set_state(ThreadState::Running);
        Err(Error::CurrentThreadMissing)
    }

    fn cpu_slot(&self, cpu: CpuIndex) -> Result<CpuIndex, Error> {
        if self.active_domain.is_some_and(|active| active.cpu == cpu)
            || CPU_SCHEDULERS[cpu].with(|slot| slot.is_some())
        {
            Ok(cpu)
        } else {
            Err(Error::CpuNotRegistered)
        }
    }

    fn schedulable_cpu_slot(&self, cpu: CpuIndex) -> Result<CpuIndex, Error> {
        if self.cpu_is_schedulable(cpu) {
            Ok(cpu)
        } else {
            Err(Error::CpuNotRegistered)
        }
    }

    fn cpu_is_schedulable(&self, cpu: CpuIndex) -> bool {
        self.schedulable_cpus.contains(cpu)
    }

    fn idle_thread(&self, cpu: CpuIndex) -> Result<Option<ThreadId>, Error> {
        if let Some(active) = self.active_domain
            && active.cpu == cpu
        {
            // SAFETY: active domain is bounded by the matching lock closure.
            return Ok(unsafe { active.local.as_ref() }.idle);
        }
        CPU_SCHEDULERS[cpu].with(|slot| {
            slot.as_ref()
                .map(|local| local.idle)
                .ok_or(Error::CpuNotRegistered)
        })
    }

    fn switching_from(&self, cpu: CpuIndex) -> Result<Option<SwitchingContext>, Error> {
        if let Some(active) = self.active_domain
            && active.cpu == cpu
        {
            // SAFETY: active domain is bounded by the matching lock closure.
            return Ok(unsafe { active.local.as_ref() }.switching_from);
        }
        CPU_SCHEDULERS[cpu].with(|slot| {
            slot.as_ref()
                .map(|local| local.switching_from)
                .ok_or(Error::CpuNotRegistered)
        })
    }

    fn peek_ready(&mut self, cpu: CpuIndex) -> Result<Option<queue::ReadyThread>, Error> {
        self.with_cpu_domain(cpu, |scheduler, local| {
            let cpu_threads = local.table.cpu_authority(local.index, &mut local.authority);
            let coordinator = scheduler.registry.write_authority();
            let threads = queue::ReadyQueueAuthority::new(coordinator, cpu_threads);
            local.run_queue.peek_next(&threads, cpu)
        })
    }

    fn queue_push(
        registry: &mut ThreadRegistry,
        target: &mut ThreadQueue,
        id: ThreadId,
        membership: QueueMembership,
    ) -> Result<(), Error> {
        let mut threads = queue::ControlQueueAuthority::new(registry.control_authority());
        queue::control_push(&mut threads, target, id, membership)
    }

    fn queue_pop(
        registry: &mut ThreadRegistry,
        target: &mut ThreadQueue,
        membership: QueueMembership,
    ) -> Result<Option<ThreadId>, Error> {
        let mut threads = queue::ControlQueueAuthority::new(registry.control_authority());
        queue::control_pop(&mut threads, target, membership)
    }

    fn queue_remove(
        registry: &mut ThreadRegistry,
        target: &mut ThreadQueue,
        id: ThreadId,
        membership: QueueMembership,
    ) -> Result<(), Error> {
        let mut threads = queue::ControlQueueAuthority::new(registry.control_authority());
        queue::control_remove(&mut threads, target, id, membership)
    }

    fn reserve_thread_slot(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        self.registry.reserve(cpu)
    }
}

impl From<WaitRecordError> for Error {
    fn from(error: WaitRecordError) -> Self {
        match error {
            WaitRecordError::GenerationExhausted => Self::WaitGenerationExhausted,
            WaitRecordError::RegistrationMismatch | WaitRecordError::InvalidPhase => {
                Self::InvalidWaitRegistration
            }
        }
    }
}

fn scheduler_invariant(_error: Error) -> ! {
    // Every caller holds the global scheduler lock after a committed queue or
    // wait-record mutation. Diagnostics could deadlock and returning would
    // expose inconsistent shared state, so retain the lock and fail closed.
    crate::hal::cpu::halt()
}
