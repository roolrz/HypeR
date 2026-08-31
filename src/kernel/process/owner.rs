// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process composition, publication, stop, and explicit retirement.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use hyper::mm::{FallibleArc, UniqueFallibleArc};
use hyper::sync::InterruptSpinLock;

use super::image::{AbiFamily, ExecutionRoute, MachineAbi, ProcessImage, UserThreadStart};
use super::lifecycle::{
    LifecycleError, ProcessLifecycle, ProcessPhase, StopDispatchProgress, TerminalReason,
};
use super::task_group::{
    PreparedTaskGroupMembership, TaskGroup, TaskGroupError, TaskGroupMembership,
};
use super::user_thread::{UserExecution, UserThread};
use crate::kernel::accounting::{
    ChargeReservation, CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::capability::{
    ClosedHandle, HandleError, HandleFlags, HandleInfo, HandleReservation, HandleTable,
    HandleValue, KernelObject, ObjectCreationError, ObjectRef, PreparedHandle, ResolvedObject,
    Rights,
};
use crate::kernel::mm::user_space::{MachineError, NativeAddressSpace, UserSlice};
use crate::kernel::sync::Completion;
use crate::kernel::task::scheduler::{self, CpuMask};
use crate::kernel::task::thread::ThreadId;

type ProcessLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

static NEXT_PROCESS_ID: AtomicU64 = AtomicU64::new(1);

const fn machine_matches_host(machine: MachineAbi) -> bool {
    let requested = match machine {
        MachineAbi::Aarch64 => crate::hal::user::HostMachine::Aarch64,
        MachineAbi::Riscv64 => crate::hal::user::HostMachine::Riscv64,
        MachineAbi::X86_64 => crate::hal::user::HostMachine::X86_64,
    };
    requested as u8 == crate::hal::user::host_machine() as u8
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
    handle_table_charges: Option<FallibleArc<TableStorageChargeRecord>>,
    handles_retired: bool,
}

struct HandleChargeState {
    entries: alloc::vec::Vec<HandleChargeEntry>,
}

struct HandleChargeEntry {
    value: HandleValue,
    charge: Option<CommittedCharge>,
}

struct HandleChargeRecord {
    state: ProcessLock<HandleChargeState>,
    next: ProcessLock<Option<FallibleArc<HandleChargeRecord>>>,
    _metadata_charge: CommittedCharge,
}

struct TableStorageChargeRecord {
    next: ProcessLock<Option<FallibleArc<TableStorageChargeRecord>>>,
    _charge: CommittedCharge,
}

struct ProcessInner {
    id: ProcessId,
    image_generation: u64,
    image: ProcessImage,
    domain: ResourceDomain,
    handles: ProcessLock<HandleTable>,
    state: ProcessLock<ProcessState>,
    stopped: Completion,
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
    inner: FallibleArc<ProcessInner>,
}

#[must_use = "publish or abort the prepared process"]
pub(crate) struct PreparedProcess {
    process: Option<Process>,
    group: Option<PreparedTaskGroupMembership>,
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
        let metadata_amount = match metadata_amount::<ProcessInner>() {
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
                handle_table_charges: None,
                handles_retired: false,
            }),
            stopped: Completion::new(),
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
        Ok(Self {
            process: Some(Process { inner }),
            group: Some(group_membership),
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
        process
    }

    /// Aborts unpublished Process construction and returns machine ownership.
    pub(crate) fn abort(mut self) -> UniqueFallibleArc<NativeAddressSpace> {
        let process = match self.process.take() {
            Some(process) => process,
            None => process_invariant_violation(),
        };
        drop(self.group.take());
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
        if self.process.is_some() || self.group.is_some() {
            process_invariant_violation();
        }
    }
}

impl Process {
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
        let execution = UserExecution::try_new(
            prepared.thread.clone(),
            prepared.address_space.clone(),
            context,
        )
        .map_err(|()| ProcessError::Allocation)?;
        let dormant = scheduler::prepare_user_thread(
            name,
            execution,
            crate::kernel::entry::user::thread_entry,
            affinity,
        )?;
        let id = dormant.id();
        let thread = prepared.thread.clone();
        // Arming scheduler ownership before Process publication is safe because
        // the dormant ID has not escaped and cannot be made runnable. Every
        // subsequent operation is an infallible publication step.
        dormant.commit_before_process_publication();
        let terminal = prepared.publish(id);
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
        if became_stopped && self.inner.stopped.complete_all().is_err() {
            crate::hal::cpu::halt();
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
        let thread_metadata_bytes =
            u64::try_from(UserThread::allocation_size()).map_err(|_| ProcessError::Allocation)?;
        let thread_metadata = self
            .inner
            .domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, thread_metadata_bytes),
            )?
            .commit();
        let thread = UserThread::try_prepared(self.clone(), thread_metadata)
            .map_err(|()| ProcessError::Allocation)?;
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
        if completed && self.inner.stopped.complete_all().is_err() {
            crate::hal::cpu::halt();
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
            let growth = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                Ok::<_, ProcessError>(
                    self.inner
                        .handles
                        .with(|table| table.reservation_growth::<N>())?,
                )
            })?;
            let storage_record = self.prepare_table_storage_charge(growth)?;
            let attempt = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                let current_growth = self
                    .inner
                    .handles
                    .with(|table| table.reservation_growth::<N>())?;
                if current_growth != growth {
                    return Ok::<_, ProcessError>(None);
                }
                let reservation = self.inner.handles.with(HandleTable::reserve)?;
                if let Some(record) = storage_record.as_ref() {
                    record
                        .next
                        .with(|next| *next = state.handle_table_charges.clone());
                    state.handle_table_charges = Some(record.clone());
                }
                Ok(Some(reservation))
            });
            match attempt {
                Ok(Some(reservation)) => break reservation,
                Ok(None) => {
                    drop(storage_record);
                }
                Err(error) => {
                    drop(storage_record);
                    return Err(error);
                }
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

    fn prepare_table_storage_charge(
        &self,
        growth: usize,
    ) -> Result<Option<FallibleArc<TableStorageChargeRecord>>, ProcessError> {
        if growth == 0 {
            return Ok(None);
        }
        let storage_bytes = growth
            .checked_mul(HandleTable::slot_storage_size())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let base = metadata_amount::<TableStorageChargeRecord>()?;
        let amount = base.with(
            ResourceKind::KernelMemoryBytes,
            base.get(ResourceKind::KernelMemoryBytes)
                .checked_add(storage_bytes)
                .ok_or(ProcessError::Allocation)?,
        );
        let charge = self.inner.domain.reserve(amount)?.commit();
        let record = FallibleArc::try_new(TableStorageChargeRecord {
            next: ProcessLock::new(None),
            _charge: charge,
        })
        .map_err(|_| ProcessError::Allocation)?;
        Ok(Some(record))
    }

    pub(crate) fn publish_handles<const N: usize>(
        &self,
        mut reservation: ProcessHandleReservation<N>,
        handles: [PreparedHandle; N],
    ) -> Result<[HandleValue; N], HandlePublishFailure<N>> {
        let mut handles = Some(handles);
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
            record
                .next
                .with(|next| *next = state.handle_charges.clone());
            state.handle_charges = Some(record);
            Ok(values)
        });
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

    /// Publishes the first process-local handle for a new kernel object.
    ///
    /// Slot, quota, object identity, and active-handle state are prepared
    /// before the Process lock commits publication. Every failure before that
    /// point rolls back both the slot reservation and unpublished authority.
    pub(crate) fn create_object<T: KernelObject>(
        &self,
        payload: T,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        let reservation = self.reserve_handles::<1>()?;
        let object = match ObjectRef::try_new(payload) {
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
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            let replacement = self
                .inner
                .handles
                .with(|table| table.replace(value, rights))?;
            replace_handle_charge_value(state, value, replacement);
            Ok(replacement)
        })
    }

    pub(crate) fn copy_to_user(
        &self,
        destination: UserSlice,
        source: &[u8],
    ) -> Result<(), ProcessError> {
        let address_space = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            state
                .address_space
                .as_ref()
                .cloned()
                .ok_or(ProcessError::AddressSpaceReferenced)
        })?;
        address_space.copy_to_user(destination, source)?;
        Ok(())
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

    pub(crate) fn retire(&self) -> Result<ProcessRetirementStep, ProcessError> {
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
        let mut address_space = match address_space.try_into_unique() {
            Ok(address_space) => address_space,
            Err(address_space) => {
                self.inner
                    .state
                    .with(|state| state.address_space = Some(address_space));
                return Ok(ProcessRetirementStep::PendingReferences);
            }
        };
        match address_space.retire() {
            Ok(()) => {
                drop(address_space);
                self.finish_retirement();
                Ok(ProcessRetirementStep::Complete)
            }
            Err(_) => Ok(ProcessRetirementStep::Retry(AddressSpaceRetirement {
                process: self.clone(),
                address_space: Some(address_space),
            })),
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
        let (membership, process_charge, mut records, mut handle_charges, mut table_charges) =
            self.inner.state.with(|state| {
                if state.lifecycle.finish_retirement().is_err() {
                    process_invariant_violation();
                }
                (
                    state.group_membership.take(),
                    state.process_charge.take(),
                    state.threads.take(),
                    state.handle_charges.take(),
                    state.handle_table_charges.take(),
                )
            });
        while let Some(record) = records {
            records = record.next.with(Option::take);
            drop(record);
        }
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
        while let Some(record) = table_charges {
            table_charges = record.next.with(Option::take);
            drop(record);
        }
        let membership = match membership {
            Some(membership) => membership,
            None => process_invariant_violation(),
        };
        membership.retire();
        drop(process_charge);
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

#[must_use = "publish or abort the process handle reservation"]
pub(crate) struct ProcessHandleReservation<const N: usize> {
    reservation: Option<HandleReservation<N>>,
    handle_charges: Option<alloc::vec::Vec<ChargeReservation>>,
    record: Option<FallibleArc<HandleChargeRecord>>,
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
    fn publish(mut self, id: ThreadId) -> Option<TerminalReason> {
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
        self.thread.publish(id, membership, charge);
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
        if became_stopped && self.process.inner.stopped.complete_all().is_err() {
            process_invariant_violation();
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
        let mut address_space = match self.address_space.take() {
            Some(address_space) => address_space,
            None => process_invariant_violation(),
        };
        match address_space.retire() {
            Ok(()) => {
                drop(address_space);
                self.process.finish_retirement();
                Ok(())
            }
            Err(error) => {
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

fn release_handle_charge(
    state: &mut ProcessState,
    value: HandleValue,
) -> (CommittedCharge, Option<FallibleArc<HandleChargeRecord>>) {
    let mut current = state.handle_charges.clone();
    let mut previous: Option<FallibleArc<HandleChargeRecord>> = None;
    while let Some(record) = current {
        let release = record.state.with(|record_state| {
            for entry in &mut record_state.entries {
                if entry.value == value {
                    let charge = match entry.charge.take() {
                        Some(charge) => charge,
                        None => process_invariant_violation(),
                    };
                    let empty = record_state
                        .entries
                        .iter()
                        .all(|entry| entry.charge.is_none());
                    return Some((charge, empty));
                }
            }
            None
        });
        let next = record.next.with(|next| next.clone());
        if let Some((charge, empty)) = release {
            if !empty {
                return (charge, None);
            }
            if let Some(previous) = previous {
                previous.next.with(|link| *link = next);
            } else {
                state.handle_charges = next;
            }
            record.next.with(|next| *next = None);
            return (charge, Some(record));
        }
        previous = Some(record);
        current = next;
    }
    process_invariant_violation()
}

fn replace_handle_charge_value(
    state: &mut ProcessState,
    previous_value: HandleValue,
    replacement_value: HandleValue,
) {
    let mut current = state.handle_charges.clone();
    while let Some(record) = current {
        let replaced = record.state.with(|record_state| {
            for entry in &mut record_state.entries {
                if entry.value == previous_value {
                    if entry.charge.is_none() {
                        process_invariant_violation();
                    }
                    entry.value = replacement_value;
                    return true;
                }
            }
            false
        });
        if replaced {
            return;
        }
        current = record.next.with(|next| next.clone());
    }
    process_invariant_violation()
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

#[cold]
fn process_invariant_violation() -> ! {
    crate::hal::cpu::halt()
}
