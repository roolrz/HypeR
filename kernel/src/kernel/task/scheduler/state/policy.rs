// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduling-class and priority changes under existing queue ownership.

use hyper::cpu::CpuIndex;

use super::super::Error;
use super::Scheduler;
use crate::kernel::task::policy::{SchedulingPolicy, ThreadPriority};
use crate::kernel::task::thread::{DeferredFifoPlacement, QueueMembership, ThreadId, ThreadState};

impl Scheduler {
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
}
