// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-owned thread registry, CPU state, and lifecycle transitions.

use alloc::boxed::Box;
use alloc::vec::Vec;
use hyper::cpu::{CpuIndex, PerCpu};

use super::queue::{self, CpuRunQueue};
use super::{CurrentVcpu, Error, MigrationStatus, SecondaryStack, Statistics};
use crate::kernel::task::policy::{
    CpuMask, PlacementPolicy, SchedulingClass, SchedulingPolicy, ThreadPriority,
};
use crate::kernel::task::thread::{
    DeferredFifoPlacement, ExecutionKind, MigrationRequest, QueueMembership, Thread, ThreadId,
    ThreadState,
};
use crate::kernel::task::wait::{
    PendingResolution, ThreadQueue, WaitMobility, WaitOutcome, WaitQueue, WaitRecordError,
    WaitTicket,
};

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

#[must_use = "a thread reservation must be published or explicitly abandoned"]
pub(super) struct ThreadReservation {
    id: ThreadId,
    cpu: CpuIndex,
    armed: bool,
}

impl ThreadReservation {
    pub const fn id(&self) -> ThreadId {
        self.id
    }

    pub const fn cpu(&self) -> CpuIndex {
        self.cpu
    }

    pub fn disarm(&mut self) {
        if !self.armed {
            crate::hal::cpu::halt()
        }
        self.armed = false;
    }
}

impl Drop for ThreadReservation {
    fn drop(&mut self) {
        if self.armed {
            crate::hal::cpu::halt()
        }
    }
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
struct SwitchingContext {
    thread: ThreadId,
}

pub(super) struct Scheduler {
    // Slot index equals ThreadId and is never reused. Individual Box
    // allocations pin contexts while this registry grows.
    #[allow(clippy::vec_box)]
    threads: Vec<Option<Box<Thread>>>,
    cpus: Vec<CpuScheduler>,
    cpu_slots: PerCpu<Option<usize>>,
    cpu_reservations: PerCpu<bool>,
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
    switching_from: Option<SwitchingContext>,
}

#[must_use = "a prepared context switch must be consumed by the architecture boundary"]
pub(super) struct PreparedContextSwitch {
    previous: *mut crate::hal::context::ThreadContext,
    next: *const crate::hal::context::ThreadContext,
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
        cpus.try_reserve(hyper::cpu::MAX_CPUS)
            .map_err(|_| Error::Allocation)?;
        cpus.push(CpuScheduler::new(boot_cpu, ThreadId::BOOTSTRAP));
        let mut cpu_slots = PerCpu::new([None; hyper::cpu::MAX_CPUS]);
        cpu_slots[boot_cpu] = Some(0);
        Ok(Self {
            threads,
            cpus,
            cpu_slots,
            cpu_reservations: PerCpu::new([false; hyper::cpu::MAX_CPUS]),
            terminated: ThreadQueue::new(),
            next_id: 1,
            context_switches: 0,
        })
    }

    pub fn current_thread(&self, cpu: CpuIndex) -> Result<ThreadId, Error> {
        Ok(self.cpus[self.cpu_slot(cpu)?].current)
    }

    pub fn registry_growth_target(&self) -> Option<usize> {
        (self.threads.len() == self.threads.capacity())
            .then(|| self.threads.capacity().saturating_mul(2).max(16))
    }

    pub fn install_registry_storage(
        &mut self,
        mut replacement: Vec<Option<Box<Thread>>>,
    ) -> Vec<Option<Box<Thread>>> {
        if replacement.capacity() > self.threads.capacity() {
            replacement.append(&mut self.threads);
            core::mem::swap(&mut replacement, &mut self.threads);
        }
        replacement
    }

    /// Reports whether no CPU owns or may still be saving this context.
    pub fn context_is_stopped(&self, id: ThreadId) -> Result<bool, Error> {
        let _ = self.thread(id)?;
        Ok(!self.cpus.iter().any(|cpu| {
            cpu.current == id
                || cpu
                    .switching_from
                    .is_some_and(|switching| switching.thread == id)
        }))
    }

    pub fn reserve_secondary(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        if self.cpu_slots[cpu].is_some() || self.cpu_reservations[cpu] {
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
        if !self.cpu_reservations[reservation.cpu] || self.cpu_slots[reservation.cpu].is_some() {
            return Err((Error::InvalidThreadState, thread));
        }
        let Some(virtual_top) = thread.kernel_stack_top() else {
            return Err((Error::Allocation, thread));
        };
        let Some(physical_top) = thread.kernel_stack_physical_top() else {
            return Err((Error::Allocation, thread));
        };
        self.publish_thread(reservation, thread)?;
        self.cpus
            .push(CpuScheduler::new(reservation.cpu, reservation.id));
        self.cpu_slots[reservation.cpu] = Some(self.cpus.len() - 1);
        self.cpu_reservations[reservation.cpu] = false;
        Ok(SecondaryStack {
            physical_top,
            virtual_top,
        })
    }

    pub fn abandon_reservation(&mut self, reservation: &ThreadReservation) -> Result<(), Error> {
        let index = usize::try_from(reservation.id.get()).map_err(|_| Error::ThreadNotFound)?;
        if !matches!(self.threads.get(index), Some(None)) {
            return Err(Error::InvalidThreadState);
        }
        if self.cpu_reservations[reservation.cpu] {
            self.cpu_reservations[reservation.cpu] = false;
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

    pub fn reserve_vcpu_thread(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        self.cpu_slot(cpu)?;
        self.reserve_thread_slot(cpu)
    }

    pub fn publish_thread(
        &mut self,
        reservation: &ThreadReservation,
        thread: Box<Thread>,
    ) -> Result<(), (Error, Box<Thread>)> {
        let Ok(index) = usize::try_from(reservation.id.get()) else {
            return Err((Error::IdentifierExhausted, thread));
        };
        if thread.id() != reservation.id
            || thread.cpu_index() != reservation.cpu
            || !matches!(self.threads.get(index), Some(None))
        {
            return Err((Error::InvalidThreadState, thread));
        }
        self.threads[index] = Some(thread);
        Ok(())
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

    #[cfg(CONFIG_ARCH_AARCH64)]
    pub fn current_vcpu_if_present(&mut self, cpu: CpuIndex) -> Result<Option<CurrentVcpu>, Error> {
        let id = self.current_thread(cpu)?;
        let thread = self.thread_mut(id)?;
        if !matches!(thread.state(), ThreadState::Running | ThreadState::Idle) {
            return Err(Error::InvalidThreadState);
        }
        if thread.execution_kind() != crate::kernel::task::thread::ExecutionKind::Vcpu {
            return Ok(None);
        }
        let stack = thread.kernel_stack_bounds().ok_or(Error::Allocation)?;
        let execution = thread
            .vcpu_execution_mut()
            .ok_or(Error::InvalidThreadState)?;
        Ok(Some(CurrentVcpu {
            thread: id,
            execution: execution as *mut _,
            stack,
        }))
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
        self.cpu_slot(target)?;
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
        if thread.execution_kind() != ExecutionKind::Kernel
            || thread.placement_policy() != PlacementPolicy::Movable
        {
            return Err(Error::MigrationUnsupported);
        }
        if !plan.affinity.contains(plan.target) || self.cpu_slots[plan.target].is_none() {
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
            .cpus
            .iter()
            .filter_map(|cpu| cpu.switching_from)
            .any(|switching| switching.thread == id)
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
                if self.cpus[cpu_slot].current != id {
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
        if !super::super::preempt::pending(cpu)? {
            return Ok(None);
        }
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        if self.thread(current)?.pending_migration().is_some() {
            if !super::super::preempt::take_pending_locked(cpu)? {
                return Err(Error::PreemptionInvariant);
            }
            return self.prepare_running_migration(cpu_slot, current).map(Some);
        }
        let candidate = self.cpus[cpu_slot]
            .run_queue
            .peek_next(&self.threads, cpu)?;
        let Some(candidate) = candidate else {
            let _ = super::super::preempt::take_pending_locked(cpu)?;
            if self.thread(current)?.state() == ThreadState::Running {
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
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
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
                let candidate = self.cpus[cpu_slot]
                    .run_queue
                    .peek_next(&self.threads, cpu)?;
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
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        let thread = self.thread(current)?;
        if thread.state() != ThreadState::Running {
            return Err(Error::CannotBlockIdle);
        }
        if mobility == WaitMobility::CpuLocal && thread.pending_migration().is_some() {
            return Err(Error::MigrationInProgress);
        }
        self.thread_mut(current)?
            .wait_record_mut()
            .arm(current, mobility, cpu)
            .map_err(Error::from)
    }

    pub fn finish_unqueued_wait(
        &mut self,
        cpu: CpuIndex,
        ticket: WaitTicket,
    ) -> Result<Option<WaitOutcome>, Error> {
        let current = self.current_thread(cpu)?;
        if current != ticket.thread() {
            return Err(Error::InvalidWaitRegistration);
        }
        self.thread_mut(current)?
            .wait_record_mut()
            .finish_unqueued(ticket)
            .map_err(Error::from)
    }

    pub fn finish_completed_wait(
        &mut self,
        cpu: CpuIndex,
        ticket: WaitTicket,
    ) -> Result<WaitOutcome, Error> {
        let current = self.current_thread(cpu)?;
        if current != ticket.thread() {
            return Err(Error::InvalidWaitRegistration);
        }
        self.thread_mut(current)?
            .wait_record_mut()
            .finish_completed(ticket)
            .map_err(Error::from)
    }

    pub fn prepare_registered_park(
        &mut self,
        cpu: CpuIndex,
        wait_queue: &WaitQueue,
        ticket: WaitTicket,
    ) -> Result<PreparedWait, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
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
                    .wait_record_mut()
                    .finish_unqueued(ticket)?
                    .ok_or(Error::InvalidWaitRegistration)?;
                return Ok(PreparedWait::Completed(outcome));
            }
            PendingResolution::Queued { .. } | PendingResolution::Stale => {
                return Err(Error::InvalidWaitRegistration);
            }
        }
        if self.cpus[cpu_slot].switching_from.is_some() {
            return Err(Error::ThreadTransitionInProgress);
        }
        self.enqueue_waiter(wait_queue, current, ticket)?;
        let ready = match self.dequeue_ready(cpu_slot) {
            Ok(ready) => ready,
            Err(error) => scheduler_invariant(error),
        };
        let next = match ready.or(self.cpus[cpu_slot].idle) {
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
                let thread = match self.thread_mut(ticket.thread()) {
                    Ok(thread) => thread,
                    Err(error) => scheduler_invariant(error),
                };
                let completion = thread.wait_record_mut().complete(ticket, outcome);
                if completion.is_err() {
                    scheduler_invariant(Error::InvalidWaitRegistration);
                }
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
                let thread = match self.thread_mut(ticket.thread()) {
                    Ok(thread) => thread,
                    Err(error) => scheduler_invariant(error),
                };
                let completion = thread.wait_record_mut().complete(ticket, outcome);
                if completion.is_err() {
                    scheduler_invariant(Error::InvalidWaitRegistration);
                }
                let ready = match self.make_ready_from_wait(ticket.thread()) {
                    Ok(ready) => ready,
                    Err(error) => scheduler_invariant(error),
                };
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
        let ticket = self
            .thread(id)?
            .wait_record()
            .queued_ticket(id, wait_queue.identity())
            .ok_or(Error::QueueCorrupted)?;
        let popped = queue::pop(
            &mut self.threads,
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
        let thread = match self.thread_mut(id) {
            Ok(thread) => thread,
            Err(error) => scheduler_invariant(error),
        };
        let completion = thread
            .wait_record_mut()
            .complete(ticket, WaitOutcome::Notified);
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
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        if !self.thread(current)?.wait_record().is_idle() {
            return Err(Error::InvalidWaitRegistration);
        }
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

    pub fn set_fifo_policy(
        &mut self,
        id: ThreadId,
        priority: ThreadPriority,
    ) -> Result<Option<CpuIndex>, Error> {
        let state = self.thread(id)?.state();
        if state == ThreadState::Terminated {
            return Err(Error::TerminatedThread);
        }
        let links = self.thread(id)?.queue_links();
        if let QueueMembership::ReadyRealTime { cpu, priority: old } = links.membership {
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
        } else if let QueueMembership::ReadyFair { cpu } = links.membership {
            self.remove_ready(id, cpu, links.membership)?;
            if !self
                .thread_mut(id)?
                .set_scheduling_policy(SchedulingPolicy::fifo(priority))
            {
                return Err(Error::InvalidThreadState);
            }
            self.enqueue_ready(id)?;
            Ok(self.ready_thread_preempts(id)?.then_some(cpu))
        } else {
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
            let thread = self.thread(id)?;
            let cpu = thread.cpu_index();
            let ready = self.cpus[self.cpu_slot(cpu)?]
                .run_queue
                .peek_next(&self.threads, cpu)?;
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

    /// Moves a non-idle thread into the Fair scheduling class.
    ///
    /// Ready membership is transferred between class queues while the global
    /// scheduler lock is held. A running RT thread lowered to Fair requests a
    /// scheduling decision only when ready RT work now outranks it.
    pub fn set_fair_policy(&mut self, id: ThreadId) -> Result<Option<CpuIndex>, Error> {
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
        let candidate = self.cpus[self.cpu_slot(cpu)?]
            .run_queue
            .peek_next(&self.threads, cpu)?;
        Ok(candidate
            .is_some_and(|ready| SchedulingPolicy::Fair.is_preempted_by(ready.policy))
            .then_some(cpu))
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

    /// Charges elapsed scheduler ticks to the running Fair entity.
    ///
    /// This IRQ-safe operation changes no run-queue membership. It reports a
    /// scheduling request only when a peer is already ready; otherwise the
    /// sole runnable Fair entity receives a fresh slice immediately.
    pub fn account_tick(&mut self, cpu: CpuIndex, elapsed: u64) -> Result<bool, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        match self.thread(current)?.state() {
            ThreadState::Idle => Ok(false),
            ThreadState::Running => {
                if !self
                    .thread_mut(current)?
                    .account_fair_ticks(elapsed, super::FAIR_QUANTUM_TICKS)
                {
                    return Ok(false);
                }
                if self.cpus[cpu_slot].run_queue.has_fair_threads() {
                    Ok(true)
                } else {
                    self.thread_mut(current)?
                        .replenish_fair_slice(super::FAIR_QUANTUM_TICKS);
                    Ok(false)
                }
            }
            _ => Err(Error::InvalidThreadState),
        }
    }

    /// Retires source-CPU context ownership from the incoming switch tail.
    pub fn complete_incoming_switch(
        &mut self,
        cpu: CpuIndex,
    ) -> Result<Option<ReadyOutcome>, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let switching = self.cpus[cpu_slot]
            .switching_from
            .take()
            .ok_or(Error::PreemptionInvariant)?;
        let Some(plan) = self.thread_mut(switching.thread)?.take_migration_request() else {
            return Ok(None);
        };
        self.complete_migration(switching.thread, plan)
    }

    pub fn detach_terminated(&mut self) -> Result<Option<Box<Thread>>, Error> {
        let mut candidate = self.terminated.head;
        while let Some(id) = candidate {
            let links = self.thread(id)?.queue_links();
            candidate = links.next;
            if self.context_is_stopped(id)? {
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
                return self.threads[index]
                    .take()
                    .map(Some)
                    .ok_or(Error::ThreadNotFound);
            }
        }
        Ok(None)
    }

    pub fn statistics(&self) -> Statistics {
        let mut stats = Statistics {
            context_switches: self.context_switches,
            ..Statistics::default()
        };
        for thread in self.threads.iter().flatten() {
            stats.threads += 1;
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
        if self.cpus[cpu_slot].switching_from.is_some() {
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
        self.cpus[cpu_slot].switching_from = Some(SwitchingContext { thread: current });
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

    fn prepare_running_migration(
        &mut self,
        cpu_slot: usize,
        current: ThreadId,
    ) -> Result<PreparedContextSwitch, Error> {
        if self.thread(current)?.state() != ThreadState::Running
            || self.thread(current)?.pending_migration().is_none()
        {
            return Err(Error::InvalidThreadState);
        }
        let next = self
            .dequeue_ready(cpu_slot)?
            .or(self.cpus[cpu_slot].idle)
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
                self.enqueue_ready(id)?;
                Ok(Some(ReadyOutcome {
                    changed: true,
                    target_cpu: plan.target,
                    should_preempt: self.ready_thread_preempts(id)?,
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
            .thread_mut(id)?
            .reassign_stopped_with_affinity(plan.target, plan.affinity)
        {
            self.restore_ready_migration(id, source, old_affinity);
            return Err(Error::InvalidThreadState);
        }
        if let Err(error) = self.enqueue_ready(id) {
            self.restore_ready_migration(id, source, old_affinity);
            return Err(error);
        }
        Ok(ReadyOutcome {
            changed: true,
            target_cpu: plan.target,
            should_preempt: self.ready_thread_preempts(id)?,
        })
    }

    fn restore_ready_migration(&mut self, id: ThreadId, cpu: CpuIndex, affinity: CpuMask) {
        let restored = self
            .thread_mut(id)
            .and_then(|thread| {
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
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].run_queue.enqueue(threads, id, cpu)
    }

    fn enqueue_ready_front(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        let cpu = thread.cpu_index();
        if thread.scheduling_class() == SchedulingClass::Idle || !thread.can_run_on(cpu) {
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

    fn remove_ready(
        &mut self,
        id: ThreadId,
        cpu: CpuIndex,
        membership: QueueMembership,
    ) -> Result<(), Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot]
            .run_queue
            .remove(threads, id, cpu, membership)
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
            .wait_record_mut()
            .queue(ticket, wait_queue.identity())?;
        if let Err(error) = queue::push(&mut self.threads, queue, id, membership) {
            scheduler_invariant(error);
        }
        match self.thread_mut(id) {
            Ok(thread) => thread.set_state(ThreadState::Blocked),
            Err(error) => scheduler_invariant(error),
        }
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
        ticket: WaitTicket,
    ) -> Result<PreparedWait, Error> {
        if let Err(error) = self.remove_waiter(wait_queue, current) {
            scheduler_invariant(error);
        }
        let thread = match self.thread_mut(current) {
            Ok(thread) => thread,
            Err(error) => scheduler_invariant(error),
        };
        if thread.wait_record_mut().rollback_queued(ticket).is_err() {
            scheduler_invariant(Error::InvalidWaitRegistration);
        }
        thread.set_state(ThreadState::Running);
        Err(Error::CurrentThreadMissing)
    }

    fn cpu_slot(&self, cpu: CpuIndex) -> Result<usize, Error> {
        self.cpu_slots[cpu].ok_or(Error::CpuNotRegistered)
    }

    fn reserve_thread_slot(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        if self.threads.len() == self.threads.capacity() {
            return Err(Error::Allocation);
        }
        let id = ThreadId::from_scheduler_index(self.next_id);
        let index = usize::try_from(self.next_id).map_err(|_| Error::IdentifierExhausted)?;
        if index != self.threads.len() || self.next_id == u64::MAX {
            return Err(Error::IdentifierExhausted);
        }
        self.threads.push(None);
        // Reservation failure leaves this slot as a tombstone. Identity values
        // are observable system-wide once published, so the namespace is
        // monotonic and never rolls back or reuses an earlier value.
        self.next_id += 1;
        Ok(ThreadReservation {
            id,
            cpu,
            armed: true,
        })
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
