// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral thread objects and execution payloads.

use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, Ordering};
use hyper::cpu::CpuIndex;

// ProcessBuilder commits one identity label for both the Process and its
// initial Thread. Keep the scheduler snapshot capacity aligned with that ABI
// contract so a name accepted by the builder cannot fail later at publication.
const _: () = assert!(hyper::abi::native::HYPER_NATIVE_PROCESS_NAME_MAX_BYTES <= usize::MAX as u64);
pub(crate) const MAX_THREAD_NAME_BYTES: usize =
    hyper::abi::native::HYPER_NATIVE_PROCESS_NAME_MAX_BYTES as usize;

use crate::kernel::mm::stack::KernelStack;
use crate::kernel::task::policy::{
    CpuMask, SchedulingClass, SchedulingPolicy, ThreadPlacement, ThreadPriority,
};
use crate::kernel::task::thread_object::{ThreadObject, ThreadObjectSnapshot, ThreadRole};
use crate::kernel::task::wait::WaitRecord;

pub type KernelThreadEntry = extern "C" fn(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel) enum ThreadRetirementError {
    PublicationRejected,
}

/// Type-erased completion owned after a subsystem execution is detached.
pub(in crate::kernel) trait ThreadRetirementAction: Send {
    fn complete(self: Box<Self>) -> Result<(), ThreadRetirementError>;
}

#[must_use = "detached execution retirement must be completed"]
pub(in crate::kernel) struct ThreadRetirement {
    action: Box<dyn ThreadRetirementAction>,
    _resource_charge: Option<crate::kernel::accounting::CommittedCharge>,
}

/// Subsystem hook for a scheduler-owned external execution allocation.
pub(in crate::kernel) trait ExternalThreadExecutionLifecycle: Send {
    fn take_thread_retirement(&mut self) -> Option<ThreadRetirement>;
}

/// Compiler-managed type erasure for one scheduler-owned execution cell.
trait ErasedExternalThreadExecution: Send {
    fn take_thread_retirement(&mut self) -> Option<ThreadRetirement>;
}

impl<T> ErasedExternalThreadExecution for UnsafeCell<T>
where
    T: ExternalThreadExecutionLifecycle + 'static,
{
    fn take_thread_retirement(&mut self) -> Option<ThreadRetirement> {
        self.get_mut().take_thread_retirement()
    }
}

/// Type-erased, uniquely owned execution payload with a stable address.
pub(in crate::kernel) struct ExternalThreadExecution {
    pointer: super::external_execution::ExternalExecutionPointer,
    owner: Box<dyn ErasedExternalThreadExecution>,
}

// SAFETY: `owner` proves the erased payload is Send and keeps the allocation
// fixed. The copied pointer is never dereferenced merely by moving this unique
// wrapper; scheduler activation establishes exclusive access after migration.
unsafe impl Send for ExternalThreadExecution {}

impl ExternalThreadExecution {
    pub(in crate::kernel) fn from_box<T>(payload: Box<UnsafeCell<T>>) -> Self
    where
        T: ExternalThreadExecutionLifecycle + 'static,
    {
        let pointer = core::ptr::NonNull::from(payload.as_ref());
        Self {
            pointer: super::external_execution::ExternalExecutionPointer::from_cell(pointer),
            owner: payload,
        }
    }

    fn pointer(&self) -> super::external_execution::ExternalExecutionPointer {
        self.pointer
    }

    fn take_retirement(&mut self) -> Option<ThreadRetirement> {
        self.owner.take_thread_retirement()
    }
}

impl ThreadRetirement {
    pub(in crate::kernel) fn from_box<T>(action: Box<T>) -> Self
    where
        T: ThreadRetirementAction + 'static,
    {
        Self {
            action,
            _resource_charge: None,
        }
    }

    pub(super) fn complete(self) -> Result<(), ThreadRetirementError> {
        let Self {
            action,
            _resource_charge,
        } = self;
        action.complete()
    }

    fn retain_charge(&mut self, charge: crate::kernel::accounting::CommittedCharge) {
        if self._resource_charge.replace(charge).is_some() {
            crate::hal::cpu::halt();
        }
    }
}

#[cfg(feature = "kernel-self-test")]
struct RetirementAccountingProbe {
    domain: crate::kernel::accounting::ResourceDomain,
    baseline: u64,
    retained: u64,
}

#[cfg(feature = "kernel-self-test")]
impl ThreadRetirementAction for RetirementAccountingProbe {
    fn complete(self: Box<Self>) -> Result<(), ThreadRetirementError> {
        let observed = self
            .domain
            .usage()
            .total(crate::kernel::accounting::ResourceKind::KernelMemoryBytes);
        if self.baseline.checked_add(self.retained) == Some(observed) {
            Ok(())
        } else {
            Err(ThreadRetirementError::PublicationRejected)
        }
    }
}

/// Verifies that an extracted retirement action remains charged while its
/// terminal callback executes and releases the charge immediately afterward.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn verify_retirement_charge_lifetime() -> bool {
    use crate::kernel::accounting::{ResourceAmount, ResourceDomain, ResourceKind, ResourceLimits};

    let domain = match ResourceDomain::try_new_root(ResourceLimits::UNLIMITED) {
        Ok(domain) => domain,
        Err(_) => return false,
    };
    let baseline = domain.usage().total(ResourceKind::KernelMemoryBytes);
    let retained = 64_u64;
    let charge = match domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, retained))
    {
        Ok(reservation) => reservation.commit(),
        Err(_) => return false,
    };
    let action = match hyper::mm::try_box(RetirementAccountingProbe {
        domain: domain.clone(),
        baseline,
        retained,
    }) {
        Ok(action) => action,
        Err(_) => return false,
    };
    let mut retirement = ThreadRetirement::from_box(action);
    retirement.retain_charge(charge);
    let charged = baseline.checked_add(retained)
        == Some(domain.usage().total(ResourceKind::KernelMemoryBytes));
    let completed = retirement.complete().is_ok();
    charged && completed && domain.usage().total(ResourceKind::KernelMemoryBytes) == baseline
}

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

enum ThreadExecution {
    Kernel,
    Vcpu(ExternalThreadExecution),
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
    ObjectAllocation,
    ObjectIdentityExhausted,
    ObjectRegistrationExhausted,
    VirtualInterrupt(crate::hal::vm::VirtualInterruptError),
}

impl From<crate::kernel::object::ObjectCreationError> for Error {
    fn from(error: crate::kernel::object::ObjectCreationError) -> Self {
        match error {
            crate::kernel::object::ObjectCreationError::Allocation => Self::ObjectAllocation,
            crate::kernel::object::ObjectCreationError::KoidExhausted => {
                Self::ObjectIdentityExhausted
            }
            crate::kernel::object::ObjectCreationError::RegistrationExhausted => {
                Self::ObjectRegistrationExhausted
            }
        }
    }
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
    object: ThreadObject,
    schedule_owner: ScheduleOwner,
    schedule: UnsafeCell<ThreadScheduleState>,
    /// Intrusive links protected by their `WaitQueue` lock plus a registry
    /// reader lane, or by exclusive coordination for lifecycle operations.
    /// Independent storage lets one queue link neighbors owned by different
    /// CPUs without borrowing their schedules.
    control_queue_links: UnsafeCell<QueueLinks>,
    /// Monotonic scheduler ticks charged while this Thread is current.
    runtime_ticks: AtomicU64,
    resources: Box<ThreadResources>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduleOwner {
    /// Owned by the transition coordinator and absent from every ready queue.
    Coordinator,
    /// Owned by one CPU domain, as current, ready, or blocked.
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
    _ownership: Option<ThreadResourceOwnership>,
}

/// Accounting ownership whose lifetime is exactly one scheduler Thread.
///
/// Payload-producing subsystems reserve these generic resources before
/// construction. The scheduler consumes the object charge while publishing
/// the canonical Thread object and retains the execution charge until the
/// detached Thread is destroyed by the reaper. A separately admitted
/// retirement charge follows an extracted terminal action through completion,
/// so destroying the Thread never leaves deferred work unaccounted.
#[must_use = "thread resources must move into a scheduler Thread"]
pub(in crate::kernel) struct ThreadResourceOwnership {
    _execution_charge: crate::kernel::accounting::CommittedCharge,
    retirement_charge: Option<crate::kernel::accounting::CommittedCharge>,
    object_charge: Option<crate::kernel::accounting::CommittedCharge>,
}

impl ThreadResourceOwnership {
    pub(in crate::kernel) const fn new(
        execution_charge: crate::kernel::accounting::CommittedCharge,
        retirement_charge: crate::kernel::accounting::CommittedCharge,
        object_charge: crate::kernel::accounting::CommittedCharge,
    ) -> Self {
        Self {
            _execution_charge: execution_charge,
            retirement_charge: Some(retirement_charge),
            object_charge: Some(object_charge),
        }
    }

    fn take_object_charge(&mut self) -> crate::kernel::accounting::CommittedCharge {
        match self.object_charge.take() {
            Some(charge) => charge,
            None => crate::hal::cpu::halt(),
        }
    }

    fn take_retirement_charge(&mut self) -> crate::kernel::accounting::CommittedCharge {
        match self.retirement_charge.take() {
            Some(charge) => charge,
            None => crate::hal::cpu::halt(),
        }
    }
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
    pub(super) fn account_runtime_ticks(&self, elapsed: u64) {
        self.runtime_ticks.fetch_add(elapsed, Ordering::Relaxed);
    }

    pub(super) fn runtime_ticks(&self) -> u64 {
        self.runtime_ticks.load(Ordering::Relaxed)
    }

    pub(super) fn role(&self) -> ThreadRole {
        self.object.role()
    }

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

    /// Heap and guarded-stack bytes retained by one scheduler-owned vCPU.
    ///
    /// This excludes both the VM binding's shared aggregate and the canonical
    /// system Thread object. Each is independently refcounted and therefore
    /// owns accounting whose lifetime follows that allocation directly.
    pub(crate) const fn external_execution_allocation_size(payload_size: usize) -> Option<usize> {
        let bytes = Self::allocation_size();
        let bytes = match bytes.checked_add(payload_size) {
            Some(bytes) => bytes,
            None => return None,
        };
        let bytes = match bytes.checked_add(crate::kernel::mm::stack::thread_stack_bytes()) {
            Some(bytes) => bytes,
            None => return None,
        };
        Some(bytes)
    }

    fn allocate_resources(
        context: crate::hal::context::ThreadContext,
        kernel_stack: Option<KernelStack>,
        execution: ThreadExecution,
        ownership: Option<ThreadResourceOwnership>,
    ) -> Result<Box<ThreadResources>, Error> {
        hyper::mm::try_box(ThreadResources {
            context: UnsafeCell::new(context),
            kernel_stack,
            execution,
            _ownership: ownership,
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
            object: ThreadObject::try_system(ThreadRole::Bootstrap)?,
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
            runtime_ticks: AtomicU64::new(0),
            resources: Self::allocate_resources(
                crate::hal::context::ThreadContext::empty(),
                None,
                ThreadExecution::Kernel,
                None,
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
            object: ThreadObject::try_system(ThreadRole::Kernel)?,
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
            runtime_ticks: AtomicU64::new(0),
            resources: Self::allocate_resources(
                context,
                Some(stack),
                ThreadExecution::Kernel,
                None,
            )?,
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
            object: ThreadObject::try_system(ThreadRole::Idle)?,
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
            runtime_ticks: AtomicU64::new(0),
            resources: Self::allocate_resources(
                context,
                Some(stack),
                ThreadExecution::Kernel,
                None,
            )?,
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
            // A secondary bootstrap continuation exists solely to complete
            // local setup and become that CPU's permanent idle Thread.
            object: ThreadObject::try_system(ThreadRole::Idle)?,
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
            runtime_ticks: AtomicU64::new(0),
            resources: Self::allocate_resources(
                crate::hal::context::ThreadContext::empty(),
                Some(KernelStack::allocate_thread().map_err(|_| Error::Allocation)?),
                ThreadExecution::Kernel,
                None,
            )?,
        })
    }

    pub(super) fn vcpu(
        id: ThreadId,
        cpu_index: CpuIndex,
        name: &str,
        execution: ExternalThreadExecution,
        mut ownership: ThreadResourceOwnership,
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
            object: ThreadObject::try_accounted_system(
                ThreadRole::Vcpu,
                ownership.take_object_charge(),
            )?,
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
            runtime_ticks: AtomicU64::new(0),
            resources: Self::allocate_resources(
                scheduling_context,
                Some(stack),
                ThreadExecution::Vcpu(execution),
                Some(ownership),
            )?,
        })
    }

    pub(super) fn user(
        id: ThreadId,
        cpu_index: CpuIndex,
        affinity: crate::kernel::task::policy::CpuMask,
        name: &str,
        object: crate::kernel::process::UserThread,
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
            object: ThreadObject::user(object),
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
            runtime_ticks: AtomicU64::new(0),
            resources: Self::allocate_resources(
                context,
                Some(stack),
                ThreadExecution::User(execution),
                None,
            )?,
        })
    }

    pub const fn id(&self) -> ThreadId {
        self.identity.id
    }

    pub(crate) fn object_snapshot(&self) -> ThreadObjectSnapshot {
        self.object.snapshot()
    }

    pub(super) fn user_thread(&self) -> Option<&crate::kernel::process::UserThread> {
        self.object.user_thread()
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
            || !matches!(
                schedule.state,
                ThreadState::Running | ThreadState::Idle | ThreadState::Blocked
            )
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

    /// Returns links under exclusive coordination or matching queue authority.
    pub(super) unsafe fn control_queue_links(&self) -> QueueLinks {
        // SAFETY: the caller owns exclusive coordination or the matching
        // queue authority. The cell is disjoint from CPU schedule storage.
        unsafe { *self.control_queue_links.get() }
    }

    /// Mutates transition-coordinator queue topology without borrowing a
    /// CPU-owned schedule.
    ///
    /// # Safety
    ///
    /// The caller must hold exclusive coordination, or the affected queue
    /// lock and a registry reader lane. An unlinked candidate also requires
    /// its CPU lock. Other referenced nodes must belong to the locked queue.
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
    pub(super) fn vcpu_execution_pointer(
        &self,
    ) -> Option<super::external_execution::ExternalExecutionPointer> {
        match &self.resources.execution {
            ThreadExecution::Vcpu(execution) => Some(execution.pointer()),
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

    /// Arms a dormant user payload before its first scheduler publication.
    pub(super) fn arm_user_execution(
        &mut self,
        ownership: crate::kernel::process::UserExecutionOwnership,
    ) -> bool {
        match &mut self.resources.execution {
            ThreadExecution::User(execution) => {
                execution.get_mut().arm_for_process_publication(ownership);
                true
            }
            _ => false,
        }
    }

    /// Extracts user ownership only after the scheduler proved context stop.
    pub(super) fn take_user_execution(
        &mut self,
    ) -> Option<(
        crate::kernel::process::UserThread,
        Box<UnsafeCell<crate::kernel::process::UserExecution>>,
    )> {
        if !matches!(self.resources.execution, ThreadExecution::User(_)) {
            return None;
        }
        let object = match self.object.user_thread() {
            Some(thread) => thread.clone(),
            None => crate::hal::cpu::halt(),
        };
        match core::mem::replace(&mut self.resources.execution, ThreadExecution::Kernel) {
            ThreadExecution::User(execution) => Some((object, execution)),
            _ => crate::hal::cpu::halt(),
        }
    }

    pub(super) fn take_vcpu_reap_publication(&mut self) -> Option<ThreadRetirement> {
        let mut retirement = match &mut self.resources.execution {
            // `detach_terminated` proved no CPU or switch tail owns this
            // payload, so the cell's unique owner may safely use `get_mut`.
            ThreadExecution::Vcpu(execution) => execution.take_retirement(),
            _ => None,
        }?;
        let ownership = match self.resources._ownership.as_mut() {
            Some(ownership) => ownership,
            None => crate::hal::cpu::halt(),
        };
        retirement.retain_charge(ownership.take_retirement_charge());
        Some(retirement)
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
    bytes: [u8; MAX_THREAD_NAME_BYTES],
    len: u8,
}

impl ThreadNameSnapshot {
    fn new(name: &str) -> Result<Self, Error> {
        if name.len() > MAX_THREAD_NAME_BYTES {
            return Err(Error::NameTooLong);
        }
        let mut bytes = [0; MAX_THREAD_NAME_BYTES];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Self {
            bytes,
            len: name.len() as u8,
        })
    }

    const fn empty() -> Self {
        Self {
            bytes: [0; MAX_THREAD_NAME_BYTES],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        let bytes = &self.bytes[..usize::from(self.len)];
        // Snapshots are built from UTF-8 input. Keep the accessor defensive if
        // a future internal constructor violates that invariant.
        core::str::from_utf8(bytes).unwrap_or("")
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
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
