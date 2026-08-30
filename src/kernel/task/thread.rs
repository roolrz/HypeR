// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral thread objects and execution payloads.

use alloc::boxed::Box;
use hyper::cpu::CpuIndex;

const THREAD_NAME_CAPACITY: usize = 32;

use crate::kernel::mm::stack::KernelStack;
use crate::kernel::task::policy::{
    CpuMask, SchedulingClass, SchedulingPolicy, ThreadPlacement, ThreadPriority,
};
use crate::kernel::task::wait::WaitRecord;

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
    const SLOT_BITS: u32 = 24;
    const SLOT_MASK: u64 = (1 << Self::SLOT_BITS) - 1;
    const IDENTITY_LIMIT: u64 = u64::MAX >> Self::SLOT_BITS;

    pub const BOOTSTRAP: Self = Self(0);

    /// Combines a never-reused identity with a reusable registry-slot hint.
    pub(super) const fn from_scheduler_parts(identity: u64, slot: usize) -> Option<Self> {
        if identity == 0 || identity > Self::IDENTITY_LIMIT || slot >= Self::SLOT_MASK as usize {
            return None;
        }
        Some(Self((identity << Self::SLOT_BITS) | (slot as u64 + 1)))
    }

    /// Reconstructs an ID retained by Process publication.
    pub(crate) const fn from_process_publication(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the private reusable slot encoded by the scheduler.
    pub(super) const fn scheduler_slot(self) -> Option<usize> {
        if self.0 == 0 {
            Some(0)
        } else {
            let encoded = self.0 & Self::SLOT_MASK;
            if encoded == 0 {
                None
            } else {
                Some((encoded - 1) as usize)
            }
        }
    }

    #[cfg(test)]
    pub(super) const fn for_test(identity: u64) -> Self {
        match Self::from_scheduler_parts(identity, 0) {
            Some(id) => id,
            None => Self::BOOTSTRAP,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadState {
    Dormant,
    Ready,
    Running,
    Idle,
    Blocked,
    /// Source switch is committed and may still execute; target publication awaits switch-tail.
    Migrating,
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueueMembership {
    None,
    ReadyRealTime { cpu: CpuIndex, priority: u8 },
    ReadyFair { cpu: CpuIndex },
    Waiting { queue: usize },
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QueueLinks {
    pub previous: Option<ThreadId>,
    pub next: Option<ThreadId>,
    pub membership: QueueMembership,
}

/// Placement change retained by a Thread until its source context is stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MigrationRequest {
    pub target: CpuIndex,
    pub affinity: CpuMask,
}

impl QueueLinks {
    pub(crate) const EMPTY: Self = Self {
        previous: None,
        next: None,
        membership: QueueMembership::None,
    };
}

pub struct VcpuExecution {
    vm: VcpuVm,
    active_execution: Option<hyper::vm::translation::ExecutionClaim>,
    pub(crate) vcpu_id: u32,
    pub(crate) hardware: crate::hal::vm::VcpuHardwareState,
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

    /// Claims the installed VM's exclusive execution interval.
    ///
    /// Timer validation has no installed address space and therefore requires
    /// no claim. Returning `true` tells the caller that release is mandatory.
    pub(crate) fn claim_execution(
        &mut self,
        cpu: CpuIndex,
    ) -> Result<bool, hyper::vm::translation::ExecutionError> {
        if self.active_execution.is_some() {
            return Err(hyper::vm::translation::ExecutionError::AlreadyActive);
        }
        let VcpuVm::Installed(binding) = &self.vm else {
            return Ok(false);
        };
        let claim = binding.claim_execution(cpu)?;
        self.active_execution = Some(claim);
        Ok(true)
    }

    /// Releases the execution capability after guest hardware is stopped.
    pub(crate) fn release_execution(
        &mut self,
        cpu: CpuIndex,
    ) -> Result<(), hyper::vm::translation::ExecutionError> {
        let Some(claim) = self.active_execution.take() else {
            return match self.vm {
                VcpuVm::Installed(_) => Err(hyper::vm::translation::ExecutionError::NotActiveOwner),
                VcpuVm::TimerValidation { .. } => Ok(()),
            };
        };
        let binding = match &self.vm {
            VcpuVm::Installed(binding) => binding,
            VcpuVm::TimerValidation { .. } => {
                // Preserve the linear claim even on this impossible binding
                // mismatch so the caller can cross a controlled fail-stop.
                self.active_execution = Some(claim);
                return Err(hyper::vm::translation::ExecutionError::WrongAddressSpace);
            }
        };
        match binding.release_execution(claim, cpu) {
            Ok(()) => Ok(()),
            Err(failure) => {
                let error = failure.error();
                self.active_execution = Some(failure.into_claim());
                Err(error)
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
        hardware: crate::hal::vm::VcpuHardwareState,
        interrupts: &crate::kernel::vm::VmInterruptController,
    ) -> Self {
        Self {
            vm: VcpuVm::TimerValidation {
                interrupts: core::ptr::from_ref(interrupts).expose_provenance(),
            },
            active_execution: None,
            vcpu_id: 0,
            hardware,
        }
    }
}

impl Drop for VcpuExecution {
    fn drop(&mut self) {
        if self.active_execution.is_some() {
            // Destruction cannot safely stop architecture hardware or prove
            // guest execution quiescent. Fail closed without taking locks.
            crate::hal::cpu::halt()
        }
    }
}

pub(crate) enum ThreadExecution {
    Kernel,
    Vcpu(Box<VcpuExecution>),
    User(Box<crate::kernel::process::UserExecution>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionKind {
    Kernel,
    Vcpu,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    NameTooLong,
    InvalidPlacement,
    VirtualInterrupt(crate::hal::vm::VirtualInterruptError),
}

impl From<crate::hal::vm::VirtualInterruptError> for Error {
    fn from(error: crate::hal::vm::VirtualInterruptError) -> Self {
        Self::VirtualInterrupt(error)
    }
}

/// A schedulable execution entity.
///
/// Every thread owns a kernel scheduling context and, except for the bootstrap
/// thread, a private kernel stack. vCPU architectural state is an attached
/// execution payload; it is deliberately separate from the context used while
/// the scheduler and exception handlers execute in the host hypervisor
/// privilege domain. A future user execution payload must strongly own its
/// Process and prepared address space before it becomes a Thread variant.
pub struct Thread {
    id: ThreadId,
    placement: ThreadPlacement,
    name: ThreadName,
    scheduling: SchedulingPolicy,
    fair_runtime: FairRuntime,
    deferred_fifo_placement: Option<DeferredFifoPlacement>,
    state: ThreadState,
    queue_links: QueueLinks,
    wait: WaitRecord,
    pending_migration: Option<MigrationRequest>,
    context: crate::hal::context::ThreadContext,
    kernel_stack: Option<KernelStack>,
    execution: ThreadExecution,
}

/// Runtime owned by the replaceable Fair scheduling implementation.
///
/// A zero slice denotes a new or expired entity. Scheduler policy replenishes
/// it only when the entity is selected or continues without a ready peer. A
/// voluntary yield resets the slice; blocking and RT-class interruption retain
/// it so neither event grants an unbounded succession of fresh quanta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FairRuntime {
    slice_remaining: u64,
}

impl FairRuntime {
    const NEW: Self = Self { slice_remaining: 0 };
}

impl Thread {
    /// Bounded scheduler-object storage charged before user-thread publication.
    pub(crate) const fn allocation_size() -> usize {
        core::mem::size_of::<Self>()
    }

    pub(super) fn bootstrap(cpu_index: CpuIndex) -> Self {
        let name = match ThreadName::new("bootstrap") {
            Ok(name) => name,
            Err(_) => ThreadName::empty(),
        };
        Self {
            id: ThreadId::BOOTSTRAP,
            placement: ThreadPlacement::pinned(cpu_index),
            name,
            scheduling: SchedulingPolicy::fair(),
            fair_runtime: FairRuntime::NEW,
            deferred_fifo_placement: None,
            state: ThreadState::Running,
            queue_links: QueueLinks::EMPTY,
            wait: WaitRecord::NEW,
            pending_migration: None,
            context: crate::hal::context::ThreadContext::empty(),
            kernel_stack: None,
            execution: ThreadExecution::Kernel,
        }
    }

    pub(super) fn kernel(
        id: ThreadId,
        cpu_index: CpuIndex,
        affinity: crate::kernel::task::policy::CpuMask,
        name: &str,
        entry: KernelThreadEntry,
        argument: usize,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut context = crate::hal::context::ThreadContext::empty();
        context.prepare(stack.top(), entry, argument);
        let placement = ThreadPlacement::movable_with_affinity(cpu_index, affinity)
            .ok_or(Error::InvalidPlacement)?;
        Ok(Self {
            id,
            placement,
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::fair(),
            fair_runtime: FairRuntime::NEW,
            deferred_fifo_placement: None,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            wait: WaitRecord::NEW,
            pending_migration: None,
            context,
            kernel_stack: Some(stack),
            execution: ThreadExecution::Kernel,
        })
    }

    /// Creates the permanent fallback Thread for one already-registered CPU.
    pub(super) fn idle(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
        entry: KernelThreadEntry,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut context = crate::hal::context::ThreadContext::empty();
        context.prepare(stack.top(), entry, 0);
        Ok(Self {
            id,
            placement: ThreadPlacement::pinned(cpu_index),
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::Idle,
            fair_runtime: FairRuntime::NEW,
            deferred_fifo_placement: None,
            state: ThreadState::Idle,
            queue_links: QueueLinks::EMPTY,
            wait: WaitRecord::NEW,
            pending_migration: None,
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
            scheduling: SchedulingPolicy::fair(),
            fair_runtime: FairRuntime::NEW,
            deferred_fifo_placement: None,
            state: ThreadState::Running,
            queue_links: QueueLinks::EMPTY,
            wait: WaitRecord::NEW,
            pending_migration: None,
            context: crate::hal::context::ThreadContext::empty(),
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
        context: crate::hal::vm::VcpuContext,
        entry: KernelThreadEntry,
    ) -> Result<Self, Error> {
        let mut hardware = crate::hal::vm::VcpuHardwareState::new(context);
        crate::hal::vm::initialize_vcpu_interrupts(&mut hardware)?;
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut scheduling_context = crate::hal::context::ThreadContext::empty();
        scheduling_context.prepare_vcpu(stack.top(), entry, 0);
        Ok(Self {
            id,
            placement: ThreadPlacement::prefer(cpu_index),
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::fair(),
            fair_runtime: FairRuntime::NEW,
            deferred_fifo_placement: None,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            wait: WaitRecord::NEW,
            pending_migration: None,
            context: scheduling_context,
            kernel_stack: Some(stack),
            execution: ThreadExecution::Vcpu(
                hyper::mm::try_box(VcpuExecution {
                    vm: VcpuVm::Installed(vm),
                    active_execution: None,
                    vcpu_id,
                    hardware,
                })
                .map_err(|_| Error::Allocation)?,
            ),
        })
    }

    pub(super) fn user(
        id: ThreadId,
        cpu_index: CpuIndex,
        affinity: crate::kernel::task::policy::CpuMask,
        name: &str,
        execution: Box<crate::kernel::process::UserExecution>,
        entry: KernelThreadEntry,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut context = crate::hal::context::ThreadContext::empty();
        context.prepare(stack.top(), entry, 0);
        let placement = ThreadPlacement::movable_with_affinity(cpu_index, affinity)
            .ok_or(Error::InvalidPlacement)?;
        Ok(Self {
            id,
            placement,
            name: ThreadName::new(name)?,
            scheduling: SchedulingPolicy::fair(),
            fair_runtime: FairRuntime::NEW,
            deferred_fifo_placement: None,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            wait: WaitRecord::NEW,
            pending_migration: None,
            context,
            kernel_stack: Some(stack),
            execution: ThreadExecution::User(execution),
        })
    }

    pub const fn id(&self) -> ThreadId {
        self.id
    }

    /// Returns the CPU that owns this thread's scheduling context.
    ///
    /// Assignment changes only through the scheduler's stopped-thread handoff.
    pub const fn cpu_index(&self) -> CpuIndex {
        self.placement.assigned_cpu()
    }

    pub(super) const fn affinity(&self) -> crate::kernel::task::policy::CpuMask {
        self.placement.affinity()
    }

    pub(super) const fn placement_policy(&self) -> crate::kernel::task::policy::PlacementPolicy {
        self.placement.policy()
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

    pub(super) fn set_scheduling_policy(&mut self, policy: SchedulingPolicy) -> bool {
        if self.scheduling_class() == SchedulingClass::Idle {
            return false;
        }
        self.scheduling = policy;
        self.fair_runtime = FairRuntime::NEW;
        self.deferred_fifo_placement = None;
        true
    }

    pub(super) fn account_fair_ticks(&mut self, elapsed: u64, quantum: u64) -> bool {
        if self.scheduling_class() != SchedulingClass::Fair {
            return false;
        }
        if self.fair_runtime.slice_remaining == 0 {
            self.fair_runtime.slice_remaining = quantum;
        }
        self.fair_runtime.slice_remaining =
            self.fair_runtime.slice_remaining.saturating_sub(elapsed);
        self.fair_runtime.slice_remaining == 0
    }

    pub(super) fn replenish_fair_slice(&mut self, quantum: u64) {
        self.fair_runtime.slice_remaining = quantum;
    }

    pub(super) fn fair_slice_expired(&self) -> bool {
        self.scheduling_class() == SchedulingClass::Fair && self.fair_runtime.slice_remaining == 0
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

    pub(super) fn replace_affinity(
        &mut self,
        affinity: crate::kernel::task::policy::CpuMask,
    ) -> bool {
        let Some(placement) = self.placement.with_affinity(affinity) else {
            return false;
        };
        self.placement = placement;
        true
    }

    /// Applies an affinity and assignment change after the context is stopped.
    pub(super) fn reassign_stopped_with_affinity(
        &mut self,
        cpu: CpuIndex,
        affinity: crate::kernel::task::policy::CpuMask,
    ) -> bool {
        let Some(placement) = self.placement.reassign_with_affinity(cpu, affinity) else {
            return false;
        };
        self.placement = placement;
        true
    }

    pub(super) const fn queue_links(&self) -> QueueLinks {
        self.queue_links
    }

    pub(super) fn set_queue_links(&mut self, links: QueueLinks) {
        self.queue_links = links;
    }

    pub(super) const fn wait_record(&self) -> &WaitRecord {
        &self.wait
    }

    pub(super) const fn wait_record_mut(&mut self) -> &mut WaitRecord {
        &mut self.wait
    }

    pub(super) const fn pending_migration(&self) -> Option<MigrationRequest> {
        self.pending_migration
    }

    pub(super) fn request_migration(&mut self, request: MigrationRequest) -> bool {
        match self.pending_migration {
            Some(existing) => existing == request,
            None => {
                self.pending_migration = Some(request);
                true
            }
        }
    }

    pub(super) fn take_migration_request(&mut self) -> Option<MigrationRequest> {
        self.pending_migration.take()
    }

    pub fn context(&self) -> &crate::hal::context::ThreadContext {
        &self.context
    }

    pub(super) fn context_mut(&mut self) -> &mut crate::hal::context::ThreadContext {
        &mut self.context
    }

    pub const fn execution_kind(&self) -> ExecutionKind {
        match self.execution {
            ThreadExecution::Kernel => ExecutionKind::Kernel,
            ThreadExecution::Vcpu(_) => ExecutionKind::Vcpu,
            ThreadExecution::User(_) => ExecutionKind::User,
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

    pub(crate) fn user_execution(&self) -> Option<&crate::kernel::process::UserExecution> {
        match &self.execution {
            ThreadExecution::User(execution) => Some(execution.as_ref()),
            _ => None,
        }
    }

    pub(super) fn user_execution_mut(
        &mut self,
    ) -> Option<&mut crate::kernel::process::UserExecution> {
        match &mut self.execution {
            ThreadExecution::User(execution) => Some(execution.as_mut()),
            _ => None,
        }
    }

    pub(super) fn take_user_execution(
        &mut self,
    ) -> Option<Box<crate::kernel::process::UserExecution>> {
        if !matches!(self.execution, ThreadExecution::User(_)) {
            return None;
        }
        match core::mem::replace(&mut self.execution, ThreadExecution::Kernel) {
            ThreadExecution::User(execution) => Some(execution),
            _ => crate::hal::cpu::halt(),
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
