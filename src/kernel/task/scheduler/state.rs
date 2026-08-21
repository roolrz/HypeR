//! Scheduler-owned thread registry, CPU state, and lifecycle transitions.

use alloc::boxed::Box;
use alloc::vec::Vec;
use hyper::cpu::{CpuIndex, PerCpu};

use super::queue::{self, ReadyQueues};
use super::{CurrentVcpu, Error, SecondaryStack, Statistics};
use crate::kernel::task::thread::{
    KernelThreadEntry, QueueMembership, Thread, ThreadId, ThreadPriority, ThreadState,
};
use crate::kernel::task::wait::WaitQueue;

pub(super) struct Scheduler {
    // Slot index equals ThreadId and is never reused. Individual Box
    // allocations pin contexts while this registry grows.
    #[allow(clippy::vec_box)]
    threads: Vec<Option<Box<Thread>>>,
    cpus: Vec<CpuScheduler>,
    cpu_slots: PerCpu<Option<usize>>,
    next_id: u64,
    context_switches: u64,
}

struct CpuScheduler {
    index: CpuIndex,
    current: ThreadId,
    idle: Option<ThreadId>,
    ready: ReadyQueues,
    /// Outgoing context whose registers/stack may still be in use between
    /// dropping the scheduler lock and completing the assembly switch.
    switching_from: Option<ThreadId>,
}

pub(super) struct SwitchPair {
    pub previous: *mut crate::arch::context::ThreadContext,
    pub next: *const crate::arch::context::ThreadContext,
}

impl CpuScheduler {
    const fn new(index: CpuIndex, current: ThreadId) -> Self {
        Self {
            index,
            current,
            idle: None,
            ready: ReadyQueues::new(),
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
        cpu: CpuIndex,
        name: &str,
        entry: KernelThreadEntry,
        argument: usize,
        priority: ThreadPriority,
    ) -> Result<ThreadId, Error> {
        self.cpu_slot(cpu)?;
        self.threads.try_reserve(1).map_err(|_| Error::Allocation)?;
        let id = self.next_thread_id()?;
        let mut thread = hyper::mm::try_box(Thread::kernel(id, cpu, name, entry, argument)?)
            .map_err(|_| Error::Allocation)?;
        thread.set_priority(priority);
        self.register_thread(thread)?;
        Ok(id)
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
        let index = usize::try_from(id.get()).map_err(|_| Error::ThreadNotFound)?;
        let thread = self
            .threads
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(Error::ThreadNotFound)?;
        if thread.state() != ThreadState::Dormant
            || thread.queue_links().membership != QueueMembership::None
            || thread.execution_kind() != crate::kernel::task::thread::ExecutionKind::Vcpu
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

    pub fn make_ready(&mut self, id: ThreadId) -> Result<bool, Error> {
        match self.thread(id)?.state() {
            ThreadState::Dormant => {
                self.enqueue_ready(id)?;
                Ok(true)
            }
            ThreadState::Blocked => Err(Error::ThreadBlocked),
            ThreadState::Ready | ThreadState::Running | ThreadState::Idle => Ok(false),
            ThreadState::Terminated => Err(Error::TerminatedThread),
        }
    }

    pub fn make_ready_from_wait(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        if thread.state() != ThreadState::Blocked
            || thread.queue_links().membership != QueueMembership::None
        {
            return Err(Error::QueueCorrupted);
        }
        self.enqueue_ready(id)
    }

    pub fn prepare_yield(&mut self, cpu: CpuIndex) -> Result<Option<SwitchPair>, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
        match self.thread(current)?.state() {
            ThreadState::Running => self.enqueue_ready(current)?,
            ThreadState::Idle => {}
            _ => return Err(Error::InvalidThreadState),
        }
        let Some(next) = self.dequeue_ready(cpu_slot)? else {
            return Ok(None);
        };
        if next == current {
            self.thread_mut(current)?.set_state(ThreadState::Running);
            return Ok(None);
        }
        self.prepare_switch(cpu_slot, current, next).map(Some)
    }

    pub fn prepare_park(
        &mut self,
        cpu: CpuIndex,
        wait_queue: &WaitQueue,
    ) -> Result<SwitchPair, Error> {
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

    pub fn prepare_exit(&mut self, cpu: CpuIndex) -> Result<SwitchPair, Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let current = self.cpus[cpu_slot].current;
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

    pub fn change_priority(&mut self, id: ThreadId, priority: ThreadPriority) -> Result<(), Error> {
        let links = self.thread(id)?.queue_links();
        if let QueueMembership::Ready { cpu, priority: old } = links.membership {
            self.remove_ready(id, cpu, old)?;
            self.thread_mut(id)?.set_priority(priority);
            self.enqueue_ready(id)
        } else {
            self.thread_mut(id)?.set_priority(priority);
            Ok(())
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
        thread.set_state(ThreadState::Idle);
        self.cpus[cpu_slot].idle = Some(current);
        Ok(stack)
    }

    pub fn finish_switch(&mut self, cpu: CpuIndex) {
        if let Ok(cpu_slot) = self.cpu_slot(cpu) {
            self.cpus[cpu_slot].switching_from = None;
        }
    }

    pub fn reap_terminated(&mut self) {
        for index in 0..self.threads.len() {
            let Some(thread) = self.threads[index].as_ref() else {
                continue;
            };
            let id = thread.id();
            let pinned = self
                .cpus
                .iter()
                .any(|cpu| cpu.current == id || cpu.switching_from == Some(id));
            if !pinned && thread.state() == ThreadState::Terminated {
                self.threads[index] = None;
            }
        }
    }

    pub fn statistics(&self) -> Statistics {
        let mut stats = Statistics {
            context_switches: self.context_switches,
            ..Statistics::default()
        };
        for thread in self.threads.iter().flatten() {
            stats.threads += 1;
            match thread.state() {
                ThreadState::Ready => stats.ready += 1,
                ThreadState::Running => stats.running += 1,
                ThreadState::Blocked => stats.blocked += 1,
                ThreadState::Idle => stats.idle += 1,
                ThreadState::Dormant | ThreadState::Terminated => {}
            }
        }
        for cpu in &self.cpus {
            stats.per_cpu_ready[cpu.index.get()] = cpu.ready.len();
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
    ) -> Result<SwitchPair, Error> {
        if self.thread(next)?.state() == ThreadState::Ready {
            self.thread_mut(next)?.set_state(ThreadState::Running);
        }
        self.cpus[cpu_slot].switching_from = Some(current);
        self.cpus[cpu_slot].current = next;
        self.context_switches = self.context_switches.saturating_add(1);
        let previous = self.thread_mut(current)?.context_mut() as *mut _;
        let next = self.thread(next)?.context() as *const _;
        Ok(SwitchPair { previous, next })
    }

    fn enqueue_ready(&mut self, id: ThreadId) -> Result<(), Error> {
        let thread = self.thread(id)?;
        let cpu = thread.cpu_index();
        let priority = thread.priority().get();
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].ready.enqueue(threads, id, cpu, priority)
    }

    fn dequeue_ready(&mut self, cpu_slot: usize) -> Result<Option<ThreadId>, Error> {
        let cpu = self.cpus[cpu_slot].index;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].ready.dequeue(threads, cpu)
    }

    fn remove_ready(&mut self, id: ThreadId, cpu: CpuIndex, priority: u8) -> Result<(), Error> {
        let cpu_slot = self.cpu_slot(cpu)?;
        let (threads, cpus) = (&mut self.threads, &mut self.cpus);
        cpus[cpu_slot].ready.remove(threads, id, cpu, priority)
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
    ) -> Result<SwitchPair, Error> {
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
