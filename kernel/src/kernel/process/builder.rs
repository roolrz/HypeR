// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Transactional construction policy for Native userspace processes.
//!
//! A builder owns the authority and allocation work needed to construct one
//! child. It deliberately does not expose the partially constructed Process.
//! The eventual syscall layer may therefore publish one supervisor Process
//! handle at the same commit that makes the initial Thread runnable.

use alloc::{string::String, vec::Vec};
use core::sync::atomic::{AtomicBool, Ordering};

use hyper::exec::startup::Layout as StartupStackLayout;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceDomainObject, ResourceError,
    ResourceKind,
};
use crate::kernel::capability::{
    HandleFlags, HandleInfo, HandleTransferOperation, HandleTransferRequest, HandleTransferRoute,
    HandleValue, InTransitCapabilities, PreparedHandle,
};
use crate::kernel::fs::{BootFile, BootFsError};
use crate::kernel::mm::user_space::NativeAddressSpace;
use crate::kernel::object::{
    KernelObject, ObjectCreationError, ObjectKind, ObjectPublication, TransferClass,
    object_allocation_size, private,
};
use crate::kernel::task::scheduler::CpuMask;
use hyper::sync::InterruptSpinLock;

use super::builder_input::{
    ABI_AFFINITY_WORDS, MAX_ARGUMENTS, MAX_ENVIRONMENT, MAX_STARTUP_HANDLES,
    MAX_TOTAL_STRING_BYTES, valid_argument, valid_environment, valid_name,
};
use super::builder_policy::BuilderStorable;
use super::{
    LoaderError, PreparedProcess, Process, TaskFactory, TaskGroup, TaskGroupObject, load_native,
};
const _: () = assert!(hyper::cpu::MAX_CPUS <= ABI_AFFINITY_WORDS * u64::BITS as usize);
const _: () = assert!(MAX_STARTUP_HANDLES > 0);
const MAX_USER_STARTUP_HANDLES: usize = MAX_STARTUP_HANDLES - 1;

/// Externally observable lifecycle of a one-shot process builder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessBuilderPhase {
    Open,
    Sealed,
    Started,
    Aborted,
}

/// Failure before the atomic child-publication commit.
#[derive(Debug)]
pub(crate) enum ProcessBuilderError<E> {
    Allocation,
    AlreadySealed,
    AlreadyStarted,
    Aborted,
    BootFile(BootFsError),
    Busy,
    DuplicateStartupPurpose,
    EmptyArguments,
    InvalidEnvironment,
    Image(LoaderError),
    InvalidAffinity,
    InvalidStartupPurpose,
    InvalidString,
    MissingName,
    NotSealed,
    Object(ObjectCreationError),
    Process(super::ProcessError),
    Resource(ResourceError),
    Stack(hyper::exec::startup::Error),
    StartupHandleLimit,
    StringLimit,
    Transaction(E),
    UnsupportedStartupKind,
}

/// One process-local handle requested for the child's startup record array.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StartupCapability {
    purpose: u32,
    value: HandleValue,
    requested_rights: Option<crate::kernel::authority::Rights>,
    expected_kind: Option<crate::kernel::object::ObjectKind>,
    operation: HandleTransferOperation,
}

impl StartupCapability {
    pub(crate) const fn new(
        purpose: u32,
        value: HandleValue,
        requested_rights: Option<crate::kernel::authority::Rights>,
        expected_kind: Option<crate::kernel::object::ObjectKind>,
        operation: HandleTransferOperation,
    ) -> Self {
        Self {
            purpose,
            value,
            requested_rights,
            expected_kind,
            operation,
        }
    }

    pub(crate) const fn purpose(self) -> u32 {
        self.purpose
    }

    const fn transfer_request(
        self,
        rights: crate::kernel::authority::Rights,
    ) -> HandleTransferRequest {
        HandleTransferRequest {
            value: self.value,
            offered_rights: None,
            rights,
            offered_kind: None,
            expected_kind: self.expected_kind,
            operation: self.operation,
        }
    }
}

struct LaunchStrings {
    arguments: Vec<String>,
    environment: Vec<String>,
    total_bytes: usize,
}

impl LaunchStrings {
    fn try_new() -> Result<Self, ProcessBuilderError<()>> {
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(MAX_ARGUMENTS)
            .map_err(|_| ProcessBuilderError::Allocation)?;
        let mut environment = Vec::new();
        environment
            .try_reserve_exact(MAX_ENVIRONMENT)
            .map_err(|_| ProcessBuilderError::Allocation)?;
        Ok(Self {
            arguments,
            environment,
            total_bytes: 0,
        })
    }

    fn add_argument(&mut self, value: &str) -> Result<(), ProcessBuilderError<()>> {
        if self.arguments.len() == MAX_ARGUMENTS {
            return Err(ProcessBuilderError::StringLimit);
        }
        if !valid_argument(value) {
            return Err(ProcessBuilderError::InvalidString);
        }
        let (owned, new_total) = copy_string(value, self.total_bytes)?;
        self.arguments
            .try_reserve(1)
            .map_err(|_| ProcessBuilderError::Allocation)?;
        self.arguments.push(owned);
        self.total_bytes = new_total;
        Ok(())
    }

    fn add_environment(&mut self, value: &str) -> Result<(), ProcessBuilderError<()>> {
        if self.environment.len() == MAX_ENVIRONMENT {
            return Err(ProcessBuilderError::StringLimit);
        }
        if !valid_environment(value) {
            return Err(ProcessBuilderError::InvalidEnvironment);
        }
        let (owned, new_total) = copy_string(value, self.total_bytes)?;
        self.environment
            .try_reserve(1)
            .map_err(|_| ProcessBuilderError::Allocation)?;
        self.environment.push(owned);
        self.total_bytes = new_total;
        Ok(())
    }

    fn argument_refs(&self) -> Result<Vec<&str>, ProcessBuilderError<()>> {
        string_refs(&self.arguments)
    }

    fn environment_refs(&self) -> Result<Vec<&str>, ProcessBuilderError<()>> {
        string_refs(&self.environment)
    }
}

fn copy_string(
    value: &str,
    current_total: usize,
) -> Result<(String, usize), ProcessBuilderError<()>> {
    let new_total = current_total
        .checked_add(value.len().saturating_add(1))
        .filter(|total| *total <= MAX_TOTAL_STRING_BYTES)
        .ok_or(ProcessBuilderError::StringLimit)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| ProcessBuilderError::Allocation)?;
    owned.push_str(value);
    Ok((owned, new_total))
}

fn string_refs(strings: &[String]) -> Result<Vec<&str>, ProcessBuilderError<()>> {
    let mut references = Vec::new();
    references
        .try_reserve_exact(strings.len())
        .map_err(|_| ProcessBuilderError::Allocation)?;
    references.extend(strings.iter().map(String::as_str));
    Ok(references)
}

struct BuilderPlan {
    // These canonical owners are captured only after the syscall layer has
    // resolved the corresponding typed handles and rights. The builder does
    // not retain an OperationPin beyond that operation or redo a namespace
    // lookup after construction.
    group: TaskGroup,
    domain: ResourceDomain,
    executable: &'static [u8],
    strings: LaunchStrings,
    thread_name: Option<String>,
    affinity: CpuMask,
    startup: Vec<StoredStartupCapability>,
}

/// Authority already detached from its source namespace for child startup.
///
/// A staged entry contains exactly one active capability owner. The wrapper's
/// explicit drop path makes an abandoned builder close that authority outside
/// the builder state lock instead of relying on an ambient source Process.
pub(super) struct StoredStartupCapability {
    purpose: u32,
    info: HandleInfo,
    authority: Option<InTransitCapabilities>,
}

impl StoredStartupCapability {
    fn new(purpose: u32, info: HandleInfo, authority: InTransitCapabilities) -> Self {
        if authority.len() != 1 {
            builder_invariant_violation();
        }
        Self {
            purpose,
            info,
            authority: Some(authority),
        }
    }

    pub(super) const fn purpose(&self) -> u32 {
        self.purpose
    }

    /// Immutable target metadata retained for transaction validation and
    /// future authority-free `ProcessBuilder` diagnostics.
    pub(super) const fn info(&self) -> HandleInfo {
        self.info
    }

    /// Consumes the staged entry at the child-publication commit.
    pub(super) fn take_authority(mut self) -> InTransitCapabilities {
        match self.authority.take() {
            Some(authority) => authority,
            None => builder_invariant_violation(),
        }
    }

    fn release_into(&mut self, retirement: &mut crate::kernel::object::ObjectRetirement) {
        if let Some(authority) = self.authority.take() {
            authority.release_into(retirement);
        }
    }
}

impl Drop for StoredStartupCapability {
    fn drop(&mut self) {
        let mut retirement = crate::kernel::object::ObjectRetirement::new();
        self.release_into(&mut retirement);
        retirement.drain();
    }
}

pub(super) struct SealedProcessBuild {
    child: Option<PreparedProcess>,
    plan: BuilderPlan,
    stack_layout: StartupStackLayout,
}

impl SealedProcessBuild {
    pub(super) fn child(&self) -> &PreparedProcess {
        match self.child.as_ref() {
            Some(child) => child,
            None => builder_invariant_violation(),
        }
    }

    pub(super) fn arguments(&self) -> Result<Vec<&str>, ProcessBuilderError<()>> {
        self.plan.strings.argument_refs()
    }

    pub(super) fn environment(&self) -> Result<Vec<&str>, ProcessBuilderError<()>> {
        self.plan.strings.environment_refs()
    }

    pub(super) fn argument_count(&self) -> usize {
        self.plan.strings.arguments.len()
    }

    pub(super) fn environment_count(&self) -> usize {
        self.plan.strings.environment.len()
    }

    pub(super) const fn stack_layout(&self) -> StartupStackLayout {
        self.stack_layout
    }

    pub(super) fn thread_name(&self) -> &str {
        match self.plan.thread_name.as_deref() {
            Some(name) => name,
            None => builder_invariant_violation(),
        }
    }

    pub(super) const fn affinity(&self) -> CpuMask {
        self.plan.affinity
    }

    pub(super) fn startup_capabilities(&self) -> &[StoredStartupCapability] {
        &self.plan.startup
    }

    /// Returns every linear owner to the process subsystem for commit.
    pub(super) fn into_parts(
        mut self,
    ) -> (
        PreparedProcess,
        StartupStackLayout,
        Vec<StoredStartupCapability>,
    ) {
        let child = match self.child.take() {
            Some(child) => child,
            None => builder_invariant_violation(),
        };
        (
            child,
            self.stack_layout,
            core::mem::take(&mut self.plan.startup),
        )
    }

    fn abort(mut self) {
        self.abort_inner();
    }

    fn abort_inner(&mut self) {
        if let Some(child) = self.child.take() {
            retire_unpublished_address_space(child.abort());
        }
    }
}

impl Drop for SealedProcessBuild {
    fn drop(&mut self) {
        self.abort_inner();
    }
}

enum BuilderState {
    Open(BuilderPlan),
    Sealed(SealedProcessBuild),
    Busy,
    /// Abort-vs-start arbitration has committed; publication is inevitable.
    CommitAuthorized,
    Started,
    Aborted,
}

/// Backend contract for the single child-publication commit.
///
/// `prepare_start` must perform every fallible operation: destination
/// reservations, initial-stack encoding and copy, dormant Thread construction,
/// scheduler admission, builder-handle consumption preparation, and
/// supervisor-handle reservation. On failure it returns the complete
/// `SealedProcessBuild`, including every builder-owned startup authority, while
/// the child remains invisible.
///
/// `commit_start` is the sole publication point and must be infallible. It
/// publishes the child Process, installs startup handles, emits only the
/// supervisor Process authority to the parent, and makes the initial Thread
/// runnable in that order under the transaction's serialization contract. It
/// runs only after abort-vs-start arbitration and never under the builder state
/// lock. Destruction and zero-handle callbacks remain deferred to
/// `finish_start`.
pub(super) trait ProcessStartTransaction {
    type Error;
    type Prepared;
    type Committed;
    type Output;

    #[expect(
        clippy::result_large_err,
        reason = "the error must retain the complete linear build owner for allocation-free rollback"
    )]
    fn prepare_start(
        &mut self,
        build: SealedProcessBuild,
    ) -> Result<Self::Prepared, StartPreparationFailure<Self::Error>>;

    fn commit_start(&mut self, prepared: Self::Prepared) -> Self::Committed;

    /// Releases detached authority and transaction storage after the builder
    /// state lock has published `Started`.
    fn finish_start(&mut self, committed: Self::Committed) -> Self::Output;

    /// Rolls every prepared reservation back without allocation or failure.
    fn cancel_start(&mut self, prepared: Self::Prepared) -> SealedProcessBuild;
}

pub(super) struct StartPreparationFailure<E> {
    pub(super) error: E,
    pub(super) build: SealedProcessBuild,
}

/// Linear owner of one not-yet-visible child construction.
pub(crate) struct ProcessBuilder {
    state: InterruptSpinLock<Option<BuilderState>, crate::hal::irq::LocalMask>,
    abort_requested: AtomicBool,
    _object_charge: CommittedCharge,
}

impl ProcessBuilder {
    pub(crate) fn try_publication(
        _factory: &TaskFactory,
        group: &TaskGroupObject,
        domain: &ResourceDomainObject,
        executable: &BootFile,
    ) -> Result<ObjectPublication<Self>, ProcessBuilderError<()>> {
        let executable = executable
            .executable_bytes()
            .map_err(ProcessBuilderError::BootFile)?;
        let object_charge = reserve_builder_charge(domain.domain())?;
        let strings = LaunchStrings::try_new()?;
        let mut startup = Vec::new();
        startup
            .try_reserve_exact(MAX_USER_STARTUP_HANDLES)
            .map_err(|_| ProcessBuilderError::Allocation)?;
        let builder = Self {
            state: InterruptSpinLock::new(Some(BuilderState::Open(BuilderPlan {
                group: group.group().clone(),
                domain: domain.domain().clone(),
                executable,
                strings,
                thread_name: None,
                affinity: CpuMask::ALL,
                startup,
            }))),
            abort_requested: AtomicBool::new(false),
            _object_charge: object_charge,
        };
        ObjectPublication::try_new(builder).map_err(ProcessBuilderError::Object)
    }

    pub(crate) fn phase(&self) -> Result<ProcessBuilderPhase, ProcessBuilderError<()>> {
        self.state.with(|state| match state.as_ref() {
            Some(BuilderState::Open(_)) => Ok(ProcessBuilderPhase::Open),
            Some(BuilderState::Sealed(_)) => Ok(ProcessBuilderPhase::Sealed),
            Some(BuilderState::Busy) => Err(ProcessBuilderError::Busy),
            Some(BuilderState::CommitAuthorized) => Err(ProcessBuilderError::Busy),
            Some(BuilderState::Started) => Ok(ProcessBuilderPhase::Started),
            Some(BuilderState::Aborted) => Ok(ProcessBuilderPhase::Aborted),
            None => builder_invariant_violation(),
        })
    }

    pub(crate) fn add_startup_capability(
        &self,
        source: &Process,
        capability: StartupCapability,
    ) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = self.take_open_plan()?;
        let info = match validate_startup_capability(&mut plan, capability, source) {
            Ok(info) => info,
            Err(error) => return self.finish_open_operation(plan, Err(error)),
        };
        let requested_rights = capability.requested_rights.unwrap_or(info.rights);
        let request = capability.transfer_request(requested_rights);
        let transfer = match source.prepare_handle_transfer(
            core::slice::from_ref(&request),
            None,
            Some(ObjectKind::PROCESS_BUILDER),
            HandleTransferRoute::StagedStartup,
        ) {
            Ok(transfer) => transfer,
            Err(error) => {
                return self.finish_open_operation(plan, Err(ProcessBuilderError::Process(error)));
            }
        };
        let authority = match transfer.commit() {
            Ok(authority) => authority,
            Err(failure) => {
                let error = failure.error;
                failure.transfer.rollback();
                return self.finish_open_operation(plan, Err(ProcessBuilderError::Process(error)));
            }
        };
        // Capacity was reserved before the source transaction. From this point
        // onward insertion cannot fail, so successful MOVE ownership is never
        // reported to userspace as a recoverable failure.
        plan.startup.push(StoredStartupCapability::new(
            capability.purpose,
            HandleInfo {
                rights: requested_rights,
                ..info
            },
            authority,
        ));
        self.finish_committed_add(plan)
    }

    pub(crate) fn set_name(&self, name: &str) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = self.take_open_plan()?;
        let result = copy_name(name).map(|name| {
            plan.thread_name = Some(name);
        });
        self.finish_open_operation(plan, result)
    }

    pub(crate) fn add_argument(&self, value: &str) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = self.take_open_plan()?;
        let result = plan.strings.add_argument(value);
        self.finish_open_operation(plan, result)
    }

    pub(crate) fn add_environment(&self, value: &str) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = self.take_open_plan()?;
        let result = plan.strings.add_environment(value);
        self.finish_open_operation(plan, result)
    }

    pub(crate) fn set_affinity(&self, affinity: CpuMask) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = self.take_open_plan()?;
        if crate::kernel::task::scheduler::validate_affinity(affinity).is_err() {
            return self.finish_open_operation(plan, Err(ProcessBuilderError::InvalidAffinity));
        }
        plan.affinity = affinity;
        self.finish_open_operation(plan, Ok(()))
    }

    /// Completes all image-loading work while the child remains unpublished.
    ///
    /// Every error leaves the builder Open and reusable. Sealing claims no
    /// source handles, creates no Thread and publishes no Process identity.
    pub(crate) fn seal(&self) -> Result<(), ProcessBuilderError<()>> {
        let plan = self.take_open_plan()?;
        let (prepared, stack_layout) = match prepare_sealed_process(&plan) {
            Ok(prepared) => prepared,
            Err(error) => return self.finish_open_operation(plan, Err(error)),
        };
        let mut sealed = Some(SealedProcessBuild {
            child: Some(prepared),
            plan,
            stack_layout,
        });
        let aborted = self.state.with(|state| {
            require_busy_state(state);
            if self.abort_requested.load(Ordering::Acquire) {
                *state = Some(BuilderState::Aborted);
                true
            } else {
                *state = Some(BuilderState::Sealed(match sealed.take() {
                    Some(sealed) => sealed,
                    None => builder_invariant_violation(),
                }));
                false
            }
        });
        if aborted {
            drop(sealed.take());
            return Err(ProcessBuilderError::Aborted);
        }
        Ok(())
    }

    /// Executes one prevalidated, allocation-free publication commit.
    ///
    /// The transaction remains locally owned across abort arbitration and the
    /// single publication commit.
    fn start_with<T: ProcessStartTransaction>(
        &self,
        transaction: &mut T,
    ) -> Result<T::Output, ProcessBuilderError<T::Error>> {
        let build = self.take_sealed_build()?;
        let prepared = match transaction.prepare_start(build) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let mut build = Some(failure.build);
                let result = self.state.with(|state| {
                    require_busy_state(state);
                    if self.abort_requested.load(Ordering::Acquire) {
                        *state = Some(BuilderState::Aborted);
                        Err(ProcessBuilderError::Aborted)
                    } else {
                        *state = Some(BuilderState::Sealed(match build.take() {
                            Some(build) => build,
                            None => builder_invariant_violation(),
                        }));
                        Err(ProcessBuilderError::Transaction(failure.error))
                    }
                });
                drop(build.take());
                return result;
            }
        };
        let mut prepared = Some(prepared);
        let authorized = self.state.with(|state| {
            require_busy_state(state);
            if self.abort_requested.load(Ordering::Acquire) {
                *state = Some(BuilderState::Aborted);
                return false;
            }
            *state = Some(BuilderState::CommitAuthorized);
            true
        });
        if !authorized {
            transaction
                .cancel_start(match prepared.take() {
                    Some(prepared) => prepared,
                    None => builder_invariant_violation(),
                })
                .abort();
            return Err(ProcessBuilderError::Aborted);
        }
        let committed = transaction.commit_start(match prepared.take() {
            Some(prepared) => prepared,
            None => builder_invariant_violation(),
        });
        self.state.with(|state| {
            require_commit_authorized_state(state);
            *state = Some(BuilderState::Started);
        });
        Ok(transaction.finish_start(committed))
    }

    pub(crate) fn abort(&self) -> Result<(), ProcessBuilderError<()>> {
        let mut abandoned_plan = None;
        let mut retired = None;
        let result = self.state.with(|state| {
            let current = match state.take() {
                Some(current) => current,
                None => builder_invariant_violation(),
            };
            match current {
                BuilderState::Open(plan) => {
                    self.abort_requested.store(true, Ordering::Release);
                    abandoned_plan = Some(plan);
                    *state = Some(BuilderState::Aborted);
                    Ok(())
                }
                BuilderState::Sealed(build) => {
                    self.abort_requested.store(true, Ordering::Release);
                    retired = Some(build);
                    *state = Some(BuilderState::Aborted);
                    Ok(())
                }
                BuilderState::Busy => {
                    self.abort_requested.store(true, Ordering::Release);
                    *state = Some(BuilderState::Busy);
                    Ok(())
                }
                BuilderState::CommitAuthorized => {
                    *state = Some(BuilderState::CommitAuthorized);
                    Err(ProcessBuilderError::AlreadyStarted)
                }
                BuilderState::Started => {
                    *state = Some(BuilderState::Started);
                    Err(ProcessBuilderError::AlreadyStarted)
                }
                BuilderState::Aborted => {
                    self.abort_requested.store(true, Ordering::Release);
                    *state = Some(BuilderState::Aborted);
                    Err(ProcessBuilderError::Aborted)
                }
            }
        });
        drop(abandoned_plan.take());
        drop(retired.take());
        result
    }

    fn take_open_plan(&self) -> Result<BuilderPlan, ProcessBuilderError<()>> {
        self.state.with(|state| {
            if self.abort_requested.load(Ordering::Acquire) {
                return Err(ProcessBuilderError::Aborted);
            }
            let current = match state.take() {
                Some(current) => current,
                None => builder_invariant_violation(),
            };
            match current {
                BuilderState::Open(plan) => {
                    *state = Some(BuilderState::Busy);
                    Ok(plan)
                }
                BuilderState::Sealed(build) => {
                    *state = Some(BuilderState::Sealed(build));
                    Err(ProcessBuilderError::AlreadySealed)
                }
                BuilderState::Busy => {
                    *state = Some(BuilderState::Busy);
                    Err(ProcessBuilderError::Busy)
                }
                BuilderState::CommitAuthorized => {
                    *state = Some(BuilderState::CommitAuthorized);
                    Err(ProcessBuilderError::AlreadyStarted)
                }
                BuilderState::Started => {
                    *state = Some(BuilderState::Started);
                    Err(ProcessBuilderError::AlreadyStarted)
                }
                BuilderState::Aborted => {
                    *state = Some(BuilderState::Aborted);
                    Err(ProcessBuilderError::Aborted)
                }
            }
        })
    }

    fn finish_open_operation(
        &self,
        plan: BuilderPlan,
        result: Result<(), ProcessBuilderError<()>>,
    ) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = Some(plan);
        let outcome = self.state.with(|state| {
            require_busy_state(state);
            if self.abort_requested.load(Ordering::Acquire) {
                *state = Some(BuilderState::Aborted);
                return Err(ProcessBuilderError::Aborted);
            }
            *state = Some(BuilderState::Open(match plan.take() {
                Some(plan) => plan,
                None => builder_invariant_violation(),
            }));
            result
        });
        drop(plan.take());
        outcome
    }

    /// Completes an add whose source-handle transfer has already committed.
    ///
    /// A concurrent final close or explicit abort orders after that commit:
    /// both operations succeed and the abort releases the newly contained
    /// authority. Returning `Aborted` here would falsely promise that a MOVE
    /// source remained unchanged.
    fn finish_committed_add(&self, plan: BuilderPlan) -> Result<(), ProcessBuilderError<()>> {
        let mut plan = Some(plan);
        let aborted = self.state.with(|state| {
            require_busy_state(state);
            if self.abort_requested.load(Ordering::Acquire) {
                *state = Some(BuilderState::Aborted);
                true
            } else {
                *state = Some(BuilderState::Open(match plan.take() {
                    Some(plan) => plan,
                    None => builder_invariant_violation(),
                }));
                false
            }
        });
        if aborted {
            drop(plan.take());
        }
        Ok(())
    }

    fn take_sealed_build<E>(&self) -> Result<SealedProcessBuild, ProcessBuilderError<E>> {
        self.state.with(|state| {
            if self.abort_requested.load(Ordering::Acquire) {
                return Err(ProcessBuilderError::Aborted);
            }
            let current = match state.take() {
                Some(current) => current,
                None => builder_invariant_violation(),
            };
            match current {
                BuilderState::Sealed(build) => {
                    *state = Some(BuilderState::Busy);
                    Ok(build)
                }
                BuilderState::Open(plan) => {
                    *state = Some(BuilderState::Open(plan));
                    Err(ProcessBuilderError::NotSealed)
                }
                BuilderState::Busy => {
                    *state = Some(BuilderState::Busy);
                    Err(ProcessBuilderError::Busy)
                }
                BuilderState::CommitAuthorized => {
                    *state = Some(BuilderState::CommitAuthorized);
                    Err(ProcessBuilderError::AlreadyStarted)
                }
                BuilderState::Started => {
                    *state = Some(BuilderState::Started);
                    Err(ProcessBuilderError::AlreadyStarted)
                }
                BuilderState::Aborted => {
                    *state = Some(BuilderState::Aborted);
                    Err(ProcessBuilderError::Aborted)
                }
            }
        })
    }
}

/// Creates and publishes one builder from independently checked authorities.
pub(crate) fn create_process_builder(
    caller: &Process,
    factory: HandleValue,
    group: HandleValue,
    domain: HandleValue,
    executable: HandleValue,
) -> Result<HandleValue, ProcessBuilderError<()>> {
    let factory = caller
        .resolve_handle::<TaskFactory>(factory, crate::kernel::authority::Rights::CREATE_PROCESS)
        .map_err(ProcessBuilderError::Process)?;
    let group = caller
        .resolve_handle::<TaskGroupObject>(
            group,
            crate::kernel::authority::Rights::TASK_GROUP_ATTACH_PROCESS,
        )
        .map_err(ProcessBuilderError::Process)?;
    let domain = caller
        .resolve_handle::<ResourceDomainObject>(
            domain,
            crate::kernel::authority::Rights::RESOURCE_DOMAIN_SPONSOR,
        )
        .map_err(ProcessBuilderError::Process)?;
    let executable = caller
        .resolve_handle::<BootFile>(executable, crate::kernel::authority::Rights::EXECUTE)
        .map_err(ProcessBuilderError::Process)?;

    let reservation = caller
        .reserve_handles::<1>()
        .map_err(ProcessBuilderError::Process)?;
    let publication = match ProcessBuilder::try_publication(
        factory.object(),
        group.object(),
        domain.object(),
        executable.object(),
    ) {
        Ok(publication) => publication,
        Err(error) => {
            caller.abort_handles(reservation);
            return Err(error);
        }
    };
    let prepared = match PreparedHandle::try_from_new_object(
        publication,
        <ProcessBuilder as KernelObject>::SUPPORTED_RIGHTS,
        HandleFlags::NONE,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            caller.abort_handles(reservation);
            return Err(ProcessBuilderError::Process(error.into()));
        }
    };
    match caller.publish_handles(reservation, [prepared]) {
        Ok([value]) => Ok(value),
        Err(failure) => Err(ProcessBuilderError::Process(failure.error)),
    }
}

/// Starts a builder through the process-owned consume-and-publish transaction.
///
/// The ABI adapter supplies only the caller namespace and numeric handle. This
/// boundary binds the resolved object KOID to the reversible handle claim and
/// keeps transaction internals out of syscall dispatch.
pub(crate) fn start_process_builder(
    caller: &Process,
    value: HandleValue,
) -> Result<
    super::owner::StartedChildProcess,
    ProcessBuilderError<super::owner::ChildProcessStartError>,
> {
    let resolved = caller
        .resolve_handle::<ProcessBuilder>(value, crate::kernel::authority::Rights::START)
        .map_err(ProcessBuilderError::Process)?;
    let mut coordinator =
        super::owner::ProcessStartCoordinator::new(caller.clone(), value, resolved.koid());
    resolved.object().start_with(&mut coordinator)
}

/// Aborts and consumes exactly the builder authority named by `value`.
///
/// Preparation is reversible, so every builder error leaves the caller's
/// handle unchanged. A successful abort is followed only by the infallible
/// handle-consumption commit and deferred authority release.
pub(crate) fn abort_process_builder(
    caller: &Process,
    value: HandleValue,
) -> Result<(), ProcessBuilderError<()>> {
    let resolved = caller
        .resolve_handle::<ProcessBuilder>(value, crate::kernel::authority::Rights::REQUEST_STOP)
        .map_err(ProcessBuilderError::Process)?;
    let consumption = caller
        .prepare_handle_consumption(
            value,
            crate::kernel::authority::Rights::REQUEST_STOP,
            ObjectKind::PROCESS_BUILDER,
            resolved.koid(),
        )
        .map_err(ProcessBuilderError::Process)?;
    match resolved.object().abort() {
        Ok(()) => {
            consumption.commit().release();
            Ok(())
        }
        Err(error) => {
            consumption.rollback();
            Err(error)
        }
    }
}

impl private::Sealed for ProcessBuilder {}
impl private::UserExportable for ProcessBuilder {}

impl KernelObject for ProcessBuilder {
    const KIND: ObjectKind = ObjectKind::PROCESS_BUILDER;
    const SUPPORTED_RIGHTS: crate::kernel::authority::Rights =
        crate::kernel::authority::Rights::TRANSFER
            .union(crate::kernel::authority::Rights::INSPECT)
            .union(crate::kernel::authority::Rights::WRITE)
            .union(crate::kernel::authority::Rights::START)
            .union(crate::kernel::authority::Rights::REQUEST_STOP);
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;

    fn on_zero_active_handles(&self, _retirement: &mut crate::kernel::object::ObjectRetirement) {
        self.abort_requested.store(true, Ordering::Release);
    }
}

fn require_busy_state(state: &Option<BuilderState>) {
    if !matches!(state, Some(BuilderState::Busy)) {
        builder_invariant_violation();
    }
}

fn require_commit_authorized_state(state: &Option<BuilderState>) {
    if !matches!(state, Some(BuilderState::CommitAuthorized)) {
        builder_invariant_violation();
    }
}

fn copy_name(name: &str) -> Result<String, ProcessBuilderError<()>> {
    if !valid_name(name) {
        return Err(ProcessBuilderError::InvalidString);
    }
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| ProcessBuilderError::Allocation)?;
    owned.push_str(name);
    Ok(owned)
}

fn prepare_sealed_process(
    plan: &BuilderPlan,
) -> Result<(PreparedProcess, StartupStackLayout), ProcessBuilderError<()>> {
    if plan.thread_name.is_none() {
        return Err(ProcessBuilderError::MissingName);
    }
    if plan.strings.arguments.is_empty() {
        return Err(ProcessBuilderError::EmptyArguments);
    }
    let reference_count = plan
        .strings
        .arguments
        .len()
        .checked_add(plan.strings.environment.len())
        .ok_or(ProcessBuilderError::Allocation)?;
    let reference_bytes = reference_count
        .checked_mul(core::mem::size_of::<&str>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ProcessBuilderError::Allocation)?;
    let _reference_charge = plan
        .domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, reference_bytes))
        .map_err(ProcessBuilderError::Resource)?
        .commit();
    let arguments = plan.strings.argument_refs()?;
    let environment = plan.strings.environment_refs()?;
    let startup_handle_count = plan
        .startup
        .len()
        .checked_add(1)
        .ok_or(ProcessBuilderError::StartupHandleLimit)?;
    let stack_layout = StartupStackLayout::try_new(
        super::INITIAL_STACK_TOP,
        &arguments,
        &environment,
        startup_handle_count,
    )
    .map_err(ProcessBuilderError::Stack)?;
    let domain = plan.domain.clone();
    let loaded = load_native(plan.executable, domain.clone(), stack_layout)
        .map_err(ProcessBuilderError::Image)?;
    let prepared = match PreparedProcess::try_new(
        loaded.image,
        plan.group.clone(),
        domain,
        loaded.address_space,
    ) {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (error, address_space) = failure.into_parts();
            retire_unpublished_address_space(address_space);
            return Err(ProcessBuilderError::Process(error));
        }
    };
    Ok((prepared, stack_layout))
}

fn validate_startup_capability(
    plan: &mut BuilderPlan,
    capability: StartupCapability,
    source: &Process,
) -> Result<HandleInfo, ProcessBuilderError<()>> {
    if capability.purpose == 0 || capability.purpose == child_root_vmar_purpose() {
        return Err(ProcessBuilderError::InvalidStartupPurpose);
    }
    // The coordinator-minted child ROOT_VMAR occupies the final ABI slot.
    if plan.startup.len() >= MAX_USER_STARTUP_HANDLES {
        return Err(ProcessBuilderError::StartupHandleLimit);
    }
    if plan
        .startup
        .iter()
        .any(|present| present.purpose == capability.purpose)
    {
        return Err(ProcessBuilderError::DuplicateStartupPurpose);
    }
    let info = source
        .handle_info(capability.value, crate::kernel::authority::Rights::NONE)
        .map_err(ProcessBuilderError::Process)?;
    if !BuilderStorable::permits_kind_id(info.kind.get()) {
        return Err(ProcessBuilderError::UnsupportedStartupKind);
    }
    plan.startup
        .try_reserve(1)
        .map_err(|_| ProcessBuilderError::Allocation)?;
    Ok(info)
}

const fn child_root_vmar_purpose() -> u32 {
    const PURPOSE: u64 = hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR;
    assert!(PURPOSE <= u32::MAX as u64);
    PURPOSE as u32
}

fn reserve_builder_charge(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, ProcessBuilderError<()>> {
    let bytes = object_allocation_size::<ProcessBuilder>()
        .and_then(|value| {
            value.checked_add(
                (MAX_ARGUMENTS + MAX_ENVIRONMENT).checked_mul(core::mem::size_of::<String>())?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                MAX_USER_STARTUP_HANDLES
                    .checked_mul(core::mem::size_of::<StoredStartupCapability>())?,
            )
        })
        .and_then(|value| value.checked_add(MAX_TOTAL_STRING_BYTES))
        .and_then(|value| value.checked_add(super::builder_input::MAX_NAME_BYTES))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ProcessBuilderError::Allocation)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )
        .map_err(ProcessBuilderError::Resource)?
        .commit())
}

fn retire_unpublished_address_space(
    address_space: hyper::mm::UniqueFallibleArc<NativeAddressSpace>,
) {
    if let Err(failure) = NativeAddressSpace::retire(address_space) {
        let (error, retained) = failure.into_parts();
        crate::pr_err!(
            "HypeR: retaining an aborted ProcessBuilder address space after cleanup error: {error:?}"
        );
        drop(retained);
    }
}

#[cold]
fn builder_invariant_violation() -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: ProcessBuilder linear-state invariant violated"
    ))
}
