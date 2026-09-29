// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! CPU-local scheduling, switch completion, and current-thread observations.
//!
//! These paths hold only the matching CPU lock. Transitions that need registry
//! coordination return a typed fallback before entering the coordinator.

use hyper::cpu::CpuIndex;

use super::super::queue::{self, CpuRunQueue};
use super::super::registry::{
    CpuScheduleAuthorityToken, CpuThreadTableAuthority, ThreadTableCapability,
};
use super::super::switch_handoff::{SwitchDisposition, SwitchHandoff};
use super::super::{
    CrashTaskSnapshot, CurrentUser, CurrentVcpu, Error, FAIR_QUANTUM_TICKS, account_cpu_time,
};
use super::{
    CPU_SCHEDULERS, CpuScheduler, LocalScheduleAttempt, LocalTailCompletion, PreparedContextSwitch,
    scheduler_invariant,
};
use crate::kernel::task::policy::SchedulingPolicy;
use crate::kernel::task::preempt;
use crate::kernel::task::thread::{DeferredFifoPlacement, ThreadId, ThreadState};

/// Charges one CPU's running Fair entity without taking the transition lock.
pub(in crate::kernel::task::scheduler) fn account_tick(
    cpu: CpuIndex,
    elapsed: u64,
) -> Result<bool, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let current = local.current;
        let has_fair_ready = local.run_queue.has_fair_threads();
        let mut threads = local.thread_authority();
        threads.with_thread_mut(current, |thread, schedule| match schedule.state {
            ThreadState::Idle => {
                thread.account_runtime_ticks(elapsed);
                account_cpu_time(cpu, thread.role(), elapsed);
                Ok(false)
            }
            ThreadState::Running => {
                thread.account_runtime_ticks(elapsed);
                account_cpu_time(cpu, thread.role(), elapsed);
                if !schedule.account_fair_ticks(elapsed, FAIR_QUANTUM_TICKS) {
                    return Ok(false);
                }
                if has_fair_ready {
                    Ok(true)
                } else {
                    schedule.replenish_fair_slice(FAIR_QUANTUM_TICKS);
                    Ok(false)
                }
            }
            _ => Err(Error::InvalidThreadState),
        })?
    })
}

/// Attempts a cooperative scheduling decision using only the current CPU lock.
pub(in crate::kernel::task::scheduler) fn prepare_local_yield(
    cpu: CpuIndex,
) -> Result<LocalScheduleAttempt, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        if local.handoff.current().is_some() {
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
        let _ = preempt::take_pending_locked(cpu)?;
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
                        schedule.replenish_fair_slice(FAIR_QUANTUM_TICKS)
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
pub(in crate::kernel::task::scheduler) fn prepare_local_preemption(
    cpu: CpuIndex,
) -> Result<LocalScheduleAttempt, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        if local.handoff.current().is_some() {
            return Err(Error::ThreadTransitionInProgress);
        }
        if !preempt::pending(cpu)? {
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
            let _ = preempt::take_pending_locked(cpu)?;
            if state == ThreadState::Running {
                let mut threads = local.thread_authority();
                threads.with_thread_mut(current, |_thread, schedule| {
                    if schedule.fair_slice_expired() {
                        schedule.replenish_fair_slice(FAIR_QUANTUM_TICKS);
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
                    let _ = preempt::take_pending_locked(cpu)?;
                    let mut threads = local.thread_authority();
                    threads.with_thread_mut(current, |_thread, schedule| {
                        schedule.deferred_fifo_placement = None
                    })?;
                    return Ok(LocalScheduleAttempt::Complete(None));
                }
                if fair_rotation {
                    let mut threads = local.thread_authority();
                    threads.with_thread_mut(current, |_thread, schedule| {
                        schedule.replenish_fair_slice(FAIR_QUANTUM_TICKS)
                    })?;
                    Some(false)
                } else {
                    Some(deferred != Some(DeferredFifoPlacement::Tail))
                }
            }
            _ => return Err(Error::InvalidThreadState),
        };
        if !preempt::take_pending_locked(cpu)? {
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
pub(in crate::kernel::task::scheduler) fn complete_local_switch_tail(
    cpu: CpuIndex,
    ticket: u64,
) -> Result<LocalTailCompletion, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let switching = local
            .handoff
            .for_ticket(ticket)
            .ok_or(Error::PreemptionInvariant)?;
        if switching.disposition != SwitchDisposition::Local {
            return Ok(LocalTailCompletion::NeedsCoordinator);
        }
        let (state, pending) = {
            let threads = local.thread_authority();
            threads.with_thread(switching.thread, |_thread, schedule| {
                (schedule.state, schedule.pending_migration)
            })?
        };
        if pending.is_some()
            || !matches!(
                state,
                ThreadState::Ready | ThreadState::Idle | ThreadState::Blocked
            )
        {
            return Ok(LocalTailCompletion::NeedsCoordinator);
        }
        local
            .handoff
            .complete(ticket)
            .ok_or(Error::PreemptionInvariant)?;
        Ok(LocalTailCompletion::Complete)
    })
}

/// Best-effort crash observation taken under one non-blocking CPU lock.
///
/// Current identity, schedule state, and immutable resource metadata come
/// from one coherent CPU-domain snapshot. Failure to acquire that lock is
/// reported as absence; crash handling must never wait for scheduler state.
pub(in crate::kernel::task::scheduler) fn try_cpu_snapshot(
    cpu: CpuIndex,
) -> Option<CrashTaskSnapshot> {
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

pub(in crate::kernel::task::scheduler) fn local_current_thread(
    cpu: CpuIndex,
) -> Result<ThreadId, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        slot.as_ref()
            .map(|local| local.current)
            .ok_or(Error::CpuNotRegistered)
    })
}

pub(in crate::kernel::task::scheduler) fn local_current_vcpu(
    cpu: CpuIndex,
) -> Result<Option<CurrentVcpu>, Error> {
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

pub(in crate::kernel::task::scheduler) fn local_current_user_thread(
    cpu: CpuIndex,
) -> Result<Option<crate::kernel::process::UserThread>, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let id = local.current;
        local
            .thread_authority()
            .with_thread(id, |thread, schedule| {
                if schedule.state != ThreadState::Running {
                    return Err(Error::InvalidThreadState);
                }
                Ok(thread.user_thread().cloned())
            })?
    })
}

pub(in crate::kernel::task::scheduler) fn local_current_user(
    cpu: CpuIndex,
) -> Result<CurrentUser, Error> {
    CPU_SCHEDULERS[cpu].with(|slot| {
        let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
        let id = local.current;
        local
            .thread_authority()
            .with_thread(id, |thread, schedule| {
                if schedule.state != ThreadState::Running {
                    return Err(Error::InvalidThreadState);
                }
                Ok(CurrentUser {
                    thread: id,
                    object: thread
                        .user_thread()
                        .cloned()
                        .ok_or(Error::InvalidThreadState)?,
                    execution: thread
                        .user_execution_pointer()
                        .ok_or(Error::InvalidThreadState)?,
                    stack: thread.kernel_stack_bounds().ok_or(Error::Allocation)?,
                })
            })?
    })
}

impl CpuScheduler {
    pub(super) const fn new(
        index: CpuIndex,
        table: ThreadTableCapability,
        current: ThreadId,
    ) -> Self {
        Self {
            index,
            table,
            authority: CpuScheduleAuthorityToken::new(),
            current,
            idle: None,
            run_queue: CpuRunQueue::new(),
            handoff: SwitchHandoff::new(),
        }
    }

    pub(super) fn thread_authority(&mut self) -> CpuThreadTableAuthority<'_> {
        self.table.cpu_authority(self.index, &mut self.authority)
    }

    pub(super) fn local_peek_ready(&mut self) -> Result<Option<queue::ReadyThread>, Error> {
        let cpu = self.index;
        let threads = queue::LocalReadyQueueAuthority::new(
            self.table.cpu_authority(cpu, &mut self.authority),
        );
        self.run_queue.peek_next(&threads, cpu)
    }

    pub(super) fn local_enqueue_ready(&mut self, id: ThreadId, front: bool) -> Result<(), Error> {
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

    pub(super) fn local_dequeue_ready(&mut self) -> Result<Option<ThreadId>, Error> {
        let cpu = self.index;
        let mut threads = queue::LocalReadyQueueAuthority::new(
            self.table.cpu_authority(cpu, &mut self.authority),
        );
        self.run_queue.dequeue(&mut threads, cpu)
    }

    pub(super) fn prepare_local_switch(
        &mut self,
        current: ThreadId,
        next: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        if self.handoff.current().is_some() || self.current != current || current == next {
            return Err(Error::ThreadTransitionInProgress);
        }
        let cpu = self.index;
        let (previous, next_context) = {
            let mut threads = self.thread_authority();
            let previous =
                threads.with_thread(current, |thread, _schedule| thread.context_pointer())?;
            let next_context = threads.with_thread_mut(next, |thread, schedule| {
                match schedule.state {
                    ThreadState::Ready => {
                        let Some(placement) = schedule.placement.mark_running(cpu) else {
                            return Err(Error::InvalidThreadState);
                        };
                        schedule.placement = placement;
                        schedule.state = ThreadState::Running;
                    }
                    ThreadState::Idle => {}
                    _ => return Err(Error::InvalidThreadState),
                }
                if schedule.fair_slice_expired() {
                    schedule.replenish_fair_slice(FAIR_QUANTUM_TICKS);
                }
                Ok::<_, Error>(thread.context_pointer().cast_const())
            })??;
            (previous, next_context)
        };
        let generation = self
            .handoff
            .begin(current, SwitchDisposition::Local)
            .unwrap_or_else(|| {
                hyper::debug::invariant_failure(
                    "task::scheduler::state::prepare_local_switch invariant",
                )
            });
        self.current = next;
        Ok(PreparedContextSwitch {
            previous,
            next: next_context,
            ticket: generation,
            armed: true,
        })
    }
}
