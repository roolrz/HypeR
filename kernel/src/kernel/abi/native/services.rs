// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-oriented service boundaries consumed by Native ABI handlers.
//!
//! Each trait describes one cohesive kernel domain. The dispatch layer uses
//! composite marker traits only to state that one concrete session supplies
//! the complete route set; no operation silently falls back to a production
//! `not supported` implementation.

use hyper::abi::native::NativeResult;

use crate::kernel::capability::{HandleInfo, HandleValue, Rights};
use crate::kernel::inspect::{HANDLE_PAGE_CAPACITY, OBJECT_PAGE_CAPACITY, Page};
use crate::kernel::ipc::{
    ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelServiceError,
    CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::{MemoryServiceError, Permissions, UserSlice};
use crate::kernel::object::{
    EventError, ObjectWaitError, SignalWaitManyOutcome, SignalWaitOutcome,
};
use crate::kernel::process::{
    ChildProcessStartError, ProcessBuilderError, ProcessError, ProcessSnapshot,
};
use crate::kernel::vfs::{DirectoryInfo, DirectoryPage, FileInfo, VfsServiceError};

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

pub(in crate::kernel) trait UserMemoryServices {
    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError>;
    fn copy_from_user(&self, source: UserSlice, destination: &mut [u8])
    -> Result<(), ProcessError>;
}

pub(in crate::kernel) trait HandleServices {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError>;
    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError>;
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
}

pub(in crate::kernel) trait ObjectServices {
    fn create_event(&self) -> Result<HandleValue, ObjectServiceError>;
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
}

pub(in crate::kernel) trait IpcServices {
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError>;
    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError>;
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
}

pub(in crate::kernel) trait HierarchyServices: UserMemoryServices {
    fn create_resource_domain(
        &self,
        parent: HandleValue,
        limits: crate::kernel::accounting::ResourceLimits,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error>;
    fn create_task_group(
        &self,
        factory: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error>;
}

pub(in crate::kernel) trait VmServices: UserMemoryServices {
    fn derive_virtual_machine_creation_lease(
        &self,
        authority: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn create_pending_virtual_machine(
        &self,
        lease: HandleValue,
        configuration: crate::kernel::vm::objects::VirtualMachineConfiguration,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn set_pending_virtual_machine_memory(
        &self,
        pending: HandleValue,
        vmo: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn set_pending_virtual_machine_bootstrap(
        &self,
        pending: HandleValue,
        bootstrap: crate::kernel::vm::objects::VirtualCpuBootstrap,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn set_pending_virtual_machine_virtual_serial(
        &self,
        pending: HandleValue,
        console: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn create_virtual_serial(&self) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn register_virtual_serial_output(
        &self,
        serial: HandleValue,
        buffer: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn write_virtual_serial(
        &self,
        serial: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<usize, crate::kernel::vm::service::Error>;
    fn seal_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn install_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<[HandleValue; 2], crate::kernel::vm::service::Error>;
    fn abort_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn request_virtual_machine_stop(
        &self,
        machine: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn virtual_machine_info(
        &self,
        machine: HandleValue,
    ) -> Result<
        (
            crate::kernel::vm::objects::VirtualMachineConfiguration,
            crate::kernel::vm::objects::VirtualMachineSnapshot,
        ),
        crate::kernel::vm::service::Error,
    >;
    fn virtual_cpu_info(
        &self,
        vcpu: HandleValue,
    ) -> Result<crate::kernel::vm::objects::VirtualCpuSnapshot, crate::kernel::vm::service::Error>;
    fn start_virtual_cpu(&self, vcpu: HandleValue)
    -> Result<(), crate::kernel::vm::service::Error>;
}

pub(in crate::kernel) trait SystemInspectServices: UserMemoryServices {
    fn memory_observation(
        &self,
        inspector: HandleValue,
    ) -> Result<crate::kernel::inspect::MemoryObservation, crate::kernel::inspect::Error>;
    fn cpu_observation(
        &self,
        inspector: HandleValue,
    ) -> Result<crate::kernel::task::scheduler::CpuTimeSnapshot, crate::kernel::inspect::Error>;
}

pub(in crate::kernel) trait InspectServices: UserMemoryServices {
    fn scan_processes(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        Page<ProcessSnapshot, { crate::kernel::inspect::PROCESS_PAGE_CAPACITY }>,
        crate::kernel::inspect::Error,
    >;
    fn scan_threads(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        Page<
            crate::kernel::inspect::TaskThreadSnapshot,
            { crate::kernel::inspect::THREAD_PAGE_CAPACITY },
        >,
        crate::kernel::inspect::Error,
    >;
    fn scan_objects(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        Page<crate::kernel::object::ObjectSnapshot, OBJECT_PAGE_CAPACITY>,
        crate::kernel::inspect::Error,
    >;
    fn scan_process_handles(
        &self,
        inspector: HandleValue,
        process_koid: u64,
        cursor: u64,
    ) -> Result<
        Page<crate::kernel::inspect::ProcessHandleSnapshot, HANDLE_PAGE_CAPACITY>,
        crate::kernel::inspect::Error,
    >;
    fn derive_task_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_object_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_task_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_object_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_task_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_object_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
}

pub(in crate::kernel) trait ConsoleServices {
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
}

pub(in crate::kernel) trait VfsServices: UserMemoryServices {
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
    fn read_directory(
        &self,
        directory: HandleValue,
        cookie: u64,
    ) -> Result<DirectoryPage, VfsServiceError>;
    fn read_file_at(
        &self,
        file: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError>;
    fn file_info(&self, file: HandleValue) -> Result<FileInfo, VfsServiceError>;
    fn directory_info(&self, directory: HandleValue) -> Result<DirectoryInfo, VfsServiceError>;
}

pub(in crate::kernel) trait MemoryServices {
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
}

pub(in crate::kernel) trait ProcessBuilderServices {
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
}

pub(in crate::kernel) trait TaskServices: UserMemoryServices {
    fn create_thread(
        &self,
        entry: u64,
        stack: u64,
        tls: u64,
        argument: u64,
    ) -> Result<HandleValue, ObjectServiceError>;
    fn start_thread(&self, thread: HandleValue) -> Result<(), ProcessError>;
    fn stop_thread(&self, thread: HandleValue) -> Result<(), ProcessError>;
    fn atomic_wait(
        &self,
        address: u64,
        expected: u32,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError>;
    fn atomic_wake(&self, address: u64, count: u32) -> Result<u64, ObjectServiceError>;
    fn sleep_thread(
        &self,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError>;
    fn process_info(&self, process: HandleValue) -> Result<ProcessSnapshot, ProcessError>;
    fn request_process_stop(&self, process: HandleValue) -> Result<(), ProcessError>;
}

pub(in crate::kernel) trait ImmediateServices:
    UserMemoryServices + HandleServices
{
}

impl<T: UserMemoryServices + HandleServices> ImmediateServices for T {}

pub(in crate::kernel) trait DeferredServices:
    UserMemoryServices
    + HandleServices
    + ObjectServices
    + IpcServices
    + HierarchyServices
    + VmServices
    + SystemInspectServices
    + InspectServices
    + ConsoleServices
    + VfsServices
    + MemoryServices
    + ProcessBuilderServices
    + TaskServices
{
}

impl<T> DeferredServices for T where
    T: UserMemoryServices
        + HandleServices
        + ObjectServices
        + IpcServices
        + HierarchyServices
        + VmServices
        + SystemInspectServices
        + InspectServices
        + ConsoleServices
        + VfsServices
        + MemoryServices
        + ProcessBuilderServices
        + TaskServices
{
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
