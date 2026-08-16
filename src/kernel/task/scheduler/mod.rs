//! Public scheduler operations and architecture context-switch boundary.

mod queue;
mod state;

use hyper::sync::InterruptSpinLock;

use self::state::{Scheduler, SwitchPair};
use super::thread::{
    KernelThreadEntry, ThreadId, ThreadPriority, ThreadState, VcpuExecution, VirtualMachineId,
};
use super::wait::WaitQueue;

type SchedulerLock = InterruptSpinLock<Option<Scheduler>, crate::arch::LocalInterruptMask>;

static SCHEDULER: SchedulerLock = InterruptSpinLock::new(None);

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
    InvalidThreadState,
    IdleThreadAlreadyInstalled,
    InvalidIdleTransition,
    CpuAlreadyRegistered,
    CpuNotRegistered,
    InvalidCpuIndex,
    Thread(super::thread::Error),
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

pub(crate) struct ParkToken(SwitchPair);

pub fn initialize() -> Result<Capabilities, Error> {
    SCHEDULER.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(Scheduler::new(crate::arch::current_cpu_index())?);
        Ok(Capabilities {
            bootstrap_thread: ThreadId::BOOTSTRAP,
        })
    })
}

pub fn current_thread_id() -> Result<ThreadId, Error> {
    let cpu = crate::arch::current_cpu_index();
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

pub fn register_secondary_cpu(cpu: usize, name: &str) -> Result<SecondaryStack, Error> {
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .register_secondary(cpu, name)
    })
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
    let cpu = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .create_kernel_thread(cpu, name, entry, argument, priority)
    })
}

pub(crate) fn vcpu_create(
    name: &str,
    virtual_machine: VirtualMachineId,
    vcpu_id: u32,
    context: crate::arch::VcpuContext,
    entry: KernelThreadEntry,
) -> Result<ThreadId, Error> {
    let cpu = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .create_vcpu_thread(cpu, name, virtual_machine, vcpu_id, context, entry)
    })
}

/// Returns the pinned vCPU payload owned by the calling CPU's current Thread.
pub(crate) fn current_vcpu() -> Result<CurrentVcpu, Error> {
    let cpu = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .current_vcpu(cpu)
    })
}

/// Enqueues a dormant thread on its owning CPU's priority ready queue.
pub fn thread_ready(id: ThreadId) -> Result<bool, Error> {
    let changed =
        SCHEDULER.with(|slot| slot.as_mut().ok_or(Error::NotInitialized)?.make_ready(id))?;
    if changed {
        crate::arch::send_event();
    }
    Ok(changed)
}

pub fn set_thread_priority(id: ThreadId, priority: ThreadPriority) -> Result<(), Error> {
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .change_priority(id, priority)
    })
}

pub fn yield_now() -> Result<(), Error> {
    ensure_sleepable()?;
    if let Some(pair) = prepare_schedule()? {
        // SAFETY: prepare_schedule pins both scheduler-owned contexts across
        // the interval where the scheduler lock is released.
        unsafe { switch_pair(pair) };
    }
    Ok(())
}

pub fn thread_become_idle() -> ! {
    crate::arch::disable_local_interrupts();
    let stack = match install_current_idle() {
        Ok(stack) => stack,
        Err(error) => {
            crate::pr_crit!("HypeR: idle-thread installation failed: {error:?}");
            crate::arch::halt()
        }
    };
    // SAFETY: The current Thread exclusively owns this newly installed stack,
    // local interrupts are masked, and the idle continuation never returns.
    unsafe { crate::kernel::mm::stack::reset_and_enter(stack, enter_clean_idle, 0) }
}

pub(crate) fn install_current_idle() -> Result<(usize, usize), Error> {
    let cpu = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .install_current_as_idle(cpu)
    })
}

extern "C" fn enter_clean_idle(_argument: usize) -> ! {
    crate::arch::enable_local_irq();
    run_idle_loop()
}

pub(crate) fn run_idle_loop() -> ! {
    loop {
        match prepare_schedule() {
            Ok(Some(pair)) => {
                // SAFETY: The scheduler pins both contexts until this CPU
                // completes a later scheduler entry on the incoming stack.
                unsafe { switch_pair(pair) };
            }
            Ok(None) => crate::arch::wait_for_event(),
            Err(error) => {
                crate::pr_crit!("HypeR: idle scheduling failed: {error:?}");
                crate::arch::halt()
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
    let cpu = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.reap_terminated();
        scheduler.prepare_park(cpu, wait_queue).map(ParkToken)
    })
}

pub(crate) fn complete_park(token: ParkToken) {
    // SAFETY: prepare_park moved the outgoing thread to a wait queue and pins
    // it through switching_from until execution continues on another stack.
    unsafe { switch_pair(token.0) };
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
            return Ok::<Option<ThreadId>, Error>(None);
        };
        before_ready(id);
        scheduler.make_ready_from_wait(id)?;
        Ok::<Option<ThreadId>, Error>(Some(id))
    })?;
    notify_awakened(usize::from(awakened.is_some()));
    Ok(awakened)
}

pub(crate) fn wake_all(wait_queue: &WaitQueue) -> Result<usize, Error> {
    let count = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let mut count = 0usize;
        while let Some(id) = scheduler.dequeue_waiter(wait_queue)? {
            scheduler.make_ready_from_wait(id)?;
            count += 1;
        }
        Ok::<usize, Error>(count)
    })?;
    notify_awakened(count);
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
    if crate::arch::local_irq_enabled() {
        Ok(())
    } else {
        Err(Error::CannotSleepWithInterruptsMasked)
    }
}

fn notify_awakened(count: usize) {
    if count != 0 {
        crate::arch::send_event();
    }
}

fn prepare_schedule() -> Result<Option<SwitchPair>, Error> {
    let cpu = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu);
        scheduler.reap_terminated();
        scheduler.prepare_yield(cpu)
    })
}

unsafe fn switch_pair(pair: SwitchPair) {
    // SAFETY: Scheduler queues contain only pinned, scheduler-owned Threads.
    unsafe { crate::arch::switch_thread_context(&mut *pair.previous, &*pair.next) };
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_exit() -> ! {
    let cpu = crate::arch::current_cpu_index();
    let result = SCHEDULER.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .prepare_exit(cpu)
    });
    match result {
        Ok(pair) => {
            // SAFETY: prepare_exit pins the terminating context until another
            // stack is installed and a later scheduler entry reaps it.
            unsafe { switch_pair(pair) };
            crate::pr_crit!("HypeR: terminated thread context resumed unexpectedly");
        }
        Err(error) => crate::pr_crit!("HypeR: thread exit failed: {error:?}"),
    }
    crate::arch::halt()
}
