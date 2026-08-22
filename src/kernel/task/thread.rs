// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral thread objects and execution payloads.

use alloc::boxed::Box;
use hyper::cpu::CpuIndex;

const THREAD_NAME_CAPACITY: usize = 32;

use crate::kernel::mm::{AddressSpaceId, stack::KernelStack};
use crate::kernel::task::policy::{
    SchedulingClass, SchedulingPolicy, ThreadPlacement, ThreadPriority,
};

pub type KernelThreadEntry = extern "C" fn(usize);

/// Queue position to use if a running FIFO thread must leave the CPU after a
/// priority change. The value is replaced by every subsequent priority change
/// and consumed by the next scheduling decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeferredFifoPlacement {
    Head,
    Tail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadId(u64);

impl ThreadId {
    pub const BOOTSTRAP: Self = Self(0);

    /// Mints an identity from the scheduler's never-reused slot namespace.
    pub(super) const fn from_scheduler_index(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadState {
    Dormant,
    Ready,
    Running,
    Idle,
    Blocked,
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueueMembership {
    None,
    Ready { cpu: CpuIndex, priority: u8 },
    Waiting { queue: usize },
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QueueLinks {
    pub previous: Option<ThreadId>,
    pub next: Option<ThreadId>,
    pub membership: QueueMembership,
}

impl QueueLinks {
    pub(crate) const EMPTY: Self = Self {
        previous: None,
        next: None,
        membership: QueueMembership::None,
    };
}

pub struct UserExecution {
    pub address_space: AddressSpaceId,
    pub context: crate::arch::context::UserContext,
}

pub struct VcpuExecution {
    vm: VcpuVm,
    pub vcpu_id: u32,
    pub context: crate::arch::vm::VcpuContext,
}

enum VcpuVm {
    Installed(crate::kernel::vm::registry::VmBinding),
    TimerValidation { interrupts: usize },
}

impl VcpuExecution {
    pub(in crate::kernel) fn vm_binding(&self) -> Option<&crate::kernel::vm::registry::VmBinding> {
        match &self.vm {
            VcpuVm::Installed(binding) => Some(binding),
            VcpuVm::TimerValidation { .. } => None,
        }
    }

    pub(crate) fn interrupts(&self) -> &crate::kernel::vm::VmInterruptController {
        match &self.vm {
            VcpuVm::Installed(binding) => binding.interrupts(),
            VcpuVm::TimerValidation { interrupts } => {
                // SAFETY: `for_timer_validation` requires the pointed-to model
                // to remain fixed and live until this execution is deactivated
                // and dropped. The reference is scoped to the execution borrow.
                unsafe {
                    &*core::ptr::with_exposed_provenance::<crate::kernel::vm::VmInterruptController>(
                        *interrupts,
                    )
                }
            }
        }
    }

    /// Builds the non-runnable execution used by architecture timer checks.
    ///
    /// This execution may activate and deactivate local virtual hardware, but
    /// it must never enter a guest or perform a VM-registry lookup.
    ///
    /// # Safety
    ///
    /// `interrupts` must remain at a fixed address and live until the returned
    /// execution has been deactivated and dropped.
    // Only AArch64 currently performs this validation. Keep the constructor
    // architecture-neutral so kernel policy contains no target cfg.
    #[allow(dead_code)]
    pub(crate) unsafe fn for_timer_validation(
        context: crate::arch::vm::VcpuContext,
        interrupts: &crate::kernel::vm::VmInterruptController,
    ) -> Self {
        Self {
            vm: VcpuVm::TimerValidation {
                interrupts: core::ptr::from_ref(interrupts).expose_provenance(),
            },
            vcpu_id: 0,
            context,
        }
    }
}

pub enum ThreadExecution {
    Kernel,
    User(Box<UserExecution>),
    Vcpu(Box<VcpuExecution>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionKind {
    Kernel,
    User,
    Vcpu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    NameTooLong,
    VirtualInterrupt(crate::arch::vm::VirtualInterruptError),
}

impl From<crate::arch::vm::VirtualInterruptError> for Error {
    fn from(error: crate::arch::vm::VirtualInterruptError) -> Self {
        Self::VirtualInterrupt(error)
    }
}

/// A schedulable execution entity.
///
/// Every thread owns a kernel scheduling context and, except for the bootstrap
/// thread, a private kernel stack. User and vCPU architectural state is an
/// attached execution payload; it is deliberately separate from the context
/// used while the scheduler and exception handlers execute in the host
/// hypervisor privilege domain.
pub struct Thread {
    id: ThreadId,
    placement: ThreadPlacement,
    name: ThreadName,
    scheduling: SchedulingPolicy,
    deferred_fifo_placement: Option<DeferredFifoPlacement>,
    state: ThreadState,
    queue_links: QueueLinks,
    context: crate::arch::context::ThreadContext,
    kernel_stack: Option<KernelStack>,
    execution: ThreadExecution,
}

impl Thread {
    pub(super) fn bootstrap(cpu_index: CpuIndex) -> Self {
        let name = match ThreadName::new("bootstrap") {
            Ok(name) => name,
            Err(_) => ThreadName::empty(),
        };
        Self {
            id: ThreadId::BOOTSTRAP,
            placement: ThreadPlacement::pinned(cpu_index),
            name,
            scheduling: SchedulingPolicy::fifo(ThreadPriority::NORMAL),
            deferred_fifo_placement: None,
            state: ThreadState::Running,
            queue_links: QueueLinks::EMPTY,
            context: crate::arch::context::ThreadContext::empty(),
            kernel_stack: None,
            execution: ThreadExecution::Kernel,
        }
    }

    pub(super) fn kernel(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
        entry: KernelThreadEntry,
        argument: usize,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut context = crate::arch::context::ThreadContext::empty();
        context.prepare(stack.top(), entry, argument);
        Ok(Self {
            id,
            placement: ThreadPlacement::movable(cpu_index),
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::fifo(ThreadPriority::NORMAL),
            deferred_fifo_placement: None,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            context,
            kernel_stack: Some(stack),
            execution: ThreadExecution::Kernel,
        })
    }

    /// Creates the already-running bootstrap context for a secondary CPU.
    pub(super) fn secondary_bootstrap(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
    ) -> Result<Self, Error> {
        Ok(Self {
            id,
            placement: ThreadPlacement::pinned(cpu_index),
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::fifo(ThreadPriority::NORMAL),
            deferred_fifo_placement: None,
            state: ThreadState::Running,
            queue_links: QueueLinks::EMPTY,
            context: crate::arch::context::ThreadContext::empty(),
            kernel_stack: Some(KernelStack::allocate_thread().map_err(|_| Error::Allocation)?),
            execution: ThreadExecution::Kernel,
        })
    }

    pub(super) fn vcpu(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
        vm: crate::kernel::vm::registry::VmBinding,
        vcpu_id: u32,
        mut context: crate::arch::vm::VcpuContext,
        entry: KernelThreadEntry,
    ) -> Result<Self, Error> {
        context.initialize_virtual_interrupts()?;
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut scheduling_context = crate::arch::context::ThreadContext::empty();
        scheduling_context.prepare(stack.top(), entry, 0);
        Ok(Self {
            id,
            placement: ThreadPlacement::prefer(cpu_index),
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::fifo(ThreadPriority::NORMAL),
            deferred_fifo_placement: None,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            context: scheduling_context,
            kernel_stack: Some(stack),
            execution: ThreadExecution::Vcpu(
                hyper::mm::try_box(VcpuExecution {
                    vm: VcpuVm::Installed(vm),
                    vcpu_id,
                    context,
                })
                .map_err(|_| Error::Allocation)?,
            ),
        })
    }

    pub const fn id(&self) -> ThreadId {
        self.id
    }

    /// Returns the CPU that owns this thread's scheduling context.
    ///
    /// Context migration requires a stopped-thread hand-off protocol and is
    /// deliberately not implicit in the shared scheduler run queue.
    pub const fn cpu_index(&self) -> CpuIndex {
        self.placement.assigned_cpu()
    }

    /// Checks the placement constraint independently of current assignment.
    pub(super) const fn can_run_on(&self, cpu: CpuIndex) -> bool {
        self.placement.affinity().contains(cpu)
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn state(&self) -> ThreadState {
        self.state
    }

    pub(crate) const fn scheduling_policy(&self) -> SchedulingPolicy {
        self.scheduling
    }

    pub(crate) const fn scheduling_class(&self) -> SchedulingClass {
        self.scheduling.class()
    }

    pub const fn priority(&self) -> Option<ThreadPriority> {
        self.scheduling.priority()
    }

    pub(super) fn set_priority(&mut self, priority: ThreadPriority) -> bool {
        if self.scheduling_class() != SchedulingClass::FixedPriority {
            return false;
        }
        self.scheduling = SchedulingPolicy::fifo(priority);
        true
    }

    pub(super) fn set_deferred_fifo_placement(&mut self, placement: Option<DeferredFifoPlacement>) {
        self.deferred_fifo_placement = placement;
    }

    pub(super) const fn deferred_fifo_placement(&self) -> Option<DeferredFifoPlacement> {
        self.deferred_fifo_placement
    }

    pub(super) fn become_idle(&mut self) {
        self.scheduling = SchedulingPolicy::Idle;
        self.state = ThreadState::Idle;
    }

    pub(super) fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }

    pub(super) fn mark_running_on(&mut self, cpu: CpuIndex) -> bool {
        let Some(placement) = self.placement.mark_running(cpu) else {
            return false;
        };
        self.placement = placement;
        self.deferred_fifo_placement = None;
        self.state = ThreadState::Running;
        true
    }

    pub(super) const fn queue_links(&self) -> QueueLinks {
        self.queue_links
    }

    pub(super) fn set_queue_links(&mut self, links: QueueLinks) {
        self.queue_links = links;
    }

    pub fn context(&self) -> &crate::arch::context::ThreadContext {
        &self.context
    }

    pub(super) fn context_mut(&mut self) -> &mut crate::arch::context::ThreadContext {
        &mut self.context
    }

    pub const fn execution_kind(&self) -> ExecutionKind {
        match self.execution {
            ThreadExecution::Kernel => ExecutionKind::Kernel,
            ThreadExecution::User(_) => ExecutionKind::User,
            ThreadExecution::Vcpu(_) => ExecutionKind::Vcpu,
        }
    }

    pub fn user_execution(&self) -> Option<&UserExecution> {
        match &self.execution {
            ThreadExecution::User(execution) => Some(execution.as_ref()),
            _ => None,
        }
    }

    pub fn vcpu_execution(&self) -> Option<&VcpuExecution> {
        match &self.execution {
            ThreadExecution::Vcpu(execution) => Some(execution.as_ref()),
            _ => None,
        }
    }

    pub(super) fn vcpu_execution_mut(&mut self) -> Option<&mut VcpuExecution> {
        match &mut self.execution {
            ThreadExecution::Vcpu(execution) => Some(execution.as_mut()),
            _ => None,
        }
    }

    pub const fn owns_kernel_stack(&self) -> bool {
        self.kernel_stack.is_some()
    }

    pub(super) fn ensure_kernel_stack(&mut self) -> Result<(usize, usize), Error> {
        if self.kernel_stack.is_none() {
            self.kernel_stack =
                Some(KernelStack::allocate_thread().map_err(|_| Error::Allocation)?);
        }
        self.kernel_stack_bounds().ok_or(Error::Allocation)
    }

    pub(super) fn kernel_stack_top(&self) -> Option<usize> {
        self.kernel_stack.as_ref().map(KernelStack::top)
    }

    pub(super) fn kernel_stack_physical_top(&self) -> Option<u64> {
        self.kernel_stack.as_ref().map(KernelStack::physical_top)
    }

    pub(super) fn kernel_stack_bounds(&self) -> Option<(usize, usize)> {
        self.kernel_stack.as_ref().map(KernelStack::bounds)
    }

    pub(super) fn kernel_stack_statistics(
        &self,
    ) -> Option<crate::kernel::mm::stack::StackStatistics> {
        self.kernel_stack.as_ref().map(KernelStack::statistics)
    }
}

struct ThreadName {
    bytes: [u8; THREAD_NAME_CAPACITY],
    len: u8,
}

impl ThreadName {
    fn new(name: &str) -> Result<Self, Error> {
        if name.len() > THREAD_NAME_CAPACITY {
            return Err(Error::NameTooLong);
        }
        let mut bytes = [0; THREAD_NAME_CAPACITY];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Self {
            bytes,
            len: name.len() as u8,
        })
    }

    const fn empty() -> Self {
        Self {
            bytes: [0; THREAD_NAME_CAPACITY],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        let bytes = &self.bytes[..usize::from(self.len)];
        // ThreadName is built from UTF-8 input. Keep the accessor safe and
        // defensive if a future internal constructor violates that invariant.
        core::str::from_utf8(bytes).unwrap_or("")
    }
}
