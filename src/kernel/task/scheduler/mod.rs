// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Public scheduler operations and architecture context-switch boundary.

mod queue;
mod state;

use hyper::cpu::CpuIndex;
use hyper::sync::{InterruptMaskGuard, InterruptSpinLock};

use self::state::{PreparedContextSwitch, Scheduler};
use super::thread::{KernelThreadEntry, ThreadId, ThreadState, VcpuExecution};

use super::policy::SchedulingPolicy;
pub use super::policy::{CpuMask, ThreadPriority};
use super::wait::WaitQueue;

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
                // fatal in release builds as well as debug builds.
                crate::pr_crit!(
                    "HypeR: dormant vCPU {} rollback failed: {error:?}",
                    self.thread.get()
                );
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
    InvalidThreadState,
    IdleThreadAlreadyInstalled,
    InvalidIdleTransition,
    CpuAlreadyRegistered,
    CpuNotRegistered,
    InvalidCpuIndex,
    EmptyCpuAffinity,
    NoRegisteredCpuInAffinity,
    PreemptionUnavailable,
    PreemptionInvariant,
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
            | super::preempt::Error::WrongCpu => Self::PreemptionInvariant,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Statistics {
    pub threads: usize,
    pub real_time_class_threads: usize,
    pub fair_class_threads: usize,
    pub idle_class_threads: usize,
    pub ready: usize,
    pub running: usize,
    pub blocked: usize,
    pub idle: usize,
    pub context_switches: u64,
    pub per_cpu_ready: [usize; hyper::config::MAX_CPUS as usize],
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
pub(crate) struct ParkCommit(PreparedContextSwitch);

#[must_use = "parking is committed only after the prepared context switch is consumed"]
pub(crate) struct ParkToken(PreparedTransition);

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
        switch.activate();
        // Assembly resumes this continuation with the switch-boundary mask;
        // the guard restores the state it owned before scheduler commit.
        drop(interrupt_mask);
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
        let thread = slot.as_ref().ok_or(Error::NotInitialized)?.thread(id)?;
        if matches!(thread.state(), ThreadState::Running | ThreadState::Idle) {
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
    let stack = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .register_secondary(cpu, name)
    })?;
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
/// in the mask. The complete mask is retained for future migration policy.
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
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .create_kernel_thread(cpu, affinity, name, entry, argument, policy)
    })
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn thread_placement(id: ThreadId) -> Result<(CpuIndex, CpuMask), Error> {
    SCHEDULER.with(|slot| {
        let thread = slot.as_ref().ok_or(Error::NotInitialized)?.thread(id)?;
        Ok((thread.cpu_index(), thread.affinity()))
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

pub(in crate::kernel) fn vcpu_create(
    name: &str,
    vm: crate::kernel::vm::registry::VmBinding,
    vcpu_id: u32,
    context: crate::hal::vm::VcpuContext,
    entry: KernelThreadEntry,
) -> Result<DormantVcpuThread, Error> {
    let cpu = current_cpu()?;
    let thread = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .create_vcpu_thread(cpu, name, vm, vcpu_id, context, entry)
    })?;
    Ok(DormantVcpuThread {
        thread,
        rollback: true,
    })
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
    // SAFETY: This CPU-pinned scheduler continuation owns the outer transition
    // mask until `PreparedTransition::activate` resumes it and drops the guard.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let pair = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
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

pub fn thread_become_idle() -> ! {
    crate::hal::irq::mask_local();
    let stack = match install_current_idle() {
        Ok(stack) => stack,
        Err(error) => {
            crate::pr_crit!("HypeR: idle-thread installation failed: {error:?}");
            crate::hal::cpu::halt()
        }
    };
    // SAFETY: The current Thread exclusively owns this newly installed stack,
    // local interrupts are masked, and the idle continuation never returns.
    unsafe { crate::kernel::mm::stack::reset_and_enter(stack, enter_clean_idle, 0) }
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

pub(crate) fn run_idle_loop() -> ! {
    loop {
        match prepare_schedule() {
            Ok(Some(pair)) => {
                pair.activate();
            }
            Ok(None) => crate::hal::cpu::wait_for_event(),
            Err(error) => {
                crate::pr_crit!("HypeR: idle scheduling failed: {error:?}");
                crate::hal::cpu::halt()
            }
        }
    }
}

pub(crate) fn prepare_park(wait_queue: &WaitQueue) -> Result<ParkToken, Error> {
    ensure_sleepable()?;
    // SAFETY: The park token keeps this CPU-pinned continuation's outer mask
    // live through the switch and restores it only after the continuation resumes.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let commit = prepare_park_locked(wait_queue)?;
    Ok(retain_park_mask(commit, interrupt_mask))
}

/// Prepares a park after the caller checked sleepability and then acquired an
/// IRQ-masking synchronization-object lock.
pub(crate) fn prepare_park_locked(wait_queue: &WaitQueue) -> Result<ParkCommit, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.reap_terminated()?;
        scheduler.prepare_park(cpu, wait_queue).map(ParkCommit)
    })
}

/// Binds a synchronization lock's retained interrupt mask to a committed park.
pub(crate) fn retain_park_mask(commit: ParkCommit, interrupt_mask: TransitionMask) -> ParkToken {
    ParkToken(PreparedTransition {
        switch: commit.0,
        interrupt_mask,
    })
}

pub(crate) fn complete_park(token: ParkToken) {
    token.0.activate();
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
        let Some(id) = scheduler.dequeue_waiter(wait_queue)? else {
            return Ok::<Option<(ThreadId, state::ReadyOutcome)>, Error>(None);
        };
        before_ready(id);
        let outcome = scheduler.make_ready_from_wait(id)?;
        Ok::<Option<(ThreadId, state::ReadyOutcome)>, Error>(Some((id, outcome)))
    })?;
    if let Some((_, outcome)) = awakened {
        publish_ready_outcome(outcome)?;
    }
    Ok(awakened.map(|(id, _)| id))
}

pub(crate) fn wake_all(wait_queue: &WaitQueue) -> Result<usize, Error> {
    let (count, targets) = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let mut count = 0usize;
        let mut targets = [false; hyper::cpu::MAX_CPUS];
        while let Some(id) = scheduler.dequeue_waiter(wait_queue)? {
            let outcome = scheduler.make_ready_from_wait(id)?;
            if outcome.should_preempt {
                targets[outcome.target_cpu.get()] = true;
            }
            count += 1;
        }
        Ok::<_, Error>((count, targets))
    })?;
    for (index, requested) in targets.into_iter().enumerate() {
        if requested {
            let cpu = CpuIndex::new(index).ok_or(Error::InvalidCpuIndex)?;
            request_reschedule(cpu)?;
        }
    }
    if count != 0 {
        crate::hal::cpu::send_event();
    }
    Ok(count)
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
    // SAFETY: This CPU-pinned scheduler continuation owns the outer transition
    // mask until `PreparedTransition::activate` resumes it and drops the guard.
    let interrupt_mask = unsafe { TransitionMask::acquire() };
    let switch = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.reap_terminated()?;
        scheduler.prepare_yield(cpu)
    })?;
    Ok(switch.map(|switch| PreparedTransition {
        switch,
        interrupt_mask,
    }))
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_exit() -> ! {
    let cpu = match current_cpu() {
        Ok(cpu) => cpu,
        Err(error) => {
            crate::pr_crit!("HypeR: thread exit on invalid CPU: {error:?}");
            crate::hal::cpu::halt()
        }
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
            crate::pr_crit!("HypeR: terminated thread context resumed unexpectedly");
        }
        Err(error) => crate::pr_crit!("HypeR: thread exit failed: {error:?}"),
    }
    crate::hal::cpu::halt()
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpuIndex)
}
