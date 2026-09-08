// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process capability coordinators for userspace-managed virtual machines.

use crate::kernel::accounting::ResourceDomainObject;
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle, Rights};
use crate::kernel::mm::user_space::VmoObject;
use crate::kernel::object::{KernelObject, ObjectPublication};
use crate::kernel::process::{Process, ProcessError};

use super::objects::{
    Error as ObjectError, PendingVirtualMachine, VirtualCpuBootstrap, VirtualCpuObject,
    VirtualCpuSnapshot, VirtualMachineConfiguration, VirtualMachineCreationAuthority,
    VirtualMachineCreationLease, VirtualMachineObject, VirtualMachineSnapshot,
};

/// ABI-independent outcome classes exposed by the VM service boundary.
///
/// VM construction internals remain private to this module. Callers choose
/// their own wire or policy representation from these stable semantic classes
/// instead of coupling to registry, scheduler, or memory implementation enums.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    NotSupported,
    InvalidArgument,
    BadHandle,
    AccessDenied,
    Busy,
    BadState,
    ResourceLimit,
    NoMemory,
    Fault,
    Internal,
}

impl From<ObjectError> for Error {
    fn from(error: ObjectError) -> Self {
        classify_object_error(error)
    }
}

impl From<ProcessError> for Error {
    fn from(error: ProcessError) -> Self {
        classify_process_error(error)
    }
}

const fn classify_object_error(error: ObjectError) -> Error {
    match error {
        ObjectError::Allocation => Error::NoMemory,
        ObjectError::BadState => Error::BadState,
        ObjectError::InvalidConfiguration => Error::InvalidArgument,
        ObjectError::Memory(error) => classify_memory_object_error(error),
        ObjectError::Object(error) => classify_object_creation_error(error),
        ObjectError::Resource(error) => classify_resource_error(error),
        ObjectError::Scheduler(error) => classify_scheduler_error(error),
        ObjectError::UnsupportedArchitecture => Error::NotSupported,
        ObjectError::Registry(error) => classify_registry_error(error),
        ObjectError::MemoryLayout(error) => classify_guest_memory_error(error),
        ObjectError::VirtualDevice(_) | ObjectError::VirtualInterrupt(_) => Error::Internal,
        ObjectError::VirtualSerial(error) => match error {
            super::virtual_serial::Error::Allocation => Error::NoMemory,
            super::virtual_serial::Error::AllocationSize => Error::Internal,
            super::virtual_serial::Error::Disconnected => Error::BadState,
            super::virtual_serial::Error::Resource(error) => classify_resource_error(error),
            super::virtual_serial::Error::WouldBlock => Error::Busy,
        },
    }
}

const fn classify_process_error(error: ProcessError) -> Error {
    match error {
        ProcessError::Allocation => Error::NoMemory,
        ProcessError::Handle(error) => classify_handle_error(error),
        ProcessError::Object(error) => classify_object_creation_error(error),
        ProcessError::Lifecycle(_) | ProcessError::AddressSpaceReferenced => Error::BadState,
        ProcessError::Resource(error) => classify_resource_error(error),
        ProcessError::Scheduler(error) => classify_scheduler_error(error),
        ProcessError::TaskGroup(error) => classify_task_group_error(error),
        ProcessError::UserEntry(_) => Error::NotSupported,
        ProcessError::UserMemory(error) => classify_machine_error(error),
    }
}

const fn classify_handle_error(error: crate::kernel::capability::HandleError) -> Error {
    use crate::kernel::capability::HandleError;

    match error {
        HandleError::Allocation => Error::NoMemory,
        HandleError::InvalidHandle | HandleError::WrongObjectType => Error::BadHandle,
        HandleError::Busy | HandleError::OutstandingReservation => Error::Busy,
        HandleError::AccessDenied => Error::AccessDenied,
        HandleError::UnsupportedRights | HandleError::UnsupportedFlags => Error::InvalidArgument,
        HandleError::UnsupportedTransfer => Error::NotSupported,
        HandleError::ObjectRetired | HandleError::TableRetired => Error::BadState,
        HandleError::ActiveHandleLimit
        | HandleError::ReservationIdExhausted
        | HandleError::ReservationTooLarge
        | HandleError::TableFull => Error::ResourceLimit,
        HandleError::ObjectAlreadyActive | HandleError::EmptyReservation => Error::Internal,
    }
}

const fn classify_object_creation_error(
    error: crate::kernel::object::ObjectCreationError,
) -> Error {
    match error {
        crate::kernel::object::ObjectCreationError::Allocation => Error::NoMemory,
        crate::kernel::object::ObjectCreationError::KoidExhausted
        | crate::kernel::object::ObjectCreationError::RegistrationExhausted => Error::ResourceLimit,
    }
}

const fn classify_resource_error(error: crate::kernel::accounting::ResourceError) -> Error {
    use crate::kernel::accounting::ResourceError;

    match error {
        ResourceError::Allocation => Error::NoMemory,
        ResourceError::HierarchyTooDeep
        | ResourceError::TooManyChargeDimensions
        | ResourceError::LimitExceeded { .. }
        | ResourceError::UsageOverflow { .. }
        | ResourceError::ChildCountExhausted
        | ResourceError::DomainIdExhausted => Error::ResourceLimit,
        ResourceError::DomainInactive(_)
        | ResourceError::OutstandingUsage
        | ResourceError::ActiveChildren
        | ResourceError::RetirementNotStarted => Error::BadState,
        ResourceError::EmptyCharge | ResourceError::LimitBelowUsage { .. } => Error::Internal,
    }
}

const fn classify_task_group_error(error: crate::kernel::process::TaskGroupError) -> Error {
    match error {
        crate::kernel::process::TaskGroupError::Allocation => Error::NoMemory,
        crate::kernel::process::TaskGroupError::CounterOverflow
        | crate::kernel::process::TaskGroupError::GenerationExhausted => Error::ResourceLimit,
        crate::kernel::process::TaskGroupError::Inactive
        | crate::kernel::process::TaskGroupError::MembersRemain => Error::BadState,
        crate::kernel::process::TaskGroupError::Resource(error) => classify_resource_error(error),
    }
}

const fn classify_scheduler_error(error: crate::kernel::task::scheduler::Error) -> Error {
    use crate::kernel::task::scheduler::Error as SchedulerError;

    match error {
        SchedulerError::Allocation => Error::NoMemory,
        SchedulerError::ThreadLimit
        | SchedulerError::IdentifierExhausted
        | SchedulerError::WaitGenerationExhausted => Error::ResourceLimit,
        SchedulerError::EmptyCpuAffinity
        | SchedulerError::NoRegisteredCpuInAffinity
        | SchedulerError::InvalidCpuIndex
        | SchedulerError::CpuNotAllowed => Error::InvalidArgument,
        SchedulerError::Thread(error) => classify_thread_error(error),
        SchedulerError::NotInitialized
        | SchedulerError::AlreadyInitialized
        | SchedulerError::CurrentThreadMissing
        | SchedulerError::ThreadNotFound
        | SchedulerError::TerminatedThread
        | SchedulerError::ThreadBlocked
        | SchedulerError::ThreadAlreadyQueued
        | SchedulerError::QueueCorrupted
        | SchedulerError::CannotBlockIdle
        | SchedulerError::CannotSleepWithInterruptsMasked
        | SchedulerError::CannotSleepWithPreemptionDisabled
        | SchedulerError::IrqTailRequiresInterruptsMasked
        | SchedulerError::UserRunRequiresInterruptsEnabled
        | SchedulerError::InvalidThreadState
        | SchedulerError::IdleThreadAlreadyInstalled
        | SchedulerError::InvalidIdleTransition
        | SchedulerError::CpuAlreadyRegistered
        | SchedulerError::CpuNotRegistered
        | SchedulerError::MigrationUnsupported
        | SchedulerError::MigrationInProgress
        | SchedulerError::ThreadTransitionInProgress
        | SchedulerError::InvalidWaitRegistration
        | SchedulerError::MigrationBlockedByCpuLocalWait
        | SchedulerError::PreemptionUnavailable
        | SchedulerError::PreemptionInvariant
        | SchedulerError::VmEntryUnavailable => Error::Internal,
    }
}

const fn classify_thread_error(error: crate::kernel::task::thread::Error) -> Error {
    match error {
        crate::kernel::task::thread::Error::Allocation
        | crate::kernel::task::thread::Error::ObjectAllocation => Error::NoMemory,
        crate::kernel::task::thread::Error::ObjectIdentityExhausted
        | crate::kernel::task::thread::Error::ObjectRegistrationExhausted => Error::ResourceLimit,
        crate::kernel::task::thread::Error::NameTooLong
        | crate::kernel::task::thread::Error::InvalidPlacement
        | crate::kernel::task::thread::Error::VirtualInterrupt(_) => Error::Internal,
    }
}

const fn classify_memory_object_error(
    error: crate::kernel::mm::user_space::MemoryObjectError,
) -> Error {
    use crate::kernel::mm::user_space::MemoryObjectError;

    match error {
        MemoryObjectError::AlreadyPublished | MemoryObjectError::WrongVariant => Error::BadState,
        MemoryObjectError::AllocationSize => Error::Internal,
        MemoryObjectError::Object(error) => classify_object_creation_error(error),
        MemoryObjectError::Resource(error) => classify_resource_error(error),
        MemoryObjectError::AddressSpace(error) => classify_address_space_error(error),
        MemoryObjectError::Vmo(error) => classify_vmo_error(error),
    }
}

const fn classify_vmo_error(
    error: crate::kernel::mm::user_space::VmoError<
        crate::kernel::mm::user_space::KernelPageError,
        crate::kernel::accounting::ResourceError,
    >,
) -> Error {
    use crate::kernel::mm::user_space::VmoError;

    match error {
        VmoError::Account(error) => classify_resource_error(error),
        VmoError::Allocation => Error::NoMemory,
        VmoError::Backend(error) => classify_page_error(error),
        VmoError::Busy => Error::Busy,
        VmoError::InvalidRange | VmoError::SizeOverflow => Error::InvalidArgument,
    }
}

const fn classify_address_space_error(
    error: crate::kernel::mm::user_space::AddressSpaceError<
        crate::kernel::mm::user_space::KernelPageError,
        crate::kernel::accounting::ResourceError,
    >,
) -> Error {
    use crate::kernel::mm::user_space::AddressSpaceError;

    match error {
        AddressSpaceError::Account(error) => classify_resource_error(error),
        AddressSpaceError::Allocation => Error::NoMemory,
        AddressSpaceError::Busy => Error::Busy,
        AddressSpaceError::Backend(error) => classify_page_error(error),
        AddressSpaceError::IdentityExhausted => Error::ResourceLimit,
        AddressSpaceError::BackingNotResident
        | AddressSpaceError::EmptyRange
        | AddressSpaceError::InvalidAddressSpace
        | AddressSpaceError::InvalidPermissions
        | AddressSpaceError::InvalidRange
        | AddressSpaceError::NotMapped
        | AddressSpaceError::Overlap
        | AddressSpaceError::ReadDenied
        | AddressSpaceError::SizeMismatch
        | AddressSpaceError::SizeOverflow
        | AddressSpaceError::StaleMapping
        | AddressSpaceError::StaleTransaction
        | AddressSpaceError::StaleVmar
        | AddressSpaceError::WriteDenied
        | AddressSpaceError::WritableExecutableBacking => Error::Fault,
    }
}

const fn classify_page_error(error: crate::kernel::mm::user_space::KernelPageError) -> Error {
    match error {
        crate::kernel::mm::user_space::KernelPageError::Allocation(_) => Error::NoMemory,
        crate::kernel::mm::user_space::KernelPageError::AddressOverflow
        | crate::kernel::mm::user_space::KernelPageError::Range => Error::Fault,
        crate::kernel::mm::user_space::KernelPageError::MissingLinearMap
        | crate::kernel::mm::user_space::KernelPageError::Unsupported => Error::Internal,
    }
}

const fn classify_machine_error(error: crate::kernel::mm::user_space::MachineError) -> Error {
    use crate::kernel::mm::user_space::MachineError;

    match error {
        MachineError::Allocation | MachineError::Page(_) => Error::NoMemory,
        MachineError::Address(_) | MachineError::InvalidRange | MachineError::SizeOverflow => {
            Error::Fault
        }
        MachineError::Logical(error) => classify_address_space_error(error),
        MachineError::Resource(error) => classify_resource_error(error),
        MachineError::Residency(_) | MachineError::Transport => Error::Busy,
        MachineError::Unsupported => Error::NotSupported,
        MachineError::Hal(_) | MachineError::Identifier(_) | MachineError::Vmo(_) => {
            Error::Internal
        }
    }
}

const fn classify_registry_error(error: super::registry::Error) -> Error {
    match error {
        #[cfg(feature = "kernel-self-test")]
        super::registry::Error::AdministrativeStopUnsupported => Error::NotSupported,
        super::registry::Error::Allocation => Error::NoMemory,
        super::registry::Error::IdentityExhausted | super::registry::Error::RegistryFull => {
            Error::ResourceLimit
        }
        super::registry::Error::Resource(error) => classify_resource_error(error),
        super::registry::Error::EndpointClosed
        | super::registry::Error::InvalidReservation
        | super::registry::Error::NotInstalled
        | super::registry::Error::Quiescing
        | super::registry::Error::Scheduler
        | super::registry::Error::StaleIdentity
        | super::registry::Error::UnknownVcpu => Error::Internal,
    }
}

const fn classify_guest_memory_error(error: super::memory::Error) -> Error {
    match error {
        super::memory::Error::AddressOverflow | super::memory::Error::InvalidRange => {
            Error::InvalidArgument
        }
        super::memory::Error::Allocation(_) | super::memory::Error::MetadataAllocation => {
            Error::NoMemory
        }
        super::memory::Error::Resource(error) => classify_resource_error(error),
        super::memory::Error::Registry(error) => classify_registry_error(error),
        super::memory::Error::Cache(_)
        | super::memory::Error::MemoryObject
        | super::memory::Error::InvalidCpu
        | super::memory::Error::Poisoned
        | super::memory::Error::Residency(_)
        | super::memory::Error::Stage2(_) => Error::Internal,
    }
}

pub(crate) fn derive_creation_lease(
    process: &Process,
    authority: HandleValue,
    domain: HandleValue,
) -> Result<HandleValue, Error> {
    let authority = process.resolve_handle::<VirtualMachineCreationAuthority>(
        authority,
        Rights::DERIVE.union(Rights::CREATE_VIRTUAL_MACHINE),
    )?;
    let domain =
        process.resolve_handle::<ResourceDomainObject>(domain, Rights::RESOURCE_DOMAIN_SPONSOR)?;
    let lease = authority.object().derive(domain.object().domain())?;
    Ok(process.create_object(
        lease,
        <VirtualMachineCreationLease as KernelObject>::SUPPORTED_RIGHTS,
    )?)
}

pub(crate) fn create_pending(
    process: &Process,
    lease: HandleValue,
    configuration: VirtualMachineConfiguration,
) -> Result<HandleValue, Error> {
    let lease_object = process
        .resolve_handle::<VirtualMachineCreationLease>(lease, Rights::CREATE_VIRTUAL_MACHINE)?;
    let domain = lease_object.object().domain().clone();
    let output = process.reserve_handles::<1>()?;
    let consumption = match process.prepare_handle_consumption(
        lease,
        Rights::CREATE_VIRTUAL_MACHINE,
        VirtualMachineCreationLease::KIND,
        lease_object.koid(),
    ) {
        Ok(consumption) => consumption,
        Err(error) => {
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let pending = match PendingVirtualMachine::try_new(configuration, &domain) {
        Ok(pending) => pending,
        Err(error) => {
            consumption.rollback();
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let prepared = match prepare_handle(
        pending,
        <PendingVirtualMachine as KernelObject>::SUPPORTED_RIGHTS,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            consumption.rollback();
            process.abort_handles(output);
            return Err(error);
        }
    };
    Ok(consumption.commit_replacement(output, [prepared])[0])
}

pub(crate) fn set_memory(
    process: &Process,
    pending: HandleValue,
    vmo: HandleValue,
) -> Result<(), Error> {
    let pending = process.resolve_handle::<PendingVirtualMachine>(pending, Rights::WRITE)?;
    let vmo = process
        .resolve_handle::<VmoObject>(vmo, Rights::READ.union(Rights::WRITE).union(Rights::MAP))?;
    pending.object().set_memory(vmo.object())?;
    Ok(())
}

pub(crate) fn set_bootstrap(
    process: &Process,
    pending: HandleValue,
    bootstrap: VirtualCpuBootstrap,
) -> Result<(), Error> {
    let pending = process.resolve_handle::<PendingVirtualMachine>(pending, Rights::WRITE)?;
    pending.object().set_bootstrap(bootstrap)?;
    Ok(())
}

pub(crate) fn set_virtual_serial(
    process: &Process,
    pending: HandleValue,
    serial: HandleValue,
) -> Result<(), Error> {
    let pending = process.resolve_handle::<PendingVirtualMachine>(pending, Rights::WRITE)?;
    let required = Rights::ASSIGN_DEVICE.union(Rights::TRANSFER);
    let serial_object =
        process.resolve_handle::<super::virtual_serial::VirtualSerial>(serial, required)?;
    let consumption = process.prepare_handle_consumption(
        serial,
        required,
        super::virtual_serial::VirtualSerial::KIND,
        serial_object.koid(),
    )?;
    let binding = serial_object.into_operation_pin().into_vm_device_binding();
    match pending.object().set_virtual_serial(binding) {
        Ok(()) => {
            consumption.commit_and_release();
            Ok(())
        }
        Err(error) => {
            consumption.rollback();
            Err(error.into())
        }
    }
}

pub(crate) fn create_virtual_serial(process: &Process) -> Result<HandleValue, Error> {
    let serial = super::virtual_serial::VirtualSerial::try_new(&process.resource_domain())
        .map_err(ObjectError::from)?;
    Ok(process.create_object(
        serial,
        <super::virtual_serial::VirtualSerial as KernelObject>::SUPPORTED_RIGHTS,
    )?)
}

pub(crate) fn seal(process: &Process, pending: HandleValue) -> Result<(), Error> {
    let pending = process.resolve_handle::<PendingVirtualMachine>(pending, Rights::WRITE)?;
    pending.object().seal()?;
    Ok(())
}

pub(crate) fn install(
    process: &Process,
    pending_value: HandleValue,
) -> Result<[HandleValue; 2], Error> {
    let pending = process.resolve_handle::<PendingVirtualMachine>(pending_value, Rights::START)?;
    let output = process.reserve_handles::<2>()?;
    let consumption = match process.prepare_handle_consumption(
        pending_value,
        Rights::START,
        PendingVirtualMachine::KIND,
        pending.koid(),
    ) {
        Ok(consumption) => consumption,
        Err(error) => {
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let domain = pending.object().resource_domain();
    let lifecycle = match pending.object().installed_lifecycle() {
        Ok(lifecycle) => lifecycle,
        Err(error) => {
            consumption.rollback();
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let vm_object = match VirtualMachineObject::try_new(lifecycle.clone(), &domain) {
        Ok(object) => object,
        Err(error) => {
            consumption.rollback();
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let vm_handle = match prepare_handle(vm_object, VirtualMachineObject::SUPPORTED_RIGHTS) {
        Ok(handle) => handle,
        Err(error) => {
            consumption.rollback();
            process.abort_handles(output);
            return Err(error);
        }
    };
    let vcpu_object = match VirtualCpuObject::try_new(lifecycle.clone(), 0, &domain) {
        Ok(object) => object,
        Err(error) => {
            drop(vm_handle);
            consumption.rollback();
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let vcpu_handle = match prepare_handle(vcpu_object, VirtualCpuObject::SUPPORTED_RIGHTS) {
        Ok(handle) => handle,
        Err(error) => {
            drop(vm_handle);
            consumption.rollback();
            process.abort_handles(output);
            return Err(error);
        }
    };
    let prepared = match pending.object().take_prepared() {
        Ok(prepared) => prepared,
        Err(error) => {
            drop((vm_handle, vcpu_handle));
            consumption.rollback();
            process.abort_handles(output);
            return Err(error.into());
        }
    };
    let installed = match prepared.install() {
        Ok(installed) => installed,
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: sealed VM failed its final registry publication: {error:?}"
        )),
    };
    drop(installed.publish_handle_lifecycle());
    Ok(consumption.commit_replacement(output, [vm_handle, vcpu_handle]))
}

pub(crate) fn abort(process: &Process, pending: HandleValue) -> Result<(), Error> {
    let resolved =
        process.resolve_handle::<PendingVirtualMachine>(pending, Rights::REQUEST_STOP)?;
    let consumption = process.prepare_handle_consumption(
        pending,
        Rights::REQUEST_STOP,
        PendingVirtualMachine::KIND,
        resolved.koid(),
    )?;
    consumption.commit_and_release();
    Ok(())
}

pub(crate) fn request_stop(process: &Process, machine: HandleValue) -> Result<(), Error> {
    let machine = process.resolve_handle::<VirtualMachineObject>(machine, Rights::REQUEST_STOP)?;
    machine.object().request_stop();
    Ok(())
}

pub(crate) fn machine_info(
    process: &Process,
    machine: HandleValue,
) -> Result<(VirtualMachineConfiguration, VirtualMachineSnapshot), Error> {
    let machine = process.resolve_handle::<VirtualMachineObject>(machine, Rights::INSPECT)?;
    Ok((
        machine.object().configuration(),
        machine.object().snapshot(),
    ))
}

pub(crate) fn vcpu_info(process: &Process, vcpu: HandleValue) -> Result<VirtualCpuSnapshot, Error> {
    let vcpu = process.resolve_handle::<VirtualCpuObject>(vcpu, Rights::INSPECT)?;
    Ok(vcpu.object().snapshot())
}

pub(crate) fn start_vcpu(process: &Process, vcpu: HandleValue) -> Result<(), Error> {
    let vcpu = process.resolve_handle::<VirtualCpuObject>(vcpu, Rights::START)?;
    vcpu.object().start()?;
    Ok(())
}

fn prepare_handle<T: crate::kernel::object::UserExportableObject>(
    payload: T,
    rights: Rights,
) -> Result<PreparedHandle, Error> {
    let publication = ObjectPublication::try_new(payload).map_err(ObjectError::from)?;
    PreparedHandle::try_from_new_object(publication, rights, HandleFlags::NONE)
        .map_err(ProcessError::from)
        .map_err(Into::into)
}
