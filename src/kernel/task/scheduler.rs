//! Scheduler policy and run-queue ownership.

use alloc::boxed::Box;
use alloc::vec::Vec;

use hyper::sync::InterruptSpinLock;

use super::thread::{KernelThreadEntry, Thread, ThreadId, ThreadState};

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
    IdleThreadAlreadyInstalled,
    InvalidIdleTransition,
    CpuAlreadyRegistered,
    CpuNotRegistered,
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

struct Scheduler {
    // Individual heap allocations pin contexts and stacks while the Vec grows.
    #[allow(clippy::vec_box)]
    threads: Vec<Box<Thread>>,
    cpus: Vec<CpuScheduler>,
    next_id: u64,
}

struct CpuScheduler {
    index: usize,
    current: ThreadId,
    idle: Option<ThreadId>,
    /// Thread this CPU is switching away from while the scheduler lock is
    /// released.
    ///
    /// The outgoing context and kernel stack stay live until
    /// `aarch64_switch_context` has saved the registers and installed the
    /// incoming stack pointer, which happens after the lock is dropped. The
    /// entry keeps `reap_terminated` from freeing that thread in the window.
    switching_from: Option<ThreadId>,
}

struct SwitchPair {
    previous: *mut crate::arch::ThreadContext,
    next: *const crate::arch::ThreadContext,
}

pub fn initialize() -> Result<Capabilities, Error> {
    SCHEDULER.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        let mut threads = Vec::new();
        threads.try_reserve(1).map_err(|_| Error::Allocation)?;
        let boot_cpu_index = crate::arch::current_cpu_index();
        threads.push(Box::new(Thread::bootstrap(boot_cpu_index)));
        let mut cpus = Vec::new();
        cpus.try_reserve(1).map_err(|_| Error::Allocation)?;
        cpus.push(CpuScheduler {
            index: boot_cpu_index,
            current: ThreadId::BOOTSTRAP,
            idle: None,
            switching_from: None,
        });
        *slot = Some(Scheduler {
            threads,
            cpus,
            next_id: 1,
        });
        Ok(Capabilities {
            bootstrap_thread: ThreadId::BOOTSTRAP,
        })
    })
}

/// Allocates and registers the initial execution context for a secondary CPU.
///
/// The returned virtual stack top must be installed by the architecture before
/// the CPU enters Rust. The context becomes that CPU's idle thread when
/// `thread_become_idle` is called on the secondary.
pub fn register_secondary_cpu(cpu_index: usize, name: &str) -> Result<usize, Error> {
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        if scheduler.cpus.iter().any(|cpu| cpu.index == cpu_index) {
            return Err(Error::CpuAlreadyRegistered);
        }
        let id = scheduler.allocate_id()?;
        scheduler
            .threads
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        scheduler
            .cpus
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        let thread = Box::new(Thread::secondary_bootstrap(id, cpu_index, name)?);
        let stack_top = thread.kernel_stack_top().ok_or(Error::Allocation)?;
        scheduler.threads.push(thread);
        scheduler.cpus.push(CpuScheduler {
            index: cpu_index,
            current: id,
            idle: None,
            switching_from: None,
        });
        Ok(stack_top)
    })
}

/// Creates a dormant kernel thread and registers it with the scheduler.
///
/// Call `thread_ready` to make the new thread eligible for execution. Keeping
/// readiness separate makes the run-queue transition reusable by kernel,
/// userspace, and vCPU execution entities.
pub fn kthread_create(
    name: &str,
    entry: KernelThreadEntry,
    argument: usize,
) -> Result<ThreadId, Error> {
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let cpu_index = crate::arch::current_cpu_index();
        scheduler.cpu_slot(cpu_index)?;
        let id = scheduler.allocate_id()?;
        scheduler
            .threads
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        scheduler.threads.push(Box::new(Thread::kernel(
            id, cpu_index, name, entry, argument,
        )?));
        Ok(id)
    })
}

/// Moves a dormant or blocked thread into the ready queue.
///
/// Returns `true` when this call changed the state and `false` when the thread
/// was already ready or running.
pub fn thread_ready(id: ThreadId) -> Result<bool, Error> {
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        let thread = scheduler
            .threads
            .iter_mut()
            .find(|thread| thread.id() == id)
            .ok_or(Error::ThreadNotFound)?;
        match thread.state() {
            ThreadState::Dormant | ThreadState::Blocked => {
                thread.set_state(ThreadState::Ready);
                Ok(true)
            }
            ThreadState::Ready | ThreadState::Running | ThreadState::Idle => Ok(false),
            ThreadState::Terminated => Err(Error::TerminatedThread),
        }
    })
}

/// Cooperatively transfers execution to the next ready thread.
pub fn yield_now() -> Result<(), Error> {
    let _switched = schedule_once()?;
    Ok(())
}

/// Converts the current bootstrap execution context into the idle thread.
///
/// The idle thread remains scheduler-owned and provides the fallback context
/// when no normal thread is ready. It never returns: ready work is scheduled,
/// otherwise the CPU waits for the next interrupt or event.
pub fn thread_become_idle() -> ! {
    if let Err(error) = install_current_idle() {
        crate::pr_crit!("HypeR: idle-thread installation failed: {error:?}");
        crate::arch::halt()
    }
    run_idle_loop()
}

/// Installs the calling CPU's current bootstrap context as its idle thread.
pub(crate) fn install_current_idle() -> Result<(), Error> {
    let cpu_index = crate::arch::current_cpu_index();
    SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.install_current_as_idle(cpu_index)
    })
}

/// Runs the calling CPU's installed idle scheduling loop.
pub(crate) fn run_idle_loop() -> ! {
    loop {
        match schedule_once() {
            Ok(true) => {}
            Ok(false) => crate::arch::wait_for_interrupt(),
            Err(error) => {
                crate::pr_crit!("HypeR: idle scheduling failed: {error:?}");
                crate::arch::halt()
            }
        }
    }
}

fn schedule_once() -> Result<bool, Error> {
    let cpu_index = crate::arch::current_cpu_index();
    let switch = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.finish_switch(cpu_index);
        scheduler.reap_terminated();
        scheduler.prepare_yield(cpu_index)
    })?;
    let Some(pair) = switch else {
        return Ok(false);
    };
    // SAFETY: Scheduler-owned Threads are individually heap allocated, so
    // their context addresses remain stable while registered. The outgoing
    // thread is pinned by this CPU's switching_from entry, and the next
    // thread owns a mapped stack.
    unsafe { switch_pair(pair) };
    Ok(true)
}

impl Scheduler {
    fn reap_terminated(&mut self) {
        self.threads.retain(|thread| {
            self.cpus
                .iter()
                .any(|cpu| cpu.current == thread.id() || cpu.switching_from == Some(thread.id()))
                || thread.state() != ThreadState::Terminated
        });
    }

    /// Releases the outgoing thread recorded by this CPU's last switch.
    ///
    /// Reaching scheduler code again proves the CPU already runs on the
    /// incoming stack, so the previous context and stack are no longer in use.
    fn finish_switch(&mut self, cpu_index: usize) {
        if let Ok(cpu_slot) = self.cpu_slot(cpu_index) {
            self.cpus[cpu_slot].switching_from = None;
        }
    }

    fn prepare_yield(&mut self, cpu_index: usize) -> Result<Option<SwitchPair>, Error> {
        let cpu_slot = self.cpu_slot(cpu_index)?;
        let current_index = self.current_thread_index(cpu_slot)?;
        let Some(next_index) = self.next_runnable_index(cpu_slot, current_index) else {
            return Ok(None);
        };
        if self.threads[current_index].state() == ThreadState::Running {
            self.threads[current_index].set_state(ThreadState::Ready);
        }
        self.prepare_switch(cpu_slot, current_index, next_index)
    }

    fn prepare_exit(&mut self) -> Result<SwitchPair, Error> {
        let cpu_slot = self.cpu_slot(crate::arch::current_cpu_index())?;
        let current_index = self.current_thread_index(cpu_slot)?;
        self.threads[current_index].set_state(ThreadState::Terminated);
        let Some(next_index) = self.next_runnable_index(cpu_slot, current_index) else {
            return Err(Error::CurrentThreadMissing);
        };
        match self.prepare_switch(cpu_slot, current_index, next_index)? {
            Some(pair) => Ok(pair),
            None => Err(Error::CurrentThreadMissing),
        }
    }

    fn prepare_switch(
        &mut self,
        cpu_slot: usize,
        current_index: usize,
        next_index: usize,
    ) -> Result<Option<SwitchPair>, Error> {
        if current_index == next_index {
            return Ok(None);
        }
        if self.threads[next_index].state() == ThreadState::Ready {
            self.threads[next_index].set_state(ThreadState::Running);
        }
        self.cpus[cpu_slot].switching_from = Some(self.threads[current_index].id());
        self.cpus[cpu_slot].current = self.threads[next_index].id();
        let previous = self.threads[current_index].context_mut() as *mut _;
        let next = self.threads[next_index].context() as *const _;
        Ok(Some(SwitchPair { previous, next }))
    }

    fn current_thread_index(&self, cpu_slot: usize) -> Result<usize, Error> {
        let current = self.cpus[cpu_slot].current;
        self.threads
            .iter()
            .position(|thread| thread.id() == current)
            .ok_or(Error::CurrentThreadMissing)
    }

    fn next_runnable_index(&self, cpu_slot: usize, current_index: usize) -> Option<usize> {
        let count = self.threads.len();
        let cpu_index = self.cpus[cpu_slot].index;
        let ready = (1..count)
            .map(|offset| (current_index + offset) % count)
            .find(|index| {
                self.threads[*index].state() == ThreadState::Ready
                    && self.threads[*index].cpu_index() == cpu_index
            });
        ready.or_else(|| {
            let idle = self.cpus[cpu_slot].idle?;
            self.threads
                .iter()
                .position(|thread| thread.id() == idle)
                .filter(|index| *index != current_index)
        })
    }

    fn install_current_as_idle(&mut self, cpu_index: usize) -> Result<(), Error> {
        let cpu_slot = self.cpu_slot(cpu_index)?;
        if self.cpus[cpu_slot].idle.is_some() {
            return Err(Error::IdleThreadAlreadyInstalled);
        }
        let current_index = self.current_thread_index(cpu_slot)?;
        if self.threads[current_index].state() != ThreadState::Running {
            return Err(Error::InvalidIdleTransition);
        }
        self.threads[current_index].set_state(ThreadState::Idle);
        self.cpus[cpu_slot].idle = Some(self.threads[current_index].id());
        Ok(())
    }

    fn cpu_slot(&self, cpu_index: usize) -> Result<usize, Error> {
        self.cpus
            .iter()
            .position(|cpu| cpu.index == cpu_index)
            .ok_or(Error::CpuNotRegistered)
    }

    fn allocate_id(&mut self) -> Result<ThreadId, Error> {
        let id = ThreadId::new(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(Error::IdentifierExhausted)?;
        Ok(id)
    }
}

unsafe fn switch_pair(pair: SwitchPair) {
    // SAFETY: The scheduler constructs SwitchPair only from distinct pinned
    // Thread allocations that remain registered across the context switch.
    unsafe { crate::arch::switch_thread_context(&mut *pair.previous, &*pair.next) };
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_exit() -> ! {
    let result = SCHEDULER.with(|slot| {
        let scheduler = slot.as_mut().ok_or(Error::NotInitialized)?;
        scheduler.prepare_exit()
    });
    match result {
        Ok(pair) => {
            // SAFETY: prepare_exit records the terminating Thread in the CPU's
            // switching_from entry, which keeps it alive until another stack is
            // active, and chooses a distinct ready Thread.
            unsafe { switch_pair(pair) };
            crate::pr_crit!("HypeR: terminated thread context resumed unexpectedly");
        }
        Err(error) => crate::pr_crit!("HypeR: thread exit failed: {error:?}"),
    }
    crate::arch::halt()
}
