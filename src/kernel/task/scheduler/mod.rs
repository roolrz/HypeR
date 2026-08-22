// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Public scheduler operations and architecture context-switch boundary.

mod queue;
mod state;

use hyper::cpu::CpuIndex;
use hyper::sync::InterruptSpinLock;

use self::state::{PreparedContextSwitch, Scheduler};
use super::thread::{KernelThreadEntry, ThreadId, ThreadState, VcpuExecution};

pub use super::policy::ThreadPriority;
use super::wait::WaitQueue;

type SchedulerLock = InterruptSpinLock<Option<Scheduler>, crate::arch::irq::LocalMask>;

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
                crate::arch::cpu::halt()
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
    InvalidThreadState,
    IdleThreadAlreadyInstalled,
    InvalidIdleTransition,
    CpuAlreadyRegistered,
    CpuNotRegistered,
    InvalidCpuIndex,
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
    pub fixed_priority_class_threads: usize,
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

#[must_use = "parking is committed only after the prepared context switch is consumed"]
pub(crate) struct ParkToken(PreparedContextSwitch);

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
    kthread_create_with_priority(name, entry, argument, ThreadPriority::NORMAL)
}

pub fn kthread_create_with_priority(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
    priority: ThreadPriority,
) -> Result<ThreadId, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .create_kernel_thread(cpu, name, entry, argument, priority)
    })
}

pub(in crate::kernel) fn vcpu_create(
    name: &str,
    vm: crate::kernel::vm::registry::VmBinding,
    vcpu_id: u32,
    context: crate::arch::vm::VcpuContext,
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

/// Enqueues a dormant thread on its owning CPU's priority ready queue.
pub fn thread_ready(id: ThreadId) -> Result<bool, Error> {
    let outcome =
        SCHEDULER.with(|slot| slot.as_mut().ok_or(Error::NotInitialized)?.make_ready(id))?;
    publish_ready_outcome(outcome)?;
    Ok(outcome.changed)
}

pub fn set_thread_priority(id: ThreadId, priority: ThreadPriority) -> Result<(), Error> {
    let target = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .change_priority(id, priority)
    })?;
    if let Some(cpu) = target {
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

/// Reconsiders the current FIFO thread at an explicit safe point.
///
/// Unlike `yield_now`, this does not rotate equal-priority FIFO peers. It
/// switches only when a pending request corresponds to a higher-priority ready
/// thread, or when idle has runnable work.
pub fn cond_resched() -> Result<bool, Error> {
    ensure_sleepable()?;
    let cpu = current_cpu()?;
    if !super::preempt::pending(cpu)? || !super::preempt::can_reschedule(cpu)? {
        return Ok(false);
    }
    let pair = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.prepare_preemption(cpu)
    })?;
    let Some(pair) = pair else {
        return Ok(false);
    };
    pair.activate();
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
    crate::arch::irq::disable_local();
    let stack = match install_current_idle() {
        Ok(stack) => stack,
        Err(error) => {
            crate::pr_crit!("HypeR: idle-thread installation failed: {error:?}");
            crate::arch::cpu::halt()
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
    crate::arch::irq::enable_local();
    run_idle_loop()
}

pub(crate) fn run_idle_loop() -> ! {
    loop {
        match prepare_schedule() {
            Ok(Some(pair)) => {
                pair.activate();
            }
            Ok(None) => crate::arch::cpu::wait_for_event(),
            Err(error) => {
                crate::pr_crit!("HypeR: idle scheduling failed: {error:?}");
                crate::arch::cpu::halt()
            }
        }
    }
}

pub(crate) fn prepare_park(wait_queue: &WaitQueue) -> Result<ParkToken, Error> {
    ensure_sleepable()?;
    prepare_park_locked(wait_queue)
}

/// Prepares a park after the caller checked sleepability and then acquired an
/// IRQ-masking synchronization-object lock.
pub(crate) fn prepare_park_locked(wait_queue: &WaitQueue) -> Result<ParkToken, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.reap_terminated()?;
        scheduler.prepare_park(cpu, wait_queue).map(ParkToken)
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
        crate::arch::cpu::send_event();
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
    if !crate::arch::irq::local_enabled() {
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
        crate::arch::cpu::send_event();
    }
    Ok(())
}

fn request_reschedule(cpu: CpuIndex) -> Result<(), Error> {
    super::preempt::request(cpu)?;
    // This is an idle wakeup only. A targeted reschedule IPI will replace it
    // when an architecture can enter the safe IRQ-tail continuation seam.
    crate::arch::cpu::send_event();
    Ok(())
}

fn prepare_schedule() -> Result<Option<PreparedContextSwitch>, Error> {
    let cpu = current_cpu()?;
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.reap_terminated()?;
        scheduler.prepare_yield(cpu)
    })
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_exit() -> ! {
    let cpu = match current_cpu() {
        Ok(cpu) => cpu,
        Err(error) => {
            crate::pr_crit!("HypeR: thread exit on invalid CPU: {error:?}");
            crate::arch::cpu::halt()
        }
    };
    let result = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .prepare_exit(cpu)
    });
    match result {
        Ok(pair) => {
            pair.activate();
            crate::pr_crit!("HypeR: terminated thread context resumed unexpectedly");
        }
        Err(error) => crate::pr_crit!("HypeR: thread exit failed: {error:?}"),
    }
    crate::arch::cpu::halt()
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpuIndex)
}
