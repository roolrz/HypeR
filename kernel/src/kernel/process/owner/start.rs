// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Atomic child Process preparation, publication, and rollback.

use super::*;

/// Failure before a child Process becomes visible in any kernel directory.
#[derive(Debug)]
pub(crate) enum ChildProcessStartError {
    Builder(ProcessBuilderError<()>),
    Process(ProcessError),
    Stack(hyper::exec::startup::Error),
    TaskObject(super::super::objects::TaskObjectError),
    VmarObject(MemoryObjectError),
}

/// Parent-side coordinator for one consume-builder-and-start transaction.
///
/// The builder handle value and KOID identify the exact authority resolved by
/// the syscall adapter. A generation match alone is insufficient because the
/// adapter releases its table lock before the builder serializes preparation.
pub(crate) struct ProcessStartCoordinator {
    parent: Process,
    builder_handle: HandleValue,
    builder_koid: Koid,
}

impl ProcessStartCoordinator {
    pub(crate) const fn new(
        parent: Process,
        builder_handle: HandleValue,
        builder_koid: Koid,
    ) -> Self {
        Self {
            parent,
            builder_handle,
            builder_koid,
        }
    }
}

/// Published child identity returned after deferred builder authority retires.
pub(crate) struct StartedChildProcess {
    process: Process,
    initial_thread: UserThread,
    supervisor_handle: HandleValue,
}

impl StartedChildProcess {
    pub(crate) const fn process(&self) -> &Process {
        &self.process
    }

    pub(crate) const fn initial_thread(&self) -> &UserThread {
        &self.initial_thread
    }

    pub(crate) const fn supervisor_handle(&self) -> HandleValue {
        self.supervisor_handle
    }
}

pub(crate) struct PreparedChildProcessStart {
    parent: Process,
    builder_handle: HandleValue,
    build: Option<SealedProcessBuild>,
    child: Process,
    child_handle_batches: alloc::vec::Vec<ProcessHandleBatchReservation>,
    prepared_startup_handles: alloc::vec::Vec<PreparedHandle>,
    retired_batch_storage: alloc::vec::Vec<RetiredHandleBatchReservationStorage>,
    retired_charge_storage: alloc::vec::Vec<alloc::vec::Vec<ChargeReservation>>,
    retired_scratch_charges: alloc::vec::Vec<CommittedCharge>,
    start_scratch_charge: Option<CommittedCharge>,
    initial_thread: Option<PreparedInitialUserThread>,
    parent_supervisor: Option<ProcessHandleReservation<1>>,
    builder_consumption: Option<PreparedHandleConsumption>,
    root_vmar_object: Option<PreparedHandle>,
    root_vmar_address_space: FallibleArc<NativeAddressSpace>,
    root_vmar_claimed: bool,
    process_object: Option<PublishableRef<ProcessObject, KernelService>>,
    supervisor_object: Option<PreparedHandle>,
}

pub(crate) struct CommittedChildProcessStart {
    child: Process,
    initial_thread: UserThread,
    supervisor_handle: HandleValue,
    retired_builder: Option<InTransitCapabilities>,
}

impl Drop for CommittedChildProcessStart {
    fn drop(&mut self) {
        if self.retired_builder.is_some() {
            process_invariant_violation();
        }
    }
}

impl ProcessStartTransaction for ProcessStartCoordinator {
    type Error = ChildProcessStartError;
    type Prepared = PreparedChildProcessStart;
    type Committed = CommittedChildProcessStart;
    type Output = StartedChildProcess;

    fn prepare_start(
        &mut self,
        build: SealedProcessBuild,
    ) -> Result<Self::Prepared, StartPreparationFailure<Self::Error>> {
        PreparedChildProcessStart::prepare(
            self.parent.clone(),
            self.builder_handle,
            self.builder_koid,
            build,
        )
    }

    fn commit_start(&mut self, prepared: Self::Prepared) -> Self::Committed {
        prepared.commit()
    }

    fn finish_start(&mut self, mut committed: Self::Committed) -> Self::Output {
        match committed.retired_builder.take() {
            Some(builder) => builder.release(),
            None => process_invariant_violation(),
        }
        StartedChildProcess {
            process: committed.child.clone(),
            initial_thread: committed.initial_thread.clone(),
            supervisor_handle: committed.supervisor_handle,
        }
    }

    fn cancel_start(&mut self, prepared: Self::Prepared) -> SealedProcessBuild {
        prepared.cancel()
    }
}

impl PreparedChildProcessStart {
    // Keep fallible preparation out of the publication coordinator's frame.
    // This boundary is part of the native syscall stack budget: preparation
    // returns before abort arbitration and the infallible commit begin.
    #[inline(never)]
    #[expect(
        clippy::result_large_err,
        reason = "the error retains the complete linear build owner for allocation-free rollback"
    )]
    fn prepare(
        parent: Process,
        builder_handle: HandleValue,
        builder_koid: Koid,
        build: SealedProcessBuild,
    ) -> Result<Self, StartPreparationFailure<ChildProcessStartError>> {
        let child = build.child().process().clone();
        let startup_count = match build.startup_capabilities().len().checked_add(1) {
            Some(count) => count,
            None => {
                return Err(start_failure(
                    ChildProcessStartError::Process(ProcessError::Allocation),
                    build,
                ));
            }
        };
        let batch_maximum = HandleBatchReservationStorage::maximum_count();
        let batch_count = match startup_count
            .checked_add(batch_maximum.saturating_sub(1))
            .map(|rounded| rounded / batch_maximum)
        {
            Some(count) => count,
            None => {
                return Err(start_failure(
                    ChildProcessStartError::Process(ProcessError::Allocation),
                    build,
                ));
            }
        };
        let start_scratch_charge = match reserve_start_scratch(
            &child,
            startup_count,
            batch_count,
            build.argument_count(),
            build.environment_count(),
            build.stack_layout().total_bytes(),
        ) {
            Ok(charge) => charge,
            Err(error) => {
                return Err(start_failure(ChildProcessStartError::Process(error), build));
            }
        };
        let mut child_handle_batches = match build
            .child()
            .reserve_startup_handle_batches(startup_count)
        {
            Ok(batches) => batches,
            Err(error) => return Err(start_failure(ChildProcessStartError::Process(error), build)),
        };

        if child_handle_batches.len() != batch_count {
            process_invariant_violation();
        }
        let mut prepared_startup_handles = alloc::vec::Vec::new();
        let mut retired_batch_storage = alloc::vec::Vec::new();
        let mut retired_charge_storage = alloc::vec::Vec::new();
        let mut retired_scratch_charges = alloc::vec::Vec::new();
        let scratch_ready = prepared_startup_handles
            .try_reserve_exact(startup_count)
            .and_then(|()| retired_batch_storage.try_reserve_exact(batch_count))
            .and_then(|()| retired_charge_storage.try_reserve_exact(batch_count))
            .and_then(|()| retired_scratch_charges.try_reserve_exact(batch_count));
        if scratch_ready.is_err() {
            abort_child_handle_batches(&child, &mut child_handle_batches);
            return Err(start_failure(
                ChildProcessStartError::Process(ProcessError::Allocation),
                build,
            ));
        }
        let arguments = match build.arguments() {
            Ok(arguments) => arguments,
            Err(error) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Builder(error), build));
            }
        };
        let environment = match build.environment() {
            Ok(environment) => environment,
            Err(error) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Builder(error), build));
            }
        };
        let mut startup_records = alloc::vec::Vec::new();
        if startup_records.try_reserve_exact(startup_count).is_err() {
            abort_child_handle_batches(&child, &mut child_handle_batches);
            return Err(start_failure(
                ChildProcessStartError::Process(ProcessError::Allocation),
                build,
            ));
        }
        let mut values = child_handle_batches
            .iter()
            .flat_map(ProcessHandleBatchReservation::values);
        for capability in build.startup_capabilities() {
            let value = match values.next() {
                Some(value) => *value,
                None => process_invariant_violation(),
            };
            startup_records.push(StartupHandle {
                purpose: capability.purpose(),
                handle: value.get(),
            });
        }
        let root_vmar_value = match values.next() {
            Some(value) => *value,
            None => process_invariant_violation(),
        };
        startup_records.push(StartupHandle {
            purpose: hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR as u32,
            handle: root_vmar_value.get(),
        });
        if values.next().is_some() {
            process_invariant_violation();
        }
        let stack = match build.stack_layout().encode(
            child.image().auxiliary(),
            &arguments,
            &environment,
            &startup_records,
        ) {
            Ok(stack) => stack,
            Err(error) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Stack(error), build));
            }
        };
        let stack_length = match u64::try_from(stack.bytes().len()) {
            Ok(length) => length,
            Err(_) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(
                    ChildProcessStartError::Stack(hyper::exec::startup::Error::TooLarge),
                    build,
                ));
            }
        };
        let stack_range = match UserSlice::new(UserAddress::new(stack.base()), stack_length) {
            Ok(range) => range,
            Err(_) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(
                    ChildProcessStartError::Stack(hyper::exec::startup::Error::AddressOverflow),
                    build,
                ));
            }
        };
        let stack_write = match build.child().reserve_initial_stack_write(stack_range) {
            Ok(write) => write,
            Err(error) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Process(error), build));
            }
        };
        if let Err(error) = stack_write.copy_from(stack.bytes()) {
            drop(stack_write);
            abort_child_handle_batches(&child, &mut child_handle_batches);
            return Err(start_failure(
                ChildProcessStartError::Process(ProcessError::UserMemory(error)),
                build,
            ));
        }
        stack_write.complete();

        let initial_thread = match build
            .child()
            .prepare_initial_user_thread(build.thread_name(), build.affinity())
        {
            Ok(thread) => thread,
            Err(error) => {
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Process(error), build));
            }
        };
        let parent_supervisor = match parent.reserve_handles::<1>() {
            Ok(reservation) => reservation,
            Err(error) => {
                drop(initial_thread);
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Process(error), build));
            }
        };
        let builder_consumption = match parent.prepare_handle_consumption(
            builder_handle,
            Rights::START,
            crate::kernel::object::ObjectKind::PROCESS_BUILDER,
            builder_koid,
        ) {
            Ok(consumption) => consumption,
            Err(error) => {
                parent.abort_handles(parent_supervisor);
                drop(initial_thread);
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(ChildProcessStartError::Process(error), build));
            }
        };
        let root_vmar_address_space = build.child().address_space_owner();
        let root_vmar_publication = match VmarObject::try_root_publication(
            root_vmar_address_space.clone(),
            &child.resource_domain(),
        ) {
            Ok(publication) => publication,
            Err(error) => {
                builder_consumption.rollback();
                parent.abort_handles(parent_supervisor);
                drop(initial_thread);
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(
                    ChildProcessStartError::VmarObject(error),
                    build,
                ));
            }
        };
        let root_vmar_object = match PreparedHandle::try_from_new_object(
            root_vmar_publication,
            VmarObject::ROOT_RIGHTS,
            HandleFlags::NONE,
        ) {
            Ok(handle) => handle,
            Err(error) => {
                VmarObject::abort_root_publication(&root_vmar_address_space);
                builder_consumption.rollback();
                parent.abort_handles(parent_supervisor);
                drop(initial_thread);
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(
                    ChildProcessStartError::Process(error.into()),
                    build,
                ));
            }
        };
        let process_object = match ProcessObject::try_service(&child) {
            Ok(object) => object,
            Err(error) => {
                drop(root_vmar_object);
                VmarObject::abort_root_publication(&root_vmar_address_space);
                builder_consumption.rollback();
                parent.abort_handles(parent_supervisor);
                drop(initial_thread);
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(
                    ChildProcessStartError::TaskObject(error),
                    build,
                ));
            }
        };
        let publication = process_object.publication();
        let supervisor_object = match PreparedHandle::try_from_new_object(
            publication,
            ProcessObject::SUPERVISOR_RIGHTS,
            HandleFlags::NONE,
        ) {
            Ok(handle) => handle,
            Err(error) => {
                drop(root_vmar_object);
                VmarObject::abort_root_publication(&root_vmar_address_space);
                builder_consumption.rollback();
                parent.abort_handles(parent_supervisor);
                drop(initial_thread);
                abort_child_handle_batches(&child, &mut child_handle_batches);
                return Err(start_failure(
                    ChildProcessStartError::Process(error.into()),
                    build,
                ));
            }
        };

        Ok(Self {
            parent,
            builder_handle,
            build: Some(build),
            child,
            child_handle_batches,
            prepared_startup_handles,
            retired_batch_storage,
            retired_charge_storage,
            retired_scratch_charges,
            start_scratch_charge: Some(start_scratch_charge),
            initial_thread: Some(initial_thread),
            parent_supervisor: Some(parent_supervisor),
            builder_consumption: Some(builder_consumption),
            root_vmar_object: Some(root_vmar_object),
            root_vmar_address_space,
            root_vmar_claimed: true,
            process_object: Some(process_object),
            supervisor_object: Some(supervisor_object),
        })
    }

    fn commit(mut self) -> CommittedChildProcessStart {
        let build = match self.build.take() {
            Some(build) => build,
            None => process_invariant_violation(),
        };
        let process_name = ProcessNameSnapshot::from_validated(build.thread_name());
        let (prepared_child, _, startup_capabilities) = build.into_parts();
        for capability in startup_capabilities {
            let (mut handles, storage_charge) = capability.take_authority().into_prepared_handles();
            if handles.len() != 1 {
                process_invariant_violation();
            }
            let handle = match handles.pop() {
                Some(handle) => handle,
                None => process_invariant_violation(),
            };
            self.prepared_startup_handles.push(handle);
            drop(handles);
            drop(storage_charge);
        }
        let root_vmar = match self.root_vmar_object.take() {
            Some(root_vmar) => root_vmar,
            None => process_invariant_violation(),
        };
        self.prepared_startup_handles.push(root_vmar);
        self.prepared_startup_handles.reverse();
        publish_unpublished_startup_handles(&mut self);

        let initial_thread = match self.initial_thread.take() {
            Some(thread) => thread,
            None => process_invariant_violation(),
        };
        let (initial_thread, thread_id, terminal) = initial_thread.publish();
        if terminal.is_some() {
            process_invariant_violation();
        }
        let process_object = match self.process_object.take() {
            Some(object) => object,
            None => process_invariant_violation(),
        };
        let child = prepared_child.publish(process_object, process_name);
        let (supervisor_handle, retired_builder) = commit_parent_builder_replacement(&mut self);
        child.commit_initial_execution(thread_id);
        drop(self.start_scratch_charge.take());
        self.root_vmar_claimed = false;
        CommittedChildProcessStart {
            child,
            initial_thread,
            supervisor_handle,
            retired_builder: Some(retired_builder),
        }
    }

    fn cancel(mut self) -> SealedProcessBuild {
        let supervisor_object = self.supervisor_object.take();
        drop(supervisor_object);
        drop(self.process_object.take());
        if let Some(consumption) = self.builder_consumption.take() {
            consumption.rollback();
        }
        if let Some(reservation) = self.parent_supervisor.take() {
            self.parent.abort_handles(reservation);
        }
        drop(self.initial_thread.take());
        drop(self.root_vmar_object.take());
        if self.root_vmar_claimed {
            VmarObject::abort_root_publication(&self.root_vmar_address_space);
            self.root_vmar_claimed = false;
        }
        abort_child_handle_batches(&self.child, &mut self.child_handle_batches);
        self.prepared_startup_handles.clear();
        drop(self.start_scratch_charge.take());
        match self.build.take() {
            Some(build) => build,
            None => process_invariant_violation(),
        }
    }
}

impl Drop for PreparedChildProcessStart {
    fn drop(&mut self) {
        if self.build.is_some()
            || !self.child_handle_batches.is_empty()
            || !self.prepared_startup_handles.is_empty()
            || self.initial_thread.is_some()
            || self.parent_supervisor.is_some()
            || self.builder_consumption.is_some()
            || self.root_vmar_object.is_some()
            || self.root_vmar_claimed
            || self.process_object.is_some()
            || self.supervisor_object.is_some()
            || self.start_scratch_charge.is_some()
        {
            process_invariant_violation();
        }
    }
}

fn reserve_start_scratch(
    child: &Process,
    startup_count: usize,
    batch_count: usize,
    argument_count: usize,
    environment_count: usize,
    stack_bytes: usize,
) -> Result<CommittedCharge, ProcessError> {
    let allocations = [
        (
            batch_count,
            core::mem::size_of::<ProcessHandleBatchReservation>(),
        ),
        (startup_count, core::mem::size_of::<PreparedHandle>()),
        (
            batch_count,
            core::mem::size_of::<RetiredHandleBatchReservationStorage>(),
        ),
        (
            batch_count,
            core::mem::size_of::<alloc::vec::Vec<ChargeReservation>>(),
        ),
        (batch_count, core::mem::size_of::<CommittedCharge>()),
        (argument_count, core::mem::size_of::<&str>()),
        (environment_count, core::mem::size_of::<&str>()),
        (startup_count, core::mem::size_of::<StartupHandle>()),
        (1, stack_bytes),
    ];
    let bytes = allocations
        .into_iter()
        .try_fold(0usize, |total, (count, size)| {
            count
                .checked_mul(size)
                .and_then(|allocation| total.checked_add(allocation))
        });
    let bytes = bytes
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ProcessError::Allocation)?;
    Ok(child
        .resource_domain()
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, bytes))?
        .commit())
}

fn start_failure(
    error: ChildProcessStartError,
    build: SealedProcessBuild,
) -> StartPreparationFailure<ChildProcessStartError> {
    StartPreparationFailure { error, build }
}

fn abort_child_handle_batches(
    child: &Process,
    batches: &mut alloc::vec::Vec<ProcessHandleBatchReservation>,
) {
    for batch in batches.drain(..) {
        child.abort_handle_batch(batch);
    }
}

fn publish_unpublished_startup_handles(prepared: &mut PreparedChildProcessStart) {
    prepared.child_handle_batches.reverse();
    prepared.child.inner.state.with(|state| {
        if state.lifecycle.phase() != ProcessPhase::Prepared {
            process_invariant_violation();
        }
        while let Some(mut reservation) = prepared.child_handle_batches.pop() {
            reservation.require_owner(&prepared.child);
            let token = match reservation.reservation.take() {
                Some(token) => token,
                None => process_invariant_violation(),
            };
            let retired = prepared.child.inner.handles.with(|table| {
                token.publish_from_reversed(table, &mut prepared.prepared_startup_handles)
            });
            prepared.retired_batch_storage.push(retired);

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
            prepared.retired_charge_storage.push(charges);
            match reservation.scratch_charge.take() {
                Some(charge) => prepared.retired_scratch_charges.push(charge),
                None => process_invariant_violation(),
            }
        }
    });
    if !prepared.prepared_startup_handles.is_empty() {
        process_invariant_violation();
    }
    prepared.retired_batch_storage.clear();
    prepared.retired_charge_storage.clear();
    prepared.retired_scratch_charges.clear();
}

fn commit_parent_builder_replacement(
    prepared: &mut PreparedChildProcessStart,
) -> (HandleValue, InTransitCapabilities) {
    let mut source = match prepared.builder_consumption.take() {
        Some(mut consumption) => match consumption.transfer.take() {
            Some(source) => source,
            None => process_invariant_violation(),
        },
        None => process_invariant_violation(),
    };
    if source.process.id() != prepared.parent.id() {
        process_invariant_violation();
    }
    let mut destination = match prepared.parent_supervisor.take() {
        Some(destination) => destination,
        None => process_invariant_violation(),
    };
    destination.require_owner(&prepared.parent);
    let mut supervisor = match prepared.supervisor_object.take() {
        Some(supervisor) => Some(supervisor),
        None => process_invariant_violation(),
    };
    let mut retired_transfer_storage = None;
    let mut builder_batch = None;
    let mut supervisor_value = None;
    prepared.parent.inner.state.with(|state| {
        if source.moved_values.as_slice() != [prepared.builder_handle] {
            process_invariant_violation();
        }
        if !handle_charge_is_live(state, prepared.builder_handle) {
            process_invariant_violation();
        }
        let claim = match source.claim.take() {
            Some(claim) => claim,
            None => process_invariant_violation(),
        };
        let destination_token = match destination.reservation.take() {
            Some(reservation) => reservation,
            None => process_invariant_violation(),
        };
        let published_supervisor = match supervisor.take() {
            Some(supervisor) => supervisor,
            None => process_invariant_violation(),
        };
        let ((detached_builder, retired), values) = prepared.parent.inner.handles.with(|table| {
            let detached = claim.commit_with_storage(table);
            let values = destination_token.publish(table, [published_supervisor]);
            (detached, values)
        });
        builder_batch = Some(detached_builder);
        retired_transfer_storage = Some(retired);
        supervisor_value = Some(values[0]);

        for value in source.moved_values.drain(..) {
            let (charge, retired_record) = release_handle_charge(state, value);
            source.released_charges.push(charge);
            if let Some(record) = retired_record {
                source.retired_records.push(record);
            }
        }

        let mut charges = match destination.handle_charges.take() {
            Some(charges) => charges,
            None => process_invariant_violation(),
        };
        let record = match destination.record.take() {
            Some(record) => record,
            None => process_invariant_violation(),
        };
        record.state.with(|record_state| {
            if record_state.entries.len() != 1 || charges.len() != 1 {
                process_invariant_violation();
            }
            let charge = match charges.pop() {
                Some(charge) => charge,
                None => process_invariant_violation(),
            };
            let entry = match record_state.entries.get_mut(0) {
                Some(entry) => entry,
                None => process_invariant_violation(),
            };
            entry.charge = Some(charge.commit());
        });
        install_handle_charge_record(state, record);
        destination.handle_charges = Some(charges);
    });

    drop(retired_transfer_storage.take());
    drop(core::mem::take(&mut source.released_charges));
    drop(core::mem::take(&mut source.retired_records));
    drop(source.entry_charge.take());
    drop(source.scratch_charge.take());
    let storage_charge = match source.handle_charge.take() {
        Some(charge) => charge,
        None => process_invariant_violation(),
    };
    drop(destination.handle_charges.take());
    drop(destination.record.take());
    let detached_builder = match builder_batch.take() {
        Some(builder) => builder,
        None => process_invariant_violation(),
    };
    let value = match supervisor_value {
        Some(value) => value,
        None => process_invariant_violation(),
    };
    (
        value,
        InTransitCapabilities::new(detached_builder, storage_charge),
    )
}
