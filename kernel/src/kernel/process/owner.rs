// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process composition, publication, stop, and explicit retirement.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use hyper::mm::{FallibleArc, UniqueFallibleArc, WeakFallibleArc};
use hyper::sync::InterruptSpinLock;

use super::directory::PreparedRegistration;
use super::image::{AbiFamily, ExecutionRoute, MachineAbi, ProcessImage, UserThreadStart};
use super::lifecycle::{
    LifecycleError, ProcessLifecycle, ProcessPhase, StopDispatchProgress, TerminalReason,
};
use super::task_group::{
    PreparedTaskGroupMembership, TaskGroup, TaskGroupError, TaskGroupMembership,
};
use super::user_thread::{UserExecution, UserExecutionOwnership, UserThread, UserThreadObject};
use crate::kernel::accounting::{
    ChargeReservation, CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::capability::{
    ClosedHandle, HANDLE_TABLE_STORAGE_SEGMENTS, HandleBatchReservation,
    HandleBatchReservationStorage, HandleError, HandleFlags, HandleInfo, HandleReservation,
    HandleScanCursor, HandleSidecar, HandleSnapshotPage, HandleTable, HandleTableStoragePlan,
    HandleTableStorageSnapshot, HandleTransferClaim, HandleTransferRequest, HandleTransferStorage,
    HandleValue, InTransitCapabilities, PreparedHandle, ResolvedObject, ResolvedWaitable, Rights,
};
use crate::kernel::mm::user_space::{
    MachineError, NativeAddressSpace, UserSlice, UserWriteReservation,
};
use crate::kernel::object::{
    KernelObject, Koid, ObjectCreationError, ObjectPublication, SignalMask, SignalSource,
    SignalState, UserExportableObject,
};
use crate::kernel::sync::Completion;
use crate::kernel::task::scheduler::{self, CpuMask};
use crate::kernel::task::thread::ThreadId;

type ProcessLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

static NEXT_PROCESS_ID: AtomicU64 = AtomicU64::new(1);

struct RetirementQueue {
    ready: RetirementList,
    delayed: RetirementList,
}

struct RetirementList {
    head: Option<Process>,
    tail: Option<Process>,
}

static RETIREMENTS: ProcessLock<RetirementQueue> = ProcessLock::new(RetirementQueue {
    ready: RetirementList {
        head: None,
        tail: None,
    },
    delayed: RetirementList {
        head: None,
        tail: None,
    },
});

fn queue_retirement(process: Process) {
    RETIREMENTS.with(|queue| queue.ready.push_back(process));
}

#[derive(Clone, Copy)]
pub(crate) struct RetirementWork {
    pub(crate) ready: bool,
    pub(crate) delayed: bool,
}

/// Reports immediately runnable and timer-delayed Process retirement work.
pub(crate) fn retirement_work(_access: &crate::kernel::reaper::ReaperAccess) -> RetirementWork {
    RETIREMENTS.with(|queue| RetirementWork {
        ready: !queue.ready.is_empty(),
        delayed: !queue.delayed.is_empty(),
    })
}

/// Makes one delayed retry round visible after the retry timer expires.
pub(crate) fn promote_delayed_retirements(_access: &mut crate::kernel::reaper::ReaperAccess) {
    RETIREMENTS.with(|queue| {
        // Expired retries precede newer arrivals. Otherwise a sustained stream
        // of stopped Processes could keep an older retained owner behind an
        // ever-growing ready tail even though its retry deadline has passed.
        queue.delayed.append(&mut queue.ready);
        core::mem::swap(&mut queue.ready, &mut queue.delayed);
    });
}

/// Performs one Process step per reaper batch, alternating with object/thread
/// destruction so the references being awaited can themselves be released.
pub(crate) fn reap_one_process(_access: &mut crate::kernel::reaper::ReaperAccess) {
    let process = RETIREMENTS.with(|queue| queue.ready.pop_front());
    let Some(process) = process else {
        return;
    };
    let retry = process.inner.retirement_retry.with(Option::take);
    let step = match retry {
        Some(retry) => match retry.retry() {
            Ok(()) => ProcessRetirementStep::Complete,
            Err((retry, _)) => ProcessRetirementStep::Retry(retry),
        },
        None => match process.retire() {
            Ok(step) => step,
            Err(ProcessError::Handle(HandleError::OutstandingReservation)) => {
                ProcessRetirementStep::InProgress
            }
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: Process retirement failed: {error:?}"
            )),
        },
    };
    match step {
        ProcessRetirementStep::Complete => {}
        ProcessRetirementStep::Retry(retry) => {
            process
                .inner
                .retirement_retry
                .with(|slot| *slot = Some(retry));
            RETIREMENTS.with(|queue| queue.delayed.push_back(process));
        }
        ProcessRetirementStep::InProgress | ProcessRetirementStep::PendingReferences => {
            RETIREMENTS.with(|queue| queue.delayed.push_back(process));
        }
    }
}

impl RetirementList {
    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn push_back(&mut self, process: Process) {
        if let Some(tail) = self.tail.as_ref() {
            tail.inner
                .retirement_next
                .with(|next| *next = Some(process.clone()));
        } else {
            self.head = Some(process.clone());
        }
        self.tail = Some(process);
    }

    fn pop_front(&mut self) -> Option<Process> {
        let process = self.head.take()?;
        self.head = process.inner.retirement_next.with(Option::take);
        if self.head.is_none() {
            self.tail = None;
        }
        Some(process)
    }

    fn append(&mut self, other: &mut Self) {
        let Some(head) = other.head.take() else {
            if other.tail.is_some() {
                process_invariant_violation();
            }
            return;
        };
        let tail = match other.tail.take() {
            Some(tail) => tail,
            None => process_invariant_violation(),
        };
        if let Some(current_tail) = self.tail.as_ref() {
            current_tail
                .inner
                .retirement_next
                .with(|next| *next = Some(head));
        } else {
            if self.head.is_some() {
                process_invariant_violation();
            }
            self.head = Some(head);
        }
        self.tail = Some(tail);
    }
}

fn machine_matches_host(machine: MachineAbi) -> bool {
    let requested = match machine {
        MachineAbi::Aarch64 => crate::hal::user::HostMachine::Aarch64,
        MachineAbi::Riscv64 => crate::hal::user::HostMachine::Riscv64,
        MachineAbi::X86_64 => crate::hal::user::HostMachine::X86_64,
    };
    requested == crate::hal::user::host_machine()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessId(u64);

impl ProcessId {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

struct ThreadRecord {
    active: AtomicBool,
    scheduler_id: AtomicU64,
    next: ProcessLock<Option<FallibleArc<ThreadRecord>>>,
    _metadata_charge: CommittedCharge,
}

struct ProcessState {
    lifecycle: ProcessLifecycle,
    address_space: Option<FallibleArc<NativeAddressSpace>>,
    group_membership: Option<TaskGroupMembership>,
    process_charge: Option<CommittedCharge>,
    threads: Option<FallibleArc<ThreadRecord>>,
    handle_charges: Option<FallibleArc<HandleChargeRecord>>,
    charge_index: HandleSidecar<HandleChargeLocation>,
    handle_table_charges: [Option<CommittedCharge>; HANDLE_TABLE_STORAGE_SEGMENTS],
    handles_retired: bool,
}

struct HandleChargeState {
    entries: alloc::vec::Vec<HandleChargeEntry>,
}

struct HandleChargeEntry {
    value: HandleValue,
    charge: Option<CommittedCharge>,
}

#[derive(Clone)]
struct HandleChargeLocation {
    record: FallibleArc<HandleChargeRecord>,
    entry: usize,
}

type PreparedTableStorage = (
    Option<HandleTableStoragePlan>,
    Option<CommittedCharge>,
    HandleSidecar<HandleChargeLocation>,
);

struct HandleChargeRecord {
    previous: ProcessLock<Option<WeakFallibleArc<HandleChargeRecord>>>,
    state: ProcessLock<HandleChargeState>,
    next: ProcessLock<Option<FallibleArc<HandleChargeRecord>>>,
    _metadata_charge: CommittedCharge,
}

pub(super) struct ProcessInner {
    // Declared first so directory metadata is detached before payload charges
    // are released by field destruction.
    pub(super) directory: super::directory::Membership,
    id: ProcessId,
    image_generation: u64,
    image: ProcessImage,
    domain: ResourceDomain,
    handles: ProcessLock<HandleTable>,
    state: ProcessLock<ProcessState>,
    object_published: AtomicBool,
    signals: SignalState,
    stopped: Completion,
    retirement_next: ProcessLock<Option<Process>>,
    retirement_retry: ProcessLock<Option<AddressSpaceRetirement>>,
    _metadata_charge: CommittedCharge,
}

#[derive(Debug)]
pub(crate) enum ProcessError {
    Allocation,
    Handle(HandleError),
    Object(ObjectCreationError),
    Lifecycle(LifecycleError),
    UserMemory(MachineError),
    Resource(ResourceError),
    Scheduler(scheduler::Error),
    TaskGroup(TaskGroupError),
    AddressSpaceReferenced,
    UserEntry(crate::hal::user::UserEntryError),
}

impl From<HandleError> for ProcessError {
    fn from(error: HandleError) -> Self {
        Self::Handle(error)
    }
}

impl From<ObjectCreationError> for ProcessError {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<LifecycleError> for ProcessError {
    fn from(error: LifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

impl From<MachineError> for ProcessError {
    fn from(error: MachineError) -> Self {
        Self::UserMemory(error)
    }
}

impl From<ResourceError> for ProcessError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<scheduler::Error> for ProcessError {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<TaskGroupError> for ProcessError {
    fn from(error: TaskGroupError) -> Self {
        Self::TaskGroup(error)
    }
}

/// Creation failure which preserves the unpublished machine address space.
#[must_use = "recover the unpublished address space with into_parts"]
pub(crate) struct ProcessCreateFailure {
    error: Option<ProcessError>,
    address_space: Option<UniqueFallibleArc<NativeAddressSpace>>,
}

impl ProcessCreateFailure {
    pub(crate) fn error(&self) -> &ProcessError {
        match self.error.as_ref() {
            Some(error) => error,
            None => process_invariant_violation(),
        }
    }

    /// Recovers both the error and the still-linear machine owner.
    pub(crate) fn into_parts(mut self) -> (ProcessError, UniqueFallibleArc<NativeAddressSpace>) {
        let error = match self.error.take() {
            Some(error) => error,
            None => process_invariant_violation(),
        };
        let address_space = match self.address_space.take() {
            Some(address_space) => address_space,
            None => process_invariant_violation(),
        };
        (error, address_space)
    }
}

impl Drop for ProcessCreateFailure {
    fn drop(&mut self) {
        if self.error.is_some() || self.address_space.is_some() {
            // NativeAddressSpace deliberately cannot retire from Drop. Force
            // callers to recover the linear owner instead of silently leaking
            // its published translation identifier and machine root.
            process_invariant_violation();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessSnapshot {
    pub(crate) id: ProcessId,
    pub(crate) image_generation: u64,
    pub(crate) phase: ProcessPhase,
    pub(crate) pending_threads: usize,
    pub(crate) active_threads: usize,
    pub(crate) terminal: Option<TerminalReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessStopReport {
    pub(crate) newly_requested: bool,
    pub(crate) dispatched_threads: usize,
    pub(crate) dispatch_complete: bool,
}

pub(crate) enum ProcessRetirementStep {
    Complete,
    InProgress,
    PendingReferences,
    Retry(AddressSpaceRetirement),
}

/// Strong process owner. Active `TaskGroup` membership pins one owner cycle until
/// explicit Process retirement removes it.
pub(crate) struct Process {
    pub(super) inner: FallibleArc<ProcessInner>,
}

#[must_use = "publish or abort the prepared process"]
pub(crate) struct PreparedProcess {
    process: Option<Process>,
    group: Option<PreparedTaskGroupMembership>,
    registration: Option<PreparedRegistration>,
}

impl PreparedProcess {
    pub(crate) fn try_new(
        image: ProcessImage,
        group: TaskGroup,
        domain: ResourceDomain,
        address_space: UniqueFallibleArc<NativeAddressSpace>,
    ) -> Result<Self, ProcessCreateFailure> {
        let group_membership = match group.prepare_membership() {
            Ok(membership) => membership,
            Err(error) => return Err(create_failure(error.into(), address_space)),
        };
        let metadata_amount = match process_metadata_amount() {
            Ok(amount) => amount,
            Err(error) => return Err(create_failure(error, address_space)),
        };
        let metadata_charge = match domain.reserve(metadata_amount) {
            Ok(charge) => charge.commit(),
            Err(error) => return Err(create_failure(error.into(), address_space)),
        };
        let process_amount = ResourceAmount::ZERO.with(ResourceKind::Processes, 1);
        let process_charge = match domain.reserve(process_amount) {
            Ok(charge) => charge.commit(),
            Err(error) => return Err(create_failure(error.into(), address_space)),
        };
        let address_space = address_space.into_shared();
        let id = match allocate_process_id() {
            Ok(id) => id,
            Err(error) => {
                return Err(create_failure_from_arc(error, address_space));
            }
        };
        let inner = ProcessInner {
            directory: super::directory::Membership::new(id),
            id,
            image_generation: 1,
            image,
            domain,
            handles: ProcessLock::new(HandleTable::new()),
            state: ProcessLock::new(ProcessState {
                lifecycle: ProcessLifecycle::prepared(),
                address_space: Some(address_space),
                group_membership: None,
                process_charge: Some(process_charge),
                threads: None,
                handle_charges: None,
                charge_index: HandleSidecar::new(),
                handle_table_charges: [const { None }; HANDLE_TABLE_STORAGE_SEGMENTS],
                handles_retired: false,
            }),
            object_published: AtomicBool::new(false),
            signals: SignalState::new(),
            stopped: Completion::new(),
            retirement_next: ProcessLock::new(None),
            retirement_retry: ProcessLock::new(None),
            _metadata_charge: metadata_charge,
        };
        let inner = match FallibleArc::try_new_or_return(inner) {
            Ok(inner) => inner,
            Err((_, inner)) => {
                let state = inner.state.into_inner();
                let address_space = match state.address_space {
                    Some(address_space) => address_space,
                    None => crate::hal::cpu::halt(),
                };
                return Err(create_failure_from_arc(
                    ProcessError::Allocation,
                    address_space,
                ));
            }
        };
        let process = Process { inner };
        let registration = match PreparedRegistration::try_new(&process) {
            Ok(registration) => registration,
            Err(error) => {
                let address_space = recover_unpublished_address_space(process);
                return Err(create_failure_from_arc(error, address_space));
            }
        };
        Ok(Self {
            process: Some(process),
            group: Some(group_membership),
            registration: Some(registration),
        })
    }

    /// Publishes complete Process ownership and then makes it group-visible.
    pub(crate) fn publish(mut self) -> Process {
        let process = match self.process.take() {
            Some(process) => process,
            None => process_invariant_violation(),
        };
        process.inner.state.with(|state| {
            if state.lifecycle.publish().is_err() {
                process_invariant_violation();
            }
        });
        let group = match self.group.take() {
            Some(group) => group,
            None => process_invariant_violation(),
        };
        let (membership, pending_stop) = group.publish(process.clone());
        process.inner.state.with(|state| {
            if state.group_membership.replace(membership).is_some() {
                process_invariant_violation();
            }
        });
        if let Some(generation) = pending_stop {
            let _ = process.request_stop(TerminalReason::TaskGroupStop { generation });
        }
        match self.registration.take() {
            Some(registration) => registration.publish(&process),
            None => process_invariant_violation(),
        }
        process
    }

    /// Aborts unpublished Process construction and returns machine ownership.
    pub(crate) fn abort(mut self) -> UniqueFallibleArc<NativeAddressSpace> {
        let process = match self.process.take() {
            Some(process) => process,
            None => process_invariant_violation(),
        };
        drop(self.group.take());
        drop(self.registration.take());
        let inner = match process.inner.try_unwrap() {
            Ok(inner) => inner,
            Err(_) => process_invariant_violation(),
        };
        let state = inner.state.into_inner();
        let address = match state.address_space {
            Some(address) => address,
            None => process_invariant_violation(),
        };
        match address.try_into_unique() {
            Ok(address) => address,
            Err(_) => process_invariant_violation(),
        }
    }
}

impl Drop for PreparedProcess {
    fn drop(&mut self) {
        if self.process.is_some() || self.group.is_some() || self.registration.is_some() {
            process_invariant_violation();
        }
    }
}

impl Process {
    pub(crate) const TERMINATED: SignalMask =
        SignalMask::from_trusted_bits(hyper::abi::native::HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED);
    pub(crate) const SUPPORTED_SIGNALS: SignalMask = Self::TERMINATED;

    pub(crate) fn id(&self) -> ProcessId {
        self.inner.id
    }

    pub(crate) fn image(&self) -> &ProcessImage {
        &self.inner.image
    }

    pub(crate) fn image_generation(&self) -> u64 {
        self.inner.image_generation
    }

    pub(crate) fn resource_domain(&self) -> ResourceDomain {
        self.inner.domain.clone()
    }

    pub(super) fn claim_object_publication(&self) -> bool {
        self.inner
            .object_published
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn abort_object_publication(&self) {
        if self
            .inner
            .object_published
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            process_invariant_violation();
        }
    }

    /// Retains the native address-space owner while Process handle admission
    /// remains open.
    ///
    /// Direct kernel references use this owner rather than manufacturing an
    /// internal handle. A userspace VMAR capability wraps the same owner and a
    /// generation-checked VMAR token.
    pub(crate) fn address_space_owner(
        &self,
    ) -> Result<FallibleArc<NativeAddressSpace>, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            state
                .address_space
                .as_ref()
                .cloned()
                .ok_or(ProcessError::AddressSpaceReferenced)
        })
    }

    pub(crate) fn snapshot(&self) -> ProcessSnapshot {
        self.inner.state.with(|state| ProcessSnapshot {
            id: self.id(),
            image_generation: self.image_generation(),
            phase: state.lifecycle.phase(),
            pending_threads: state.lifecycle.pending_threads(),
            active_threads: state.lifecycle.active_threads(),
            terminal: state.lifecycle.terminal(),
        })
    }

    pub(crate) fn start(&self) -> Result<(), ProcessError> {
        self.inner.state.with(|state| state.lifecycle.start())?;
        Ok(())
    }

    pub(crate) fn join(&self) -> Result<TerminalReason, crate::kernel::sync::Error> {
        self.inner.stopped.wait()?;
        match self.snapshot().terminal {
            Some(reason) => Ok(reason),
            None => process_invariant_violation(),
        }
    }

    pub(crate) fn try_join(&self) -> Option<TerminalReason> {
        if !self.inner.stopped.try_wait() {
            return None;
        }
        match self.snapshot().terminal {
            Some(reason) => Some(reason),
            None => process_invariant_violation(),
        }
    }

    pub(super) fn signal_source(&self) -> SignalSource<'_> {
        SignalSource::new(&self.inner.signals, Self::SUPPORTED_SIGNALS)
    }

    pub(crate) fn create_initial_user_thread(
        &self,
        name: &str,
        affinity: CpuMask,
    ) -> Result<UserThread, ProcessError> {
        self.create_user_thread(name, self.image().initial_thread(), affinity)
    }

    pub(crate) fn create_user_thread(
        &self,
        name: &str,
        start: UserThreadStart,
        affinity: CpuMask,
    ) -> Result<UserThread, ProcessError> {
        if !machine_matches_host(self.image().machine())
            || self.image().family() != AbiFamily::Native
            || self.image().route() != ExecutionRoute::NativeKernel
        {
            return Err(ProcessError::UserEntry(
                crate::hal::user::UserEntryError::Unsupported,
            ));
        }
        let prepared = self.prepare_user_thread()?;
        let context = crate::hal::user::prepare_context(
            start.entry().get(),
            start.stack().get(),
            start.tls().get(),
        )
        .map_err(ProcessError::UserEntry)?;
        let execution = UserExecution::try_new(prepared.address_space.clone(), context)
            .map_err(|()| ProcessError::Allocation)?;
        let dormant = scheduler::prepare_user_thread(
            name,
            prepared.thread.clone(),
            execution,
            crate::kernel::entry::user::thread_entry,
            affinity,
        )?;
        let id = dormant.id();
        let thread = prepared.thread.clone();
        // Arming scheduler ownership before Process publication is safe because
        // the dormant ID has not escaped and cannot be made runnable. Every
        // subsequent operation is an infallible publication step.
        let terminal = prepared.publish(id, dormant);
        if let Some(reason) = terminal {
            let _ = scheduler::request_user_thread_stop(id, reason);
        }
        Ok(thread)
    }

    fn prepare_user_thread(&self) -> Result<PreparedUserThread, ProcessError> {
        self.inner
            .state
            .with(|state| state.lifecycle.reserve_thread())?;
        let prepared = self.prepare_user_thread_after_admission();
        if prepared.is_err() {
            self.abort_pending_thread();
        }
        prepared
    }

    fn abort_pending_thread(&self) {
        let became_stopped = match self
            .inner
            .state
            .with(|state| state.lifecycle.abort_thread())
        {
            Ok(became_stopped) => became_stopped,
            Err(_) => process_invariant_violation(),
        };
        if became_stopped {
            self.publish_stopped();
        }
    }

    fn prepare_user_thread_after_admission(&self) -> Result<PreparedUserThread, ProcessError> {
        let record_charge = self
            .inner
            .domain
            .reserve(metadata_amount::<ThreadRecord>()?)?
            .commit();
        let record = FallibleArc::try_new(ThreadRecord {
            active: AtomicBool::new(false),
            scheduler_id: AtomicU64::new(0),
            next: ProcessLock::new(None),
            _metadata_charge: record_charge,
        })
        .map_err(|_| ProcessError::Allocation)?;
        let thread_metadata_bytes = UserThread::allocation_size()
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let thread_metadata = self
            .inner
            .domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, thread_metadata_bytes),
            )?
            .commit();
        let thread = UserThread::try_prepared(self, thread_metadata)?;
        let stack_bytes = u64::try_from(crate::kernel::mm::stack::thread_stack_bytes())
            .map_err(|_| ProcessError::Allocation)?;
        let thread_object_bytes =
            u64::try_from(crate::kernel::task::thread::Thread::allocation_size())
                .map_err(|_| ProcessError::Allocation)?;
        let user_execution_bytes = u64::try_from(UserExecution::allocation_size())
            .map_err(|_| ProcessError::Allocation)?;
        let execution_bytes = stack_bytes
            .checked_add(thread_object_bytes)
            .and_then(|bytes| bytes.checked_add(user_execution_bytes))
            .ok_or(ProcessError::Allocation)?;
        let stack_pages = stack_bytes / hyper::mm::PAGE_SIZE;
        let execution_charge = self
            .inner
            .domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::Threads, 1)
                    .with(ResourceKind::KernelMemoryBytes, execution_bytes)
                    .with(ResourceKind::CommittedPages, stack_pages),
            )?
            .commit();
        let address_space = self.inner.state.with(|state| {
            state
                .address_space
                .as_ref()
                .cloned()
                .ok_or(ProcessError::AddressSpaceReferenced)
        })?;
        Ok(PreparedUserThread {
            process: self.clone(),
            thread,
            record: Some(record),
            execution_charge: Some(execution_charge),
            address_space,
            committed: false,
        })
    }

    pub(crate) fn request_stop(&self, reason: TerminalReason) -> ProcessStopReport {
        let (newly_requested, completed, effective_reason, pending_threads) =
            self.inner.state.with(|state| {
                let before = state.lifecycle.phase();
                let newly_requested = state.lifecycle.request_stop(reason);
                let effective_reason = match state.lifecycle.terminal() {
                    Some(reason) => reason,
                    None => process_invariant_violation(),
                };
                (
                    newly_requested,
                    before != ProcessPhase::Stopped
                        && state.lifecycle.phase() == ProcessPhase::Stopped,
                    effective_reason,
                    state.lifecycle.pending_threads(),
                )
            });
        if completed {
            self.publish_stopped();
        }
        let mut current = self.inner.state.with(|state| state.threads.clone());
        let mut dispatched_threads = 0usize;
        let mut dispatch = StopDispatchProgress::new(pending_threads);
        while let Some(record) = current {
            current = record.next.with(|next| next.clone());
            if !record.active.load(Ordering::Acquire) {
                continue;
            }
            let raw = record.scheduler_id.load(Ordering::Relaxed);
            if raw == 0 {
                dispatch.observe(false);
                continue;
            }
            dispatched_threads = dispatched_threads.saturating_add(1);
            let id = ThreadId::from_process_publication(raw);
            dispatch.observe(scheduler::request_user_thread_stop(id, effective_reason).is_ok());
        }
        ProcessStopReport {
            newly_requested,
            dispatched_threads,
            dispatch_complete: dispatch.is_complete(),
        }
    }

    pub(crate) fn reserve_handles<const N: usize>(
        &self,
    ) -> Result<ProcessHandleReservation<N>, ProcessError> {
        let reservation = loop {
            let snapshot = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                Ok::<_, ProcessError>(
                    self.inner
                        .handles
                        .with(|table| table.reservation_storage_snapshot_for(N))?,
                )
            })?;
            let (mut storage_plan, mut storage_charge, mut index_plan) =
                self.prepare_table_storage_plan(snapshot)?;
            let attempt = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                let current = self
                    .inner
                    .handles
                    .with(|table| table.reservation_storage_snapshot_for(N))?;
                if current != snapshot {
                    return Ok::<_, ProcessError>(None);
                }
                let reservation = self
                    .inner
                    .handles
                    .with(|table| table.reserve_with_plan(&mut storage_plan))?;
                install_table_storage_charge(state, snapshot, &mut storage_charge);
                state
                    .charge_index
                    .install(core::mem::replace(&mut index_plan, HandleSidecar::new()));
                Ok(Some(reservation))
            });
            match attempt {
                Ok(Some(reservation)) => break reservation,
                Ok(None) => drop((storage_plan, storage_charge)),
                Err(error) => return Err(error),
            }
        };
        let values = reservation.values();
        let entries_bytes = N
            .checked_mul(core::mem::size_of::<HandleChargeEntry>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let metadata_base = metadata_amount::<HandleChargeRecord>()?;
        let metadata_request = metadata_base.with(
            ResourceKind::KernelMemoryBytes,
            metadata_base
                .get(ResourceKind::KernelMemoryBytes)
                .checked_add(entries_bytes)
                .ok_or(ProcessError::Allocation)?,
        );
        let metadata_charge = match self.inner.domain.reserve(metadata_request) {
            Ok(charge) => charge.commit(),
            Err(error) => {
                self.abort_raw_handle_reservation(reservation);
                return Err(error.into());
            }
        };
        let mut handle_charges = alloc::vec::Vec::new();
        if handle_charges.try_reserve_exact(N).is_err() {
            self.abort_raw_handle_reservation(reservation);
            return Err(ProcessError::Allocation);
        }
        for _ in 0..N {
            let charge = match self
                .inner
                .domain
                .reserve(ResourceAmount::ZERO.with(ResourceKind::Handles, 1))
            {
                Ok(charge) => charge,
                Err(error) => {
                    self.abort_raw_handle_reservation(reservation);
                    return Err(error.into());
                }
            };
            handle_charges.push(charge);
        }
        let mut entries = alloc::vec::Vec::new();
        if entries.try_reserve_exact(N).is_err() {
            self.abort_raw_handle_reservation(reservation);
            return Err(ProcessError::Allocation);
        }
        for value in values {
            entries.push(HandleChargeEntry {
                value,
                charge: None,
            });
        }
        let record = match FallibleArc::try_new(HandleChargeRecord {
            previous: ProcessLock::new(None),
            state: ProcessLock::new(HandleChargeState { entries }),
            next: ProcessLock::new(None),
            _metadata_charge: metadata_charge,
        }) {
            Ok(record) => record,
            Err(_) => {
                self.abort_raw_handle_reservation(reservation);
                return Err(ProcessError::Allocation);
            }
        };
        Ok(ProcessHandleReservation {
            reservation: Some(reservation),
            handle_charges: Some(handle_charges),
            record: Some(record),
        })
    }

    pub(crate) fn reserve_handle_batch(
        &self,
        count: usize,
    ) -> Result<ProcessHandleBatchReservation, ProcessError> {
        HandleBatchReservationStorage::validate_count(count)?;
        let entries_bytes = count
            .checked_mul(core::mem::size_of::<HandleChargeEntry>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let metadata_base = metadata_amount::<HandleChargeRecord>()?;
        let metadata_request = metadata_base.with(
            ResourceKind::KernelMemoryBytes,
            metadata_base
                .get(ResourceKind::KernelMemoryBytes)
                .checked_add(entries_bytes)
                .ok_or(ProcessError::Allocation)?,
        );
        let charge_scratch_bytes = count
            .checked_mul(core::mem::size_of::<ChargeReservation>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let reservation_scratch_bytes = HandleBatchReservationStorage::allocation_size(count)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let scratch_bytes = charge_scratch_bytes
            .checked_add(reservation_scratch_bytes)
            .ok_or(ProcessError::Allocation)?;
        let scratch_request =
            ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, scratch_bytes);
        let scratch_charge = self.inner.domain.reserve(scratch_request)?.commit();
        let mut reservation_storage = Some(HandleBatchReservationStorage::try_new(count)?);
        let reservation = loop {
            let snapshot = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                Ok::<_, ProcessError>(
                    self.inner
                        .handles
                        .with(|table| table.reservation_storage_snapshot_for(count))?,
                )
            })?;
            let (mut storage_plan, mut storage_charge, mut index_plan) =
                self.prepare_table_storage_plan(snapshot)?;
            let attempt = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                let current = self
                    .inner
                    .handles
                    .with(|table| table.reservation_storage_snapshot_for(count))?;
                if current != snapshot {
                    return Ok::<_, ProcessError>(None);
                }
                let reservation = self.inner.handles.with(|table| {
                    table.reserve_batch_with_plan(
                        count,
                        &mut reservation_storage,
                        &mut storage_plan,
                    )
                })?;
                install_table_storage_charge(state, snapshot, &mut storage_charge);
                state
                    .charge_index
                    .install(core::mem::replace(&mut index_plan, HandleSidecar::new()));
                Ok(Some(reservation))
            });
            match attempt {
                Ok(Some(reservation)) => break reservation,
                Ok(None) => drop((storage_plan, storage_charge)),
                Err(error) => return Err(error),
            }
        };
        let metadata = self.inner.domain.reserve(metadata_request);
        let metadata = match metadata {
            Ok(charge) => charge.commit(),
            Err(error) => {
                self.abort_raw_handle_batch_reservation(reservation);
                return Err(error.into());
            }
        };
        let mut charges = alloc::vec::Vec::new();
        let mut entries = alloc::vec::Vec::new();
        if charges.try_reserve_exact(count).is_err() || entries.try_reserve_exact(count).is_err() {
            self.abort_raw_handle_batch_reservation(reservation);
            return Err(ProcessError::Allocation);
        }
        let mut charge_error = None;
        for value in reservation.values() {
            let charge = match self
                .inner
                .domain
                .reserve(ResourceAmount::ZERO.with(ResourceKind::Handles, 1))
            {
                Ok(charge) => charge,
                Err(error) => {
                    charge_error = Some(error);
                    break;
                }
            };
            charges.push(charge);
            entries.push(HandleChargeEntry {
                value: *value,
                charge: None,
            });
        }
        if let Some(error) = charge_error {
            self.abort_raw_handle_batch_reservation(reservation);
            return Err(error.into());
        }
        let record = match FallibleArc::try_new(HandleChargeRecord {
            previous: ProcessLock::new(None),
            state: ProcessLock::new(HandleChargeState { entries }),
            next: ProcessLock::new(None),
            _metadata_charge: metadata,
        }) {
            Ok(record) => record,
            Err(_) => {
                self.abort_raw_handle_batch_reservation(reservation);
                return Err(ProcessError::Allocation);
            }
        };
        Ok(ProcessHandleBatchReservation {
            reservation: Some(reservation),
            handle_charges: Some(charges),
            record: Some(record),
            scratch_charge: Some(scratch_charge),
        })
    }

    fn prepare_table_storage_plan(
        &self,
        snapshot: HandleTableStorageSnapshot,
    ) -> Result<PreparedTableStorage, ProcessError> {
        let storage_bytes = snapshot
            .growth_bytes()
            .and_then(|bytes| {
                bytes.checked_add(HandleSidecar::<HandleChargeLocation>::growth_bytes(
                    snapshot,
                )?)
            })
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let charge = if storage_bytes == 0 {
            None
        } else {
            Some(
                self.inner
                    .domain
                    .reserve(
                        ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, storage_bytes),
                    )?
                    .commit(),
            )
        };
        let plan = HandleTableStoragePlan::try_new(snapshot)?;
        Ok((Some(plan), charge, HandleSidecar::prepare(snapshot)?))
    }

    pub(crate) fn publish_handles<const N: usize>(
        &self,
        mut reservation: ProcessHandleReservation<N>,
        handles: [PreparedHandle; N],
    ) -> Result<[HandleValue; N], HandlePublishFailure<N>> {
        let mut handles = Some(handles);
        let mut retired_charge_storage = None;
        let result = self.inner.state.with(|state| {
            if require_handle_phase(state.lifecycle.phase()).is_err() {
                let token = match reservation.reservation.take() {
                    Some(token) => token,
                    None => process_invariant_violation(),
                };
                self.inner.handles.with(|table| token.abort(table));
                return Err(ProcessError::Lifecycle(LifecycleError::AdmissionClosed));
            }
            let token = match reservation.reservation.take() {
                Some(token) => token,
                None => process_invariant_violation(),
            };
            let prepared = match handles.take() {
                Some(handles) => handles,
                None => process_invariant_violation(),
            };
            let values = self
                .inner
                .handles
                .with(|table| token.publish(table, prepared));
            let mut charges = match reservation.handle_charges.take() {
                Some(charges) => charges,
                None => process_invariant_violation(),
            };
            let record = match reservation.record.take() {
                Some(record) => record,
                None => process_invariant_violation(),
            };
            record.state.with(|state| {
                if state.entries.len() != charges.len() {
                    process_invariant_violation();
                }
                for entry in state.entries.iter_mut().rev() {
                    let charge = match charges.pop() {
                        Some(charge) => charge,
                        None => process_invariant_violation(),
                    };
                    entry.charge = Some(charge.commit());
                }
                if !charges.is_empty() {
                    process_invariant_violation();
                }
            });
            install_handle_charge_record(state, record);
            retired_charge_storage = Some(charges);
            Ok(values)
        });
        drop(retired_charge_storage.take());
        match result {
            Ok(values) => Ok(values),
            Err(error) => {
                drop(reservation.handle_charges.take());
                drop(reservation.record.take());
                Err(HandlePublishFailure {
                    error,
                    handles: match handles.take() {
                        Some(handles) => handles,
                        None => process_invariant_violation(),
                    },
                })
            }
        }
    }

    // Boxing the error would add a fallible allocation precisely on rollback;
    // the large variant is the linear owner required to preserve authority.
    #[allow(clippy::result_large_err)]
    pub(crate) fn publish_handle_batch(
        &self,
        mut reservation: ProcessHandleBatchReservation,
        handles: InTransitCapabilities,
    ) -> Result<(), HandleBatchPublishFailure> {
        let (handles, storage_charge) = handles.into_prepared_handles();
        let mut handles = Some(handles);
        let mut storage_charge = Some(storage_charge);
        let mut retired_reservation_storage = None;
        let mut retired_charge_storage = None;
        let result = self.inner.state.with(|state| {
            if require_handle_phase(state.lifecycle.phase()).is_err() {
                let token = match reservation.reservation.take() {
                    Some(token) => token,
                    None => process_invariant_violation(),
                };
                retired_reservation_storage =
                    Some(self.inner.handles.with(|table| token.abort(table)));
                return Err(ProcessError::Lifecycle(LifecycleError::AdmissionClosed));
            }
            let token = match reservation.reservation.take() {
                Some(token) => token,
                None => process_invariant_violation(),
            };
            let prepared = match handles.take() {
                Some(handles) => handles,
                None => process_invariant_violation(),
            };
            retired_reservation_storage = Some(
                self.inner
                    .handles
                    .with(|table| token.publish(table, prepared)),
            );
            let mut charges = match reservation.handle_charges.take() {
                Some(charges) => charges,
                None => process_invariant_violation(),
            };
            let record = match reservation.record.take() {
                Some(record) => record,
                None => process_invariant_violation(),
            };
            record.state.with(|record_state| {
                if record_state.entries.len() != charges.len() {
                    process_invariant_violation();
                }
                for entry in record_state.entries.iter_mut().rev() {
                    let charge = match charges.pop() {
                        Some(charge) => charge,
                        None => process_invariant_violation(),
                    };
                    entry.charge = Some(charge.commit());
                }
            });
            install_handle_charge_record(state, record);
            retired_charge_storage = Some(charges);
            Ok(())
        });
        drop(retired_reservation_storage.take());
        drop(retired_charge_storage.take());
        drop(reservation.scratch_charge.take());
        match result {
            Ok(()) => {
                drop(storage_charge.take());
                Ok(())
            }
            Err(error) => {
                drop(reservation.handle_charges.take());
                drop(reservation.record.take());
                let prepared = match handles.take() {
                    Some(handles) => handles,
                    None => process_invariant_violation(),
                };
                Err(HandleBatchPublishFailure {
                    error,
                    handles: InTransitCapabilities::from_prepared_handles(
                        prepared,
                        match storage_charge.take() {
                            Some(charge) => charge,
                            None => process_invariant_violation(),
                        },
                    ),
                })
            }
        }
    }

    pub(crate) fn abort_handle_batch(&self, mut reservation: ProcessHandleBatchReservation) {
        let token = match reservation.reservation.take() {
            Some(token) => token,
            None => process_invariant_violation(),
        };
        let retired = self
            .inner
            .state
            .with(|_| self.inner.handles.with(|table| token.abort(table)));
        drop(retired);
        drop(reservation.handle_charges.take());
        drop(reservation.record.take());
        drop(reservation.scratch_charge.take());
    }

    pub(crate) fn abort_handles<const N: usize>(
        &self,
        mut reservation: ProcessHandleReservation<N>,
    ) {
        let token = match reservation.reservation.take() {
            Some(token) => token,
            None => process_invariant_violation(),
        };
        self.abort_raw_handle_reservation(token);
        drop(reservation.handle_charges.take());
        drop(reservation.record.take());
    }

    fn abort_raw_handle_reservation<const N: usize>(&self, reservation: HandleReservation<N>) {
        self.inner.state.with(|_| {
            self.inner.handles.with(|table| reservation.abort(table));
        });
    }

    fn abort_raw_handle_batch_reservation(&self, reservation: HandleBatchReservation) {
        let retired = self
            .inner
            .state
            .with(|_| self.inner.handles.with(|table| reservation.abort(table)));
        drop(retired);
    }

    pub(crate) fn resolve_handle<T: KernelObject>(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<ResolvedObject<T>, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok(self
                .inner
                .handles
                .with(|table| table.resolve(value, rights))?)
        })
    }

    pub(crate) fn resolve_waitable(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<ResolvedWaitable, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok(self
                .inner
                .handles
                .with(|table| table.resolve_waitable(value, rights))?)
        })
    }

    /// Claims source handles without changing their process-local values.
    ///
    /// The returned transaction owns every active capability while exact
    /// lookups report `Busy`. It can either restore the same values or perform
    /// one final generation-advancing move into an in-transit batch.
    pub(crate) fn prepare_handle_transfer(
        &self,
        requests: &[HandleTransferRequest],
        forbidden_object: Option<Koid>,
        forbidden_kind: Option<crate::kernel::object::ObjectKind>,
    ) -> Result<PreparedProcessHandleTransfer, ProcessError> {
        HandleTransferStorage::validate_count(requests.len())?;
        let entry_bytes = HandleTransferClaim::entry_allocation_size(requests.len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let handle_bytes = HandleTransferClaim::handle_allocation_size(requests.len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let scratch_bytes = requests
            .len()
            .checked_mul(
                core::mem::size_of::<CommittedCharge>()
                    .saturating_add(core::mem::size_of::<FallibleArc<HandleChargeRecord>>()),
            )
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let entry_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, entry_bytes))?
            .commit();
        let handle_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, handle_bytes))?
            .commit();
        let scratch_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, scratch_bytes))?
            .commit();
        let mut storage = Some(HandleTransferStorage::try_new(requests.len())?);
        let mut released_charges = alloc::vec::Vec::new();
        released_charges
            .try_reserve_exact(requests.len())
            .map_err(|_| ProcessError::Allocation)?;
        let mut retired_records = alloc::vec::Vec::new();
        retired_records
            .try_reserve_exact(requests.len())
            .map_err(|_| ProcessError::Allocation)?;
        let claim = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok::<_, ProcessError>(self.inner.handles.with(|table| {
                table.prepare_transfer_with_storage(
                    requests,
                    forbidden_object,
                    forbidden_kind,
                    &mut storage,
                )
            })?)
        })?;
        Ok(PreparedProcessHandleTransfer {
            process: self.clone(),
            claim: Some(claim),
            entry_charge: Some(entry_charge),
            handle_charge: Some(handle_charge),
            scratch_charge: Some(scratch_charge),
            released_charges,
            retired_records,
        })
    }

    /// Publishes the first process-local handle for a new kernel object.
    ///
    /// Slot, quota, object identity, and active-handle state are prepared
    /// before the Process lock commits publication. Every failure before that
    /// point rolls back both the slot reservation and unpublished authority.
    pub(crate) fn create_object<T: UserExportableObject>(
        &self,
        payload: T,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        let reservation = self.reserve_handles::<1>()?;
        let object = match ObjectPublication::try_new(payload) {
            Ok(object) => object,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        let prepared = match PreparedHandle::try_from_new_object(object, rights, HandleFlags::NONE)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        match self.publish_handles(reservation, [prepared]) {
            Ok(values) => Ok(values[0]),
            Err(failure) => Err(failure.error),
        }
    }

    /// Publishes this Process's first userspace authority to an existing thread.
    ///
    /// The thread already has a canonical object identity. This transaction
    /// mints its one initial handle without constructing a second wrapper or
    /// allowing authority resurrection after the final handle closes.
    pub(crate) fn publish_thread_handle(
        &self,
        thread: &UserThread,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        if thread.process_id() != self.id() {
            return Err(ProcessError::Handle(HandleError::AccessDenied));
        }
        let reservation = self.reserve_handles::<1>()?;
        let prepared = match PreparedHandle::try_from_new_object(
            thread.publication(),
            rights,
            HandleFlags::NONE,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        match self.publish_handles(reservation, [prepared]) {
            Ok(values) => Ok(values[0]),
            Err(failure) => Err(failure.error),
        }
    }

    /// Resolves a thread handle into the same canonical kernel object owner.
    pub(crate) fn resolve_user_thread_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<UserThread, ProcessError> {
        let resolved = self.resolve_handle::<UserThreadObject>(value, rights)?;
        Ok(UserThread::from_operation_pin(
            resolved.into_operation_pin(),
        ))
    }

    /// Publishes a same-kind object pair in one handle-table transaction.
    ///
    /// Both erased objects and both active owners exist before publication, so
    /// userspace can never observe only one endpoint of a newly created pair.
    pub(crate) fn create_object_pair<T: UserExportableObject>(
        &self,
        first: T,
        second: T,
        rights: Rights,
    ) -> Result<[HandleValue; 2], ProcessError> {
        let reservation = self.reserve_handles::<2>()?;
        let first = match ObjectPublication::try_new(first) {
            Ok(object) => object,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        let second = match ObjectPublication::try_new(second) {
            Ok(object) => object,
            Err(error) => {
                self.abort_handles(reservation);
                drop(first);
                return Err(error.into());
            }
        };
        let first = match PreparedHandle::try_from_new_object(first, rights, HandleFlags::NONE) {
            Ok(handle) => handle,
            Err(error) => {
                self.abort_handles(reservation);
                drop(second);
                return Err(error.into());
            }
        };
        let second = match PreparedHandle::try_from_new_object(second, rights, HandleFlags::NONE) {
            Ok(handle) => handle,
            Err(error) => {
                self.abort_handles(reservation);
                drop(first);
                return Err(error.into());
            }
        };
        match self.publish_handles(reservation, [first, second]) {
            Ok(values) => Ok(values),
            Err(failure) => Err(failure.error),
        }
    }

    pub(crate) fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            let info = self.inner.handles.with(|table| table.get_info(value))?;
            if !info.rights.contains(required_rights) {
                return Err(ProcessError::Handle(HandleError::AccessDenied));
            }
            Ok(info)
        })
    }

    /// Returns one bounded, authority-free page of this Process handle graph.
    pub(crate) fn scan_handles(
        &self,
        cursor: HandleScanCursor,
    ) -> Result<HandleSnapshotPage, ProcessError> {
        self.inner
            .handles
            .with(|table| table.scan_handles(cursor))
            .map_err(Into::into)
    }

    pub(crate) fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        let reservation = self.reserve_handles::<1>()?;
        let prepared = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok::<_, ProcessError>(
                self.inner
                    .handles
                    .with(|table| table.duplicate(value, rights))?,
            )
        });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error);
            }
        };
        match self.publish_handles(reservation, [prepared]) {
            Ok(values) => Ok(values[0]),
            Err(failure) => Err(failure.error),
        }
    }

    pub(crate) fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        loop {
            let snapshot = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                Ok::<_, ProcessError>(
                    self.inner
                        .handles
                        .with(|table| table.replace_storage_snapshot(value, rights))?,
                )
            })?;
            let (mut storage_plan, mut storage_charge, mut index_plan) = match snapshot {
                Some(snapshot) => self.prepare_table_storage_plan(snapshot)?,
                None => (None, None, HandleSidecar::new()),
            };
            let attempt = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                let current = self
                    .inner
                    .handles
                    .with(|table| table.replace_storage_snapshot(value, rights))?;
                if current != snapshot {
                    return Ok::<_, ProcessError>(None);
                }
                let replacement = self
                    .inner
                    .handles
                    .with(|table| table.replace_with_plan(value, rights, &mut storage_plan))?;
                if let Some(snapshot) = snapshot {
                    install_table_storage_charge(state, snapshot, &mut storage_charge);
                    state
                        .charge_index
                        .install(core::mem::replace(&mut index_plan, HandleSidecar::new()));
                }
                replace_handle_charge_value(state, value, replacement);
                Ok(Some(replacement))
            });
            match attempt {
                Ok(Some(replacement)) => return Ok(replacement),
                Ok(None) => drop((storage_plan, storage_charge)),
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn copy_to_user(
        &self,
        destination: UserSlice,
        source: &[u8],
    ) -> Result<(), ProcessError> {
        let address_space = self.address_space_owner()?;
        address_space.copy_to_user(destination, source)?;
        Ok(())
    }

    pub(crate) fn copy_from_user(
        &self,
        source: UserSlice,
        destination: &mut [u8],
    ) -> Result<(), ProcessError> {
        let address_space = self.address_space_owner()?;
        address_space.copy_from_user(source, destination)?;
        Ok(())
    }

    /// Pins one exact output range across a capability publication transaction.
    pub(crate) fn reserve_user_write(
        &self,
        destination: UserSlice,
    ) -> Result<UserWriteReservation, ProcessError> {
        let address_space = self.address_space_owner()?;
        Ok(NativeAddressSpace::reserve_user_write(
            address_space,
            destination,
        )?)
    }

    pub(crate) fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        let (closed, charge, retired_record): (
            ClosedHandle,
            CommittedCharge,
            Option<FallibleArc<HandleChargeRecord>>,
        ) = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            let closed = self.inner.handles.with(|table| table.remove(value))?;
            let (charge, retired_record) = release_handle_charge(state, value);
            Ok::<_, ProcessError>((closed, charge, retired_record))
        })?;
        closed.complete();
        drop(charge);
        drop(retired_record);
        Ok(())
    }

    fn retire(&self) -> Result<ProcessRetirementStep, ProcessError> {
        let phase = self.inner.state.with(|state| state.lifecycle.phase());
        if phase == ProcessPhase::Stopped {
            let mut cursor = self.inner.state.with(|state| {
                let cursor = self.inner.handles.with(HandleTable::begin_teardown)?;
                state.lifecycle.begin_retirement()?;
                Ok::<_, ProcessError>(cursor)
            })?;
            loop {
                let closed = self
                    .inner
                    .handles
                    .with(|table| table.remove_next(&mut cursor));
                let Some(closed) = closed else {
                    break;
                };
                closed.complete();
            }
            self.inner
                .handles
                .with(|table| table.finish_teardown(cursor));
            self.inner.state.with(|state| state.handles_retired = true);
        } else if phase != ProcessPhase::Retiring {
            return Err(ProcessError::Lifecycle(LifecycleError::NotStopped));
        } else if !self.inner.state.with(|state| state.handles_retired) {
            return Ok(ProcessRetirementStep::InProgress);
        }

        let address_space = self.inner.state.with(|state| {
            if !state.handles_retired {
                process_invariant_violation();
            }
            state.address_space.take()
        });
        let Some(address_space) = address_space else {
            return Ok(ProcessRetirementStep::InProgress);
        };
        let address_space = match address_space.try_into_unique() {
            Ok(address_space) => address_space,
            Err(address_space) => {
                self.inner
                    .state
                    .with(|state| state.address_space = Some(address_space));
                return Ok(ProcessRetirementStep::PendingReferences);
            }
        };
        match NativeAddressSpace::retire(address_space) {
            Ok(()) => {
                self.finish_retirement();
                Ok(ProcessRetirementStep::Complete)
            }
            Err(failure) => {
                let (_, address_space) = failure.into_parts();
                Ok(ProcessRetirementStep::Retry(AddressSpaceRetirement {
                    process: self.clone(),
                    address_space: Some(address_space),
                }))
            }
        }
    }

    pub(super) fn with_run_admission<R>(
        &self,
        expected_image_generation: u64,
        operation: impl FnOnce() -> R,
    ) -> Result<R, super::user_thread::RunAdmissionError> {
        self.inner.state.with(|state| {
            if state.lifecycle.phase() != ProcessPhase::Running {
                return Err(super::user_thread::RunAdmissionError::AdmissionClosed);
            }
            if expected_image_generation != self.image_generation() {
                return Err(super::user_thread::RunAdmissionError::StaleImage);
            }
            Ok(operation())
        })
    }

    fn finish_retirement(&self) {
        let retired_table_storage = self.inner.handles.with(HandleTable::take_retired_storage);
        let (membership, process_charge, mut records, mut handle_charges, table_charges) =
            self.inner.state.with(|state| {
                if state.lifecycle.phase() != ProcessPhase::Retiring {
                    process_invariant_violation();
                }
                (
                    state.group_membership.take(),
                    state.process_charge.take(),
                    state.threads.take(),
                    state.handle_charges.take(),
                    core::mem::replace(
                        &mut state.handle_table_charges,
                        [const { None }; HANDLE_TABLE_STORAGE_SEGMENTS],
                    ),
                )
            });
        while let Some(record) = records {
            records = record.next.with(Option::take);
            drop(record);
        }
        let index = self
            .inner
            .state
            .with(|state| core::mem::replace(&mut state.charge_index, HandleSidecar::new()));
        drop(index);
        while let Some(record) = handle_charges {
            handle_charges = record.next.with(Option::take);
            let charges = record.state.with(|state| {
                let mut charges = alloc::vec::Vec::new();
                core::mem::swap(&mut charges, &mut state.entries);
                charges
            });
            drop(charges);
            drop(record);
        }
        drop(retired_table_storage);
        drop(table_charges);
        let membership = match membership {
            Some(membership) => membership,
            None => process_invariant_violation(),
        };
        membership.retire();
        drop(process_charge);
        // Retired is an observation of completed cleanup, not its start.
        self.inner.state.with(|state| {
            if state.lifecycle.finish_retirement().is_err() {
                process_invariant_violation();
            }
        });
    }

    /// Publishes object-visible termination before waking Process joiners.
    fn publish_stopped(&self) {
        if self
            .inner
            .signals
            .update(SignalMask::EMPTY, Self::TERMINATED)
            .is_err()
            || self.inner.stopped.complete_all().is_err()
        {
            process_invariant_violation();
        }
        queue_retirement(self.clone());
        crate::kernel::reaper::request();
    }
}

impl Clone for Process {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub(crate) struct HandlePublishFailure<const N: usize> {
    pub(crate) error: ProcessError,
    pub(crate) handles: [PreparedHandle; N],
}

/// Process-owned reversible source-handle transaction.
#[must_use = "commit or roll back the process handle transfer"]
pub(crate) struct PreparedProcessHandleTransfer {
    process: Process,
    claim: Option<HandleTransferClaim>,
    entry_charge: Option<CommittedCharge>,
    handle_charge: Option<CommittedCharge>,
    scratch_charge: Option<CommittedCharge>,
    released_charges: alloc::vec::Vec<CommittedCharge>,
    retired_records: alloc::vec::Vec<FallibleArc<HandleChargeRecord>>,
}

impl PreparedProcessHandleTransfer {
    /// Restores every claimed source at its original numeric value.
    pub(crate) fn rollback(mut self) {
        let claim = match self.claim.take() {
            Some(claim) => claim,
            None => process_invariant_violation(),
        };
        let retired = self.process.inner.state.with(|_| {
            self.process
                .inner
                .handles
                .with(|table| claim.rollback_with_storage(table))
        });
        drop(retired);
        drop(self.entry_charge.take());
        drop(self.handle_charge.take());
        drop(self.scratch_charge.take());
    }

    /// Permanently consumes all source values and returns their active owners.
    ///
    /// Admission is the only recoverable check. Once it succeeds, accounting
    /// extraction and generation advancement are infallible and serialized by
    /// the Process lock. Released accounting owners are dropped afterward.
    // The recoverable error must retain this complete linear transaction.
    // Boxing it would make rollback depend on a new allocation.
    #[allow(clippy::result_large_err)]
    pub(crate) fn commit(mut self) -> Result<InTransitCapabilities, HandleTransferCommitFailure> {
        let result = self.process.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            let claim = match self.claim.as_ref() {
                Some(claim) => claim,
                None => process_invariant_violation(),
            };
            for value in claim.values() {
                if !handle_charge_is_live(state, value) {
                    process_invariant_violation();
                }
            }
            for value in claim.values() {
                let (charge, retired_record) = release_handle_charge(state, value);
                self.released_charges.push(charge);
                if let Some(record) = retired_record {
                    self.retired_records.push(record);
                }
            }
            let claim = match self.claim.take() {
                Some(claim) => claim,
                None => process_invariant_violation(),
            };
            Ok(self
                .process
                .inner
                .handles
                .with(|table| claim.commit_with_storage(table)))
        });
        let (handles, retired_transfer_storage) = match result {
            Ok(handles) => handles,
            Err(error) => {
                return Err(HandleTransferCommitFailure {
                    error,
                    transfer: self,
                });
            }
        };
        drop(retired_transfer_storage);
        drop(core::mem::take(&mut self.released_charges));
        drop(core::mem::take(&mut self.retired_records));
        drop(self.entry_charge.take());
        drop(self.scratch_charge.take());
        let storage_charge = match self.handle_charge.take() {
            Some(charge) => charge,
            None => process_invariant_violation(),
        };
        Ok(InTransitCapabilities::new(handles, storage_charge))
    }
}

impl Drop for PreparedProcessHandleTransfer {
    fn drop(&mut self) {
        if self.claim.is_some()
            || self.entry_charge.is_some()
            || self.handle_charge.is_some()
            || self.scratch_charge.is_some()
            || !self.released_charges.is_empty()
            || !self.retired_records.is_empty()
        {
            process_invariant_violation();
        }
    }
}

/// Recoverable final-commit failure retaining the exact rollback owner.
#[must_use = "inspect the error and roll back the retained transfer"]
pub(crate) struct HandleTransferCommitFailure {
    pub(crate) error: ProcessError,
    pub(crate) transfer: PreparedProcessHandleTransfer,
}

#[must_use = "publish or abort the process handle reservation"]
pub(crate) struct ProcessHandleReservation<const N: usize> {
    reservation: Option<HandleReservation<N>>,
    handle_charges: Option<alloc::vec::Vec<ChargeReservation>>,
    record: Option<FallibleArc<HandleChargeRecord>>,
}

#[must_use = "publish or abort the process handle batch reservation"]
pub(crate) struct ProcessHandleBatchReservation {
    reservation: Option<HandleBatchReservation>,
    handle_charges: Option<alloc::vec::Vec<ChargeReservation>>,
    record: Option<FallibleArc<HandleChargeRecord>>,
    scratch_charge: Option<CommittedCharge>,
}

impl<const N: usize> ProcessHandleReservation<N> {
    /// Future generation-tagged values which resolve only after publication.
    pub(crate) fn values(&self) -> [HandleValue; N] {
        match self.reservation.as_ref() {
            Some(reservation) => reservation.values(),
            None => process_invariant_violation(),
        }
    }
}

impl ProcessHandleBatchReservation {
    /// Future numeric values which remain unresolved until batch publication.
    pub(crate) fn values(&self) -> &[HandleValue] {
        match self.reservation.as_ref() {
            Some(reservation) => reservation.values(),
            None => process_invariant_violation(),
        }
    }
}

impl Drop for ProcessHandleBatchReservation {
    fn drop(&mut self) {
        if self.reservation.is_some()
            || self.handle_charges.is_some()
            || self.record.is_some()
            || self.scratch_charge.is_some()
        {
            process_invariant_violation();
        }
    }
}

#[must_use = "recover the in-transit handles from the failed publication"]
pub(crate) struct HandleBatchPublishFailure {
    pub(crate) error: ProcessError,
    pub(crate) handles: InTransitCapabilities,
}

impl<const N: usize> Drop for ProcessHandleReservation<N> {
    fn drop(&mut self) {
        if self.reservation.is_some() || self.handle_charges.is_some() || self.record.is_some() {
            process_invariant_violation();
        }
    }
}

struct PreparedUserThread {
    process: Process,
    thread: UserThread,
    record: Option<FallibleArc<ThreadRecord>>,
    execution_charge: Option<CommittedCharge>,
    address_space: FallibleArc<NativeAddressSpace>,
    committed: bool,
}

impl PreparedUserThread {
    fn publish(
        mut self,
        id: ThreadId,
        dormant: crate::kernel::task::scheduler::DormantUserThread,
    ) -> Option<TerminalReason> {
        let record = match self.record.take() {
            Some(record) => record,
            None => process_invariant_violation(),
        };
        let charge = match self.execution_charge.take() {
            Some(charge) => charge,
            None => process_invariant_violation(),
        };
        let process_record = record.clone();
        let membership = ProcessThreadMembership {
            process: self.process.clone(),
            record: Some(record),
        };
        dormant.commit_before_process_publication(UserExecutionOwnership::new(membership, charge));
        self.thread.publish(id);
        let terminal = self.process.inner.state.with(|state| {
            if state.lifecycle.publish_thread().is_err() {
                process_invariant_violation();
            }
            process_record
                .next
                .with(|next| *next = state.threads.clone());
            process_record
                .scheduler_id
                .store(id.get(), Ordering::Relaxed);
            process_record.active.store(true, Ordering::Release);
            state.threads = Some(process_record);
            state.lifecycle.terminal()
        });
        self.committed = true;
        terminal
    }
}

impl Drop for PreparedUserThread {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        self.process.abort_pending_thread();
    }
}

pub(super) struct ProcessThreadMembership {
    process: Process,
    record: Option<FallibleArc<ThreadRecord>>,
}

impl ProcessThreadMembership {
    pub(super) const fn process(&self) -> &Process {
        &self.process
    }

    pub(super) fn detach(mut self, terminal: TerminalReason) {
        let record = match self.record.take() {
            Some(record) => record,
            None => process_invariant_violation(),
        };
        if !record.active.swap(false, Ordering::AcqRel) {
            process_invariant_violation();
        }
        let status = match terminal {
            TerminalReason::ThreadExited { status }
            | TerminalReason::ProcessExited { status }
            | TerminalReason::LastThreadExited { status } => status,
            _ => 0,
        };
        let detached = self.process.inner.state.with(|state| {
            let before = state.lifecycle.phase();
            state.lifecycle.detach_thread(status)?;
            let detached = unlink_thread_record(state, &record);
            Ok::<_, LifecycleError>((
                before != ProcessPhase::Stopped && state.lifecycle.phase() == ProcessPhase::Stopped,
                detached,
            ))
        });
        let (became_stopped, detached_record) = match detached {
            Ok(result) => result,
            Err(_) => process_invariant_violation(),
        };
        drop(detached_record);
        drop(record);
        if became_stopped {
            self.process.publish_stopped();
        }
    }
}

impl Drop for ProcessThreadMembership {
    fn drop(&mut self) {
        if self.record.is_some() {
            process_invariant_violation();
        }
    }
}

#[must_use = "retry or deliberately retain committed process retirement"]
pub(crate) struct AddressSpaceRetirement {
    process: Process,
    address_space: Option<UniqueFallibleArc<NativeAddressSpace>>,
}

impl AddressSpaceRetirement {
    pub(crate) fn retry(mut self) -> Result<(), (Self, MachineError)> {
        let address_space = match self.address_space.take() {
            Some(address_space) => address_space,
            None => process_invariant_violation(),
        };
        match NativeAddressSpace::retire(address_space) {
            Ok(()) => {
                self.process.finish_retirement();
                Ok(())
            }
            Err(failure) => {
                let (error, address_space) = failure.into_parts();
                self.address_space = Some(address_space);
                Err((self, error))
            }
        }
    }
}

impl Drop for AddressSpaceRetirement {
    fn drop(&mut self) {
        if self.address_space.is_some() {
            process_invariant_violation();
        }
    }
}

fn metadata_amount<T>() -> Result<ResourceAmount, ProcessError> {
    Ok(ResourceAmount::ZERO
        .with(ResourceKind::KernelObjects, 1)
        .with(
            ResourceKind::KernelMemoryBytes,
            u64::try_from(FallibleArc::<T>::allocation_size())
                .map_err(|_| ProcessError::Allocation)?,
        ))
}

fn process_metadata_amount() -> Result<ResourceAmount, ProcessError> {
    let bytes = FallibleArc::<ProcessInner>::allocation_size()
        .checked_add(super::directory::registration_size())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ProcessError::Allocation)?;
    Ok(ResourceAmount::ZERO
        .with(ResourceKind::KernelObjects, 1)
        .with(ResourceKind::KernelMemoryBytes, bytes))
}

fn install_table_storage_charge(
    state: &mut ProcessState,
    snapshot: HandleTableStorageSnapshot,
    prepared: &mut Option<CommittedCharge>,
) {
    let bytes = match snapshot.growth_bytes() {
        Some(bytes) => bytes,
        None => process_invariant_violation(),
    };
    if bytes == 0 {
        if prepared.is_some() {
            process_invariant_violation();
        }
    } else {
        let charge = match prepared.take() {
            Some(charge) => charge,
            None => process_invariant_violation(),
        };
        let Some(slot) = state
            .handle_table_charges
            .iter_mut()
            .find(|slot| slot.is_none())
        else {
            process_invariant_violation();
        };
        *slot = Some(charge);
    }
}

fn require_handle_phase(phase: ProcessPhase) -> Result<(), ProcessError> {
    match phase {
        ProcessPhase::Created | ProcessPhase::Running => Ok(()),
        _ => Err(ProcessError::Lifecycle(LifecycleError::AdmissionClosed)),
    }
}

fn unlink_thread_record(
    state: &mut ProcessState,
    target: &FallibleArc<ThreadRecord>,
) -> FallibleArc<ThreadRecord> {
    let mut current = state.threads.clone();
    let mut previous: Option<FallibleArc<ThreadRecord>> = None;
    while let Some(record) = current {
        let next = record.next.with(|next| next.clone());
        if core::ptr::eq::<ThreadRecord>(&*record, &**target) {
            if let Some(previous) = previous {
                previous.next.with(|link| *link = next);
            } else {
                state.threads = next;
            }
            // Keep the published successor immutable for lock-free stop
            // traversals which may already retain this detached record.
            // The returned owner is dropped outside the Process lock; once
            // the last traversal releases it, its successor reference follows.
            return record;
        }
        previous = Some(record);
        current = next;
    }
    process_invariant_violation()
}

fn install_handle_charge_record(state: &mut ProcessState, record: FallibleArc<HandleChargeRecord>) {
    record.state.with(|entries| {
        for (entry, charge) in entries.entries.iter().enumerate() {
            if state
                .charge_index
                .replace(
                    charge.value,
                    Some(HandleChargeLocation {
                        record: record.clone(),
                        entry,
                    }),
                )
                .is_some()
            {
                process_invariant_violation();
            }
        }
    });
    let previous = state.handle_charges.take();
    if let Some(previous) = previous.as_ref() {
        previous
            .previous
            .with(|link| *link = Some(FallibleArc::downgrade(&record)));
    }
    record.next.with(|link| *link = previous);
    state.handle_charges = Some(record);
}

fn release_handle_charge(
    state: &mut ProcessState,
    value: HandleValue,
) -> (CommittedCharge, Option<FallibleArc<HandleChargeRecord>>) {
    let location = match state.charge_index.replace(value, None) {
        Some(location) => location,
        None => process_invariant_violation(),
    };
    let record = location.record;
    let (charge, empty) = record.state.with(|state| {
        let entry = &mut state.entries[location.entry];
        if entry.value != value {
            process_invariant_violation();
        }
        let charge = match entry.charge.take() {
            Some(charge) => charge,
            None => process_invariant_violation(),
        };
        (
            charge,
            state.entries.iter().all(|entry| entry.charge.is_none()),
        )
    });
    if !empty {
        return (charge, None);
    }
    let previous = record.previous.with(Option::take);
    let next = record.next.with(Option::take);
    if let Some(next) = next.as_ref() {
        next.previous.with(|link| *link = previous.clone());
    }
    if let Some(previous) = previous {
        let previous = match previous.upgrade() {
            Some(previous) => previous,
            None => process_invariant_violation(),
        };
        previous.next.with(|link| *link = next);
    } else {
        state.handle_charges = next;
    }
    (charge, Some(record))
}

fn handle_charge_is_live(state: &ProcessState, value: HandleValue) -> bool {
    state.charge_index.get(value).is_some_and(|location| {
        location.record.state.with(|state| {
            let entry = &state.entries[location.entry];
            entry.value == value && entry.charge.is_some()
        })
    })
}

fn replace_handle_charge_value(
    state: &mut ProcessState,
    previous: HandleValue,
    replacement: HandleValue,
) {
    let location = match state.charge_index.replace(previous, None) {
        Some(location) => location,
        None => process_invariant_violation(),
    };
    location.record.state.with(|state| {
        let entry = &mut state.entries[location.entry];
        if entry.value != previous || entry.charge.is_none() {
            process_invariant_violation();
        }
        entry.value = replacement;
    });
    if state
        .charge_index
        .replace(replacement, Some(location))
        .is_some()
    {
        process_invariant_violation();
    }
}

fn allocate_process_id() -> Result<ProcessId, ProcessError> {
    let mut current = NEXT_PROCESS_ID.load(Ordering::Relaxed);
    loop {
        if current == 0 {
            return Err(ProcessError::Allocation);
        }
        let next = current.checked_add(1).ok_or(ProcessError::Allocation)?;
        match NEXT_PROCESS_ID.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(ProcessId(current)),
            Err(observed) => current = observed,
        }
    }
}

fn create_failure(
    error: ProcessError,
    address_space: UniqueFallibleArc<NativeAddressSpace>,
) -> ProcessCreateFailure {
    ProcessCreateFailure {
        error: Some(error),
        address_space: Some(address_space),
    }
}

fn create_failure_from_arc(
    error: ProcessError,
    address_space: FallibleArc<NativeAddressSpace>,
) -> ProcessCreateFailure {
    match address_space.try_into_unique() {
        Ok(address_space) => create_failure(error, address_space),
        Err(_) => process_invariant_violation(),
    }
}

fn recover_unpublished_address_space(process: Process) -> FallibleArc<NativeAddressSpace> {
    let inner = match process.inner.try_unwrap() {
        Ok(inner) => inner,
        Err(_) => process_invariant_violation(),
    };
    let state = inner.state.into_inner();
    match state.address_space {
        Some(address_space) => address_space,
        None => process_invariant_violation(),
    }
}

#[cold]
fn process_invariant_violation() -> ! {
    crate::hal::cpu::halt()
}
