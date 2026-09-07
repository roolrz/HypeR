// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `HypeR` Native syscall validation and dispatch.

use alloc::vec::Vec;

use hyper::abi::native::{
    HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE, HYPER_NATIVE_STATUS_ACCESS_DENIED,
    HYPER_NATIVE_STATUS_BAD_HANDLE, HYPER_NATIVE_STATUS_BAD_STATE,
    HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, HYPER_NATIVE_STATUS_BUSY, HYPER_NATIVE_STATUS_CANCELLED,
    HYPER_NATIVE_STATUS_FAULT, HYPER_NATIVE_STATUS_INTERNAL, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HYPER_NATIVE_STATUS_NO_MEMORY, HYPER_NATIVE_STATUS_NOT_FOUND,
    HYPER_NATIVE_STATUS_NOT_SUPPORTED, HYPER_NATIVE_STATUS_PEER_CLOSED,
    HYPER_NATIVE_STATUS_RESOURCE_LIMIT, HYPER_NATIVE_STATUS_TIMED_OUT,
    HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_SYS_ABI_QUERY,
    HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE, HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
    HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE, HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE,
    HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE, HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
    HYPER_NATIVE_SYS_CONSOLE_READ, HYPER_NATIVE_SYS_CONSOLE_WRITE,
    HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY, HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE,
    HYPER_NATIVE_SYS_EVENT_CREATE, HYPER_NATIVE_SYS_EVENT_SIGNAL,
    HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO, HYPER_NATIVE_SYS_FILE_READ_AT,
    HYPER_NATIVE_SYS_HANDLE_CLOSE, HYPER_NATIVE_SYS_HANDLE_DUPLICATE,
    HYPER_NATIVE_SYS_HANDLE_GET_INFO, HYPER_NATIVE_SYS_HANDLE_REPLACE,
    HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO, HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES, HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS,
    HYPER_NATIVE_SYS_OBJECT_WAIT_MANY, HYPER_NATIVE_SYS_OBJECT_WAIT_ONE,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT, HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT, HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE, HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY, HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_START, HYPER_NATIVE_SYS_PROCESS_EXIT,
    HYPER_NATIVE_SYS_PROCESS_GET_INFO, HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES, HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS,
    HYPER_NATIVE_SYS_THREAD_EXIT, HYPER_NATIVE_SYS_THREAD_YIELD, HYPER_NATIVE_SYS_VMAR_ALLOCATE,
    HYPER_NATIVE_SYS_VMAR_DESTROY, HYPER_NATIVE_SYS_VMAR_MAP, HYPER_NATIVE_SYS_VMAR_PROTECT,
    HYPER_NATIVE_SYS_VMAR_UNMAP, HYPER_NATIVE_SYS_VMO_CREATE, HYPER_NATIVE_SYS_VMO_READ,
    HYPER_NATIVE_SYS_VMO_WRITE, HyperNativeHandleInfo, HyperNativeHandleInspection,
    HyperNativeObjectBasicInfo, HyperNativeObjectInspection, HyperNativeProcessInfo,
    HyperNativeStatus, HyperNativeTaskProcess, HyperNativeTaskThread, NativeInvocation,
    NativeResult,
};

use crate::kernel::accounting::ResourceError;
use crate::kernel::capability::{HandleError, HandleInfo, HandleValue, Rights};
use crate::kernel::inspect::{
    HANDLE_PAGE_CAPACITY, OBJECT_PAGE_CAPACITY, Page, ProcessHandleSnapshot, TaskThreadSnapshot,
};
use crate::kernel::ipc::{
    ByteChannelError, ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelError,
    CapabilityChannelServiceError, CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::{
    AddressError, AddressSpaceError, MachineError, MemoryServiceError, Permissions, UserAddress,
    UserSlice,
};
use crate::kernel::object::{
    EventError, ObjectCreationError, ObjectWaitError, SignalWaitError, SignalWaitManyOutcome,
    SignalWaitOutcome,
};
use crate::kernel::process::{
    ChildProcessStartError, ProcessBuilderError, ProcessError, ProcessPhase, ProcessSnapshot,
    TerminalReason,
};
use crate::kernel::task::TimedWaitError;
use crate::kernel::vfs::{VfsError, VfsServiceError};

const HANDLE_INFO_SIZE: usize = core::mem::size_of::<HyperNativeHandleInfo>();
const OBJECT_BASIC_INFO_SIZE: usize = core::mem::size_of::<HyperNativeObjectBasicInfo>();
const PROCESS_INFO_SIZE: usize = core::mem::size_of::<HyperNativeProcessInfo>();
type Arguments = [u64; hyper::abi::native::HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS];
type ProcessBuilderHandleRequest = (
    HandleValue,
    HandleValue,
    u32,
    crate::kernel::object::ObjectKind,
    Option<Rights>,
    crate::kernel::capability::HandleTransferOperation,
);

/// Reports whether the current implementation is audited for masked entry.
///
/// New syscall numbers default to the deferred Thread path until their full
/// implementation is proven nonblocking and added deliberately here.
pub(in crate::kernel) const fn is_immediate(number: u64) -> bool {
    matches!(
        number,
        HYPER_NATIVE_SYS_ABI_QUERY
            | HYPER_NATIVE_SYS_HANDLE_CLOSE
            | HYPER_NATIVE_SYS_HANDLE_GET_INFO
            | HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO
    )
}

/// Narrow Process service boundary consumed by the Native ABI adapter.
///
/// Implementations finish every handle-table operation before copying user
/// memory. Keeping those two phases separate prevents faults or backend work
/// from extending the Process lock graph.
pub(in crate::kernel) trait UserOutputServices {
    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError>;
}

pub(in crate::kernel) trait ImmediateServices: UserOutputServices {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError>;

    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError>;
}

/// Services which may grow storage and therefore run with interrupts enabled.
pub(in crate::kernel) trait AllocatingServices {
    fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError>;
    fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError>;
    fn create_event(&self) -> Result<HandleValue, ObjectServiceError>;
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError>;
    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError>;
}

/// Sleepable object services invoked only after architecture entry unwinds.
pub(in crate::kernel) trait DeferredServices:
    AllocatingServices + UserOutputServices
{
    fn signal_event(
        &self,
        value: HandleValue,
        clear: u64,
        set: u64,
    ) -> Result<(), ObjectServiceError>;

    fn wait_one(
        &self,
        value: HandleValue,
        requested: u64,
        deadline: u64,
    ) -> Result<SignalWaitOutcome, ObjectServiceError>;

    fn wait_many(
        &self,
        items: UserSlice,
        item_count: usize,
        deadline: u64,
    ) -> Result<SignalWaitManyOutcome, ObjectServiceError>;

    fn process_info(&self, process: HandleValue) -> Result<ProcessSnapshot, ProcessError>;

    fn scan_processes(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        Page<ProcessSnapshot, { crate::kernel::inspect::PROCESS_PAGE_CAPACITY }>,
        crate::kernel::inspect::Error,
    > {
        let _ = (inspector, cursor);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn scan_threads(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        Page<TaskThreadSnapshot, { crate::kernel::inspect::THREAD_PAGE_CAPACITY }>,
        crate::kernel::inspect::Error,
    > {
        let _ = (inspector, cursor);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn scan_objects(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        Page<crate::kernel::object::ObjectSnapshot, OBJECT_PAGE_CAPACITY>,
        crate::kernel::inspect::Error,
    > {
        let _ = (inspector, cursor);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn scan_process_handles(
        &self,
        inspector: HandleValue,
        process_koid: u64,
        cursor: u64,
    ) -> Result<Page<ProcessHandleSnapshot, HANDLE_PAGE_CAPACITY>, crate::kernel::inspect::Error>
    {
        let _ = (inspector, process_koid, cursor);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn derive_task_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let _ = (inspector, process);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn derive_object_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let _ = (inspector, process);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn derive_task_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let _ = (inspector, group);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn derive_object_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let _ = (inspector, group);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn derive_task_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let _ = (inspector, domain);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn derive_object_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let _ = (inspector, domain);
        Err(crate::kernel::inspect::Error::AccessDenied)
    }

    fn write_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<(), ByteChannelServiceError>;

    fn read_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError>;

    fn try_send_capability_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
    ) -> Result<(), CapabilityChannelServiceError>;

    fn receive_capability_channel(
        &self,
        endpoint: HandleValue,
        deadline: u64,
        bytes: Option<UserSlice>,
        slots: Option<UserSlice>,
    ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError>;

    fn read_console(
        &self,
        console: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<usize, ConsoleServiceError>;

    fn write_console(
        &self,
        console: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<usize, ConsoleServiceError>;

    fn open_file(
        &self,
        root: HandleValue,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError>;

    fn open_directory(
        &self,
        root: HandleValue,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError>;

    fn read_file_at(
        &self,
        file: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError>;

    fn create_vmo(&self, size: u64) -> Result<HandleValue, MemoryServiceError>;
    fn create_file_executable_vmo(
        &self,
        file: HandleValue,
    ) -> Result<HandleValue, MemoryServiceError>;
    fn read_vmo(
        &self,
        vmo: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(), MemoryServiceError>;
    fn write_vmo(
        &self,
        vmo: HandleValue,
        offset: u64,
        input: Option<UserSlice>,
    ) -> Result<(), MemoryServiceError>;
    #[allow(clippy::too_many_arguments)]
    fn map_vmo(
        &self,
        vmar: HandleValue,
        vmo: HandleValue,
        vmo_offset: u64,
        address: u64,
        size: u64,
        permissions: Permissions,
    ) -> Result<(), MemoryServiceError>;
    fn allocate_vmar(
        &self,
        parent: HandleValue,
        address: u64,
        size: u64,
    ) -> Result<HandleValue, MemoryServiceError>;
    fn protect_vmar(
        &self,
        vmar: HandleValue,
        address: u64,
        size: u64,
        permissions: Permissions,
    ) -> Result<(), MemoryServiceError>;
    fn unmap_vmar(
        &self,
        vmar: HandleValue,
        address: u64,
        size: u64,
    ) -> Result<(), MemoryServiceError>;
    fn destroy_vmar(&self, vmar: HandleValue) -> Result<(), MemoryServiceError>;

    fn create_process_builder(
        &self,
        factory: HandleValue,
        group: HandleValue,
        domain: HandleValue,
        executable: HandleValue,
    ) -> Result<HandleValue, ProcessBuilderServiceError>;

    fn set_process_builder_name(
        &self,
        builder: HandleValue,
        name: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError>;

    fn add_process_builder_argument(
        &self,
        builder: HandleValue,
        argument: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError>;

    fn add_process_builder_environment(
        &self,
        builder: HandleValue,
        environment: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError>;

    fn set_process_builder_affinity(
        &self,
        builder: HandleValue,
        words: Option<UserSlice>,
        word_count: usize,
    ) -> Result<(), ProcessBuilderServiceError>;

    fn add_process_builder_handle(
        &self,
        builder: HandleValue,
        source: HandleValue,
        purpose: u32,
        expected_kind: crate::kernel::object::ObjectKind,
        requested_rights: Option<Rights>,
        operation: crate::kernel::capability::HandleTransferOperation,
    ) -> Result<(), ProcessBuilderServiceError>;

    fn seal_process_builder(&self, builder: HandleValue) -> Result<(), ProcessBuilderServiceError>;

    fn start_process_builder(
        &self,
        builder: HandleValue,
    ) -> Result<HandleValue, ProcessBuilderServiceError>;

    fn abort_process_builder(&self, builder: HandleValue)
    -> Result<(), ProcessBuilderServiceError>;

    fn request_process_stop(&self, process: HandleValue) -> Result<(), ProcessError>;
}

#[derive(Debug)]
pub(in crate::kernel) enum ProcessBuilderServiceError {
    InvalidInput,
    Process(ProcessError),
    Builder(ProcessBuilderError<()>),
    Start(ProcessBuilderError<ChildProcessStartError>),
}

impl From<ProcessError> for ProcessBuilderServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<ProcessBuilderError<()>> for ProcessBuilderServiceError {
    fn from(error: ProcessBuilderError<()>) -> Self {
        Self::Builder(error)
    }
}

#[derive(Debug)]
pub(in crate::kernel) enum ConsoleServiceError {
    Process(ProcessError),
    Io(crate::kernel::device::console::IoError),
}

impl From<ProcessError> for ConsoleServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<crate::kernel::device::console::IoError> for ConsoleServiceError {
    fn from(error: crate::kernel::device::console::IoError) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub(in crate::kernel) enum ObjectServiceError {
    InvalidInput,
    Process(ProcessError),
    Event(EventError),
    Wait(ObjectWaitError),
}

impl From<ProcessError> for ObjectServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<EventError> for ObjectServiceError {
    fn from(error: EventError) -> Self {
        Self::Event(error)
    }
}

impl From<ObjectWaitError> for ObjectServiceError {
    fn from(error: ObjectWaitError) -> Self {
        Self::Wait(error)
    }
}

/// Policy action produced after architecture entry has fully unwound.
///
/// This value owns no architecture frame, CPU pin, or address-space guard, so
/// the caller may safely execute its scheduling or lifecycle effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel) enum DeferredAction {
    Return(NativeResult),
    Yield(NativeResult),
    ExitThread { status: i64 },
    ExitProcess { status: i64 },
}

/// Dispatches one owned, nonblocking invocation.
///
/// Architecture entry retains its private raw frame but passes no frame borrow
/// or architecture offset into this adapter. Unknown numbers and malformed
/// fixed-width values fail closed. Operations that may block must use a
/// separate deferred dispatcher.
pub(in crate::kernel) fn dispatch_immediate(
    services: &impl ImmediateServices,
    invocation: NativeInvocation,
) -> NativeResult {
    let arguments = invocation.arguments();
    match invocation.number() {
        HYPER_NATIVE_SYS_ABI_QUERY => sys_abi_query(),
        HYPER_NATIVE_SYS_HANDLE_CLOSE => sys_handle_close(services, arguments),
        HYPER_NATIVE_SYS_HANDLE_GET_INFO => sys_handle_get_info(services, arguments),
        HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO => sys_object_get_basic_info(services, arguments),
        _ => sys_not_supported(),
    }
}

/// Executes one syscall after the machine context has returned to its Thread.
///
/// The returned action separates ABI decoding from scheduler and Process
/// policy. A blocking service may park the current Thread before producing the
/// action. Unknown calls remain ordinary returning failures even though they
/// conservatively use the deferred path.
pub(in crate::kernel) fn dispatch_deferred(
    services: &impl DeferredServices,
    invocation: NativeInvocation,
) -> DeferredAction {
    match invocation.number() {
        HYPER_NATIVE_SYS_HANDLE_DUPLICATE => {
            DeferredAction::Return(sys_handle_duplicate(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_HANDLE_REPLACE => {
            DeferredAction::Return(sys_handle_replace(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_EVENT_CREATE => {
            DeferredAction::Return(sys_event_create(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE => {
            DeferredAction::Return(sys_byte_channel_create(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE => DeferredAction::Return(
            sys_capability_channel_create(services, invocation.arguments()),
        ),
        HYPER_NATIVE_SYS_THREAD_YIELD => sys_thread_yield(),
        HYPER_NATIVE_SYS_THREAD_EXIT => sys_thread_exit(invocation.arguments()),
        HYPER_NATIVE_SYS_PROCESS_EXIT => sys_process_exit(invocation.arguments()),
        HYPER_NATIVE_SYS_EVENT_SIGNAL => sys_event_signal(services, invocation.arguments()),
        HYPER_NATIVE_SYS_OBJECT_WAIT_ONE => sys_object_wait_one(services, invocation.arguments()),
        HYPER_NATIVE_SYS_OBJECT_WAIT_MANY => sys_object_wait_many(services, invocation.arguments()),
        HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE => {
            sys_byte_channel_write(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_BYTE_CHANNEL_READ => {
            sys_byte_channel_read(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND => {
            sys_capability_channel_try_send(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE => {
            sys_capability_channel_receive(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CONSOLE_READ => sys_console_read(services, invocation.arguments()),
        HYPER_NATIVE_SYS_CONSOLE_WRITE => sys_console_write(services, invocation.arguments()),
        HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE => {
            sys_directory_open_file(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY => {
            sys_directory_open_directory(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_FILE_READ_AT => sys_file_read_at(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMO_CREATE => sys_vmo_create(services, invocation.arguments()),
        HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO => {
            sys_file_create_executable_vmo(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VMO_READ => sys_vmo_read(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMO_WRITE => sys_vmo_write(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_ALLOCATE => sys_vmar_allocate(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_MAP => sys_vmar_map(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_PROTECT => sys_vmar_protect(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_UNMAP => sys_vmar_unmap(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_DESTROY => sys_vmar_destroy(services, invocation.arguments()),
        HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE => {
            sys_process_builder_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME => {
            sys_process_builder_set_name(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT => {
            sys_process_builder_add_argument(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT => {
            sys_process_builder_add_environment(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY => {
            sys_process_builder_set_affinity(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE => {
            sys_process_builder_add_handle(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL => {
            sys_process_builder_seal(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_START => {
            sys_process_builder_start(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT => {
            sys_process_builder_abort(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP => {
            sys_process_request_stop(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_GET_INFO => sys_process_get_info(services, invocation.arguments()),
        HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES => {
            sys_task_inspector_scan_processes(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS => {
            sys_task_inspector_scan_threads(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS => {
            sys_task_inspector_derive_process(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS => {
            sys_object_inspector_scan_objects(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES => {
            sys_object_inspector_scan_handles(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS => {
            sys_object_inspector_derive_process(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP => {
            sys_task_inspector_derive_task_group(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP => {
            sys_object_inspector_derive_task_group(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN => {
            sys_task_inspector_derive_resource_domain(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN => {
            sys_object_inspector_derive_resource_domain(services, invocation.arguments())
        }
        _ => DeferredAction::Return(sys_not_supported()),
    }
}

// Keep each syscall as a distinct machine frame. The routing match must not
// inherit the largest handler's stack requirement, and crash traces should
// identify the operation which was active at the fault boundary.

#[inline(never)]
fn sys_abi_query() -> NativeResult {
    success([HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE])
}

#[inline(never)]
fn sys_handle_close(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .close_handle(value)
            .map_err(status_from_process_error)
    });
    status_only(result)
}

#[inline(never)]
fn sys_handle_duplicate(services: &impl AllocatingServices, arguments: &Arguments) -> NativeResult {
    let result = parse_handle_and_rights(arguments[0], arguments[1]).and_then(|(value, rights)| {
        services
            .duplicate_handle(value, rights)
            .map_err(status_from_process_error)
    });
    handle_result(result)
}

#[inline(never)]
fn sys_handle_replace(services: &impl AllocatingServices, arguments: &Arguments) -> NativeResult {
    let result = parse_handle_and_rights(arguments[0], arguments[1]).and_then(|(value, rights)| {
        services
            .replace_handle(value, rights)
            .map_err(status_from_process_error)
    });
    handle_result(result)
}

#[inline(never)]
fn sys_handle_get_info(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    let result =
        prepare_info_request(arguments, HANDLE_INFO_SIZE).and_then(|(value, destination)| {
            let info = services
                .handle_info(value, Rights::NONE)
                .map_err(status_from_process_error)?;
            let record = encode_handle_info(info);
            services
                .copy_to_user(destination, &record)
                .map_err(status_from_process_error)
        });
    status_only(result)
}

#[inline(never)]
fn sys_object_get_basic_info(
    services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result =
        prepare_info_request(arguments, OBJECT_BASIC_INFO_SIZE).and_then(|(value, destination)| {
            let info = services
                .handle_info(value, Rights::INSPECT)
                .map_err(status_from_process_error)?;
            let record = encode_object_basic_info(info);
            services
                .copy_to_user(destination, &record)
                .map_err(status_from_process_error)
        });
    status_only(result)
}

#[inline(never)]
fn sys_event_create(services: &impl AllocatingServices, arguments: &Arguments) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    handle_result(
        services
            .create_event()
            .map_err(status_from_object_service_error),
    )
}

#[inline(never)]
fn sys_byte_channel_create(
    services: &impl AllocatingServices,
    arguments: &Arguments,
) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    match services
        .create_byte_channel()
        .map_err(status_from_byte_channel_service_error)
    {
        Ok([first, second]) => success([first.get(), second.get()]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
fn sys_capability_channel_create(
    services: &impl AllocatingServices,
    arguments: &Arguments,
) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    match services
        .create_capability_channel()
        .map_err(status_from_capability_channel_service_error)
    {
        Ok([first, second]) => success([first.get(), second.get()]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
fn sys_event_signal(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .signal_event(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_not_supported() -> NativeResult {
    failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED)
}

#[inline(never)]
fn sys_thread_yield() -> DeferredAction {
    DeferredAction::Yield(success([0, 0]))
}

#[inline(never)]
fn sys_thread_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitThread {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
fn sys_process_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitProcess {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
fn sys_object_wait_one(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .wait_one(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    let result = match result {
        Ok(SignalWaitOutcome::Observed(snapshot)) => success([snapshot.signals().bits(), 0]),
        Ok(SignalWaitOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
fn sys_object_wait_many(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_wait_many(arguments).and_then(|(items, item_count, deadline)| {
        services
            .wait_many(items, item_count, deadline)
            .map_err(status_from_object_service_error)
    });
    let result = match result {
        Ok(SignalWaitManyOutcome::Observed { index, snapshot }) => match u64::try_from(index) {
            Ok(index) => success([index, snapshot.signals().bits()]),
            Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Ok(SignalWaitManyOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitManyOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
fn sys_byte_channel_write(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_byte_channel_io(arguments).and_then(|(endpoint, bytes)| {
        services
            .write_byte_channel(endpoint, bytes)
            .map_err(status_from_byte_channel_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_byte_channel_read(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_byte_channel_io(arguments).and_then(|(endpoint, bytes)| {
        services
            .read_byte_channel(endpoint, bytes)
            .map_err(status_from_byte_channel_service_error)
    });
    let result = match result {
        Ok(ByteChannelReadOutcome::Received { bytes }) => success([bytes, 0]),
        Ok(ByteChannelReadOutcome::BufferTooSmall { bytes }) => NativeResult::for_syscall(
            HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
            HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
            [bytes, 0],
        ),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
fn sys_capability_channel_try_send(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result =
        parse_capability_channel_send(arguments).and_then(|(endpoint, bytes, dispositions)| {
            services
                .try_send_capability_channel(endpoint, bytes, dispositions)
                .map_err(status_from_capability_channel_service_error)
        });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_capability_channel_receive(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_capability_channel_receive(arguments).and_then(
        |(endpoint, deadline, bytes, slots)| {
            services
                .receive_capability_channel(endpoint, deadline, bytes, slots)
                .map_err(status_from_capability_channel_service_error)
        },
    );
    DeferredAction::Return(capability_receive_result(result))
}

fn capability_receive_result(
    result: Result<CapabilityReceiveOutcome, HyperNativeStatus>,
) -> NativeResult {
    match result {
        Ok(CapabilityReceiveOutcome::Delivered(info)) => {
            match (u64::try_from(info.bytes), u64::try_from(info.handles)) {
                (Ok(bytes), Ok(handles)) => success([bytes, handles]),
                _ => failure(HYPER_NATIVE_STATUS_INTERNAL),
            }
        }
        Ok(CapabilityReceiveOutcome::Failed(CapabilityChannelError::BufferTooSmall {
            required_bytes,
            required_handles,
        })) => match (
            u64::try_from(required_bytes),
            u64::try_from(required_handles),
        ) {
            (Ok(bytes), Ok(handles)) => NativeResult::for_syscall(
                HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE,
                HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
                [bytes, handles],
            ),
            _ => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Ok(CapabilityReceiveOutcome::Failed(error)) => {
            failure(status_from_capability_channel_error(error))
        }
        Err(status) => failure(status),
    }
}

#[inline(never)]
fn sys_process_builder_create(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result =
        parse_builder_create(arguments).and_then(|[factory, group, domain, executable]| {
            services
                .create_process_builder(factory, group, domain, executable)
                .map_err(status_from_process_builder_service_error)
        });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_process_builder_set_name(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(
        arguments,
        hyper::abi::native::HYPER_NATIVE_PROCESS_NAME_MAX_BYTES,
    )
    .and_then(|(builder, text)| {
        services
            .set_process_builder_name(builder, text)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_builder_add_argument(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(
        arguments,
        hyper::abi::native::HYPER_NATIVE_PROCESS_ARGUMENT_MAX_BYTES,
    )
    .and_then(|(builder, text)| {
        services
            .add_process_builder_argument(builder, text)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_builder_add_environment(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(
        arguments,
        hyper::abi::native::HYPER_NATIVE_PROCESS_ENVIRONMENT_MAX_BYTES,
    )
    .and_then(|(builder, text)| {
        services
            .add_process_builder_environment(builder, text)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_builder_set_affinity(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_affinity(arguments).and_then(|(builder, words, word_count)| {
        services
            .set_process_builder_affinity(builder, words, word_count)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_builder_add_handle(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_handle(arguments).and_then(
        |(builder, source, purpose, expected_kind, requested_rights, operation)| {
            services
                .add_process_builder_handle(
                    builder,
                    source,
                    purpose,
                    expected_kind,
                    requested_rights,
                    operation,
                )
                .map_err(status_from_process_builder_service_error)
        },
    );
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_builder_seal(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|builder| {
        services
            .seal_process_builder(builder)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_builder_start(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|builder| {
        services
            .start_process_builder(builder)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_process_builder_abort(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|builder| {
        services
            .abort_process_builder(builder)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_request_stop(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|process| {
        services
            .request_process_stop(process)
            .map_err(status_from_process_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_process_get_info(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result =
        prepare_info_request(arguments, PROCESS_INFO_SIZE).and_then(|(process, destination)| {
            let snapshot = services
                .process_info(process)
                .map_err(status_from_process_error)?;
            let record = encode_process_info(snapshot);
            services
                .copy_to_user(destination, &record)
                .map_err(status_from_process_error)
        });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_task_inspector_scan_processes(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_scan(
        arguments,
        crate::kernel::inspect::PROCESS_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeTaskProcess>(),
    )
    .and_then(|(inspector, cursor, destination)| {
        let page = services
            .scan_processes(inspector, cursor)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_task_process)
    });
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
fn sys_task_inspector_scan_threads(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_scan(
        arguments,
        crate::kernel::inspect::THREAD_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeTaskThread>(),
    )
    .and_then(|(inspector, cursor, destination)| {
        let page = services
            .scan_threads(inspector, cursor)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_task_thread)
    });
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
fn sys_object_inspector_scan_objects(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_scan(
        arguments,
        OBJECT_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeObjectInspection>(),
    )
    .and_then(|(inspector, cursor, destination)| {
        let page = services
            .scan_objects(inspector, cursor)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_object_inspection)
    });
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
fn sys_object_inspector_scan_handles(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle_inspector_scan(arguments).and_then(
        |(inspector, process_koid, cursor, destination)| {
            let page = services
                .scan_process_handles(inspector, process_koid, cursor)
                .map_err(status_from_inspection_error)?;
            copy_encoded_page(services, destination, &page, encode_handle_inspection)
        },
    );
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
fn sys_task_inspector_derive_process(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, process)| {
        services
            .derive_task_inspector(inspector, process)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_object_inspector_derive_process(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, process)| {
        services
            .derive_object_inspector(inspector, process)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_task_inspector_derive_task_group(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, group)| {
        services
            .derive_task_inspector_for_task_group(inspector, group)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_object_inspector_derive_task_group(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, group)| {
        services
            .derive_object_inspector_for_task_group(inspector, group)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_task_inspector_derive_resource_domain(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, domain)| {
        services
            .derive_task_inspector_for_resource_domain(inspector, domain)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_object_inspector_derive_resource_domain(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, domain)| {
        services
            .derive_object_inspector_for_resource_domain(inspector, domain)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_console_read(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_console_io(arguments).and_then(|(console, bytes)| {
        services
            .read_console(console, bytes)
            .map_err(status_from_console_service_error)
    });
    DeferredAction::Return(console_io_result(HYPER_NATIVE_SYS_CONSOLE_READ, result))
}

#[inline(never)]
fn sys_console_write(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_console_io(arguments).and_then(|(console, bytes)| {
        services
            .write_console(console, bytes)
            .map_err(status_from_console_service_error)
    });
    DeferredAction::Return(console_io_result(HYPER_NATIVE_SYS_CONSOLE_WRITE, result))
}

#[inline(never)]
fn sys_directory_open_file(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|root| {
        if arguments[4] != 0
            || arguments[5] != 0
            || arguments[2] == 0
            || arguments[2] > hyper::abi::native::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES
        {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let path = UserSlice::new(UserAddress::new(arguments[1]), arguments[2])
            .map_err(status_from_address_error)?;
        let rights = Rights::from_bits(arguments[3]).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .open_file(root, path, rights)
            .map_err(status_from_vfs_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_directory_open_directory(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_directory_open(arguments).and_then(|(root, path, rights)| {
        services
            .open_directory(root, path, rights)
            .map_err(status_from_vfs_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

fn parse_directory_open(
    arguments: &Arguments,
) -> Result<(HandleValue, UserSlice, Rights), HyperNativeStatus> {
    let root = parse_handle(arguments[0])?;
    if arguments[4] != 0
        || arguments[5] != 0
        || arguments[2] == 0
        || arguments[2] > hyper::abi::native::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let path = UserSlice::new(UserAddress::new(arguments[1]), arguments[2])
        .map_err(status_from_address_error)?;
    let rights = Rights::from_bits(arguments[3]).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    Ok((root, path, rights))
}

#[inline(never)]
fn sys_file_read_at(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|file| {
        if arguments[1] != 0 || arguments[4] > hyper::abi::native::HYPER_NATIVE_FILE_MAX_READ_BYTES
        {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let output = optional_user_slice(arguments[3], arguments[4])?;
        services
            .read_file_at(file, arguments[2], output)
            .map_err(status_from_vfs_service_error)
    });
    let result = match result {
        Ok((actual, file_size)) => success([actual, file_size]),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
fn sys_vmo_create(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = if arguments[1..].iter().any(|value| *value != 0) {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        services
            .create_vmo(arguments[0])
            .map_err(status_from_memory_service_error)
    };
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_file_create_executable_vmo(
    services: &impl DeferredServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = if arguments[1..].iter().any(|value| *value != 0) {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        parse_handle(arguments[0]).and_then(|file| {
            services
                .create_file_executable_vmo(file)
                .map_err(status_from_memory_service_error)
        })
    };
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_vmo_read(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_vmo_transfer(arguments).and_then(|(vmo, offset, bytes)| {
        services
            .read_vmo(vmo, offset, bytes)
            .map_err(status_from_memory_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_vmo_write(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_vmo_transfer(arguments).and_then(|(vmo, offset, bytes)| {
        services
            .write_vmo(vmo, offset, bytes)
            .map_err(status_from_memory_service_error)
    });
    DeferredAction::Return(status_only(result))
}

fn parse_vmo_transfer(
    arguments: &Arguments,
) -> Result<(HandleValue, u64, Option<UserSlice>), HyperNativeStatus> {
    if arguments[4] != 0
        || arguments[5] != 0
        || arguments[3] > hyper::abi::native::HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    Ok((
        parse_handle(arguments[0])?,
        arguments[1],
        optional_user_slice(arguments[2], arguments[3])?,
    ))
}

#[inline(never)]
fn sys_vmar_allocate(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = if arguments[3..].iter().any(|value| *value != 0) {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        parse_handle(arguments[0]).and_then(|parent| {
            services
                .allocate_vmar(parent, arguments[1], arguments[2])
                .map_err(status_from_memory_service_error)
        })
    };
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
fn sys_vmar_map(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|vmar| {
        let vmo = parse_handle(arguments[1])?;
        let permissions = crate::kernel::mm::user_space::abi_permissions(arguments[5])
            .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .map_vmo(
                vmar,
                vmo,
                arguments[2],
                arguments[3],
                arguments[4],
                permissions,
            )
            .map_err(status_from_memory_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_vmar_protect(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = if arguments[4] != 0 || arguments[5] != 0 {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        parse_handle(arguments[0]).and_then(|vmar| {
            let permissions = crate::kernel::mm::user_space::abi_permissions(arguments[3])
                .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
            services
                .protect_vmar(vmar, arguments[1], arguments[2], permissions)
                .map_err(status_from_memory_service_error)
        })
    };
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_vmar_unmap(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = if arguments[3..].iter().any(|value| *value != 0) {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        parse_handle(arguments[0]).and_then(|vmar| {
            services
                .unmap_vmar(vmar, arguments[1], arguments[2])
                .map_err(status_from_memory_service_error)
        })
    };
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_vmar_destroy(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = if arguments[1..].iter().any(|value| *value != 0) {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        parse_handle(arguments[0]).and_then(|vmar| {
            services
                .destroy_vmar(vmar)
                .map_err(status_from_memory_service_error)
        })
    };
    DeferredAction::Return(status_only(result))
}

fn parse_handle(raw: u64) -> Result<HandleValue, HyperNativeStatus> {
    HandleValue::try_from_raw(raw).map_err(status_from_handle_error)
}

fn parse_handle_and_rights(
    raw_handle: u64,
    raw_rights: u64,
) -> Result<(HandleValue, Rights), HyperNativeStatus> {
    let value = parse_handle(raw_handle)?;
    let rights = Rights::from_bits(raw_rights).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    Ok((value, rights))
}

fn parse_wait_many(arguments: &Arguments) -> Result<(UserSlice, usize, u64), HyperNativeStatus> {
    if arguments[1] == 0
        || arguments[1] > hyper::abi::native::HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let item_count =
        usize::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let bytes = arguments[1]
        .checked_mul(core::mem::size_of::<hyper::abi::native::HyperNativeObjectWaitItem>() as u64)
        .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let items =
        UserSlice::new(UserAddress::new(arguments[0]), bytes).map_err(status_from_address_error)?;
    Ok((items, item_count, arguments[2]))
}

fn parse_byte_channel_io(
    arguments: &Arguments,
) -> Result<(HandleValue, Option<UserSlice>), HyperNativeStatus> {
    if arguments[1] != 0
        || arguments[3] > hyper::abi::native::HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let endpoint = parse_handle(arguments[0])?;
    let bytes = optional_user_slice(arguments[2], arguments[3])?;
    Ok((endpoint, bytes))
}

fn parse_capability_channel_send(
    arguments: &Arguments,
) -> Result<(HandleValue, Option<UserSlice>, Option<UserSlice>), HyperNativeStatus> {
    if arguments[1] != 0
        || arguments[3] > hyper::abi::native::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES
        || arguments[5] > hyper::abi::native::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let disposition_bytes = capability_record_bytes(arguments[5])?;
    Ok((
        parse_handle(arguments[0])?,
        optional_user_slice(arguments[2], arguments[3])?,
        optional_user_slice(arguments[4], disposition_bytes)?,
    ))
}

fn parse_capability_channel_receive(
    arguments: &Arguments,
) -> Result<(HandleValue, u64, Option<UserSlice>, Option<UserSlice>), HyperNativeStatus> {
    if arguments[3] > hyper::abi::native::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES
        || arguments[5] > hyper::abi::native::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let slot_bytes = capability_record_bytes(arguments[5])?;
    Ok((
        parse_handle(arguments[0])?,
        arguments[1],
        optional_user_slice(arguments[2], arguments[3])?,
        optional_user_slice(arguments[4], slot_bytes)?,
    ))
}

fn capability_record_bytes(record_count: u64) -> Result<u64, HyperNativeStatus> {
    record_count
        .checked_mul(
            core::mem::size_of::<hyper::abi::native::HyperNativeCapabilityDisposition>() as u64,
        )
        .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
}

fn parse_builder_create(arguments: &Arguments) -> Result<[HandleValue; 4], HyperNativeStatus> {
    Ok([
        parse_handle(arguments[0])?,
        parse_handle(arguments[1])?,
        parse_handle(arguments[2])?,
        parse_handle(arguments[3])?,
    ])
}

fn parse_builder_text(
    arguments: &Arguments,
    maximum_bytes: u64,
) -> Result<(HandleValue, Option<UserSlice>), HyperNativeStatus> {
    if arguments[2] > maximum_bytes {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    Ok((
        parse_handle(arguments[0])?,
        optional_user_slice(arguments[1], arguments[2])?,
    ))
}

fn parse_builder_affinity(
    arguments: &Arguments,
) -> Result<(HandleValue, Option<UserSlice>, usize), HyperNativeStatus> {
    if arguments[2] > hyper::abi::native::HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let bytes = arguments[2]
        .checked_mul(core::mem::size_of::<u64>() as u64)
        .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let word_count =
        usize::try_from(arguments[2]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    Ok((
        parse_handle(arguments[0])?,
        optional_user_slice(arguments[1], bytes)?,
        word_count,
    ))
}

fn parse_builder_handle(
    arguments: &Arguments,
) -> Result<ProcessBuilderHandleRequest, HyperNativeStatus> {
    let purpose = u32::try_from(arguments[2]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let raw_kind = u32::try_from(arguments[3]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let expected_kind = parse_object_kind(raw_kind)?;
    let requested_rights =
        if arguments[4] == hyper::abi::native::HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS {
            None
        } else {
            Some(Rights::from_bits(arguments[4]).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?)
        };
    let operation = match arguments[5] {
        hyper::abi::native::HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE => {
            crate::kernel::capability::HandleTransferOperation::Move
        }
        hyper::abi::native::HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE => {
            crate::kernel::capability::HandleTransferOperation::Copy
        }
        _ => return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
    };
    Ok((
        parse_handle(arguments[0])?,
        parse_handle(arguments[1])?,
        purpose,
        expected_kind,
        requested_rights,
        operation,
    ))
}

fn parse_object_kind(raw: u32) -> Result<crate::kernel::object::ObjectKind, HyperNativeStatus> {
    use crate::kernel::object::ObjectKind;

    match raw {
        hyper::abi::native::HYPER_NATIVE_OBJECT_EVENT => Ok(ObjectKind::EVENT),
        hyper::abi::native::HYPER_NATIVE_OBJECT_BYTE_CHANNEL => Ok(ObjectKind::BYTE_CHANNEL),
        hyper::abi::native::HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL => {
            Ok(ObjectKind::CAPABILITY_CHANNEL)
        }
        hyper::abi::native::HYPER_NATIVE_OBJECT_THREAD => Ok(ObjectKind::THREAD),
        hyper::abi::native::HYPER_NATIVE_OBJECT_PROCESS => Ok(ObjectKind::PROCESS),
        hyper::abi::native::HYPER_NATIVE_OBJECT_TASK_GROUP => Ok(ObjectKind::TASK_GROUP),
        hyper::abi::native::HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN => Ok(ObjectKind::RESOURCE_DOMAIN),
        hyper::abi::native::HYPER_NATIVE_OBJECT_TASK_FACTORY => Ok(ObjectKind::TASK_FACTORY),
        hyper::abi::native::HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY => {
            Ok(ObjectKind::EXECUTABLE_AUTHORITY)
        }
        hyper::abi::native::HYPER_NATIVE_OBJECT_VMO => Ok(ObjectKind::VMO),
        hyper::abi::native::HYPER_NATIVE_OBJECT_VMAR => Ok(ObjectKind::VMAR),
        hyper::abi::native::HYPER_NATIVE_OBJECT_CONSOLE => Ok(ObjectKind::CONSOLE),
        hyper::abi::native::HYPER_NATIVE_OBJECT_DIRECTORY => Ok(ObjectKind::DIRECTORY),
        hyper::abi::native::HYPER_NATIVE_OBJECT_FILE => Ok(ObjectKind::FILE),
        hyper::abi::native::HYPER_NATIVE_OBJECT_PROCESS_BUILDER => Ok(ObjectKind::PROCESS_BUILDER),
        hyper::abi::native::HYPER_NATIVE_OBJECT_TASK_INSPECTOR => Ok(ObjectKind::TASK_INSPECTOR),
        hyper::abi::native::HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR => {
            Ok(ObjectKind::OBJECT_INSPECTOR)
        }
        _ => Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
    }
}

fn parse_console_io(
    arguments: &Arguments,
) -> Result<(HandleValue, Option<UserSlice>), HyperNativeStatus> {
    if arguments[1] != 0
        || arguments[3] > hyper::abi::native::HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let console = parse_handle(arguments[0])?;
    let bytes = optional_user_slice(arguments[2], arguments[3])?;
    Ok((console, bytes))
}

fn optional_user_slice(
    raw_address: u64,
    length: u64,
) -> Result<Option<UserSlice>, HyperNativeStatus> {
    if length == 0 {
        return Ok(None);
    }
    UserSlice::new(UserAddress::new(raw_address), length)
        .map(Some)
        .map_err(status_from_address_error)
}

fn prepare_info_request(
    arguments: &Arguments,
    record_size: usize,
) -> Result<(HandleValue, UserSlice), HyperNativeStatus> {
    let record_size = u64::try_from(record_size).map_err(|_| HYPER_NATIVE_STATUS_INTERNAL)?;
    if arguments[2] != record_size {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let value = parse_handle(arguments[0])?;
    let destination = UserSlice::new(UserAddress::new(arguments[1]), record_size)
        .map_err(status_from_address_error)?;
    Ok((value, destination))
}

fn parse_inspector_scan(
    arguments: &Arguments,
    capacity: usize,
    record_size: usize,
) -> Result<(HandleValue, u64, UserSlice), HyperNativeStatus> {
    let requested =
        usize::try_from(arguments[3]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    if requested != capacity {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let bytes = capacity
        .checked_mul(record_size)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(HYPER_NATIVE_STATUS_INTERNAL)?;
    let destination =
        UserSlice::new(UserAddress::new(arguments[2]), bytes).map_err(status_from_address_error)?;
    Ok((parse_handle(arguments[0])?, arguments[1], destination))
}

fn parse_handle_inspector_scan(
    arguments: &Arguments,
) -> Result<(HandleValue, u64, u64, UserSlice), HyperNativeStatus> {
    let requested =
        usize::try_from(arguments[4]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    if requested != HANDLE_PAGE_CAPACITY || arguments[1] == 0 {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let bytes = HANDLE_PAGE_CAPACITY
        .checked_mul(core::mem::size_of::<HyperNativeHandleInspection>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(HYPER_NATIVE_STATUS_INTERNAL)?;
    let destination =
        UserSlice::new(UserAddress::new(arguments[3]), bytes).map_err(status_from_address_error)?;
    Ok((
        parse_handle(arguments[0])?,
        arguments[1],
        arguments[2],
        destination,
    ))
}

fn parse_inspector_derivation(
    arguments: &Arguments,
) -> Result<(HandleValue, HandleValue), HyperNativeStatus> {
    Ok((parse_handle(arguments[0])?, parse_handle(arguments[1])?))
}

fn copy_encoded_page<T: Copy, const N: usize, const R: usize>(
    services: &impl UserOutputServices,
    destination: UserSlice,
    page: &Page<T, N>,
    encode: impl Fn(T) -> [u8; R],
) -> Result<(usize, u64), HyperNativeStatus> {
    let byte_count = page
        .len()
        .checked_mul(R)
        .ok_or(HYPER_NATIVE_STATUS_INTERNAL)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(byte_count)
        .map_err(|_| HYPER_NATIVE_STATUS_NO_MEMORY)?;
    for entry in page.entries() {
        bytes.extend_from_slice(&encode(*entry));
    }
    let output = UserSlice::new(
        destination.base(),
        u64::try_from(byte_count).map_err(|_| HYPER_NATIVE_STATUS_INTERNAL)?,
    )
    .map_err(status_from_address_error)?;
    services
        .copy_to_user(output, &bytes)
        .map_err(status_from_process_error)?;
    Ok((page.len(), page.next()))
}

fn encode_handle_info(info: HandleInfo) -> [u8; HANDLE_INFO_SIZE] {
    encode_handle_info_fields(info.kind.get(), info.flags.bits(), info.rights.bits())
}

fn encode_handle_info_fields(object_kind: u32, flags: u32, rights: u64) -> [u8; HANDLE_INFO_SIZE] {
    const KIND: usize = core::mem::offset_of!(HyperNativeHandleInfo, object_kind);
    const FLAGS: usize = core::mem::offset_of!(HyperNativeHandleInfo, flags);
    const RIGHTS: usize = core::mem::offset_of!(HyperNativeHandleInfo, rights);
    let mut record = [0_u8; HANDLE_INFO_SIZE];
    record[KIND..KIND + 4].copy_from_slice(&object_kind.to_ne_bytes());
    record[FLAGS..FLAGS + 4].copy_from_slice(&flags.to_ne_bytes());
    record[RIGHTS..RIGHTS + 8].copy_from_slice(&rights.to_ne_bytes());
    record
}

fn encode_object_basic_info(info: HandleInfo) -> [u8; OBJECT_BASIC_INFO_SIZE] {
    encode_object_basic_info_fields(info.koid.get(), info.kind.get())
}

fn encode_object_basic_info_fields(koid: u64, object_kind: u32) -> [u8; OBJECT_BASIC_INFO_SIZE] {
    const KOID: usize = core::mem::offset_of!(HyperNativeObjectBasicInfo, koid);
    const KIND: usize = core::mem::offset_of!(HyperNativeObjectBasicInfo, object_kind);
    let mut record = [0_u8; OBJECT_BASIC_INFO_SIZE];
    record[KOID..KOID + 8].copy_from_slice(&koid.to_ne_bytes());
    record[KIND..KIND + 4].copy_from_slice(&object_kind.to_ne_bytes());
    // The generated record's remaining bytes are the reserved-zero field.
    record
}

fn encode_process_info(snapshot: ProcessSnapshot) -> [u8; PROCESS_INFO_SIZE] {
    let (reason, detail0, detail1) = match snapshot.terminal {
        None => (hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_NONE, 0, 0),
        Some(TerminalReason::Requested) => (
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_REQUESTED,
            0,
            0,
        ),
        Some(TerminalReason::ThreadExited { status }) => (
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_THREAD_EXITED,
            status as u64,
            0,
        ),
        Some(TerminalReason::ProcessExited { status }) => (
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_PROCESS_EXITED,
            status as u64,
            0,
        ),
        Some(TerminalReason::LastThreadExited { status }) => (
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_LAST_THREAD_EXITED,
            status as u64,
            0,
        ),
        Some(TerminalReason::Fault { class, code }) => (
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_FAULT,
            u64::from(class),
            code,
        ),
        Some(TerminalReason::TaskGroupStop { generation }) => (
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_TASK_GROUP_STOP,
            generation,
            0,
        ),
    };
    encode_process_info_fields(
        process_phase(snapshot.phase),
        reason as u32,
        detail0,
        detail1,
    )
}

fn encode_process_info_fields(
    phase: u32,
    reason: u32,
    detail0: u64,
    detail1: u64,
) -> [u8; PROCESS_INFO_SIZE] {
    const PHASE: usize = core::mem::offset_of!(HyperNativeProcessInfo, phase);
    const REASON: usize = core::mem::offset_of!(HyperNativeProcessInfo, terminal_reason);
    const DETAIL0: usize = core::mem::offset_of!(HyperNativeProcessInfo, detail0);
    const DETAIL1: usize = core::mem::offset_of!(HyperNativeProcessInfo, detail1);
    let mut record = [0_u8; PROCESS_INFO_SIZE];
    record[PHASE..PHASE + 4].copy_from_slice(&phase.to_ne_bytes());
    record[REASON..REASON + 4].copy_from_slice(&reason.to_ne_bytes());
    record[DETAIL0..DETAIL0 + 8].copy_from_slice(&detail0.to_ne_bytes());
    record[DETAIL1..DETAIL1 + 8].copy_from_slice(&detail1.to_ne_bytes());
    record
}

fn encode_task_process(snapshot: ProcessSnapshot) -> [u8; 96] {
    let mut record = [0_u8; 96];
    write_u64(&mut record, 0, snapshot.koid.get());
    write_u32(&mut record, 8, process_phase(snapshot.phase));
    write_u32(&mut record, 12, terminal_reason(snapshot.terminal));
    write_u32(
        &mut record,
        16,
        u32::try_from(snapshot.pending_threads).unwrap_or(u32::MAX),
    );
    write_u32(
        &mut record,
        20,
        u32::try_from(snapshot.active_threads).unwrap_or(u32::MAX),
    );
    let name = snapshot.name.as_bytes();
    write_u32(&mut record, 24, name.len() as u32);
    record[32..32 + name.len()].copy_from_slice(name);
    record
}

fn encode_task_thread(snapshot: TaskThreadSnapshot) -> [u8; 96] {
    let mut record = [0_u8; 96];
    write_u64(&mut record, 0, snapshot.koid.get());
    write_u64(
        &mut record,
        8,
        snapshot
            .process_koid
            .map_or(0, crate::kernel::object::Koid::get),
    );
    let role = match snapshot.role {
        crate::kernel::task::ThreadRole::Bootstrap => {
            hyper::abi::native::HYPER_NATIVE_THREAD_ROLE_BOOTSTRAP
        }
        crate::kernel::task::ThreadRole::Idle => hyper::abi::native::HYPER_NATIVE_THREAD_ROLE_IDLE,
        crate::kernel::task::ThreadRole::Kernel => {
            hyper::abi::native::HYPER_NATIVE_THREAD_ROLE_KERNEL
        }
        crate::kernel::task::ThreadRole::User => hyper::abi::native::HYPER_NATIVE_THREAD_ROLE_USER,
        crate::kernel::task::ThreadRole::Vcpu => hyper::abi::native::HYPER_NATIVE_THREAD_ROLE_VCPU,
    };
    let registry_phase = match snapshot.registry_phase {
        crate::kernel::task::ThreadObjectRegistryPhase::Resident => {
            hyper::abi::native::HYPER_NATIVE_THREAD_REGISTRY_RESIDENT
        }
        crate::kernel::task::ThreadObjectRegistryPhase::Retiring => {
            hyper::abi::native::HYPER_NATIVE_THREAD_REGISTRY_RETIRING
        }
    };
    write_u32(&mut record, 16, role as u32);
    write_u32(&mut record, 20, registry_phase as u32);
    let name = snapshot.name.as_bytes();
    write_u32(&mut record, 24, name.len() as u32);
    record[32..32 + name.len()].copy_from_slice(name);
    record
}

fn encode_object_inspection(snapshot: crate::kernel::object::ObjectSnapshot) -> [u8; 96] {
    let mut record = [0_u8; 96];
    write_u64(&mut record, 0, snapshot.koid.get());
    write_u32(&mut record, 8, snapshot.kind.get());
    let (state, active) = match snapshot.handles {
        crate::kernel::object::ObjectHandleState::Unpublished => (
            hyper::abi::native::HYPER_NATIVE_OBJECT_HANDLE_STATE_UNPUBLISHED,
            0,
        ),
        crate::kernel::object::ObjectHandleState::Active(count) => (
            hyper::abi::native::HYPER_NATIVE_OBJECT_HANDLE_STATE_ACTIVE,
            u64::try_from(count).unwrap_or(u64::MAX),
        ),
        crate::kernel::object::ObjectHandleState::Retired => (
            hyper::abi::native::HYPER_NATIVE_OBJECT_HANDLE_STATE_RETIRED,
            0,
        ),
    };
    write_u32(&mut record, 12, state as u32);
    write_u64(&mut record, 16, active);
    write_u64(&mut record, 24, snapshot.supported_rights.bits());
    write_u64(
        &mut record,
        32,
        u64::try_from(snapshot.strong_references).unwrap_or(u64::MAX),
    );
    write_u64(
        &mut record,
        40,
        usize_to_u64(snapshot.references.kernel_service),
    );
    write_u64(&mut record, 48, usize_to_u64(snapshot.references.scheduler));
    write_u64(
        &mut record,
        56,
        usize_to_u64(snapshot.references.operation_pin),
    );
    write_u64(
        &mut record,
        64,
        usize_to_u64(snapshot.references.user_authority),
    );
    write_u64(
        &mut record,
        72,
        usize_to_u64(snapshot.references.publication),
    );
    write_u64(
        &mut record,
        80,
        usize_to_u64(snapshot.references.diagnostic),
    );
    write_u64(
        &mut record,
        88,
        usize_to_u64(snapshot.references.retirement),
    );
    record
}

fn encode_handle_inspection(snapshot: ProcessHandleSnapshot) -> [u8; 40] {
    let mut record = [0_u8; 40];
    write_u64(&mut record, 0, snapshot.process_koid.get());
    write_u64(&mut record, 8, snapshot.handle.value.get());
    write_u64(&mut record, 16, snapshot.handle.info.koid.get());
    write_u64(&mut record, 24, snapshot.handle.info.rights.bits());
    write_u32(&mut record, 32, snapshot.handle.info.kind.get());
    write_u32(&mut record, 36, snapshot.handle.info.flags.bits());
    record
}

fn terminal_reason(reason: Option<TerminalReason>) -> u32 {
    match reason {
        None => hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_NONE as u32,
        Some(TerminalReason::Requested) => {
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_REQUESTED as u32
        }
        Some(TerminalReason::ThreadExited { .. }) => {
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_THREAD_EXITED as u32
        }
        Some(TerminalReason::ProcessExited { .. }) => {
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_PROCESS_EXITED as u32
        }
        Some(TerminalReason::LastThreadExited { .. }) => {
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_LAST_THREAD_EXITED as u32
        }
        Some(TerminalReason::Fault { .. }) => {
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_FAULT as u32
        }
        Some(TerminalReason::TaskGroupStop { .. }) => {
            hyper::abi::native::HYPER_NATIVE_PROCESS_TERMINAL_TASK_GROUP_STOP as u32
        }
    }
}

fn write_u32(record: &mut [u8], offset: usize, value: u32) {
    record[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn write_u64(record: &mut [u8], offset: usize, value: u64) {
    record[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

const fn process_phase(phase: ProcessPhase) -> u32 {
    match phase {
        ProcessPhase::Prepared => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_PREPARED as u32,
        ProcessPhase::Created => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_CREATED as u32,
        ProcessPhase::Running => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_RUNNING as u32,
        ProcessPhase::Stopping => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_STOPPING as u32,
        ProcessPhase::Stopped => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_STOPPED as u32,
        ProcessPhase::Retiring => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_RETIRING as u32,
        ProcessPhase::Retired => hyper::abi::native::HYPER_NATIVE_PROCESS_PHASE_RETIRED as u32,
    }
}

fn handle_result(result: Result<HandleValue, HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(value) => success([value.get(), 0]),
        Err(status) => failure(status),
    }
}

fn scan_result(result: Result<(usize, u64), HyperNativeStatus>) -> NativeResult {
    match result {
        Ok((count, next)) => match u64::try_from(count) {
            Ok(count) => success([count, next]),
            Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Err(status) => failure(status),
    }
}

fn status_only(result: Result<(), HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    }
}

fn console_io_result(syscall: u64, result: Result<usize, HyperNativeStatus>) -> NativeResult {
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

const fn success(values: [u64; 2]) -> NativeResult {
    NativeResult::new(hyper::abi::native::HYPER_NATIVE_STATUS_OK, values)
}

const fn failure(status: HyperNativeStatus) -> NativeResult {
    NativeResult::new(status, [0, 0])
}

fn status_from_process_error(error: ProcessError) -> HyperNativeStatus {
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
        ProcessError::UserEntry(_) => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        ProcessError::UserMemory(error) => status_from_machine_error(error),
    }
}

fn status_from_inspection_error(error: crate::kernel::inspect::Error) -> HyperNativeStatus {
    match error {
        crate::kernel::inspect::Error::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        crate::kernel::inspect::Error::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        crate::kernel::inspect::Error::NotFound => HYPER_NATIVE_STATUS_NOT_FOUND,
        crate::kernel::inspect::Error::Object(error) => status_from_object_creation_error(error),
        crate::kernel::inspect::Error::Process(error) => status_from_process_error(error),
        crate::kernel::inspect::Error::Resource(error) => status_from_resource_error(error),
        crate::kernel::inspect::Error::Scheduler(error) => status_from_scheduler_error(error),
    }
}

const fn status_from_scheduler_error(
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
        | Error::InvalidThreadState
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
        | Error::VmEntryUnavailable
        | Error::Thread(_) => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

const fn status_from_task_group_error(
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

const fn status_from_object_creation_error(error: ObjectCreationError) -> HyperNativeStatus {
    match error {
        ObjectCreationError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ObjectCreationError::KoidExhausted | ObjectCreationError::RegistrationExhausted => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
    }
}

fn status_from_object_service_error(error: ObjectServiceError) -> HyperNativeStatus {
    match error {
        ObjectServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ObjectServiceError::Process(error) => status_from_process_error(error),
        ObjectServiceError::Event(error) => status_from_event_error(error),
        ObjectServiceError::Wait(error) => status_from_object_wait_error(error),
    }
}

fn status_from_byte_channel_service_error(error: ByteChannelServiceError) -> HyperNativeStatus {
    match error {
        ByteChannelServiceError::InvalidBuffer => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ByteChannelServiceError::Process(error) => status_from_process_error(error),
        ByteChannelServiceError::Channel(error) => status_from_byte_channel_error(error),
    }
}

fn status_from_capability_channel_service_error(
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

const fn status_from_capability_channel_error(error: CapabilityChannelError) -> HyperNativeStatus {
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

fn status_from_process_builder_service_error(
    error: ProcessBuilderServiceError,
) -> HyperNativeStatus {
    match error {
        ProcessBuilderServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ProcessBuilderServiceError::Process(error) => status_from_process_error(error),
        ProcessBuilderServiceError::Builder(error) => status_from_process_builder_error(error),
        ProcessBuilderServiceError::Start(error) => status_from_process_builder_start_error(error),
    }
}

fn status_from_process_builder_start_error(
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

fn status_from_process_builder_error(error: ProcessBuilderError<()>) -> HyperNativeStatus {
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

fn status_from_child_process_start_error(error: ChildProcessStartError) -> HyperNativeStatus {
    match error {
        ChildProcessStartError::Builder(error) => status_from_process_builder_error(error),
        ChildProcessStartError::Process(error) => status_from_process_error(error),
        ChildProcessStartError::Stack(error) => status_from_startup_stack_error(error),
        ChildProcessStartError::TaskObject(error) => status_from_task_object_error(error),
        ChildProcessStartError::VmarObject(error) => status_from_memory_object_error(error),
    }
}

const fn status_from_task_object_error(
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

const fn status_from_memory_object_error(
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

const fn status_from_vmo_error(
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

fn status_from_memory_service_error(error: MemoryServiceError) -> HyperNativeStatus {
    match error {
        MemoryServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        MemoryServiceError::Machine(error) => status_from_machine_error(error),
        MemoryServiceError::MemoryObject(error) => status_from_memory_object_error(error),
        MemoryServiceError::Process(error) => status_from_process_error(error),
        MemoryServiceError::Scheduler(error) => status_from_scheduler_error(error),
        MemoryServiceError::Vfs(error) => status_from_vfs_error(error),
    }
}

const fn status_from_loader_error(error: crate::kernel::process::LoaderError) -> HyperNativeStatus {
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

const fn status_from_elf_error(error: hyper::exec::elf::Error) -> HyperNativeStatus {
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

const fn status_from_startup_stack_error(error: hyper::exec::startup::Error) -> HyperNativeStatus {
    match error {
        hyper::exec::startup::Error::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        hyper::exec::startup::Error::TooLarge => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        hyper::exec::startup::Error::AddressOverflow
        | hyper::exec::startup::Error::EmbeddedNul
        | hyper::exec::startup::Error::LayoutMismatch => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    }
}

fn status_from_console_service_error(error: ConsoleServiceError) -> HyperNativeStatus {
    match error {
        ConsoleServiceError::Process(error) => status_from_process_error(error),
        ConsoleServiceError::Io(crate::kernel::device::console::IoError::WouldBlock) => {
            HYPER_NATIVE_STATUS_WOULD_BLOCK
        }
    }
}

fn status_from_vfs_service_error(error: VfsServiceError) -> HyperNativeStatus {
    match error {
        VfsServiceError::InvalidInput => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        VfsServiceError::Process(error) => status_from_process_error(error),
        VfsServiceError::FileSystem(error) => status_from_vfs_error(error),
    }
}

const fn status_from_vfs_error(error: VfsError) -> HyperNativeStatus {
    match error {
        VfsError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        VfsError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        VfsError::Backend(_) => HYPER_NATIVE_STATUS_INTERNAL,
        VfsError::Cache(crate::kernel::io_cache::CacheError::Allocation) => {
            HYPER_NATIVE_STATUS_NO_MEMORY
        }
        VfsError::Cache(_) => HYPER_NATIVE_STATUS_INTERNAL,
        VfsError::InvalidPath => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        VfsError::Missing => HYPER_NATIVE_STATUS_NOT_FOUND,
        VfsError::NotDirectory | VfsError::NotRegularFile => HYPER_NATIVE_STATUS_BAD_STATE,
        VfsError::NotExecutable => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        VfsError::Object(error) => status_from_object_creation_error(error),
        VfsError::Resource(error) => status_from_resource_error(error),
    }
}

const fn status_from_byte_channel_error(error: ByteChannelError) -> HyperNativeStatus {
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

const fn status_from_event_error(error: EventError) -> HyperNativeStatus {
    match error {
        EventError::InvalidSignals => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        EventError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        EventError::Resource(error) => status_from_resource_error(error),
        EventError::SignalWait(error) => status_from_signal_wait_error(error),
    }
}

const fn status_from_object_wait_error(error: ObjectWaitError) -> HyperNativeStatus {
    match error {
        ObjectWaitError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        ObjectWaitError::Deadline(error) => status_from_deadline_error(error),
        ObjectWaitError::InvalidSignals => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ObjectWaitError::Resource(error) => status_from_resource_error(error),
        ObjectWaitError::Signal(error) => status_from_signal_wait_error(error),
        ObjectWaitError::Timer(error) => status_from_timed_wait_error(error),
    }
}

const fn status_from_deadline_error(error: crate::kernel::time::Error) -> HyperNativeStatus {
    match error {
        crate::kernel::time::Error::Conversion(_) | crate::kernel::time::Error::DeadlineTooFar => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        _ => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

const fn status_from_timed_wait_error(error: TimedWaitError) -> HyperNativeStatus {
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

const fn status_from_signal_wait_error(error: SignalWaitError) -> HyperNativeStatus {
    match error {
        SignalWaitError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        SignalWaitError::SequenceExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        SignalWaitError::Scheduler(_) => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

const fn status_from_handle_error(error: HandleError) -> HyperNativeStatus {
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

const fn status_from_resource_error(error: ResourceError) -> HyperNativeStatus {
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

const fn status_from_machine_error(error: MachineError) -> HyperNativeStatus {
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

const fn status_from_address_error(_: AddressError) -> HyperNativeStatus {
    HYPER_NATIVE_STATUS_FAULT
}

const fn status_from_logical_error(
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

const fn status_from_page_error(
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

#[cfg(feature = "kernel-self-test")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelfTestError {
    AbiQuery,
    UnknownNumber,
    InvalidHandle,
    InvalidRights,
    InvalidRecordSize,
    ChannelValidation,
    CapabilityChannelValidation,
    CapabilityChannelErrorMapping,
    ConsoleValidation,
    ConsoleErrorMapping,
    ProcessBuilderValidation,
    ObjectErrorMapping,
    ChannelErrorMapping,
    RecordEncoding,
    ValidationReachedService,
    DeferredDispatch,
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn run_self_test() -> Result<(), SelfTestError> {
    use core::cell::Cell;

    struct RejectingServices {
        calls: Cell<usize>,
    }

    impl RejectingServices {
        fn reached(&self) -> Result<(), SelfTestError> {
            if self.calls.get() == 0 {
                Ok(())
            } else {
                Err(SelfTestError::ValidationReachedService)
            }
        }
    }

    impl UserOutputServices for RejectingServices {
        fn copy_to_user(&self, _: UserSlice, _: &[u8]) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
    }

    impl ImmediateServices for RejectingServices {
        fn close_handle(&self, _: HandleValue) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn handle_info(&self, _: HandleValue, _: Rights) -> Result<HandleInfo, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
    }

    impl AllocatingServices for RejectingServices {
        fn duplicate_handle(&self, _: HandleValue, _: Rights) -> Result<HandleValue, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
        fn replace_handle(&self, _: HandleValue, _: Rights) -> Result<HandleValue, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
        fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }
        fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ByteChannelServiceError::Process(ProcessError::Allocation))
        }
        fn create_capability_channel(
            &self,
        ) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(CapabilityChannelServiceError::Process(
                ProcessError::Allocation,
            ))
        }
    }

    impl DeferredServices for RejectingServices {
        fn signal_event(&self, _: HandleValue, _: u64, _: u64) -> Result<(), ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

        fn wait_one(
            &self,
            _: HandleValue,
            _: u64,
            _: u64,
        ) -> Result<SignalWaitOutcome, ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

        fn wait_many(
            &self,
            _: UserSlice,
            _: usize,
            _: u64,
        ) -> Result<SignalWaitManyOutcome, ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

        fn process_info(&self, _: HandleValue) -> Result<ProcessSnapshot, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn write_byte_channel(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<(), ByteChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ByteChannelServiceError::Process(ProcessError::Allocation))
        }

        fn read_byte_channel(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ByteChannelServiceError::Process(ProcessError::Allocation))
        }

        fn try_send_capability_channel(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
            _: Option<UserSlice>,
        ) -> Result<(), CapabilityChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(CapabilityChannelServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn receive_capability_channel(
            &self,
            _: HandleValue,
            _: u64,
            _: Option<UserSlice>,
            _: Option<UserSlice>,
        ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(CapabilityChannelServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn read_console(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<usize, ConsoleServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ConsoleServiceError::Process(ProcessError::Allocation))
        }

        fn write_console(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<usize, ConsoleServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ConsoleServiceError::Process(ProcessError::Allocation))
        }

        fn open_file(
            &self,
            _: HandleValue,
            _: UserSlice,
            _: Rights,
        ) -> Result<HandleValue, VfsServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(VfsServiceError::Process(ProcessError::Allocation))
        }

        fn open_directory(
            &self,
            _: HandleValue,
            _: UserSlice,
            _: Rights,
        ) -> Result<HandleValue, VfsServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(VfsServiceError::Process(ProcessError::Allocation))
        }

        fn read_file_at(
            &self,
            _: HandleValue,
            _: u64,
            _: Option<UserSlice>,
        ) -> Result<(u64, u64), VfsServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(VfsServiceError::Process(ProcessError::Allocation))
        }

        fn create_vmo(&self, _: u64) -> Result<HandleValue, MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn create_file_executable_vmo(
            &self,
            _: HandleValue,
        ) -> Result<HandleValue, MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn read_vmo(
            &self,
            _: HandleValue,
            _: u64,
            _: Option<UserSlice>,
        ) -> Result<(), MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn write_vmo(
            &self,
            _: HandleValue,
            _: u64,
            _: Option<UserSlice>,
        ) -> Result<(), MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn map_vmo(
            &self,
            _: HandleValue,
            _: HandleValue,
            _: u64,
            _: u64,
            _: u64,
            _: Permissions,
        ) -> Result<(), MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn allocate_vmar(
            &self,
            _: HandleValue,
            _: u64,
            _: u64,
        ) -> Result<HandleValue, MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn protect_vmar(
            &self,
            _: HandleValue,
            _: u64,
            _: u64,
            _: Permissions,
        ) -> Result<(), MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn unmap_vmar(&self, _: HandleValue, _: u64, _: u64) -> Result<(), MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn destroy_vmar(&self, _: HandleValue) -> Result<(), MemoryServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(MemoryServiceError::Process(ProcessError::Allocation))
        }

        fn create_process_builder(
            &self,
            _: HandleValue,
            _: HandleValue,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn set_process_builder_name(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn add_process_builder_argument(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn add_process_builder_environment(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn set_process_builder_affinity(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
            _: usize,
        ) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn add_process_builder_handle(
            &self,
            _: HandleValue,
            _: HandleValue,
            _: u32,
            _: crate::kernel::object::ObjectKind,
            _: Option<Rights>,
            _: crate::kernel::capability::HandleTransferOperation,
        ) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn seal_process_builder(&self, _: HandleValue) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn start_process_builder(
            &self,
            _: HandleValue,
        ) -> Result<HandleValue, ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn abort_process_builder(&self, _: HandleValue) -> Result<(), ProcessBuilderServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessBuilderServiceError::Process(
                ProcessError::Allocation,
            ))
        }

        fn request_process_stop(&self, _: HandleValue) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
    }

    fn invoke(number: u64, arguments: [u64; 6]) -> NativeInvocation {
        NativeInvocation::new(number, arguments, 0x1000)
    }

    let services = RejectingServices {
        calls: Cell::new(0),
    };
    let query = dispatch_immediate(&services, invoke(HYPER_NATIVE_SYS_ABI_QUERY, [u64::MAX; 6]));
    if query.status() != hyper::abi::native::HYPER_NATIVE_STATUS_OK
        || query.values() != &[HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE]
    {
        return Err(SelfTestError::AbiQuery);
    }
    let unknown = dispatch_immediate(&services, invoke(u64::MAX, [u64::MAX; 6]));
    if unknown.status() != HYPER_NATIVE_STATUS_NOT_SUPPORTED || unknown.values() != &[0, 0] {
        return Err(SelfTestError::UnknownNumber);
    }
    if dispatch_deferred(
        &services,
        invoke(HYPER_NATIVE_SYS_THREAD_YIELD, [u64::MAX; 6]),
    ) != DeferredAction::Yield(success([0, 0]))
        || dispatch_deferred(
            &services,
            invoke(HYPER_NATIVE_SYS_THREAD_EXIT, [u64::MAX; 6]),
        ) != (DeferredAction::ExitThread { status: -1 })
        || dispatch_deferred(
            &services,
            invoke(HYPER_NATIVE_SYS_PROCESS_EXIT, [42, 0, 0, 0, 0, 0]),
        ) != (DeferredAction::ExitProcess { status: 42 })
        || dispatch_deferred(&services, invoke(u64::MAX, [u64::MAX; 6]))
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED))
    {
        return Err(SelfTestError::DeferredDispatch);
    }
    let timer_allocation = ObjectWaitError::Timer(TimedWaitError::Time(
        crate::kernel::time::Error::TimerQueue(hyper::time::TimerQueueError::Allocation),
    ));
    if status_from_object_wait_error(timer_allocation) != HYPER_NATIVE_STATUS_NO_MEMORY {
        return Err(SelfTestError::ObjectErrorMapping);
    }
    if status_from_byte_channel_error(ByteChannelError::WouldBlock)
        != HYPER_NATIVE_STATUS_WOULD_BLOCK
        || status_from_byte_channel_error(ByteChannelError::PeerClosed)
            != HYPER_NATIVE_STATUS_PEER_CLOSED
        || status_from_byte_channel_error(ByteChannelError::MessageTooLarge)
            != HYPER_NATIVE_STATUS_INVALID_ARGUMENT
    {
        return Err(SelfTestError::ChannelErrorMapping);
    }
    if status_from_capability_channel_error(CapabilityChannelError::WouldBlock)
        != HYPER_NATIVE_STATUS_WOULD_BLOCK
        || status_from_capability_channel_error(CapabilityChannelError::UserMemoryFault)
            != HYPER_NATIVE_STATUS_FAULT
        || status_from_capability_channel_error(CapabilityChannelError::BufferTooSmall {
            required_bytes: 1,
            required_handles: 1,
        }) != HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL
        || status_from_capability_channel_error(CapabilityChannelError::Cancelled)
            != HYPER_NATIVE_STATUS_CANCELLED
    {
        return Err(SelfTestError::CapabilityChannelErrorMapping);
    }
    if capability_receive_result(Ok(CapabilityReceiveOutcome::Failed(
        CapabilityChannelError::BufferTooSmall {
            required_bytes: 4096,
            required_handles: 16,
        },
    ))) != NativeResult::for_syscall(
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE,
        HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
        [4096, 16],
    ) {
        return Err(SelfTestError::CapabilityChannelErrorMapping);
    }
    if status_from_console_service_error(ConsoleServiceError::Io(
        crate::kernel::device::console::IoError::WouldBlock,
    )) != HYPER_NATIVE_STATUS_WOULD_BLOCK
        || console_io_result(
            HYPER_NATIVE_SYS_CONSOLE_READ,
            Err(HYPER_NATIVE_STATUS_WOULD_BLOCK),
        ) != NativeResult::for_syscall(
            HYPER_NATIVE_SYS_CONSOLE_READ,
            HYPER_NATIVE_STATUS_WOULD_BLOCK,
            [0, 0],
        )
    {
        return Err(SelfTestError::ConsoleErrorMapping);
    }
    let bad_handle = dispatch_immediate(
        &services,
        invoke(HYPER_NATIVE_SYS_HANDLE_CLOSE, [0, 0, 0, 0, 0, 0]),
    );
    if bad_handle.status() != HYPER_NATIVE_STATUS_BAD_HANDLE {
        return Err(SelfTestError::InvalidHandle);
    }
    let bad_rights = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_HANDLE_DUPLICATE,
            [1_u64 << 24 | 1, u64::MAX, 0, 0, 0, 0],
        ),
    );
    if bad_rights != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)) {
        return Err(SelfTestError::InvalidRights);
    }
    let bad_size = dispatch_immediate(
        &services,
        invoke(
            HYPER_NATIVE_SYS_HANDLE_GET_INFO,
            [
                1_u64 << 24 | 1,
                0x2000,
                HANDLE_INFO_SIZE as u64 - 1,
                0,
                0,
                0,
            ],
        ),
    );
    if bad_size.status() != HYPER_NATIVE_STATUS_INVALID_ARGUMENT {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let empty_wait_many = dispatch_deferred(
        &services,
        invoke(HYPER_NATIVE_SYS_OBJECT_WAIT_MANY, [0x2000, 0, 0, 0, 0, 0]),
    );
    let oversized_wait_many = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_OBJECT_WAIT_MANY,
            [
                0x2000,
                hyper::abi::native::HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS + 1,
                0,
                0,
                0,
                0,
            ],
        ),
    );
    let bad_process_info_size = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_PROCESS_GET_INFO,
            [
                1_u64 << 24 | 1,
                0x2000,
                PROCESS_INFO_SIZE as u64 - 1,
                0,
                0,
                0,
            ],
        ),
    );
    if empty_wait_many != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_wait_many
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_process_info_size
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
    {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let bad_channel_create = dispatch_deferred(
        &services,
        invoke(HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE, [1, 0, 0, 0, 0, 0]),
    );
    let bad_channel_write = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE,
            [1_u64 << 24 | 1, 1, 0, 0, 0, 0],
        ),
    );
    let bad_channel_read = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
            [
                1_u64 << 24 | 1,
                0,
                0,
                hyper::abi::native::HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES + 1,
                0,
                0,
            ],
        ),
    );
    if bad_channel_create != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_channel_write
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_channel_read != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
    {
        return Err(SelfTestError::ChannelValidation);
    }
    let bad_capability_create = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE,
            [1, 0, 0, 0, 0, 0],
        ),
    );
    let bad_capability_send_options = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
            [1_u64 << 24 | 1, 1, 0, 0, 0, 0],
        ),
    );
    let oversized_capability_send = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
            [
                1_u64 << 24 | 1,
                0,
                0,
                hyper::abi::native::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES + 1,
                0,
                0,
            ],
        ),
    );
    let oversized_capability_receive = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE,
            [
                1_u64 << 24 | 1,
                0,
                0,
                0,
                0,
                hyper::abi::native::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES + 1,
            ],
        ),
    );
    if bad_capability_create
        != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_capability_send_options
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_capability_send
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_capability_receive
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
    {
        return Err(SelfTestError::CapabilityChannelValidation);
    }
    let bad_console_options = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CONSOLE_READ,
            [1_u64 << 24 | 1, 1, 0, 0, 0, 0],
        ),
    );
    let oversized_console_write = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CONSOLE_WRITE,
            [
                1_u64 << 24 | 1,
                0,
                0x2000,
                hyper::abi::native::HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES + 1,
                0,
                0,
            ],
        ),
    );
    if bad_console_options != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_console_write
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
    {
        return Err(SelfTestError::ConsoleValidation);
    }
    let oversized_builder_name = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME,
            [
                1_u64 << 24 | 1,
                0x2000,
                hyper::abi::native::HYPER_NATIVE_PROCESS_NAME_MAX_BYTES + 1,
                0,
                0,
                0,
            ],
        ),
    );
    let oversized_affinity = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY,
            [
                1_u64 << 24 | 1,
                0x2000,
                hyper::abi::native::HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS + 1,
                0,
                0,
                0,
            ],
        ),
    );
    let bad_process_builder_kind = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
            [
                1_u64 << 24 | 1,
                1_u64 << 24 | 2,
                1,
                u64::MAX,
                0,
                hyper::abi::native::HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE,
            ],
        ),
    );
    let bad_process_builder_operation = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
            [
                1_u64 << 24 | 1,
                1_u64 << 24 | 2,
                1,
                u64::from(hyper::abi::native::HYPER_NATIVE_OBJECT_EVENT),
                0,
                u64::MAX,
            ],
        ),
    );
    let same_rights_builder_handle = parse_builder_handle(&[
        1_u64 << 24 | 1,
        1_u64 << 24 | 2,
        1,
        u64::from(hyper::abi::native::HYPER_NATIVE_OBJECT_EVENT),
        hyper::abi::native::HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS,
        hyper::abi::native::HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE,
    ]);
    if oversized_builder_name
        != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_affinity
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_process_builder_kind
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_process_builder_operation
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || !matches!(
            same_rights_builder_handle,
            Ok((
                _,
                _,
                _,
                _,
                None,
                crate::kernel::capability::HandleTransferOperation::Move
            ))
        )
    {
        return Err(SelfTestError::ProcessBuilderValidation);
    }
    let handle_record = encode_handle_info_fields(0x1122_3344, 0x5566_7788, 0x99aa_bbcc_ddee_ff00);
    if handle_record
        != [
            0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb,
            0xaa, 0x99,
        ]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    let object_record = encode_object_basic_info_fields(0x1122_3344_5566_7788, 0x99aa_bbcc);
    if object_record
        != [
            0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0xcc, 0xbb, 0xaa, 0x99, 0, 0, 0, 0,
        ]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    let process_record = encode_process_info_fields(
        0x1122_3344,
        0x5566_7788,
        0x99aa_bbcc_ddee_ff00,
        0x0123_4567_89ab_cdef,
    );
    if process_record
        != [
            0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb,
            0xaa, 0x99, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    services.reached()
}
