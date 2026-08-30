// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Public scheduler operations and architecture context-switch boundary.

mod queue;
mod state;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::marker::PhantomData;

use hyper::cpu::CpuIndex;
use hyper::sync::{InterruptMaskGuard, InterruptSpinLock};

use self::state::{PreparedContextSwitch, Scheduler, ThreadReservation};
use super::thread::{KernelThreadEntry, Thread, ThreadId, ThreadState, VcpuExecution};

use super::policy::SchedulingPolicy;
pub use super::policy::{CpuMask, ThreadPriority};
use super::wait::{WaitMobility, WaitOutcome, WaitQueue, WaitTicket};

/// Scheduler ticks granted by the initial Fair round-robin backend.
///
/// Milliseconds are rounded up so every configured nonzero quantum spans at
/// least one periodic scheduler tick. Kconfig bounds make the product exact.
const CONFIGURED_FAIR_QUANTUM_TICKS: u64 =
    (hyper::config::SCHED_FAIR_QUANTUM_MS as u64 * hyper::config::TIMER_HZ as u64).div_ceil(1_000);
const FAIR_QUANTUM_TICKS: u64 = if CONFIGURED_FAIR_QUANTUM_TICKS == 0 {
    1
} else {
    CONFIGURED_FAIR_QUANTUM_TICKS
};

type SchedulerLock = InterruptSpinLock<Option<Scheduler>, crate::hal::irq::LocalMask>;
type TransitionMask = InterruptMaskGuard<crate::hal::irq::LocalMask>;
type TransitionMaskState =
    <crate::hal::irq::LocalMask as hyper::hal::interrupt::InterruptMask>::State;

static SCHEDULER: SchedulerLock = InterruptSpinLock::new(None);

/// Scheduler ownership of one prepared, non-runnable vCPU thread.
///
/// Dropping the capability before installation removes the dormant thread.
/// The VM installation transaction consumes it after publishing the aggregate;
/// no safe API exposes the reserved `ThreadId` before that point.
/// A rollback invariant violation is fail-stop because continuing could leave
/// a scheduler-owned raw binding to an allocation about to be released.
pub(in crate::kernel) struct DormantVcpuThread {
    thread: ThreadId,
    rollback: bool,
}

/// Scheduler ownership of one non-runnable, unpublished user Thread.
pub(in crate::kernel) struct DormantUserThread {
    thread: ThreadId,
    rollback: bool,
}

impl DormantUserThread {
    pub(in crate::kernel) const fn id(&self) -> ThreadId {
        self.thread
    }

    /// Arms explicit reaper retirement and ends rollback before Process
    /// membership performs its final publication.
    pub(in crate::kernel) fn commit_before_process_publication(mut self) {
        let result = SCHEDULER.with(|slot| {
            slot.as_mut()
                .ok_or(Error::NotInitialized)?
                .arm_dormant_user(self.thread)
        });
        if result.is_err() {
            crate::hal::cpu::halt();
        }
        self.rollback = false;
    }
}

impl Drop for DormantUserThread {
    fn drop(&mut self) {
        if !self.rollback {
            return;
        }
        let thread = match SCHEDULER.with(|slot| {
            slot.as_mut()
                .ok_or(Error::NotInitialized)?
                .take_dormant_user(self.thread)
        }) {
            Ok(thread) => thread,
            Err(_) => crate::hal::cpu::halt(),
        };
        drop(thread);
    }
}

impl DormantVcpuThread {
    /// Returns the reserved identity to the VM installation transaction.
    ///
    /// # Safety
    ///
    /// The caller must not expose this identity to `thread_ready` before the
    /// VM aggregate backing this thread's binding has been installed.
    pub(in crate::kernel) const unsafe fn id_for_vm_install(&self) -> ThreadId {
        self.thread
    }

    /// Transfers the installed dormant thread to scheduler ownership.
    ///
    /// # Safety
    ///
    /// The VM aggregate backing this thread's binding must already occupy its
    /// permanent registry slot.
    pub(in crate::kernel) unsafe fn commit_after_vm_install(mut self) -> ThreadId {
        self.rollback = false;
        self.thread
    }
}

impl Drop for DormantVcpuThread {
    fn drop(&mut self) {
        if !self.rollback {
            return;
        }
        let thread = match SCHEDULER.with(|slot| {
            slot.as_mut()
                .ok_or(Error::NotInitialized)?
                .take_dormant_vcpu(self.thread)
        }) {
            Ok(thread) => thread,
            Err(error) => {
                // Continuing would let the VM transaction release the
                // allocation referenced by this scheduler-owned Thread. This
                // is a soundness boundary, so a violated rollback invariant is
                // fatal in release builds as well as debug builds. Drop can
                // run under arbitrary locks, so diagnostics are unsafe here.
                let _ = error;
                crate::hal::cpu::halt()
            }
        };
        // Drop the stack, architecture context, and raw VM binding only after
        // releasing the scheduler lock; their allocators have independent
        // locks and the enclosing VM transaction is still alive.
        drop(thread);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    NotInitialized,
    Allocation,
    ThreadLimit,
    IdentifierExhausted,
    CurrentThreadMissing,
    ThreadNotFound,
    TerminatedThread,
    ThreadBlocked,
    ThreadAlreadyQueued,
    QueueCorrupted,
    CannotBlockIdle,
    CannotSleepWithInterruptsMasked,
    CannotSleepWithPreemptionDisabled,
    IrqTailRequiresInterruptsMasked,
    UserRunRequiresInterruptsEnabled,
    InvalidThreadState,
    IdleThreadAlreadyInstalled,
    InvalidIdleTransition,
    CpuAlreadyRegistered,
    CpuNotRegistered,
    InvalidCpuIndex,
    EmptyCpuAffinity,
    NoRegisteredCpuInAffinity,
    CpuNotAllowed,
    MigrationUnsupported,
    MigrationInProgress,
    ThreadTransitionInProgress,
    WaitGenerationExhausted,
    InvalidWaitRegistration,
    MigrationBlockedByCpuLocalWait,
    PreemptionUnavailable,
    PreemptionInvariant,
    VmEntryUnavailable,
    Thread(super::thread::Error),
}

impl From<super::preempt::Error> for Error {
    fn from(error: super::preempt::Error) -> Self {
        match error {
            super::preempt::Error::Offline | super::preempt::Error::InvalidCpu => {
                Self::PreemptionUnavailable
            }
            super::preempt::Error::AlreadyOnline
            | super::preempt::Error::DisableDepthOverflow
            | super::preempt::Error::DisableDepthUnderflow
            | super::preempt::Error::IrqDepthOverflow
            | super::preempt::Error::IrqDepthUnderflow
            | super::preempt::Error::WrongCpu
            | super::preempt::Error::AlreadyDisabled => Self::PreemptionInvariant,
        }
    }
}

impl From<super::thread::Error> for Error {
    fn from(error: super::thread::Error) -> Self {
        Self::Thread(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub bootstrap_thread: ThreadId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecondaryStack {
    pub physical_top: u64,
    pub virtual_top: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct CurrentVcpu {
    pub thread: ThreadId,
    pub execution: *mut VcpuExecution,
    pub stack: (usize, usize),
}

#[derive(Clone, Copy)]
pub(crate) struct CurrentUser {
    pub thread: ThreadId,
    pub execution: *mut crate::kernel::process::UserExecution,
    pub stack: (usize, usize),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Statistics {
    pub threads: usize,
    pub real_time_class_threads: usize,
    pub fair_class_threads: usize,
    pub idle_class_threads: usize,
    pub ready: usize,
    pub running: usize,
    pub blocked: usize,
    pub migrating: usize,
    pub idle: usize,
    pub context_switches: u64,
    pub per_cpu_ready: [usize; hyper::config::MAX_CPUS as usize],
}

/// Completion state of an explicit scheduler placement change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationStatus {
    /// Assignment and any ready-queue membership already moved to the target.
    Completed,
    /// Asynchronous source-context handoff was accepted.
    ///
    /// IRQ-tail preemption may complete the handoff before the requesting call
    /// returns. This status is therefore not a completion token; callers which
    /// need observation must query placement or use a higher-level handshake.
    Pending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CrashTaskSnapshot {
    pub id: ThreadId,
    pub state: ThreadState,
    pub execution: super::thread::ExecutionKind,
    pub stack: Option<(usize, usize)>,
    pub stack_statistics: Option<crate::kernel::mm::stack::StackStatistics>,
}

#[must_use = "a committed park must retain its IRQ mask through context handoff"]
pub(crate) struct ParkCommit {
    switch: PreparedContextSwitch,
    ticket: WaitTicket,
}

#[must_use = "parking is committed only after the prepared context switch is consumed"]
pub(crate) struct ParkToken {
    transition: PreparedTransition,
    ticket: WaitTicket,
}

/// Armed current-Thread wait which must be finished or committed to a park.
#[must_use = "an armed wait registration must be finished or parked"]
pub(crate) struct WaitRegistration {
    ticket: WaitTicket,
    active: bool,
    // Registrations belong to the Thread continuation which armed them.
    not_send_or_sync: PhantomData<Rc<()>>,
}

impl WaitRegistration {
    pub(crate) const fn ticket(&self) -> WaitTicket {
        self.ticket
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for WaitRegistration {
    fn drop(&mut self) {
        if self.active {
            // This linear owner can be abandoned from arbitrary lock/IRQ
            // context. Do not enter diagnostics while scheduler state still
            // retains the registered wait.
            crate::hal::cpu::halt()
        }
    }
}

pub(crate) enum PrepareWait {
    Park(ParkCommit),
    Completed(WaitOutcome),
}

pub(crate) enum PreparedWait {
    Park(ParkToken),
    Completed(WaitOutcome),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolveWait {
    pub won: bool,
    pub made_ready: bool,
}

struct PreparedTransition {
    switch: PreparedContextSwitch,
    interrupt_mask: TransitionMask,
}

impl PreparedTransition {
    fn activate(self) {
        let Self {
            switch,
            interrupt_mask,
        } = self;
        // SAFETY: The architecture switch stores this exact state into the
        // outgoing ThreadContext before changing stacks. Its incoming tail
        // retires source ownership before the state can be restored.
        let restore_state = unsafe { interrupt_mask.into_restore_state() };
        switch.activate(restore_state);
    }
}

pub fn initialize() -> Result<Capabilities, Error> {
    let cpu = current_cpu()?;
    if SCHEDULER.with(|slot| slot.is_some()) {
        return Err(Error::AlreadyInitialized);
    }
    let scheduler = Scheduler::new(cpu)?;
    let preemption = super::preempt::prepare_cpu(cpu)?;
    let capabilities = SCHEDULER.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(scheduler);
        Ok(Capabilities {
            bootstrap_thread: ThreadId::BOOTSTRAP,
        })
    })?;
    preemption.commit();
    Ok(capabilities)
}

pub fn current_thread_id() -> Result<ThreadId, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        slot.as_ref()
            .ok_or(Error::NotInitialized)?
            .current_thread(cpu)
    })
}

pub fn statistics() -> Result<Statistics, Error> {
    SCHEDULER.with(|slot| Ok(slot.as_ref().ok_or(Error::NotInitialized)?.statistics()))
}

pub fn thread_stack_statistics(
    id: ThreadId,
) -> Result<Option<crate::kernel::mm::stack::StackStatistics>, Error> {
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_ref().ok_or(Error::NotInitialized)?;
        let thread = scheduler.thread(id)?;
        if matches!(
            thread.state(),
            ThreadState::Running | ThreadState::Idle | ThreadState::Migrating
        ) || !scheduler.context_is_stopped(id)?
        {
            return Err(Error::InvalidThreadState);
        }
        Ok(thread.kernel_stack_statistics())
    })
}

/// Captures current-task metadata without waiting on a potentially held lock.
pub(crate) fn crash_snapshot(cpu: usize) -> Option<CrashTaskSnapshot> {
    let cpu = CpuIndex::new(cpu)?;
    SCHEDULER
        .try_with(|slot| {
            let scheduler = slot.as_ref()?;
            let thread = scheduler.thread(scheduler.current_thread(cpu).ok()?).ok()?;
            Some(CrashTaskSnapshot {
                id: thread.id(),
                state: thread.state(),
                execution: thread.execution_kind(),
                stack: thread.kernel_stack_bounds(),
                stack_statistics: thread.kernel_stack_statistics(),
            })
        })
        .flatten()
}

pub fn register_secondary_cpu(cpu: CpuIndex, name: &str) -> Result<SecondaryStack, Error> {
    let preemption = super::preempt::prepare_cpu(cpu)?;
    let reservation = reserve_thread(|scheduler| scheduler.reserve_secondary(cpu))?;
    let thread =
        match prepare_boxed_thread(Thread::secondary_bootstrap(reservation.id(), cpu, name)) {
            Ok(thread) => thread,
            Err(error) => {
                abandon_reservation(reservation)?;
                return Err(error);
            }
        };
    let stack = publish_secondary(reservation, thread)?;
    preemption.commit();
    Ok(stack)
}

pub fn kthread_create(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
) -> Result<ThreadId, Error> {
    kthread_create_with_policy_and_affinity(
        name,
        entry,
        argument,
        SchedulingPolicy::fair(),
        CpuMask::ALL,
    )
}

/// Creates a dormant kernel thread constrained to `affinity`.
///
/// The scheduler prefers the calling CPU when it is admitted and registered;
/// otherwise it deterministically selects the lowest-numbered registered CPU
/// in the mask. Explicit migration and affinity updates retain this constraint.
pub fn kthread_create_with_affinity(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
    affinity: CpuMask,
) -> Result<ThreadId, Error> {
    kthread_create_with_policy_and_affinity(
        name,
        entry,
        argument,
        SchedulingPolicy::fair(),
        affinity,
    )
}

/// Creates a dormant real-time FIFO kernel thread.
pub fn kthread_create_fifo(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
    priority: ThreadPriority,
) -> Result<ThreadId, Error> {
    kthread_create_fifo_with_affinity(name, entry, argument, priority, CpuMask::ALL)
}

/// Creates a dormant real-time FIFO kernel thread with explicit affinity.
pub fn kthread_create_fifo_with_affinity(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
    priority: ThreadPriority,
    affinity: CpuMask,
) -> Result<ThreadId, Error> {
    kthread_create_with_policy_and_affinity(
        name,
        entry,
        argument,
        SchedulingPolicy::fifo(priority),
        affinity,
    )
}

fn kthread_create_with_policy_and_affinity(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
    policy: SchedulingPolicy,
    affinity: CpuMask,
) -> Result<ThreadId, Error> {
    let cpu = current_cpu()?;
    let reservation = reserve_thread(|scheduler| scheduler.reserve_kernel_thread(cpu, affinity))?;
    let id = reservation.id();
    let mut thread = match prepare_boxed_thread(Thread::kernel(
        id,
        reservation.cpu(),
        affinity,
        name,
        entry,
        argument,
    )) {
        Ok(thread) => thread,
        Err(error) => {
            abandon_reservation(reservation)?;
            return Err(error);
        }
    };
    if !thread.set_scheduling_policy(policy) {
        abandon_reservation(reservation)?;
        drop(thread);
        return Err(Error::InvalidThreadState);
    }
    publish_thread(reservation, thread)?;
    Ok(id)
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn thread_placement(id: ThreadId) -> Result<(CpuIndex, CpuMask), Error> {
    SCHEDULER.with(|slot| {
        let thread = slot.as_ref().ok_or(Error::NotInitialized)?.thread(id)?;
        Ok((thread.cpu_index(), thread.affinity()))
    })
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn registry_slot_count() -> Result<usize, Error> {
    SCHEDULER.with(|slot| {
        slot.as_ref()
            .ok_or(Error::NotInitialized)
            .map(Scheduler::registry_slot_count)
    })
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn discard_dormant_kernel_thread(id: ThreadId) -> Result<(), Error> {
    let thread = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .take_dormant_kernel_thread(id)
    })?;
    drop(thread);
    Ok(())
}

/// Confirms that scheduler registry backing was prepared during initialization.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn prepare_thread_accounting_probe() -> Result<(), Error> {
    SCHEDULER.with(|slot| slot.as_ref().map(|_| ()).ok_or(Error::NotInitialized))
}

pub(in crate::kernel) fn vcpu_create(
    name: &str,
    vm: crate::kernel::vm::registry::VmBinding,
    vcpu_id: u32,
    context: crate::hal::vm::VcpuContext,
    entry_ready: &crate::hal::vm::VmEntryReady,
    entry: KernelThreadEntry,
) -> Result<DormantVcpuThread, Error> {
    let cpu = current_cpu()?;
    let execution = VcpuExecution::installed(vm, vcpu_id, context, entry_ready)?;
    let reservation = reserve_thread(|scheduler| scheduler.reserve_vcpu_thread(cpu))?;
    let id = reservation.id();
    let thread =
        match prepare_boxed_thread(Thread::vcpu(id, reservation.cpu(), name, execution, entry)) {
            Ok(thread) => thread,
            Err(error) => {
                abandon_reservation(reservation)?;
                return Err(error);
            }
        };
    publish_thread(reservation, thread)?;
    Ok(DormantVcpuThread {
        thread: id,
        rollback: true,
    })
}

pub(in crate::kernel) fn prepare_user_thread(
    name: &str,
    execution: alloc::boxed::Box<crate::kernel::process::UserExecution>,
    entry: KernelThreadEntry,
    affinity: CpuMask,
) -> Result<DormantUserThread, Error> {
    let cpu = current_cpu()?;
    let reservation = reserve_thread(|scheduler| scheduler.reserve_kernel_thread(cpu, affinity))?;
    let id = reservation.id();
    let thread = match prepare_boxed_thread(Thread::user(
        id,
        reservation.cpu(),
        affinity,
        name,
        execution,
        entry,
    )) {
        Ok(thread) => thread,
        Err(error) => {
            abandon_reservation(reservation)?;
            return Err(error);
        }
    };
    publish_thread(reservation, thread)?;
    Ok(DormantUserThread {
        thread: id,
        rollback: true,
    })
}

pub(in crate::kernel) fn request_user_thread_stop(
    id: ThreadId,
    reason: crate::kernel::process::TerminalReason,
) -> Result<(), Error> {
    let target = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .request_user_stop(id, reason)
    })?;
    if let Some(cpu) = target {
        request_reschedule(cpu)?;
    }
    Ok(())
}

fn prepare_boxed_thread(
    thread: Result<Thread, super::thread::Error>,
) -> Result<Box<Thread>, Error> {
    hyper::mm::try_box(thread?).map_err(|_| Error::Allocation)
}

fn reserve_thread(
    reserve: impl Fn(&mut Scheduler) -> Result<ThreadReservation, Error>,
) -> Result<ThreadReservation, Error> {
    SCHEDULER.with(|slot| reserve(slot.as_mut().ok_or(Error::NotInitialized)?))
}

fn publish_thread(mut reservation: ThreadReservation, thread: Box<Thread>) -> Result<(), Error> {
    let (result, retired) = SCHEDULER.with(|slot| match slot.as_mut() {
        Some(scheduler) => {
            let result = match scheduler.publish_thread(&reservation, thread) {
                Ok(()) => Ok(()),
                Err((error, thread)) => match scheduler.abandon_reservation(&reservation) {
                    Ok(()) => Err((error, thread)),
                    Err(_) => crate::hal::cpu::halt(),
                },
            };
            (result, true)
        }
        None => (Err((Error::NotInitialized, thread)), false),
    });
    if retired {
        reservation.disarm();
    }
    match result {
        Ok(()) => Ok(()),
        Err((error, thread)) => {
            drop(thread);
            Err(error)
        }
    }
}

fn publish_secondary(
    mut reservation: ThreadReservation,
    thread: Box<Thread>,
) -> Result<SecondaryStack, Error> {
    let (result, retired) = SCHEDULER.with(|slot| match slot.as_mut() {
        Some(scheduler) => {
            let result = match scheduler.publish_secondary(&reservation, thread) {
                Ok(stack) => Ok(stack),
                Err((error, thread)) => match scheduler.abandon_reservation(&reservation) {
                    Ok(()) => Err((error, thread)),
                    Err(_) => crate::hal::cpu::halt(),
                },
            };
            (result, true)
        }
        None => (Err((Error::NotInitialized, thread)), false),
    });
    if retired {
        reservation.disarm();
    }
    match result {
        Ok(stack) => Ok(stack),
        Err((error, thread)) => {
            drop(thread);
            Err(error)
        }
    }
}

fn abandon_reservation(mut reservation: ThreadReservation) -> Result<(), Error> {
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .abandon_reservation(&reservation)
    })?;
    reservation.disarm();
    Ok(())
}

/// Returns the pinned vCPU payload owned by the calling CPU's current Thread.
pub(crate) fn current_vcpu() -> Result<CurrentVcpu, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .current_vcpu(cpu)
    })
}

/// Returns the pinned native-user payload owned by the current Thread.
///
/// The dedicated guard proves that the raw pointer cannot migrate or be
/// reclaimed until the caller closes its machine-active borrow.
pub(crate) fn current_user(_pin: &UserRunGuard) -> Result<CurrentUser, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .current_user(cpu)
    })
}

/// Returns the pinned vCPU payload when the current Thread owns one.
///
/// This does not borrow or publish the execution. The raw owner pointer is
/// consumed only by the masked architecture IRQ-tail continuation, where the
/// current Thread cannot change before the scheduler transaction begins.
#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn current_vcpu_if_present() -> Result<Option<CurrentVcpu>, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .current_vcpu_if_present(cpu)
    })
}

/// Enqueues a dormant thread on its owning CPU's priority ready queue.
pub fn thread_ready(id: ThreadId) -> Result<bool, Error> {
    let outcome =
        SCHEDULER.with(|slot| slot.as_mut().ok_or(Error::NotInitialized)?.make_ready(id))?;
    publish_ready_outcome(outcome)?;
    Ok(outcome.changed)
}

/// Publishes native-user lifecycle and scheduler readiness as one transaction.
pub(in crate::kernel) fn ready_user_thread(id: ThreadId) -> Result<bool, Error> {
    let outcome = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let thread = scheduler.thread(id)?;
        if !matches!(
            thread.state(),
            ThreadState::Dormant | ThreadState::Ready | ThreadState::Running
        ) {
            return Err(Error::InvalidThreadState);
        }
        let execution = thread.user_execution().ok_or(Error::InvalidThreadState)?;
        execution
            .thread()
            .mark_runnable()
            .map_err(|_| Error::InvalidThreadState)?;
        match scheduler.make_ready(id) {
            Ok(outcome) => Ok(outcome),
            Err(_) => crate::hal::cpu::halt(),
        }
    })?;
    publish_ready_outcome(outcome)?;
    Ok(outcome.changed)
}

/// Moves a kernel Thread to one allowed registered CPU.
///
/// Dormant, Ready, and fully stopped Blocked Threads move synchronously. A
/// running or switch-in-flight Thread returns [`MigrationStatus::Pending`]
/// when deferred handoff is accepted; target publication occurs only after the
/// source context is saved and may finish before the caller observes the return.
/// Idle, bootstrap, user, and vCPU Threads do not currently expose a safe
/// migration contract and are rejected.
pub fn migrate_thread(id: ThreadId, target: CpuIndex) -> Result<MigrationStatus, Error> {
    let outcome = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .migrate_thread(id, target)
    })?;
    publish_migration_outcome(outcome)
}

/// Replaces a kernel Thread's CPU affinity and migrates it when required.
///
/// The affinity update and assignment change are one scheduler transaction.
/// If the current assignment remains allowed, the Thread keeps its run-queue
/// position. Otherwise the completion semantics match [`migrate_thread`].
pub fn set_thread_affinity(id: ThreadId, affinity: CpuMask) -> Result<MigrationStatus, Error> {
    let outcome = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .set_thread_affinity(id, affinity)
    })?;
    publish_migration_outcome(outcome)
}

/// Assigns or updates a thread's real-time FIFO policy.
///
/// Calling this for a Fair thread is an explicit class transition into RT.
pub fn set_thread_fifo_policy(id: ThreadId, priority: ThreadPriority) -> Result<(), Error> {
    let target = update_thread_fifo_policy(id, priority)?;
    if let Some(cpu) = target {
        request_reschedule(cpu)?;
    }
    Ok(())
}

fn update_thread_fifo_policy(
    id: ThreadId,
    priority: ThreadPriority,
) -> Result<Option<CpuIndex>, Error> {
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .set_fifo_policy(id, priority)
    })
}

/// Applies FIFO policy and reports whether the transition requested a switch.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn set_thread_fifo_policy_for_test(
    id: ThreadId,
    priority: ThreadPriority,
) -> Result<bool, Error> {
    let target = update_thread_fifo_policy(id, priority)?;
    if let Some(cpu) = target {
        request_reschedule(cpu)?;
    }
    Ok(target.is_some())
}

/// Assigns a thread to the ordinary Fair scheduling class.
///
/// Ready threads are moved atomically between class queues. Lowering a running
/// RT FIFO thread to Fair publishes a deferred preemption request when RT work
/// is already ready; the caller never switches inside this operation.
pub fn set_thread_fair_policy(id: ThreadId) -> Result<(), Error> {
    let target = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .set_fair_policy(id)
    })?;
    if let Some(cpu) = target {
        request_reschedule(cpu)?;
    }
    Ok(())
}

/// Charges periodic scheduler ticks and publishes a deferred Fair preemption.
///
/// This function is allocation-free and may run in IRQ context. It never
/// switches directly; the architecture IRQ-tail continuation consumes the
/// coalesced request after completing interrupt accounting.
pub(crate) fn account_tick(elapsed_ticks: u64) -> Result<(), Error> {
    let cpu = current_cpu()?;
    let should_reschedule = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .account_tick(cpu, elapsed_ticks)
    })?;
    if should_reschedule {
        // The real timer path is already inside IRQ accounting, so the common
        // request helper suppresses a redundant self-IPI. Keeping the generic
        // path also makes direct accounting calls prompt a scheduling point.
        request_reschedule(cpu)?;
    }
    Ok(())
}

pub fn yield_now() -> Result<(), Error> {
    ensure_sleepable()?;
    if let Some(pair) = prepare_schedule()? {
        pair.activate();
    }
    Ok(())
}

/// Reconsiders the current thread at an explicit safe point.
///
/// Unlike `yield_now`, this rotates Fair peers only after slice expiry and
/// never rotates equal-priority RT FIFO peers.
pub fn cond_resched() -> Result<bool, Error> {
    ensure_sleepable()?;
    cond_resched_inner()
}

/// Reconsiders scheduling from an outermost architecture IRQ continuation.
///
/// IRQ accounting and controller completion must already be complete. This
/// operation is AArch64-only until secondary architectures provide equivalent
/// private-stack exception continuations and interrupt-state context transfer.
#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn cond_resched_from_irq_tail() -> Result<bool, Error> {
    if crate::hal::irq::local_enabled() {
        return Err(Error::IrqTailRequiresInterruptsMasked);
    }
    let cpu = current_cpu()?;
    if !super::preempt::can_reschedule(cpu)? {
        return Ok(false);
    }
    cond_resched_inner()
}

fn cond_resched_inner() -> Result<bool, Error> {
    let cpu = current_cpu()?;
    if !super::preempt::pending(cpu)? || !super::preempt::can_reschedule(cpu)? {
        return Ok(false);
    }
    // SAFETY: This continuation transfers the exact prior interrupt state into
    // its saved ThreadContext at the final machine-switch boundary.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let pair = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.prepare_preemption(cpu)
    })?;
    let Some(pair) = pair else {
        return Ok(false);
    };
    PreparedTransition {
        switch: pair,
        interrupt_mask,
    }
    .activate();
    Ok(true)
}

/// Capability proving that the current thread cannot pass a preemption point.
///
/// The guard is CPU-local. It may protect a bounded CPU-local borrow but must
/// not cross a blocking operation or guest entry.
#[must_use = "dropping the guard restores preemption without scheduling"]
pub struct PreemptionGuard(super::preempt::PreemptionGuard);

// SAFETY: The underlying per-CPU disable-depth owner prevents every scheduler
// preemption point from migrating this context until the guard is released.
unsafe impl hyper::cpu::PinnedExecution for PreemptionGuard {}

/// Exclusive preemption level used by a native-user machine run.
///
/// Unlike a generic guard this can be created only from depth zero, making
/// the IRQ-tail `depth == 1` unwind rule exact and auditable.
#[must_use = "native-user pinning must span active translation and machine state"]
pub(crate) struct UserRunGuard(super::preempt::PreemptionGuard);

// SAFETY: Construction atomically changes the current CPU's disable depth
// from zero to one while IRQs are masked. The guard cannot move safely between
// Rust threads and its underlying token rejects release on a different CPU.
unsafe impl hyper::cpu::PinnedExecution for UserRunGuard {}

pub(crate) fn user_run_guard() -> Result<UserRunGuard, Error> {
    if !crate::hal::irq::local_enabled() {
        return Err(Error::UserRunRequiresInterruptsEnabled);
    }
    super::preempt::disable_for_user_run()
        .map(UserRunGuard)
        .map_err(Into::into)
}

pub(crate) fn finish_user_run(guard: UserRunGuard) -> Result<bool, Error> {
    if !guard.0.release()? {
        return Err(Error::PreemptionInvariant);
    }
    cond_resched()
}

/// Prevents asynchronous scheduling while CPU-local state is borrowed.
pub fn preempt_disable() -> Result<PreemptionGuard, Error> {
    super::preempt::disable()
        .map(PreemptionGuard)
        .map_err(Into::into)
}

/// Releases a preemption guard and immediately observes deferred requests.
pub fn preempt_enable_and_reschedule(guard: PreemptionGuard) -> Result<bool, Error> {
    if guard.0.release()? {
        cond_resched()
    } else {
        Ok(false)
    }
}

pub(crate) fn install_current_idle() -> Result<(usize, usize), Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .install_current_as_idle(cpu)
    })
}

extern "C" fn enter_clean_idle(_argument: usize) -> ! {
    crate::hal::irq::enable_local();
    run_idle_loop()
}

extern "C" fn idle_thread_entry(argument: usize) {
    enter_clean_idle(argument)
}

pub(crate) fn run_idle_loop() -> ! {
    run_idle_loop_inner(None)
}

/// Enters the idle loop and publishes secondary readiness at its first queue check.
pub(crate) fn run_idle_loop_after(ready: fn()) -> ! {
    run_idle_loop_inner(Some(ready))
}

fn run_idle_loop_inner(mut ready: Option<fn()>) -> ! {
    loop {
        if let Err(error) = idle_wait_or_schedule(ready.take()) {
            crate::kernel::crash::fatal(format_args!("HypeR: idle scheduling failed: {error:?}"));
        }
    }
}

/// Closes the idle queue-check-to-sleep window under one interrupt mask.
fn idle_wait_or_schedule(ready: Option<fn()>) -> Result<(), Error> {
    let cpu = current_cpu()?;
    reap_terminated_threads()?;
    // SAFETY: The guard remains on this CPU. It is either consumed directly
    // into an outgoing context or dropped after the architecture returns from
    // its masked idle wait.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let switch = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let switch = scheduler.prepare_yield(cpu)?;
        // The callback must be allocation-free and must not re-enter the
        // scheduler. Publish only after the first queue observation completed
        // successfully, while retaining the lock that excludes a boot-side
        // enqueue between admission and that observation.
        if let Some(ready) = ready {
            ready();
        }
        Ok::<Option<PreparedContextSwitch>, Error>(switch)
    })?;
    if let Some(switch) = switch {
        PreparedTransition {
            switch,
            interrupt_mask,
        }
        .activate();
    } else {
        crate::hal::cpu::wait_for_interrupt_masked();
        drop(interrupt_mask);
    }
    Ok(())
}

pub(crate) fn prepare_park(wait_queue: &WaitQueue) -> Result<ParkToken, Error> {
    ensure_sleepable()?;
    // SAFETY: The park token retains the outer mask until its exact saved state
    // is consumed into the outgoing ThreadContext at the machine-switch boundary.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let registration = begin_wait(WaitMobility::Migratable)?;
    match prepare_registered_park_locked(wait_queue, registration)? {
        PrepareWait::Park(commit) => Ok(retain_park_mask(commit, interrupt_mask)),
        PrepareWait::Completed(_) => Err(Error::InvalidWaitRegistration),
    }
}

/// Arms a generation-qualified wait owned by the current Thread.
pub(crate) fn begin_wait(mobility: WaitMobility) -> Result<WaitRegistration, Error> {
    let cpu = current_cpu()?;
    let ticket = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.arm_wait(cpu, mobility)
    })?;
    Ok(WaitRegistration {
        ticket,
        active: true,
        not_send_or_sync: PhantomData,
    })
}

/// Consumes an unqueued registration under its condition-object lock.
///
/// `None` means no resolver won and the caller may consume the protected
/// condition. `Some` means timeout/cancellation already won and condition
/// state must remain untouched.
pub(crate) fn finish_wait(
    mut registration: WaitRegistration,
) -> Result<Option<WaitOutcome>, Error> {
    let cpu = current_cpu()?;
    let result = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .finish_unqueued_wait(cpu, registration.ticket)
    });
    if result.is_ok() {
        registration.disarm();
    }
    result
}

/// Atomically queues an armed registration or consumes an earlier resolution.
///
/// The caller holds the condition object's IRQ-masking lock, closing the
/// condition-check-to-park window. Queue membership and the embedded wait
/// record are published in one scheduler-lock transaction.
pub(crate) fn prepare_registered_park_locked(
    wait_queue: &WaitQueue,
    mut registration: WaitRegistration,
) -> Result<PrepareWait, Error> {
    let cpu = current_cpu()?;
    let result = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        match scheduler.prepare_registered_park(cpu, wait_queue, registration.ticket)? {
            state::PreparedWait::Park { switch, ticket } => {
                Ok(PrepareWait::Park(ParkCommit { switch, ticket }))
            }
            state::PreparedWait::Completed(outcome) => Ok(PrepareWait::Completed(outcome)),
        }
    });
    if matches!(&result, Ok(_) | Err(Error::CurrentThreadMissing)) {
        // CurrentThreadMissing is the one recoverable prepare failure: state
        // rolled queue membership and WaitRecord back to Idle before return.
        registration.disarm();
    }
    result
}

/// Closes the registration-to-park boundary for a wait without an enclosing
/// condition-object lock.
pub(crate) fn prepare_registered_park(
    wait_queue: &WaitQueue,
    registration: WaitRegistration,
) -> Result<PreparedWait, Error> {
    // SAFETY: either the committed park consumes this exact mask state, or the
    // already-completed path drops it before returning to the caller.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    match prepare_registered_park_locked(wait_queue, registration)? {
        PrepareWait::Park(commit) => {
            Ok(PreparedWait::Park(retain_park_mask(commit, interrupt_mask)))
        }
        PrepareWait::Completed(outcome) => {
            drop(interrupt_mask);
            Ok(PreparedWait::Completed(outcome))
        }
    }
}

/// Binds a synchronization lock's retained interrupt mask to a committed park.
pub(crate) fn retain_park_mask(commit: ParkCommit, interrupt_mask: TransitionMask) -> ParkToken {
    ParkToken {
        transition: PreparedTransition {
            switch: commit.switch,
            interrupt_mask,
        },
        ticket: commit.ticket,
    }
}

pub(crate) fn complete_park(token: ParkToken) -> WaitOutcome {
    token.transition.activate();
    let result = current_cpu().and_then(|cpu| {
        SCHEDULER.with(|slot| {
            slot.as_mut()
                .ok_or(Error::NotInitialized)?
                .finish_completed_wait(cpu, token.ticket)
        })
    });
    match result {
        Ok(outcome) => outcome,
        Err(error) => scheduler_invariant("committed wait completion", error),
    }
}

pub(crate) fn wake_one(wait_queue: &WaitQueue) -> Result<Option<ThreadId>, Error> {
    wake_one_with(wait_queue, |_| {})
}

pub(crate) fn wake_one_with(
    wait_queue: &WaitQueue,
    before_ready: impl FnOnce(ThreadId),
) -> Result<Option<ThreadId>, Error> {
    let awakened = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.notify_one_with(wait_queue, before_ready)
    })?;
    if let Some((_, outcome)) = awakened {
        publish_committed_ready(outcome);
    }
    Ok(awakened.map(|(id, _)| id))
}

pub(crate) fn wake_all(wait_queue: &WaitQueue) -> Result<usize, Error> {
    let (count, targets) = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let mut count = 0usize;
        let mut targets = [false; hyper::cpu::MAX_CPUS];
        while let Some((_, outcome)) = scheduler.notify_one_with(wait_queue, |_| {})? {
            if outcome.should_preempt {
                targets[outcome.target_cpu.get()] = true;
            }
            count += 1;
        }
        Ok::<_, Error>((count, targets))
    })?;
    for (index, requested) in targets.into_iter().enumerate() {
        if requested {
            let Some(cpu) = CpuIndex::new(index) else {
                scheduler_invariant("wait-all CPU publication", Error::InvalidCpuIndex);
            };
            if let Err(error) = request_reschedule(cpu) {
                scheduler_invariant("wait-all ready publication", error);
            }
        }
    }
    if count != 0 {
        crate::hal::cpu::send_event();
    }
    Ok(count)
}

/// Attempts exact-ticket timeout or cancellation arbitration.
///
/// Stale tickets and already-completed registrations are clean losers. A
/// queued winner is made runnable under the scheduler lock; reschedule/event
/// publication happens only after releasing that lock.
pub(crate) fn resolve_wait(ticket: WaitTicket, outcome: WaitOutcome) -> Result<ResolveWait, Error> {
    if outcome == WaitOutcome::Notified {
        return Err(Error::InvalidWaitRegistration);
    }
    let resolved = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .resolve_wait(ticket, outcome)
    })?;
    if let Some(ready) = resolved.ready {
        publish_committed_ready(ready);
    }
    Ok(ResolveWait {
        won: resolved.won,
        made_ready: resolved.ready.is_some(),
    })
}

pub(crate) fn cancel_waiter(wait_queue: &WaitQueue, id: ThreadId) -> Result<bool, Error> {
    let resolved = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .cancel_waiter(wait_queue, id)
    })?;
    if let Some(ready) = resolved.ready {
        publish_committed_ready(ready);
    }
    Ok(resolved.won)
}

pub(crate) fn waiter_count(wait_queue: &WaitQueue) -> Result<usize, Error> {
    SCHEDULER.with(|slot| {
        let _ = slot.as_ref().ok_or(Error::NotInitialized)?;
        // SAFETY: SCHEDULER is held exclusively for all WaitQueue access.
        Ok(unsafe { &*wait_queue.state_pointer() }.len)
    })
}

pub(crate) fn ensure_sleepable() -> Result<(), Error> {
    if !crate::hal::irq::local_enabled() {
        return Err(Error::CannotSleepWithInterruptsMasked);
    }
    let cpu = current_cpu()?;
    super::preempt::can_reschedule(cpu)?
        .then_some(())
        .ok_or(Error::CannotSleepWithPreemptionDisabled)
}

fn publish_ready_outcome(outcome: state::ReadyOutcome) -> Result<(), Error> {
    if outcome.should_preempt {
        request_reschedule(outcome.target_cpu)?;
    } else if outcome.changed {
        crate::hal::cpu::send_event();
    }
    Ok(())
}

fn publish_committed_ready(outcome: state::ReadyOutcome) {
    if let Err(error) = publish_ready_outcome(outcome) {
        scheduler_invariant("committed ready publication", error);
    }
}

fn publish_migration_outcome(outcome: state::MigrationOutcome) -> Result<MigrationStatus, Error> {
    if let Some(ready) = outcome.target_ready {
        publish_ready_outcome(ready)?;
    }
    if let Some(source) = outcome.source_reschedule {
        request_reschedule(source)?;
    }
    Ok(outcome.status)
}

fn scheduler_invariant(operation: &str, error: Error) -> ! {
    // Callers cross this boundary only after releasing the scheduler lock, so
    // coordinated crash-stop can freeze sibling CPUs and preserve snapshots.
    crate::kernel::crash::fatal(format_args!(
        "HypeR: {operation} invariant failed: {error:?}"
    ))
}

fn request_reschedule(cpu: CpuIndex) -> Result<(), Error> {
    // Determine local IRQ ownership before arming the coalesced request. Once
    // the pending bit elects this caller as notifier, notification must not
    // fail in a way which leaves later publishers suppressed behind that bit.
    let notify = super::preempt::notification_required(cpu)?;
    if super::preempt::request(cpu)? && notify {
        super::super::irq::reschedule::notify(cpu);
    }
    Ok(())
}

fn prepare_schedule() -> Result<Option<PreparedTransition>, Error> {
    let cpu = current_cpu()?;
    reap_terminated_threads()?;
    // SAFETY: This continuation transfers the exact prior interrupt state into
    // its saved ThreadContext at the final machine-switch boundary.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let switch = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.prepare_yield(cpu)
    })?;
    Ok(switch.map(|switch| PreparedTransition {
        switch,
        interrupt_mask,
    }))
}

/// Detaches one stopped Thread at a time, then releases its owned resources
/// without holding the global IRQ-masking scheduler lock.
fn reap_terminated_threads() -> Result<(), Error> {
    loop {
        let thread = SCHEDULER.with(|slot| {
            slot.as_mut()
                .ok_or(Error::NotInitialized)?
                .detach_terminated()
        })?;
        let Some(mut thread) = thread else {
            return Ok(());
        };
        if let Some(execution) = thread.take_user_execution() {
            drop(thread);
            (*execution).complete_detach();
            continue;
        }
        drop(thread);
    }
}

/// Terminates the current non-idle kernel Thread and schedules a successor.
pub(crate) fn exit_current() -> ! {
    kernel_thread_exit()
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_exit() -> ! {
    let cpu = match current_cpu() {
        Ok(cpu) => cpu,
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: thread exit on invalid CPU: {error:?}"
        )),
    };
    // SAFETY: The exiting continuation is CPU-pinned and transfers this outer
    // mask directly into the non-returning prepared context transition.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let result = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .prepare_exit(cpu)
    });
    match result {
        Ok(pair) => {
            PreparedTransition {
                switch: pair,
                interrupt_mask,
            }
            .activate();
            crate::kernel::crash::fatal(format_args!(
                "HypeR: terminated thread context resumed unexpectedly"
            ));
        }
        Err(error) => {
            crate::kernel::crash::fatal(format_args!("HypeR: thread exit failed: {error:?}"))
        }
    }
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpuIndex)
}

/// Completes source ownership on the incoming stack with interrupts masked.
extern "C" fn finish_context_switch_tail() {
    let result = current_cpu().and_then(|cpu| {
        SCHEDULER.with(|slot| {
            slot.as_mut()
                .ok_or(Error::NotInitialized)?
                .complete_incoming_switch(cpu)
        })
    });
    match result {
        Ok(Some(outcome)) => {
            if let Err(error) = publish_ready_outcome(outcome) {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: switch-tail migration publication failed: {error:?}"
                ));
            }
        }
        Ok(None) => {}
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: incoming context-switch completion failed: {error:?}"
        )),
    }
}
