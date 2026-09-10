// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native semantic-error to wire-status translation and result construction.

use hyper::abi::native::{
    HYPER_NATIVE_STATUS_ACCESS_DENIED, HYPER_NATIVE_STATUS_BAD_HANDLE,
    HYPER_NATIVE_STATUS_BAD_STATE, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, HYPER_NATIVE_STATUS_BUSY,
    HYPER_NATIVE_STATUS_CANCELLED, HYPER_NATIVE_STATUS_FAULT, HYPER_NATIVE_STATUS_INTERNAL,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_STATUS_NO_MEMORY,
    HYPER_NATIVE_STATUS_NOT_FOUND, HYPER_NATIVE_STATUS_NOT_SUPPORTED, HYPER_NATIVE_STATUS_OK,
    HYPER_NATIVE_STATUS_PEER_CLOSED, HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
    HYPER_NATIVE_STATUS_TIMED_OUT, HYPER_NATIVE_STATUS_WOULD_BLOCK, HyperNativeStatus,
    NativeResult,
};

use crate::kernel::accounting::ResourceError;
use crate::kernel::capability::{HandleError, HandleValue};
use crate::kernel::ipc::{
    ByteChannelError, ByteChannelServiceError, CapabilityChannelError,
    CapabilityChannelServiceError,
};
use crate::kernel::mm::user_space::{
    AddressError, AddressSpaceError, MachineError, MemoryServiceError,
};
use crate::kernel::object::{EventError, ObjectCreationError, ObjectWaitError, SignalWaitError};
use crate::kernel::process::{ChildProcessStartError, ProcessBuilderError, ProcessError};
use crate::kernel::task::TimedWaitError;
use crate::kernel::vfs::{VfsError, VfsServiceError};

use super::services::{ConsoleServiceError, ObjectServiceError, ProcessBuilderServiceError};

pub(super) fn handle_result(result: Result<HandleValue, HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(value) => success([value.get(), 0]),
        Err(status) => failure(status),
    }
}

pub(super) fn scan_result(result: Result<(usize, u64), HyperNativeStatus>) -> NativeResult {
    match result {
        Ok((count, next)) => match u64::try_from(count) {
            Ok(count) => success([count, next]),
            Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Err(status) => failure(status),
    }
}

pub(super) fn status_only(result: Result<(), HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    }
}

pub(super) fn info_result(result: Result<u64, HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(supported_size) => success([supported_size, 0]),
        Err(status) => failure(status),
    }
}

pub(super) fn console_io_result(
    syscall: u64,
    result: Result<usize, HyperNativeStatus>,
) -> NativeResult {
    match result {
        Ok(actual) => match u64::try_from(actual) {
            Ok(actual) => success([actual, 0]),
            Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Err(HYPER_NATIVE_STATUS_WOULD_BLOCK) => {
            NativeResult::for_syscall(syscall, HYPER_NATIVE_STATUS_WOULD_BLOCK, [0, 0])
        }
        Err(status) => failure(status),
    }
}

pub(super) const fn success(values: [u64; 2]) -> NativeResult {
    NativeResult::new(HYPER_NATIVE_STATUS_OK, values)
}

pub(super) const fn failure(status: HyperNativeStatus) -> NativeResult {
    NativeResult::new(status, [0, 0])
}

pub(super) fn status_from_process_error(error: ProcessError) -> HyperNativeStatus {
    match error {
        ProcessError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ProcessError::Handle(error) => status_from_handle_error(error),
        ProcessError::Object(error) => status_from_object_creation_error(error),
        ProcessError::Lifecycle(_) | ProcessError::AddressSpaceReferenced => {
            HYPER_NATIVE_STATUS_BAD_STATE
        }
        ProcessError::Resource(error) => status_from_resource_error(error),
        ProcessError::Scheduler(error) => status_from_scheduler_error(error),
        ProcessError::TaskGroup(error) => status_from_task_group_error(error),
        ProcessError::UserEntry(crate::hal::user::UserEntryError::InvalidContext) => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        ProcessError::UserEntry(_) => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        ProcessError::UserMemory(error) => status_from_machine_error(error),
    }
}

pub(super) fn status_from_inspection_error(
    error: crate::kernel::inspect::Error,
) -> HyperNativeStatus {
    match error {
        crate::kernel::inspect::Error::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        crate::kernel::inspect::Error::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        crate::kernel::inspect::Error::NotFound => HYPER_NATIVE_STATUS_NOT_FOUND,
        crate::kernel::inspect::Error::Object(error) => status_from_object_creation_error(error),
        crate::kernel::inspect::Error::Process(error) => status_from_process_error(error),
        crate::kernel::inspect::Error::Resource(error) => status_from_resource_error(error),
        crate::kernel::inspect::Error::Scheduler(error) => status_from_scheduler_error(error),
        crate::kernel::inspect::Error::Unavailable => HYPER_NATIVE_STATUS_BAD_STATE,
        crate::kernel::inspect::Error::InconsistentAccounting => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

pub(super) const fn status_from_scheduler_error(
    error: crate::kernel::task::scheduler::Error,
) -> HyperNativeStatus {
    use crate::kernel::task::scheduler::Error;

    match error {
        Error::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        Error::ThreadLimit | Error::IdentifierExhausted | Error::WaitGenerationExhausted => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
        Error::EmptyCpuAffinity
        | Error::NoRegisteredCpuInAffinity
        | Error::InvalidCpuIndex
        | Error::CpuNotAllowed => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        Error::InvalidThreadState => HYPER_NATIVE_STATUS_BAD_STATE,
        Error::NotInitialized
        | Error::AlreadyInitialized
        | Error::CurrentThreadMissing
        | Error::ThreadNotFound
        | Error::TerminatedThread
        | Error::ThreadBlocked
        | Error::ThreadAlreadyQueued
        | Error::QueueCorrupted
        | Error::CannotBlockIdle
        | Error::CannotSleepWithInterruptsMasked
        | Error::CannotSleepWithPreemptionDisabled
        | Error::IrqTailRequiresInterruptsMasked
        | Error::UserRunRequiresInterruptsEnabled
        | Error::IdleThreadAlreadyInstalled
        | Error::InvalidIdleTransition
        | Error::CpuAlreadyRegistered
        | Error::CpuNotRegistered
        | Error::MigrationUnsupported
        | Error::MigrationInProgress
        | Error::ThreadTransitionInProgress
        | Error::InvalidWaitRegistration
        | Error::MigrationBlockedByCpuLocalWait
        | Error::PreemptionUnavailable
        | Error::PreemptionInvariant
        | Error::VmEntryUnavailable => HYPER_NATIVE_STATUS_INTERNAL,
        Error::Thread(error) => status_from_thread_error(error),
    }
}

pub(super) const fn status_from_thread_error(
    error: crate::kernel::task::thread::Error,
) -> HyperNativeStatus {
    use crate::kernel::task::thread::Error;

    match error {
        Error::Allocation | Error::ObjectAllocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        Error::ObjectIdentityExhausted | Error::ObjectRegistrationExhausted => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
        Error::NameTooLong | Error::InvalidPlacement | Error::VirtualInterrupt(_) => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}

pub(super) const fn status_from_task_group_error(
    error: crate::kernel::process::TaskGroupError,
) -> HyperNativeStatus {
    use crate::kernel::process::TaskGroupError;

    match error {
        TaskGroupError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        TaskGroupError::CounterOverflow | TaskGroupError::GenerationExhausted => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
        TaskGroupError::Inactive | TaskGroupError::MembersRemain => HYPER_NATIVE_STATUS_BAD_STATE,
        TaskGroupError::Resource(error) => status_from_resource_error(error),
    }
}

pub(super) const fn status_from_object_creation_error(
    error: ObjectCreationError,
) -> HyperNativeStatus {
    match error {
        ObjectCreationError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ObjectCreationError::KoidExhausted | ObjectCreationError::RegistrationExhausted => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
    }
}

pub(super) fn status_from_object_service_error(error: ObjectServiceError) -> HyperNativeStatus {
    match error {
        ObjectServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ObjectServiceError::Process(error) => status_from_process_error(error),
        ObjectServiceError::Event(error) => status_from_event_error(error),
        ObjectServiceError::WaitSet(error) => match error {
            crate::kernel::object::WaitSetError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
            crate::kernel::object::WaitSetError::Unsupported => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
            crate::kernel::object::WaitSetError::InvalidInput => {
                HYPER_NATIVE_STATUS_INVALID_ARGUMENT
            }
            crate::kernel::object::WaitSetError::Full => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
            crate::kernel::object::WaitSetError::Missing => HYPER_NATIVE_STATUS_NOT_FOUND,
            crate::kernel::object::WaitSetError::Busy => HYPER_NATIVE_STATUS_BUSY,
            crate::kernel::object::WaitSetError::Closed => HYPER_NATIVE_STATUS_CANCELLED,
            crate::kernel::object::WaitSetError::TimedOut => HYPER_NATIVE_STATUS_TIMED_OUT,
            crate::kernel::object::WaitSetError::Resource(error) => {
                status_from_resource_error(error)
            }
            crate::kernel::object::WaitSetError::Wait(error) => {
                status_from_object_wait_error(error)
            }
        },
        ObjectServiceError::Wait(error) => status_from_object_wait_error(error),
    }
}

pub(super) fn status_from_byte_channel_service_error(
    error: ByteChannelServiceError,
) -> HyperNativeStatus {
    match error {
        ByteChannelServiceError::InvalidBuffer => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ByteChannelServiceError::Process(error) => status_from_process_error(error),
        ByteChannelServiceError::Channel(error) => status_from_byte_channel_error(error),
    }
}

pub(super) fn status_from_capability_channel_service_error(
    error: CapabilityChannelServiceError,
) -> HyperNativeStatus {
    match error {
        CapabilityChannelServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        CapabilityChannelServiceError::Process(error) => status_from_process_error(error),
        CapabilityChannelServiceError::Channel(error) => {
            status_from_capability_channel_error(error)
        }
        CapabilityChannelServiceError::Wait(error) => status_from_object_wait_error(error),
    }
}

pub(super) const fn status_from_capability_channel_error(
    error: CapabilityChannelError,
) -> HyperNativeStatus {
    match error {
        CapabilityChannelError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        CapabilityChannelError::AllocationSize | CapabilityChannelError::Internal => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
        CapabilityChannelError::EndpointClosed | CapabilityChannelError::BadState => {
            HYPER_NATIVE_STATUS_BAD_STATE
        }
        CapabilityChannelError::PeerClosed => HYPER_NATIVE_STATUS_PEER_CLOSED,
        CapabilityChannelError::WouldBlock => HYPER_NATIVE_STATUS_WOULD_BLOCK,
        CapabilityChannelError::ReceiverQueueFull | CapabilityChannelError::ResourceLimit => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
        CapabilityChannelError::BufferTooSmall { .. } => HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
        CapabilityChannelError::InvalidHandle | CapabilityChannelError::WrongObjectType => {
            HYPER_NATIVE_STATUS_BAD_HANDLE
        }
        CapabilityChannelError::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        CapabilityChannelError::UnsupportedTransfer => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        CapabilityChannelError::Busy => HYPER_NATIVE_STATUS_BUSY,
        CapabilityChannelError::UserMemoryFault => HYPER_NATIVE_STATUS_FAULT,
        CapabilityChannelError::TimedOut => HYPER_NATIVE_STATUS_TIMED_OUT,
        CapabilityChannelError::Cancelled => HYPER_NATIVE_STATUS_CANCELLED,
        CapabilityChannelError::Resource(error) => status_from_resource_error(error),
    }
}

pub(super) fn status_from_process_builder_service_error(
    error: ProcessBuilderServiceError,
) -> HyperNativeStatus {
    match error {
        ProcessBuilderServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ProcessBuilderServiceError::Process(error) => status_from_process_error(error),
        ProcessBuilderServiceError::Builder(error) => status_from_process_builder_error(error),
        ProcessBuilderServiceError::Start(error) => status_from_process_builder_start_error(error),
    }
}

pub(super) fn status_from_process_builder_start_error(
    error: ProcessBuilderError<ChildProcessStartError>,
) -> HyperNativeStatus {
    match error {
        ProcessBuilderError::Transaction(error) => status_from_child_process_start_error(error),
        ProcessBuilderError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ProcessBuilderError::AlreadySealed
        | ProcessBuilderError::AlreadyStarted
        | ProcessBuilderError::Aborted
        | ProcessBuilderError::MissingName
        | ProcessBuilderError::NotSealed => HYPER_NATIVE_STATUS_BAD_STATE,
        ProcessBuilderError::Busy => HYPER_NATIVE_STATUS_BUSY,
        ProcessBuilderError::ExecutableFile(error) => status_from_vfs_error(error),
        ProcessBuilderError::DuplicateStartupPurpose
        | ProcessBuilderError::EmptyArguments
        | ProcessBuilderError::InvalidEnvironment
        | ProcessBuilderError::InvalidAffinity
        | ProcessBuilderError::InvalidStartupPurpose
        | ProcessBuilderError::InvalidString => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ProcessBuilderError::Image(error) => status_from_loader_error(error),
        ProcessBuilderError::Object(error) => status_from_object_creation_error(error),
        ProcessBuilderError::Process(error) => status_from_process_error(error),
        ProcessBuilderError::Resource(error) => status_from_resource_error(error),
        ProcessBuilderError::Stack(error) => status_from_startup_stack_error(error),
        ProcessBuilderError::StartupHandleLimit | ProcessBuilderError::StringLimit => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
        ProcessBuilderError::UnsupportedStartupKind => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
    }
}

pub(super) fn status_from_process_builder_error(
    error: ProcessBuilderError<()>,
) -> HyperNativeStatus {
    match error {
        ProcessBuilderError::Transaction(()) => HYPER_NATIVE_STATUS_INTERNAL,
        ProcessBuilderError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ProcessBuilderError::AlreadySealed
        | ProcessBuilderError::AlreadyStarted
        | ProcessBuilderError::Aborted
        | ProcessBuilderError::MissingName
        | ProcessBuilderError::NotSealed => HYPER_NATIVE_STATUS_BAD_STATE,
        ProcessBuilderError::Busy => HYPER_NATIVE_STATUS_BUSY,
        ProcessBuilderError::ExecutableFile(error) => status_from_vfs_error(error),
        ProcessBuilderError::DuplicateStartupPurpose
        | ProcessBuilderError::EmptyArguments
        | ProcessBuilderError::InvalidEnvironment
        | ProcessBuilderError::InvalidAffinity
        | ProcessBuilderError::InvalidStartupPurpose
        | ProcessBuilderError::InvalidString => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ProcessBuilderError::Image(error) => status_from_loader_error(error),
        ProcessBuilderError::Object(error) => status_from_object_creation_error(error),
        ProcessBuilderError::Process(error) => status_from_process_error(error),
        ProcessBuilderError::Resource(error) => status_from_resource_error(error),
        ProcessBuilderError::Stack(error) => status_from_startup_stack_error(error),
        ProcessBuilderError::StartupHandleLimit | ProcessBuilderError::StringLimit => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
        ProcessBuilderError::UnsupportedStartupKind => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
    }
}

pub(super) fn status_from_child_process_start_error(
    error: ChildProcessStartError,
) -> HyperNativeStatus {
    match error {
        ChildProcessStartError::Builder(error) => status_from_process_builder_error(error),
        ChildProcessStartError::Process(error) => status_from_process_error(error),
        ChildProcessStartError::Stack(error) => status_from_startup_stack_error(error),
        ChildProcessStartError::TaskObject(error) => status_from_task_object_error(error),
        ChildProcessStartError::VmarObject(error) => status_from_memory_object_error(error),
    }
}

pub(super) const fn status_from_task_object_error(
    error: crate::kernel::process::TaskObjectError,
) -> HyperNativeStatus {
    match error {
        crate::kernel::process::TaskObjectError::AlreadyPublished => HYPER_NATIVE_STATUS_BAD_STATE,
        crate::kernel::process::TaskObjectError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        crate::kernel::process::TaskObjectError::Object(error) => {
            status_from_object_creation_error(error)
        }
        crate::kernel::process::TaskObjectError::Resource(error) => {
            status_from_resource_error(error)
        }
        crate::kernel::process::TaskObjectError::TaskGroup(_) => HYPER_NATIVE_STATUS_BAD_STATE,
    }
}

pub(super) fn status_from_hierarchy_error(
    error: crate::kernel::process::hierarchy::Error,
) -> HyperNativeStatus {
    match error {
        crate::kernel::process::hierarchy::Error::NotSupported => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        crate::kernel::process::hierarchy::Error::Process(error) => {
            status_from_process_error(error)
        }
        crate::kernel::process::hierarchy::Error::ResourceDomain(error) => match error {
            crate::kernel::accounting::ResourceDomainObjectError::AlreadyPublished => {
                HYPER_NATIVE_STATUS_BAD_STATE
            }
            crate::kernel::accounting::ResourceDomainObjectError::AllocationSize => {
                HYPER_NATIVE_STATUS_INTERNAL
            }
            crate::kernel::accounting::ResourceDomainObjectError::Object(error) => {
                status_from_object_creation_error(error)
            }
            crate::kernel::accounting::ResourceDomainObjectError::Resource(error) => {
                status_from_resource_error(error)
            }
        },
        crate::kernel::process::hierarchy::Error::Task(error) => {
            status_from_task_object_error(error)
        }
    }
}

pub(super) const fn status_from_memory_object_error(
    error: crate::kernel::mm::user_space::MemoryObjectError,
) -> HyperNativeStatus {
    match error {
        crate::kernel::mm::user_space::MemoryObjectError::AlreadyPublished => {
            HYPER_NATIVE_STATUS_BAD_STATE
        }
        crate::kernel::mm::user_space::MemoryObjectError::AllocationSize => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
        crate::kernel::mm::user_space::MemoryObjectError::Object(error) => {
            status_from_object_creation_error(error)
        }
        crate::kernel::mm::user_space::MemoryObjectError::Resource(error) => {
            status_from_resource_error(error)
        }
        crate::kernel::mm::user_space::MemoryObjectError::AddressSpace(error) => {
            status_from_logical_error(error)
        }
        crate::kernel::mm::user_space::MemoryObjectError::Vmo(error) => {
            status_from_vmo_error(error)
        }
        crate::kernel::mm::user_space::MemoryObjectError::WrongVariant => {
            HYPER_NATIVE_STATUS_BAD_STATE
        }
    }
}

pub(super) const fn status_from_vmo_error(
    error: crate::kernel::mm::user_space::VmoError<
        crate::kernel::mm::user_space::KernelPageError,
        ResourceError,
    >,
) -> HyperNativeStatus {
    match error {
        crate::kernel::mm::user_space::VmoError::Account(error) => {
            status_from_resource_error(error)
        }
        crate::kernel::mm::user_space::VmoError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        crate::kernel::mm::user_space::VmoError::Backend(error) => status_from_page_error(error),
        crate::kernel::mm::user_space::VmoError::Busy => HYPER_NATIVE_STATUS_BUSY,
        crate::kernel::mm::user_space::VmoError::InvalidRange
        | crate::kernel::mm::user_space::VmoError::SizeOverflow => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
    }
}

pub(super) fn status_from_memory_service_error(error: MemoryServiceError) -> HyperNativeStatus {
    match error {
        MemoryServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        MemoryServiceError::Machine(error) => status_from_machine_error(error),
        MemoryServiceError::MemoryObject(error) => status_from_memory_object_error(error),
        MemoryServiceError::Process(error) => status_from_process_error(error),
        MemoryServiceError::Scheduler(error) => status_from_scheduler_error(error),
        MemoryServiceError::Vfs(error) => status_from_vfs_error(error),
    }
}

pub(super) fn status_from_vm_service_error(
    error: crate::kernel::vm::service::Error,
) -> HyperNativeStatus {
    match error {
        crate::kernel::vm::service::Error::NotSupported => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        crate::kernel::vm::service::Error::InvalidArgument => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        crate::kernel::vm::service::Error::BadHandle => HYPER_NATIVE_STATUS_BAD_HANDLE,
        crate::kernel::vm::service::Error::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        crate::kernel::vm::service::Error::Busy => HYPER_NATIVE_STATUS_BUSY,
        crate::kernel::vm::service::Error::BadState => HYPER_NATIVE_STATUS_BAD_STATE,
        crate::kernel::vm::service::Error::ResourceLimit => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        crate::kernel::vm::service::Error::NoMemory => HYPER_NATIVE_STATUS_NO_MEMORY,
        crate::kernel::vm::service::Error::Fault => HYPER_NATIVE_STATUS_FAULT,
        crate::kernel::vm::service::Error::Internal => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

pub(super) const fn status_from_loader_error(
    error: crate::kernel::process::LoaderError,
) -> HyperNativeStatus {
    match error {
        crate::kernel::process::LoaderError::Address => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        crate::kernel::process::LoaderError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        crate::kernel::process::LoaderError::Elf(error) => status_from_elf_error(error),
        crate::kernel::process::LoaderError::Image(_) => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        crate::kernel::process::LoaderError::Machine(error) => status_from_machine_error(error),
        crate::kernel::process::LoaderError::Resource(error) => status_from_resource_error(error),
        crate::kernel::process::LoaderError::Scheduler(_) => HYPER_NATIVE_STATUS_INTERNAL,
        crate::kernel::process::LoaderError::UnsupportedMachine => {
            HYPER_NATIVE_STATUS_NOT_SUPPORTED
        }
    }
}

pub(super) const fn status_from_elf_error(error: hyper::exec::elf::Error) -> HyperNativeStatus {
    match error {
        hyper::exec::elf::Error::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        hyper::exec::elf::Error::UnsupportedClass
        | hyper::exec::elf::Error::UnsupportedDataEncoding
        | hyper::exec::elf::Error::UnsupportedFileType
        | hyper::exec::elf::Error::UnsupportedInterpreter
        | hyper::exec::elf::Error::UnsupportedMachine
        | hyper::exec::elf::Error::UnsupportedOperatingSystemAbi
        | hyper::exec::elf::Error::UnsupportedAbiVersion
        | hyper::exec::elf::Error::UnsupportedRelocation
        | hyper::exec::elf::Error::UnsupportedTls => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        _ => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    }
}

pub(super) const fn status_from_startup_stack_error(
    error: hyper::exec::startup::Error,
) -> HyperNativeStatus {
    match error {
        hyper::exec::startup::Error::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        hyper::exec::startup::Error::TooLarge => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        hyper::exec::startup::Error::AddressOverflow
        | hyper::exec::startup::Error::EmbeddedNul
        | hyper::exec::startup::Error::LayoutMismatch => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    }
}

pub(super) fn status_from_console_service_error(error: ConsoleServiceError) -> HyperNativeStatus {
    match error {
        ConsoleServiceError::Process(error) => status_from_process_error(error),
        ConsoleServiceError::Io(crate::kernel::device::console::IoError::WouldBlock) => {
            HYPER_NATIVE_STATUS_WOULD_BLOCK
        }
    }
}

pub(super) fn status_from_vfs_service_error(error: VfsServiceError) -> HyperNativeStatus {
    match error {
        VfsServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        VfsServiceError::Process(error) => status_from_process_error(error),
        VfsServiceError::FileSystem(error) => status_from_vfs_error(error),
    }
}

pub(super) const fn status_from_vfs_error(error: VfsError) -> HyperNativeStatus {
    match error {
        VfsError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        VfsError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        VfsError::Backend(_) => HYPER_NATIVE_STATUS_INTERNAL,
        VfsError::Cache(crate::kernel::io_cache::CacheError::Allocation) => {
            HYPER_NATIVE_STATUS_NO_MEMORY
        }
        VfsError::Cache(_) => HYPER_NATIVE_STATUS_INTERNAL,
        VfsError::InvalidDirectoryCookie => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        VfsError::InvalidPath => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        VfsError::AlreadyExists => hyper::abi::native::HYPER_NATIVE_STATUS_ALREADY_EXISTS,
        VfsError::NotEmpty => hyper::abi::native::HYPER_NATIVE_STATUS_NOT_EMPTY,
        VfsError::InvalidSize => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        VfsError::Missing => HYPER_NATIVE_STATUS_NOT_FOUND,
        VfsError::NotDirectory | VfsError::NotRegularFile => HYPER_NATIVE_STATUS_BAD_STATE,
        VfsError::NotExecutable => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        VfsError::Object(error) => status_from_object_creation_error(error),
        VfsError::Resource(error) => status_from_resource_error(error),
    }
}

pub(super) const fn status_from_byte_channel_error(error: ByteChannelError) -> HyperNativeStatus {
    match error {
        ByteChannelError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ByteChannelError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        ByteChannelError::MessageTooLarge => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ByteChannelError::EndpointClosed => HYPER_NATIVE_STATUS_BAD_STATE,
        ByteChannelError::PeerClosed => HYPER_NATIVE_STATUS_PEER_CLOSED,
        ByteChannelError::WouldBlock => HYPER_NATIVE_STATUS_WOULD_BLOCK,
        ByteChannelError::Busy | ByteChannelError::StaleMessage => HYPER_NATIVE_STATUS_BUSY,
        ByteChannelError::SequenceExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        ByteChannelError::Resource(error) => status_from_resource_error(error),
    }
}

pub(super) const fn status_from_event_error(error: EventError) -> HyperNativeStatus {
    match error {
        EventError::InvalidSignals => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        EventError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        EventError::Resource(error) => status_from_resource_error(error),
        EventError::SignalWait(error) => status_from_signal_wait_error(error),
    }
}

pub(super) const fn status_from_object_wait_error(error: ObjectWaitError) -> HyperNativeStatus {
    match error {
        ObjectWaitError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        ObjectWaitError::Deadline(error) => status_from_deadline_error(error),
        ObjectWaitError::InvalidSignals => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ObjectWaitError::Resource(error) => status_from_resource_error(error),
        ObjectWaitError::Signal(error) => status_from_signal_wait_error(error),
        ObjectWaitError::Timer(error) => status_from_timed_wait_error(error),
    }
}

pub(super) const fn status_from_deadline_error(
    error: crate::kernel::time::Error,
) -> HyperNativeStatus {
    match error {
        crate::kernel::time::Error::Conversion(_) | crate::kernel::time::Error::DeadlineTooFar => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        _ => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

pub(super) const fn status_from_timed_wait_error(error: TimedWaitError) -> HyperNativeStatus {
    match error {
        TimedWaitError::Allocation
        | TimedWaitError::Time(crate::kernel::time::Error::TimerQueue(
            hyper::time::TimerQueueError::Allocation,
        )) => HYPER_NATIVE_STATUS_NO_MEMORY,
        TimedWaitError::Scheduler(_)
        | TimedWaitError::Time(_)
        | TimedWaitError::TimerCleanup(_) => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

pub(super) const fn status_from_signal_wait_error(error: SignalWaitError) -> HyperNativeStatus {
    match error {
        SignalWaitError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        SignalWaitError::SequenceExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        SignalWaitError::Scheduler(_) => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

pub(super) const fn status_from_handle_error(error: HandleError) -> HyperNativeStatus {
    match error {
        HandleError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        HandleError::InvalidHandle | HandleError::WrongObjectType => HYPER_NATIVE_STATUS_BAD_HANDLE,
        HandleError::Busy => HYPER_NATIVE_STATUS_BUSY,
        HandleError::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        HandleError::UnsupportedRights | HandleError::UnsupportedFlags => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        HandleError::UnsupportedTransfer => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        HandleError::ObjectRetired | HandleError::TableRetired => HYPER_NATIVE_STATUS_BAD_STATE,
        HandleError::ActiveHandleLimit
        | HandleError::ReservationIdExhausted
        | HandleError::ReservationTooLarge
        | HandleError::TableFull => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        HandleError::OutstandingReservation => HYPER_NATIVE_STATUS_BUSY,
        HandleError::ObjectAlreadyActive | HandleError::EmptyReservation => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}

pub(super) const fn status_from_resource_error(error: ResourceError) -> HyperNativeStatus {
    match error {
        ResourceError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ResourceError::HierarchyTooDeep
        | ResourceError::TooManyChargeDimensions
        | ResourceError::LimitExceeded { .. }
        | ResourceError::UsageOverflow { .. }
        | ResourceError::ChildCountExhausted
        | ResourceError::DomainIdExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        ResourceError::DomainInactive(_)
        | ResourceError::OutstandingUsage
        | ResourceError::ActiveChildren
        | ResourceError::RetirementNotStarted => HYPER_NATIVE_STATUS_BAD_STATE,
        ResourceError::EmptyCharge | ResourceError::LimitBelowUsage { .. } => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}

pub(super) const fn status_from_machine_error(error: MachineError) -> HyperNativeStatus {
    match error {
        MachineError::Allocation | MachineError::Page(_) => HYPER_NATIVE_STATUS_NO_MEMORY,
        MachineError::Address(error) => status_from_address_error(error),
        MachineError::Logical(error) => status_from_logical_error(error),
        MachineError::Resource(error) => status_from_resource_error(error),
        MachineError::Residency(_) | MachineError::Transport => HYPER_NATIVE_STATUS_BUSY,
        MachineError::Unsupported => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        MachineError::Hal(_) | MachineError::Identifier(_) | MachineError::Vmo(_) => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
        MachineError::InvalidRange | MachineError::SizeOverflow => HYPER_NATIVE_STATUS_FAULT,
    }
}

pub(super) const fn status_from_address_error(_: AddressError) -> HyperNativeStatus {
    HYPER_NATIVE_STATUS_FAULT
}

pub(super) const fn status_from_logical_error(
    error: AddressSpaceError<crate::kernel::mm::user_space::KernelPageError, ResourceError>,
) -> HyperNativeStatus {
    match error {
        AddressSpaceError::Account(error) => status_from_resource_error(error),
        AddressSpaceError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        AddressSpaceError::Busy => HYPER_NATIVE_STATUS_BUSY,
        AddressSpaceError::Backend(error) => status_from_page_error(error),
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
        | AddressSpaceError::WritableExecutableBacking => HYPER_NATIVE_STATUS_FAULT,
        AddressSpaceError::IdentityExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
    }
}

pub(super) const fn status_from_page_error(
    error: crate::kernel::mm::user_space::KernelPageError,
) -> HyperNativeStatus {
    match error {
        crate::kernel::mm::user_space::KernelPageError::Allocation(_) => {
            HYPER_NATIVE_STATUS_NO_MEMORY
        }
        crate::kernel::mm::user_space::KernelPageError::AddressOverflow
        | crate::kernel::mm::user_space::KernelPageError::Range => HYPER_NATIVE_STATUS_FAULT,
        crate::kernel::mm::user_space::KernelPageError::MissingLinearMap
        | crate::kernel::mm::user_space::KernelPageError::Unsupported => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}
