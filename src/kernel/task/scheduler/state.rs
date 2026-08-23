// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-owned thread registry, CPU state, and lifecycle transitions.

use alloc::boxed::Box;
use alloc::vec::Vec;
use hyper::cpu::{CpuIndex, PerCpu};

use super::queue::{self, CpuRunQueue};
use super::{CurrentVcpu, Error, SecondaryStack, Statistics};
use crate::kernel::task::policy::{CpuMask, SchedulingClass, ThreadPriority};
use crate::kernel::task::thread::{
    DeferredFifoPlacement, KernelThreadEntry, QueueMembership, Thread, ThreadId, ThreadState,
};
use crate::kernel::task::wait::{ThreadQueue, WaitQueue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadyOutcome {
    pub changed: bool,
    pub target_cpu: CpuIndex,
    pub should_preempt: bool,
}

pub(super) struct Scheduler {
    // Slot index equals ThreadId and is never reused. Individual Box
    // allocations pin contexts while this registry grows.
    #[allow(clippy::vec_box)]
    threads: Vec<Option<Box<Thread>>>,
    cpus: Vec<CpuScheduler>,
    cpu_slots: PerCpu<Option<usize>>,
    terminated: ThreadQueue,
    next_id: u64,
    context_switches: u64,
}

struct CpuScheduler {
    index: CpuIndex,
    current: ThreadId,
    idle: Option<ThreadId>,
    run_queue: CpuRunQueue,
    /// Outgoing context whose registers/stack may still be in use between
    /// dropping the scheduler lock and completing the assembly switch.
    switching_from: Option<ThreadId>,
}

#[must_use = "a prepared context switch must be consumed by the architecture boundary"]
pub(super) struct PreparedContextSwitch {
    previous: *mut crate::arch::context::ThreadContext,
    next: *const crate::arch::context::ThreadContext,
    armed: bool,
}

impl PreparedContextSwitch {
    /// Activates the only context switch represented by this transition.
    ///
    /// The scheduler transaction is already committed, so this is the sole
    /// operation which can disarm the fail-stop Drop path.
    pub(super) fn activate(mut self) {
        self.armed = false;
        // SAFETY: Scheduler queues contain only pinned, scheduler-owned
        // Threads. switching_from retains the outgoing allocation until the
        // resumed continuation next enters the scheduler.
        unsafe { crate::arch::context::switch_thread_context(&mut *self.previous, &*self.next) };
    }
}

impl Drop for PreparedContextSwitch {
    fn drop(&mut self) {
        if self.armed {
            crate::pr_crit!("HypeR: prepared context switch was not activated");
            crate::arch::cpu::halt()
        }
    }
}

impl CpuScheduler {
    const fn new(index: CpuIndex, current: ThreadId) -> Self {
        Self {
            index,
            current,
            idle: None,
            run_queue: CpuRunQueue::new(),
            switching_from: None,
        }
    }
}

impl Scheduler {
    pub fn new(boot_cpu: CpuIndex) -> Result<Self, Error> {
        let mut threads = Vec::new();
        threads.try_reserve(1).map_err(|_| Error::Allocation)?;
        threads.push(Some(
            hyper::mm::try_box(Thread::bootstrap(boot_cpu)).map_err(|_| Error::Allocation)?,
        ));
        let mut cpus = Vec::new();
        cpus.try_reserve(1).map_err(|_| Error::Allocation)?;
        cpus.push(CpuScheduler::new(boot_cpu, ThreadId::BOOTSTRAP));
        let mut cpu_slots = PerCpu::new([None; hyper::cpu::MAX_CPUS]);
        cpu_slots[boot_cpu] = Some(0);
        Ok(Self {
            threads,
            cpus,
            cpu_slots,
            terminated: ThreadQueue::new(),
            next_id: 1,
            context_switches: 0,
        })
    }

    pub fn current_thread(&self, cpu: CpuIndex) -> Result<ThreadId, Error> {
        Ok(self.cpus[self.cpu_slot(cpu)?].current)
    }

    pub fn register_secondary(
        &mut self,
        cpu: CpuIndex,
        name: &str,
    ) -> Result<SecondaryStack, Error> {
        if self.cpu_slots[cpu].is_some() {
            return Err(Error::CpuAlreadyRegistered);
        }
        self.reserve_thread_and_cpu()?;
        let id = self.next_thread_id()?;
        let thread = hyper::mm::try_box(Thread::secondary_bootstrap(id, cpu, name)?)
            .map_err(|_| Error::Allocation)?;
        let virtual_top = thread.kernel_stack_top().ok_or(Error::Allocation)?;
        let physical_top = thread
            .kernel_stack_physical_top()
            .ok_or(Error::Allocation)?;
        self.register_thread(thread)?;
        self.cpus.push(CpuScheduler::new(cpu, id));
        self.cpu_slots[cpu] = Some(self.cpus.len() - 1);
        Ok(SecondaryStack {
            physical_top,
            virtual_top,
        })
    }

    pub fn create_kernel_thread(
        &mut self,
        preferred_cpu: CpuIndex,
        affinity: CpuMask,
        name: &str,
        entry: KernelThreadEntry,
        argument: usize,
        priority: ThreadPriority,
    ) -> Result<ThreadId, Error> {
        let cpu = self.select_cpu(preferred_cpu, affinity)?;
        self.threads.try_reserve(1).map_err(|_| Error::Allocation)?;
        let id = self.next_thread_id()?;
        let mut thread =
            hyper::mm::try_box(Thread::kernel(id, cpu, affinity, name, entry, argument)?)
                .map_err(|_| Error::Allocation)?;
        if !thread.set_priority(priority) {
            return Err(Error::InvalidThreadState);
        }
        self.register_thread(thread)?;
        Ok(id)
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
        if affinity.contains(preferred_cpu) && self.cpu_slots[preferred_cpu].is_some() {
            return Ok(preferred_cpu);
        }
        self.cpus
            .iter()
            .map(|cpu| cpu.index)
            .filter(|cpu| affinity.contains(*cpu))
            .min()
            .ok_or(Error::NoRegisteredCpuInAffinity)
    }

    pub fn create_vcpu_thread(
        &mut self,
        cpu: CpuIndex,
        name: &str,
        vm: crate::kernel::vm::registry::VmBinding,
        vcpu_id: u32,
        context: crate::arch::vm::VcpuContext,
        entry: KernelThreadEntry,
    ) -> Result<ThreadId, Error> {
        self.cpu_slot(cpu)?;
        self.threads.try_reserve(1).map_err(|_| Error::Allocation)?;
        let id = self.next_thread_id()?;
        let thread = hyper::mm::try_box(Thread::vcpu(id, cpu, name, vm, vcpu_id, context, entry)?)
            .map_err(|_| Error::Allocation)?;
        self.register_thread(thread)?;
        Ok(id)
    }

    pub fn take_dormant_vcpu(&mut self, id: ThreadId) -> Result<Box<Thread>, Error> {
        self.take_dormant_thread(id, crate::kernel::task::thread::ExecutionKind::Vcpu)
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
        let index = usize::try_from(id.get()).map_err(|_| Error::ThreadNotFound)?;
        let thread = self
            .threads
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(Error::ThreadNotFound)?;
        if thread.state() != ThreadState::Dormant
            || thread.queue_links().membership != QueueMembership::None
            || thread.execution_kind() != execution
        {
            return Err(Error::InvalidThreadState);
        }
        self.threads[index].take().ok_or(Error::ThreadNotFound)
    }

    pub fn current_vcpu(&mut self, cpu: CpuIndex) -> Result<CurrentVcpu, Error> {
        let id = self.current_thread(cpu)?;
        let thread = self.thread_mut(id)?;
        if thread.state() != ThreadState::Running {
            return Err(Error::InvalidThreadState);
        }
        let stack = thread.kernel_stack_bounds().ok_or(Error::Allocation)?;
        let execution = thread
            .vcpu_execution_mut()
            .ok_or(Error::InvalidThreadState)? as *mut _;
        Ok(CurrentVcpu {
            thread: id,
            execution,
            stack,
        })
    }

    pub fn make_ready(&mut self, id: ThreadId) -> Result<ReadyOutcome, Error> {
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

    /// Selects a higher-priority FIFO thread at a cooperative safe point.
    ///
    /// A preempted FIFO thread is returned to the head of its own priority
    /// queue. Equal-priority peers therefore cannot overtake it merely because
    /// a higher-priority thread ran temporarily.
    pub fn prepare_preemption(
        &mut self,
        cpu: CpuIndex,
    ) -> Result<Option<PreparedContextSwitch>, Error> {
        if !super::super::preempt::pending(cpu)? {
            return Ok(None);
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        let candidate = self.cpus[cpu_slot]
            .run_queue
            .peek_highest(&self.threads, cpu)?;
        let Some((expected_next, ready_priority)) = candidate else {
            let _ = super::super::preempt::take_pending_locked(cpu)?;
            if self.thread(current)?.state() == ThreadState::Running {
                // Deferred placement orders current only against peers present
                // at the priority-change decision. If none remains eligible,
                // future peers must arrive behind the still-running current.
                self.thread_mut(current)?.set_deferred_fifo_placement(None);
            }
            return Ok(None);
        };
        let placement = match self.thread(current)?.state() {
            ThreadState::Idle => None,
            ThreadState::Running => {
                let current_priority = self
                    .thread(current)?
                    .priority()
                    .ok_or(Error::InvalidThreadState)?;
                let placement = self.thread(current)?.deferred_fifo_placement();
                if ready_priority > current_priority
                    || (ready_priority == current_priority
                        && placement != Some(DeferredFifoPlacement::Tail))
                {
                    let _ = super::super::preempt::take_pending_locked(cpu)?;
                    self.thread_mut(current)?.set_deferred_fifo_placement(None);
                    return Ok(None);
                }
                Some(placement.unwrap_or(DeferredFifoPlacement::Head))
            }
            _ => return Err(Error::InvalidThreadState),
        };
        if !super::super::preempt::take_pending_locked(cpu)? {
            return Ok(None);
        }
        if let Some(placement) = placement {
            self.thread_mut(current)?.set_deferred_fifo_placement(None);
            match placement {
                DeferredFifoPlacement::Head => self.enqueue_ready_front(current)?,
                DeferredFifoPlacement::Tail => self.enqueue_ready(current)?,
            }
        }
        let next = self
            .dequeue_ready(cpu_slot)?
            .ok_or(Error::CurrentThreadMissing)?;
        if next != expected_next {
            return Err(Error::QueueCorrupted);
        }
        self.prepare_switch(cpu_slot, current, next).map(Some)
    }

    pub fn prepare_yield(&mut self, cpu: CpuIndex) -> Result<Option<PreparedContextSwitch>, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        match self.thread(current)?.state() {
            ThreadState::Running => {
                let current_priority = self
                    .thread(current)?
                    .priority()
                    .ok_or(Error::InvalidThreadState)?;
                let candidate = self.cpus[cpu_slot]
                    .run_queue
                    .peek_highest(&self.threads, cpu)?;
                let Some((expected_next, ready_priority)) = candidate else {
                    return Ok(None);
                };
                if ready_priority > current_priority {
                    return Ok(None);
                }
                self.enqueue_ready(current)?;
                let next = self
                    .dequeue_ready(cpu_slot)?
                    .ok_or(Error::CurrentThreadMissing)?;
                if next != expected_next {
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

    pub fn prepare_park(
        &mut self,
        cpu: CpuIndex,
        wait_queue: &WaitQueue,
    ) -> Result<PreparedContextSwitch, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        if self.thread(current)?.state() != ThreadState::Running {
            return Err(Error::CannotBlockIdle);
        }
        self.enqueue_waiter(wait_queue, current)?;
        let next = match self.dequeue_ready(cpu_slot)?.or(self.cpus[cpu_slot].idle) {
            Some(id) => id,
            None => return self.rollback_failed_park(wait_queue, current),
        };
        if next == current {
            return self.rollback_failed_park(wait_queue, current);
        }
        self.prepare_switch(cpu_slot, current, next)
    }

    pub fn prepare_exit(&mut self, cpu: CpuIndex) -> Result<PreparedContextSwitch, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        queue::push(
            &mut self.threads,
            &mut self.terminated,
            current,
            QueueMembership::Terminated,
        )?;
        self.thread_mut(current)?.set_state(ThreadState::Terminated);
        let next = self
            .dequeue_ready(cpu_slot)?
            .or(self.cpus[cpu_slot].idle)
            .ok_or(Error::CurrentThreadMissing)?;
        if next == current {
            return Err(Error::CurrentThreadMissing);
        }
        self.prepare_switch(cpu_slot, current, next)
    }

    pub fn dequeue_waiter(&mut self, wait_queue: &WaitQueue) -> Result<Option<ThreadId>, Error> {
        let membership = QueueMembership::Waiting {
            queue: wait_queue.identity(),
        };
        // SAFETY: Every caller holds the global scheduler lock exclusively.
        queue::pop(
            &mut self.threads,
            unsafe { &mut *wait_queue.state_pointer() },
            membership,
        )
    }

    pub fn change_priority(
        &mut self,
        id: ThreadId,
        priority: ThreadPriority,
    ) -> Result<Option<CpuIndex>, Error> {
        let state = self.thread(id)?.state();
        if state == ThreadState::Terminated {
            return Err(Error::TerminatedThread);
        }
        let links = self.thread(id)?.queue_links();
        if let QueueMembership::Ready { cpu, priority: old } = links.membership {
            if old == priority.get() {
                return Ok(None);
            }
            self.remove_ready(id, cpu, old)?;
            if !self.thread_mut(id)?.set_priority(priority) {
                return Err(Error::InvalidThreadState);
            }
            if priority.get() < old {
                self.enqueue_ready(id)?;
            } else {
                self.enqueue_ready_front(id)?;
            }
            Ok(self.ready_thread_preempts(id)?.then_some(cpu))
        } else {
            let old = self
                .thread(id)?
                .priority()
                .ok_or(Error::InvalidThreadState)?;
            if old == priority {
                return Ok(None);
            }
            self.thread_mut(id)?
                .set_priority(priority)
                .then_some(())
                .ok_or(Error::InvalidThreadState)?;
            if state == ThreadState::Running {
                self.thread_mut(id)?.set_deferred_fifo_placement(None);
            }
            let thread = self.thread(id)?;
            let cpu = thread.cpu_index();
            let ready = self.cpus[self.cpu_slot(cpu)?].run_queue.highest_priority();
            let should_reschedule = state == ThreadState::Running
                && ready
                    .is_some_and(|ready| ready < priority || (priority < old && ready == priority));
            if should_reschedule {
                let placement = if priority < old {
                    DeferredFifoPlacement::Tail
                } else {
                    DeferredFifoPlacement::Head
                };
                self.thread_mut(id)?
                    .set_deferred_fifo_placement(Some(placement));
                Ok(Some(cpu))
            } else {
                Ok(None)
            }
        }
    }

    pub fn install_current_as_idle(&mut self, cpu: CpuIndex) -> Result<(usize, usize), Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        if self.cpus[cpu_slot].idle.is_some() {
            return Err(Error::IdleThreadAlreadyInstalled);
        }
        let current = self.cpus[cpu_slot].current;
        if self.thread(current)?.state() != ThreadState::Running {
            return Err(Error::InvalidIdleTransition);
        }
        let thread = self.thread_mut(current)?;
        let stack = thread.ensure_kernel_stack()?;
        thread.become_idle();
        self.cpus[cpu_slot].idle = Some(current);
        Ok(stack)
    }

    pub fn finish_switch(&mut self, cpu: CpuIndex) {
        if let Ok(cpu_slot) = self.cpu_slot(cpu) {
            self.cpus[cpu_slot].switching_from = None;
        }
    }

    pub fn reap_terminated(&mut self) -> Result<(), Error> {
        let mut candidate = self.terminated.head;
        while let Some(id) = candidate {
            let links = self.thread(id)?.queue_links();
            candidate = links.next;
            let pinned = self
                .cpus
                .iter()
                .any(|cpu| cpu.current == id || cpu.switching_from == Some(id));
            if !pinned {
                if self.thread(id)?.state() != ThreadState::Terminated {
                    return Err(Error::QueueCorrupted);
                }
                queue::remove(
                    &mut self.threads,
                    &mut self.terminated,
                    id,
                    QueueMembership::Terminated,
                )?;
                let index = usize::try_from(id.get()).map_err(|_| Error::ThreadNotFound)?;
                self.threads[index] = None;
            }
        }
        Ok(())
    }

    pub fn statistics(&self) -> Statistics {
        let mut stats = Statistics {
            context_switches: self.context_switches,
            ..Statistics::default()
        };
        for thread in self.threads.iter().flatten() {
            stats.threads += 1;
            match thread.scheduling_class() {
                SchedulingClass::FixedPriority => stats.fixed_priority_class_threads += 1,
                SchedulingClass::Idle => stats.idle_class_threads += 1,
            }
            match thread.state() {
                ThreadState::Ready => stats.ready += 1,
                ThreadState::Running => stats.running += 1,
                ThreadState::Blocked => stats.blocked += 1,
                ThreadState::Idle => stats.idle += 1,
                ThreadState::Dormant | ThreadState::Terminated => {}
            }
        }
        for cpu in &self.cpus {
            stats.per_cpu_ready[cpu.index.get()] = cpu.run_queue.len();
        }
        stats
    }

    pub fn thread(&self, id: ThreadId) -> Result<&Thread, Error> {
        queue::thread_ref(&self.threads, id)
    }

    fn thread_mut(&mut self, id: ThreadId) -> Result<&mut Thread, Error> {
        queue::thread_mut(&mut self.threads, id)
    }

    fn prepare_switch(
        &mut self,
        cpu_slot: usize,
        current: ThreadId,
        next: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        let target_cpu = self.cpus[cpu_slot].index;
        if self.thread(next)?.state() == ThreadState::Ready
            && !self.thread_mut(next)?.mark_running_on(target_cpu)
        {
            return Err(Error::InvalidThreadState);
        }
        self.cpus[cpu_slot].switching_from = Some(current);
        self.cpus[cpu_slot].current = next;
        self.context_switches = self.context_switches.saturating_add(1);
        let previous = self.thread_mut(current)?.context_mut() as *mut _;
        let next = self.thread(next)?.context() as *const _;
        Ok(PreparedContextSwitch {
            previous,
            next,
            armed: true,
        })
    }

    fn enqueue_ready(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        let cpu = thread.cpu_index();
        if thread.scheduling_class() != SchedulingClass::FixedPriority || !thread.can_run_on(cpu) {
            return Err(Error::InvalidThreadState);
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].run_queue.enqueue(threads, id, cpu)
    }

    fn enqueue_ready_front(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        let cpu = thread.cpu_index();
        if thread.scheduling_class() != SchedulingClass::FixedPriority || !thread.can_run_on(cpu) {
            return Err(Error::InvalidThreadState);
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].run_queue.enqueue_front(threads, id, cpu)
    }

    fn ready_thread_preempts(&self, id: ThreadId) -> Result<bool, Error> {
        let candidate = self.thread(id)?;
        let candidate_policy = candidate.scheduling_policy();
        let cpu_slot = self.cpu_slot(candidate.cpu_index())?;
        let current = self.thread(self.cpus[cpu_slot].current)?;
        match current.state() {
            ThreadState::Idle | ThreadState::Running => Ok(current
                .scheduling_policy()
                .is_preempted_by(candidate_policy)),
            _ => Err(Error::InvalidThreadState),
        }
    }

    fn dequeue_ready(&mut self, cpu_slot: usize) -> Result<Option<ThreadId>, Error> {
        let cpu = self.cpus[cpu_slot].index;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].run_queue.dequeue(threads, cpu)
    }

    fn remove_ready(&mut self, id: ThreadId, cpu: CpuIndex, priority: u8) -> Result<(), Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].run_queue.remove(threads, id, cpu, priority)
    }

    fn enqueue_waiter(&mut self, wait_queue: &WaitQueue, id: ThreadId) -> Result<(), Error> {
        let membership = QueueMembership::Waiting {
            queue: wait_queue.identity(),
        };
        // SAFETY: Every caller holds the global scheduler lock exclusively.
        let queue = unsafe { &mut *wait_queue.state_pointer() };
        queue::push(&mut self.threads, queue, id, membership)?;
        self.thread_mut(id)?.set_state(ThreadState::Blocked);
        Ok(())
    }

    fn remove_waiter(&mut self, wait_queue: &WaitQueue, id: ThreadId) -> Result<(), Error> {
        let membership = QueueMembership::Waiting {
            queue: wait_queue.identity(),
        };
        // SAFETY: Every caller holds the global scheduler lock exclusively.
        queue::remove(
            &mut self.threads,
            unsafe { &mut *wait_queue.state_pointer() },
            id,
            membership,
        )
    }

    fn rollback_failed_park(
        &mut self,
        wait_queue: &WaitQueue,
        current: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        self.remove_waiter(wait_queue, current)?;
        self.thread_mut(current)?.set_state(ThreadState::Running);
        Err(Error::CurrentThreadMissing)
    }

    fn cpu_slot(&self, cpu: CpuIndex) -> Result<usize, Error> {
        self.cpu_slots[cpu].ok_or(Error::CpuNotRegistered)
    }

    fn next_thread_id(&self) -> Result<ThreadId, Error> {
        let id = ThreadId::from_scheduler_index(self.next_id);
        let index = usize::try_from(self.next_id).map_err(|_| Error::IdentifierExhausted)?;
        if index != self.threads.len() || self.next_id == u64::MAX {
            return Err(Error::IdentifierExhausted);
        }
        Ok(id)
    }

    fn register_thread(&mut self, thread: Box<Thread>) -> Result<(), Error> {
        if thread.id() != self.next_thread_id()? {
            return Err(Error::IdentifierExhausted);
        }
        self.threads.push(Some(thread));
        self.next_id += 1;
        Ok(())
    }

    fn reserve_thread_and_cpu(&mut self) -> Result<(), Error> {
        self.threads.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.cpus.try_reserve(1).map_err(|_| Error::Allocation)
    }
}
