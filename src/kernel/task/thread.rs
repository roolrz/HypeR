// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral thread objects and execution payloads.

use alloc::boxed::Box;
use core::cell::UnsafeCell;
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
    terminal_mmio_report: Option<crate::kernel::vm::UnhandledMmioReport>,
    reap_publication: Option<crate::kernel::vm::registry::VcpuReapPublication>,
    pub(crate) vcpu_id: u32,
    pub(crate) hardware: crate::hal::vm::VcpuHardwareState,
}

// Keep migration eligibility compiler-proven. CPU-affine execution and
// residency claims live exclusively in `vm::active_vcpu`, never in this
// scheduler-owned payload.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<VcpuExecution>();
};

enum VcpuVm {
    Installed(crate::kernel::vm::registry::VmBinding),
    TimerValidation { interrupts: usize },
}

impl VcpuExecution {
    pub(in crate::kernel) fn installed(
        vm: crate::kernel::vm::registry::VmBinding,
        vcpu_id: u32,
        context: crate::hal::vm::VcpuContext,
        entry_ready: &crate::hal::vm::VmEntryReady,
    ) -> Result<Self, Error> {
        let mut hardware = crate::hal::vm::VcpuHardwareState::new(context, entry_ready);
        crate::hal::vm::initialize_vcpu_interrupts(&mut hardware)?;
        Ok(Self {
            vm: VcpuVm::Installed(vm),
            terminal_mmio_report: None,
            reap_publication: None,
            vcpu_id,
            hardware,
        })
    }

    pub(in crate::kernel) fn vm_binding(&self) -> Option<&crate::kernel::vm::registry::VmBinding> {
        match &self.vm {
            VcpuVm::Installed(binding) => Some(binding),
            VcpuVm::TimerValidation { .. } => None,
        }
    }

    // Only selected guest platforms with in-kernel MMIO models consume this
    // view today. Keep the stable Thread payload API available to those
    // modules without changing VcpuExecution's layout by host architecture.
    #[allow(dead_code)]
    pub(in crate::kernel) fn device_context(
        &mut self,
    ) -> Option<(
        &crate::kernel::vm::registry::VmBinding,
        &mut crate::hal::vm::VcpuHardwareState,
        u32,
    )> {
        match &self.vm {
            VcpuVm::Installed(binding) => Some((binding, &mut self.hardware, self.vcpu_id)),
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

    /// Retains a terminal MMIO diagnostic until stopped hardware is detached.
    // Terminal MMIO diagnostics are currently produced by the AArch64 guest
    // platform; the storage and ownership protocol remain architecture-neutral.
    #[allow(dead_code)]
    pub(in crate::kernel) fn publish_terminal_mmio_report(
        &mut self,
        report: crate::kernel::vm::UnhandledMmioReport,
    ) -> Result<(), ()> {
        if self.terminal_mmio_report.is_some() {
            return Err(());
        }
        self.terminal_mmio_report = Some(report);
        Ok(())
    }

    pub(in crate::kernel) const fn terminal_mmio_report_pending(&self) -> bool {
        self.terminal_mmio_report.is_some()
    }

    /// Takes the owned report only after terminal hardware detachment.
    pub(in crate::kernel) fn take_terminal_mmio_report(
        &mut self,
    ) -> Option<crate::kernel::vm::UnhandledMmioReport> {
        self.terminal_mmio_report.take()
    }

    pub(in crate::kernel) fn arm_reap_publication(
        &mut self,
        thread: ThreadId,
        reason: crate::kernel::vm::registry::VcpuClosureReason,
    ) -> Result<(), ()> {
        if self.reap_publication.is_some() {
            return Err(());
        }
        let Some(binding) = self.vm_binding() else {
            return Err(());
        };
        self.reap_publication = Some(crate::kernel::vm::registry::VcpuReapPublication::new(
            binding.id(),
            self.vcpu_id,
            thread,
            reason,
        ));
        Ok(())
    }

    fn take_reap_publication(
        &mut self,
    ) -> Option<crate::kernel::vm::registry::VcpuReapPublication> {
        self.reap_publication.take()
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
            terminal_mmio_report: None,
            reap_publication: None,
            vcpu_id: 0,
            hardware,
        }
    }
}

pub(crate) enum ThreadExecution {
    Kernel,
    Vcpu(Box<UnsafeCell<VcpuExecution>>),
    User(Box<UnsafeCell<crate::kernel::process::UserExecution>>),
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
/// privilege domain. A user execution payload strongly owns its Process and
/// prepared address space before it becomes a Thread variant.
pub struct Thread {
    identity: ThreadIdentity,
    schedule_owner: ScheduleOwner,
    schedule: UnsafeCell<ThreadScheduleState>,
    /// Waiting/terminated intrusive links owned only by `TransitionLock`.
    ///
    /// This cell is independent from the CPU-owned scheduling domain so a
    /// global control queue may safely link neighbors owned by different CPUs.
    control_queue_links: UnsafeCell<QueueLinks>,
    resources: Box<ThreadResources>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduleOwner {
    /// Owned by the transition coordinator and absent from every ready queue.
    Coordinator,
    /// Owned by one CPU domain, either as current or on that CPU's ready queue.
    Cpu(CpuIndex),
}

/// Immutable identity published for the complete registry lifetime.
struct ThreadIdentity {
    id: ThreadId,
    name: ThreadNameSnapshot,
}

/// State mutated only by scheduler transactions.
///
/// Keeping queue topology, placement, policy, and wait/migration state in one
/// explicit domain permits its ownership to move without moving Thread
/// identity or execution resources. Stored state is transition-lock-owned;
/// running state is owned by one CPU scheduler lock. The state has no internal
/// synchronization, so the linear residence token is the access authority.
pub(super) struct ThreadScheduleState {
    pub(super) placement: ThreadPlacement,
    pub(super) scheduling: SchedulingPolicy,
    fair_runtime: FairRuntime,
    pub(super) deferred_fifo_placement: Option<DeferredFifoPlacement>,
    pub(super) state: ThreadState,
    pub(super) ready_queue_links: QueueLinks,
    pub(super) wait: WaitRecord,
    pub(super) pending_migration: Option<MigrationRequest>,
}

impl ThreadScheduleState {
    pub(super) fn fair_slice_expired(&self) -> bool {
        self.scheduling.class() == SchedulingClass::Fair && self.fair_runtime.slice_remaining == 0
    }

    pub(super) fn scheduling_class(&self) -> SchedulingClass {
        self.scheduling.class()
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

    pub(super) fn expire_fair_slice(&mut self) {
        if self.scheduling_class() == SchedulingClass::Fair {
            self.fair_runtime.slice_remaining = 0;
        }
    }
}

/// Stable machine resources governed by the running/stopped context protocol.
///
/// This value has its own private heap allocation. Mutating `Thread` identity
/// or scheduling state therefore cannot create an exclusive reference that
/// covers a machine pointer retained by assembly or an execution runner. The
/// Box is never replaced while the Thread is published; resource extraction
/// is permitted only after scheduler context ownership has stopped.
struct ThreadResources {
    /// Assembly owns this cell between switch preparation and switch tail.
    context: UnsafeCell<crate::hal::context::ThreadContext>,
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
    pub(super) fn schedule_is_coordinator_owned(&self) -> bool {
        self.schedule_owner == ScheduleOwner::Coordinator
    }

    /// CPU owning this Thread's linear running-schedule token, if any.
    pub(super) fn schedule_owner_cpu(&self) -> Option<CpuIndex> {
        match self.schedule_owner {
            ScheduleOwner::Coordinator => None,
            ScheduleOwner::Cpu(cpu) => Some(cpu),
        }
    }

    /// Bounded scheduler storage charged before user-thread publication.
    pub(crate) const fn allocation_size() -> usize {
        core::mem::size_of::<Self>() + core::mem::size_of::<ThreadResources>()
    }

    fn allocate_resources(
        context: crate::hal::context::ThreadContext,
        kernel_stack: Option<KernelStack>,
        execution: ThreadExecution,
    ) -> Result<Box<ThreadResources>, Error> {
        hyper::mm::try_box(ThreadResources {
            context: UnsafeCell::new(context),
            kernel_stack,
            execution,
        })
        .map_err(|_| Error::Allocation)
    }

    pub(super) fn bootstrap(cpu_index: CpuIndex) -> Result<Self, Error> {
        let name = match ThreadNameSnapshot::new("bootstrap") {
            Ok(name) => name,
            Err(_) => ThreadNameSnapshot::empty(),
        };
        Ok(Self {
            identity: ThreadIdentity {
                id: ThreadId::BOOTSTRAP,
                name,
            },
            schedule_owner: ScheduleOwner::Coordinator,
            schedule: UnsafeCell::new(ThreadScheduleState {
                placement: ThreadPlacement::pinned(cpu_index),
                scheduling: SchedulingPolicy::fair(),
                fair_runtime: FairRuntime::NEW,
                deferred_fifo_placement: None,
                state: ThreadState::Running,
                ready_queue_links: QueueLinks::EMPTY,
                wait: WaitRecord::NEW,
                pending_migration: None,
            }),
            control_queue_links: UnsafeCell::new(QueueLinks::EMPTY),
            resources: Self::allocate_resources(
                crate::hal::context::ThreadContext::empty(),
                None,
                ThreadExecution::Kernel,
            )?,
        })
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
            identity: ThreadIdentity {
                id,
                name: ThreadNameSnapshot::new(name)?,
            },
            schedule_owner: ScheduleOwner::Coordinator,
            schedule: UnsafeCell::new(ThreadScheduleState {
                placement,
                scheduling: SchedulingPolicy::fair(),
                fair_runtime: FairRuntime::NEW,
                deferred_fifo_placement: None,
                state: ThreadState::Dormant,
                ready_queue_links: QueueLinks::EMPTY,
                wait: WaitRecord::NEW,
                pending_migration: None,
            }),
            control_queue_links: UnsafeCell::new(QueueLinks::EMPTY),
            resources: Self::allocate_resources(context, Some(stack), ThreadExecution::Kernel)?,
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
            identity: ThreadIdentity {
                id,
                name: ThreadNameSnapshot::new(name)?,
            },
            schedule_owner: ScheduleOwner::Coordinator,
            schedule: UnsafeCell::new(ThreadScheduleState {
                placement: ThreadPlacement::pinned(cpu_index),
                scheduling: SchedulingPolicy::Idle,
                fair_runtime: FairRuntime::NEW,
                deferred_fifo_placement: None,
                state: ThreadState::Idle,
                ready_queue_links: QueueLinks::EMPTY,
                wait: WaitRecord::NEW,
                pending_migration: None,
            }),
            control_queue_links: UnsafeCell::new(QueueLinks::EMPTY),
            resources: Self::allocate_resources(context, Some(stack), ThreadExecution::Kernel)?,
        })
    }

    /// Creates the already-running bootstrap context for a secondary CPU.
    pub(super) fn secondary_bootstrap(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
    ) -> Result<Self, Error> {
        Ok(Self {
            identity: ThreadIdentity {
                id,
                name: ThreadNameSnapshot::new(name)?,
            },
            schedule_owner: ScheduleOwner::Coordinator,
            schedule: UnsafeCell::new(ThreadScheduleState {
                placement: ThreadPlacement::pinned(cpu_index),
                scheduling: SchedulingPolicy::fair(),
                fair_runtime: FairRuntime::NEW,
                deferred_fifo_placement: None,
                state: ThreadState::Running,
                ready_queue_links: QueueLinks::EMPTY,
                wait: WaitRecord::NEW,
                pending_migration: None,
            }),
            control_queue_links: UnsafeCell::new(QueueLinks::EMPTY),
            resources: Self::allocate_resources(
                crate::hal::context::ThreadContext::empty(),
                Some(KernelStack::allocate_thread().map_err(|_| Error::Allocation)?),
                ThreadExecution::Kernel,
            )?,
        })
    }

    pub(super) fn vcpu(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
        execution: VcpuExecution,
        entry: KernelThreadEntry,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut scheduling_context = crate::hal::context::ThreadContext::empty();
        scheduling_context.prepare_vcpu(stack.top(), entry, 0);
        Ok(Self {
            identity: ThreadIdentity {
                id,
                name: ThreadNameSnapshot::new(name)?,
            },
            schedule_owner: ScheduleOwner::Coordinator,
            schedule: UnsafeCell::new(ThreadScheduleState {
                placement: ThreadPlacement::prefer(cpu_index),
                scheduling: SchedulingPolicy::fair(),
                fair_runtime: FairRuntime::NEW,
                deferred_fifo_placement: None,
                state: ThreadState::Dormant,
                ready_queue_links: QueueLinks::EMPTY,
                wait: WaitRecord::NEW,
                pending_migration: None,
            }),
            control_queue_links: UnsafeCell::new(QueueLinks::EMPTY),
            resources: Self::allocate_resources(
                scheduling_context,
                Some(stack),
                ThreadExecution::Vcpu(
                    hyper::mm::try_box(UnsafeCell::new(execution))
                        .map_err(|_| Error::Allocation)?,
                ),
            )?,
        })
    }

    pub(super) fn user(
        id: ThreadId,
        cpu_index: CpuIndex,
        affinity: crate::kernel::task::policy::CpuMask,
        name: &str,
        execution: Box<UnsafeCell<crate::kernel::process::UserExecution>>,
        entry: KernelThreadEntry,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate_thread().map_err(|_| Error::Allocation)?;
        let mut context = crate::hal::context::ThreadContext::empty();
        context.prepare(stack.top(), entry, 0);
        let placement = ThreadPlacement::movable_with_affinity(cpu_index, affinity)
            .ok_or(Error::InvalidPlacement)?;
        Ok(Self {
            identity: ThreadIdentity {
                id,
                name: ThreadNameSnapshot::new(name)?,
            },
            schedule_owner: ScheduleOwner::Coordinator,
            schedule: UnsafeCell::new(ThreadScheduleState {
                placement,
                scheduling: SchedulingPolicy::fair(),
                fair_runtime: FairRuntime::NEW,
                deferred_fifo_placement: None,
                state: ThreadState::Dormant,
                ready_queue_links: QueueLinks::EMPTY,
                wait: WaitRecord::NEW,
                pending_migration: None,
            }),
            control_queue_links: UnsafeCell::new(QueueLinks::EMPTY),
            resources: Self::allocate_resources(
                context,
                Some(stack),
                ThreadExecution::User(execution),
            )?,
        })
    }

    pub const fn id(&self) -> ThreadId {
        self.identity.id
    }

    fn stored_schedule(&self) -> &ThreadScheduleState {
        if self.schedule_owner != ScheduleOwner::Coordinator {
            crate::hal::cpu::halt();
        }
        // SAFETY: coordinator access is serialized by TransitionLock, and a
        // Cpu owner is rejected before the cell is dereferenced.
        unsafe { &*self.schedule.get() }
    }

    fn stored_schedule_mut(&mut self) -> &mut ThreadScheduleState {
        if self.schedule_owner != ScheduleOwner::Coordinator {
            crate::hal::cpu::halt();
        }
        self.schedule.get_mut()
    }

    pub(super) fn with_coordinator_schedule_mut<R>(
        &mut self,
        operation: impl FnOnce(&mut ThreadScheduleState) -> R,
    ) -> R {
        operation(self.stored_schedule_mut())
    }

    /// Transfers one coordinator-owned schedule into a CPU scheduling domain.
    pub(super) fn claim_schedule(&mut self, cpu: CpuIndex) -> bool {
        if self.schedule_owner != ScheduleOwner::Coordinator {
            return false;
        }
        let schedule = self.schedule.get_mut();
        if schedule.placement.assigned_cpu() != cpu
            || !matches!(schedule.state, ThreadState::Running | ThreadState::Idle)
            || schedule.ready_queue_links.membership != QueueMembership::None
        {
            return false;
        }
        self.schedule_owner = ScheduleOwner::Cpu(cpu);
        true
    }

    /// Returns a CPU-owned schedule to the coordinator after it is absent
    /// from that CPU's current and ready ownership structures.
    pub(super) fn release_schedule(&mut self, cpu: CpuIndex) -> bool {
        if self.schedule_owner != ScheduleOwner::Cpu(cpu) {
            return false;
        }
        self.schedule_owner = ScheduleOwner::Coordinator;
        true
    }

    /// Accesses CPU-owned schedule state under the matching CPU lock.
    ///
    /// # Safety
    ///
    /// The caller must hold CPU `cpu`'s scheduler lock and must have
    /// revalidated `schedule_owner_cpu() == Some(cpu)` after acquiring it.
    pub(super) unsafe fn with_cpu_schedule<R>(
        &self,
        cpu: CpuIndex,
        operation: impl FnOnce(&ThreadScheduleState) -> R,
    ) -> Option<R> {
        if self.schedule_owner != ScheduleOwner::Cpu(cpu) {
            return None;
        }
        // SAFETY: guaranteed by the caller's matching CPU-lock authority.
        Some(operation(unsafe { &*self.schedule.get() }))
    }

    /// Mutably accesses CPU-owned schedule state under the matching CPU lock.
    ///
    /// # Safety
    ///
    /// The caller must hold CPU `cpu`'s scheduler lock exclusively and must
    /// have revalidated the owner locator after acquiring it.
    pub(super) unsafe fn with_cpu_schedule_mut<R>(
        &self,
        cpu: CpuIndex,
        operation: impl FnOnce(&mut ThreadScheduleState) -> R,
    ) -> Option<R> {
        if self.schedule_owner != ScheduleOwner::Cpu(cpu) {
            return None;
        }
        // SAFETY: guaranteed by the caller's matching CPU-lock authority.
        Some(operation(unsafe { &mut *self.schedule.get() }))
    }

    /// Commits coordinator-owned state to one ready queue.
    ///
    /// Queue code performs every fallible topology check before this operation;
    /// failure therefore indicates an internal transaction bug and must not be
    /// recovered after neighboring links have changed.
    pub(super) fn publish_ready_ownership(&mut self, cpu: CpuIndex, links: QueueLinks) -> bool {
        if links.membership
            != match links.membership {
                QueueMembership::ReadyRealTime { priority, .. } => {
                    QueueMembership::ReadyRealTime { cpu, priority }
                }
                QueueMembership::ReadyFair { .. } => QueueMembership::ReadyFair { cpu },
                _ => return false,
            }
        {
            return false;
        }
        if self.schedule_owner != ScheduleOwner::Coordinator {
            return false;
        }
        let schedule = self.schedule.get_mut();
        if schedule.ready_queue_links.membership != QueueMembership::None
            || self.control_queue_links.get_mut().membership != QueueMembership::None
        {
            return false;
        }
        schedule.ready_queue_links = links;
        schedule.state = ThreadState::Ready;
        self.schedule_owner = ScheduleOwner::Cpu(cpu);
        true
    }

    /// Returns the CPU that owns this thread's scheduling context.
    ///
    /// Assignment changes only through the scheduler's stopped-thread handoff.
    pub fn cpu_index(&self) -> CpuIndex {
        self.stored_schedule().placement.assigned_cpu()
    }

    pub(super) fn affinity(&self) -> crate::kernel::task::policy::CpuMask {
        self.stored_schedule().placement.affinity()
    }

    pub(super) fn placement_policy(&self) -> crate::kernel::task::policy::PlacementPolicy {
        self.stored_schedule().placement.policy()
    }

    pub fn name(&self) -> &str {
        self.identity.name.as_str()
    }

    pub(crate) const fn name_snapshot(&self) -> ThreadNameSnapshot {
        self.identity.name
    }

    pub fn state(&self) -> ThreadState {
        self.stored_schedule().state
    }

    pub(crate) fn scheduling_policy(&self) -> SchedulingPolicy {
        self.stored_schedule().scheduling
    }

    pub(crate) fn scheduling_class(&self) -> SchedulingClass {
        self.stored_schedule().scheduling.class()
    }

    pub fn priority(&self) -> Option<ThreadPriority> {
        self.stored_schedule().scheduling.priority()
    }

    pub(super) fn set_scheduling_policy(&mut self, policy: SchedulingPolicy) -> bool {
        if self.scheduling_class() == SchedulingClass::Idle {
            return false;
        }
        let schedule = self.stored_schedule_mut();
        schedule.scheduling = policy;
        schedule.fair_runtime = FairRuntime::NEW;
        schedule.deferred_fifo_placement = None;
        true
    }

    pub(super) fn fair_slice_expired(&self) -> bool {
        let schedule = self.stored_schedule();
        schedule.scheduling.class() == SchedulingClass::Fair
            && schedule.fair_runtime.slice_remaining == 0
    }

    pub(super) fn deferred_fifo_placement(&self) -> Option<DeferredFifoPlacement> {
        self.stored_schedule().deferred_fifo_placement
    }

    pub(super) fn queue_links(&self) -> QueueLinks {
        // SAFETY: coordinator schedule access is legal only under the global
        // transition authority, which also owns the control-link cell.
        unsafe { self.combined_queue_links(self.stored_schedule().ready_queue_links) }
    }

    /// Combines disjoint ready and control topology under transition authority.
    ///
    /// # Safety
    ///
    /// The caller must hold the global `TransitionLock`. A CPU lock alone does
    /// not authorize reading the control-link cell.
    pub(super) unsafe fn combined_queue_links(&self, ready: QueueLinks) -> QueueLinks {
        // SAFETY: inherited from this method's caller contract.
        let control = unsafe { self.control_queue_links() };
        match (ready.membership, control.membership) {
            (QueueMembership::None, _) => control,
            (_, QueueMembership::None) => ready,
            _ => crate::hal::cpu::halt(),
        }
    }

    pub(super) fn table_owned_ready_queue_links(&self) -> Option<QueueLinks> {
        match self.schedule_owner {
            ScheduleOwner::Coordinator => Some(self.stored_schedule().ready_queue_links),
            ScheduleOwner::Cpu(_) => None,
        }
    }

    /// Returns links owned exclusively by the transition coordinator.
    pub(super) unsafe fn control_queue_links(&self) -> QueueLinks {
        // SAFETY: the caller holds TransitionLock control authority and the
        // cell is disjoint from schedule.
        unsafe { *self.control_queue_links.get() }
    }

    /// Mutates transition-coordinator queue topology without borrowing a
    /// CPU-owned schedule.
    ///
    /// # Safety
    ///
    /// The caller must hold the registry's unique control-queue authority.
    pub(super) unsafe fn with_control_queue_links_mut<R>(
        &self,
        operation: impl FnOnce(&mut QueueLinks) -> R,
    ) -> R {
        // SAFETY: guaranteed by the caller's linear control authority.
        operation(unsafe { &mut *self.control_queue_links.get() })
    }

    pub(super) fn wait_record(&self) -> &WaitRecord {
        &self.stored_schedule().wait
    }

    pub(super) fn pending_migration(&self) -> Option<MigrationRequest> {
        self.stored_schedule().pending_migration
    }

    /// Returns the stable machine-context address without minting a Rust borrow.
    ///
    /// The scheduler's switch protocol exclusively owns mutation from switch
    /// preparation until the incoming tail retires `switching_from`.
    pub(super) fn context_pointer(&self) -> *mut crate::hal::context::ThreadContext {
        self.resources.context.get()
    }

    pub fn execution_kind(&self) -> ExecutionKind {
        match self.resources.execution {
            ThreadExecution::Kernel => ExecutionKind::Kernel,
            ThreadExecution::Vcpu(_) => ExecutionKind::Vcpu,
            ThreadExecution::User(_) => ExecutionKind::User,
        }
    }

    /// Returns the stable vCPU payload address without creating `&mut`.
    ///
    /// Current-vCPU admission and hardware ownership serialize all dereference
    /// of this pointer. Repeated scheduler queries therefore cannot invalidate
    /// a previously issued raw capability by retagging an exclusive reference.
    pub(super) fn vcpu_execution_pointer(&self) -> Option<*mut VcpuExecution> {
        match &self.resources.execution {
            ThreadExecution::Vcpu(execution) => Some(execution.get()),
            _ => None,
        }
    }

    /// Returns the stable user payload address through a shared reference.
    ///
    /// `UserExecution` confines machine-register mutation to its own
    /// `UnsafeCell`; scheduler identity and lifecycle observation are shared.
    pub(super) fn user_execution_pointer(
        &self,
    ) -> Option<core::ptr::NonNull<crate::kernel::process::UserExecution>> {
        match &self.resources.execution {
            ThreadExecution::User(execution) => core::ptr::NonNull::new(execution.get()),
            _ => None,
        }
    }

    pub(crate) fn user_execution(&self) -> Option<&crate::kernel::process::UserExecution> {
        match &self.resources.execution {
            // SAFETY: running UserExecution mutates only its internal machine
            // context cell. Whole-object mutation is restricted to dormant or
            // stopped ownership below, where no shared pointer is live.
            ThreadExecution::User(execution) => Some(unsafe { &*execution.get() }),
            _ => None,
        }
    }

    /// Arms a dormant user payload before its first scheduler publication.
    pub(super) fn arm_user_execution(&mut self) -> bool {
        match &mut self.resources.execution {
            ThreadExecution::User(execution) => {
                execution.get_mut().arm_after_process_publication();
                true
            }
            _ => false,
        }
    }

    /// Extracts user ownership only after the scheduler proved context stop.
    pub(super) fn take_user_execution(
        &mut self,
    ) -> Option<Box<UnsafeCell<crate::kernel::process::UserExecution>>> {
        if !matches!(self.resources.execution, ThreadExecution::User(_)) {
            return None;
        }
        match core::mem::replace(&mut self.resources.execution, ThreadExecution::Kernel) {
            ThreadExecution::User(execution) => Some(execution),
            _ => crate::hal::cpu::halt(),
        }
    }

    pub(super) fn take_vcpu_reap_publication(
        &mut self,
    ) -> Option<crate::kernel::vm::registry::VcpuReapPublication> {
        match &mut self.resources.execution {
            // `detach_terminated` proved no CPU or switch tail owns this
            // payload, so the cell's unique owner may safely use `get_mut`.
            ThreadExecution::Vcpu(execution) => execution.get_mut().take_reap_publication(),
            _ => None,
        }
    }

    pub const fn owns_kernel_stack(&self) -> bool {
        self.resources.kernel_stack.is_some()
    }

    pub(super) fn ensure_kernel_stack(&mut self) -> Result<(usize, usize), Error> {
        if self.resources.kernel_stack.is_none() {
            self.resources.kernel_stack =
                Some(KernelStack::allocate_thread().map_err(|_| Error::Allocation)?);
        }
        self.kernel_stack_bounds().ok_or(Error::Allocation)
    }

    pub(super) fn kernel_stack_top(&self) -> Option<usize> {
        self.resources.kernel_stack.as_ref().map(KernelStack::top)
    }

    pub(super) fn kernel_stack_physical_top(&self) -> Option<u64> {
        self.resources
            .kernel_stack
            .as_ref()
            .map(KernelStack::physical_top)
    }

    pub(super) fn kernel_stack_bounds(&self) -> Option<(usize, usize)> {
        self.resources
            .kernel_stack
            .as_ref()
            .map(KernelStack::bounds)
    }

    pub(super) fn kernel_stack_statistics(
        &self,
    ) -> Option<crate::kernel::mm::stack::StackStatistics> {
        self.resources
            .kernel_stack
            .as_ref()
            .map(KernelStack::statistics)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ThreadNameSnapshot {
    bytes: [u8; THREAD_NAME_CAPACITY],
    len: u8,
}

impl ThreadNameSnapshot {
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

    pub fn as_str(&self) -> &str {
        let bytes = &self.bytes[..usize::from(self.len)];
        // Snapshots are built from UTF-8 input. Keep the accessor defensive if
        // a future internal constructor violates that invariant.
        core::str::from_utf8(bytes).unwrap_or("")
    }
}

impl core::fmt::Debug for ThreadNameSnapshot {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl core::fmt::Display for ThreadNameSnapshot {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
