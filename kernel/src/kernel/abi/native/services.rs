// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-oriented service boundaries consumed by Native ABI handlers.
//!
//! Each trait describes one cohesive kernel domain. The dispatch layer uses
//! composite marker traits only to state that one concrete session supplies
//! the complete route set; no operation silently falls back to a production
//! `not supported` implementation.

use crate::kernel::capability::InvocationReceiver;

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
    WaitSet(crate::kernel::object::WaitSetError),
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
    fn resolve_invocation(&self, value: HandleValue) -> Result<InvocationReceiver, ProcessError>;
    fn validate_invocation_context(
        &self,
        receiver: &InvocationReceiver,
    ) -> Result<(), ProcessError>;
    fn close_handle(&self, value: &InvocationReceiver) -> Result<(), ProcessError>;
    fn handle_info(
        &self,
        value: &InvocationReceiver,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError>;
    fn duplicate_handle(
        &self,
        value: &InvocationReceiver,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError>;
    fn replace_handle(
        &self,
        value: &InvocationReceiver,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError>;
}

impl From<crate::kernel::object::WaitSetError> for ObjectServiceError {
    fn from(error: crate::kernel::object::WaitSetError) -> Self {
        Self::WaitSet(error)
    }
}

pub(in crate::kernel) trait ObjectServices {
    #[cfg(feature = "kernel-trace")]
    fn trace_thread_id(&self) -> u64 {
        0
    }
    fn current_process_id(&self) -> u64;
    fn wait_set_create(&self, capacity: usize) -> Result<HandleValue, ObjectServiceError>;
    fn wait_set_add(
        &self,
        set: &InvocationReceiver,
        source: HandleValue,
        signals: u64,
    ) -> Result<u64, ObjectServiceError>;
    fn wait_set_rearm(
        &self,
        set: &InvocationReceiver,
        registration: u64,
    ) -> Result<(), ObjectServiceError>;
    fn wait_set_remove(
        &self,
        set: &InvocationReceiver,
        registration: u64,
    ) -> Result<(), ObjectServiceError>;
    fn wait_set_wait(
        &self,
        set: &InvocationReceiver,
        deadline: u64,
        output: UserSlice,
    ) -> Result<(), ObjectServiceError>;

    fn create_event(&self) -> Result<HandleValue, ObjectServiceError>;
    fn signal_event(
        &self,
        value: &InvocationReceiver,
        clear: u64,
        set: u64,
    ) -> Result<(), ObjectServiceError>;
    fn wait_one(
        &self,
        value: &InvocationReceiver,
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
        endpoint: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<(), ByteChannelServiceError>;
    fn read_byte_channel(
        &self,
        endpoint: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError>;
    fn try_send_capability_channel(
        &self,
        endpoint: &InvocationReceiver,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
    ) -> Result<(), CapabilityChannelServiceError>;
    fn receive_capability_channel(
        &self,
        endpoint: &InvocationReceiver,
        deadline: u64,
        bytes: Option<UserSlice>,
        slots: Option<UserSlice>,
    ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError>;
}

pub(in crate::kernel) trait HierarchyServices: UserMemoryServices {
    fn create_resource_domain(
        &self,
        parent: &InvocationReceiver,
        limits: crate::kernel::accounting::ResourceLimits,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error>;
    fn create_task_group(
        &self,
        factory: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error>;
}

pub(in crate::kernel) trait DeviceServices: UserMemoryServices {
    fn device_firmware_read(
        &self,
        authority: &InvocationReceiver,
        node: u32,
        field: u32,
        name: &str,
    ) -> Result<alloc::vec::Vec<u8>, crate::kernel::device::assigned::service::MatchError>;
    fn device_claim_bundle(
        &self,
        authority: &InvocationReceiver,
        entries: &[(u32, u32, u64)],
        irq_node: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn device_mmio(
        &self,
        device: &InvocationReceiver,
        offset: u64,
        width: u32,
        write: bool,
        value: u64,
    ) -> Result<u64, crate::kernel::vm::service::Error>;
    fn device_irq_pending(
        &self,
        device: &InvocationReceiver,
    ) -> Result<u64, crate::kernel::vm::service::Error>;
    fn device_irq_complete(
        &self,
        device: &InvocationReceiver,
        sequence: u64,
        asserted: bool,
    ) -> Result<(), crate::kernel::vm::service::Error>;

    fn device_dma_fault_info(
        &self,
        device: &InvocationReceiver,
    ) -> Result<[u8; 40], crate::kernel::vm::service::Error>;

    fn device_profile_info(
        &self,
        device: &InvocationReceiver,
    ) -> Result<[u8; 32], crate::kernel::vm::service::Error>;
    fn device_resource_info(
        &self,
        device: &InvocationReceiver,
        index: u32,
    ) -> Result<[u8; 32], crate::kernel::vm::service::Error>;
    fn claim_device_matching(
        &self,
        authority: &InvocationReceiver,
        profile: u32,
        identity_kind: u32,
        identity: &str,
    ) -> Result<HandleValue, crate::kernel::device::assigned::service::MatchError>;
    fn claim_device(
        &self,
        authority: &InvocationReceiver,
        index: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn physical_device_info(
        &self,
        device: &InvocationReceiver,
    ) -> Result<crate::kernel::device::assigned::Info, crate::kernel::vm::service::Error>;
    fn vmo_dma_extent(
        &self,
        authority: &InvocationReceiver,
        vmo: HandleValue,
        offset: u64,
        length: u64,
    ) -> Result<crate::kernel::device::assigned::DmaExtent, crate::kernel::vm::service::Error>;
    fn assign_physical_device(
        &self,
        pending: &InvocationReceiver,
        device: HandleValue,
        base: u64,
        irq: u32,
    ) -> Result<(), crate::kernel::vm::service::Error>;
}

pub(in crate::kernel) trait GuestIoServices: UserMemoryServices {
    fn transfer_guest_memory(
        &self,
        _memory: &InvocationReceiver,
        _write: bool,
        _offset: u64,
        _buffer: UserSlice,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }
    fn scalar_guest_memory(
        &self,
        _memory: &InvocationReceiver,
        _write: bool,
        _offset: u64,
        _width: u32,
        _value: u64,
    ) -> Result<u64, crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }
    fn create_guest_interrupt(
        &self,
        _machine: &InvocationReceiver,
        _irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }
    fn set_guest_interrupt_level(
        &self,
        _interrupt: &InvocationReceiver,
        _asserted: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }

    fn create_guest_mapping(
        &self,
        _backend: &InvocationReceiver,
        _memory: HandleValue,
        _frontend: u64,
    ) -> Result<(HandleValue, u64), crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }
    fn release_guest_mapping(
        &self,
        _mapping: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }
    fn create_native_block(
        &self,
        _memory: &InvocationReceiver,
        _backend: HandleValue,
        _guest_base: u64,
        _notification_base: u64,
        _notification_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        Err(crate::kernel::vm::service::Error::NotSupported)
    }
    fn activate_native_block(
        &self,
        _block: &InvocationReceiver,
        _readonly: bool,
    ) -> Result<(u64, bool), crate::kernel::block::service::ActivationError> {
        Err(crate::kernel::block::service::ActivationError::Device(
            hyper::fs::block::Error::Unsupported,
        ))
    }
    fn transfer_native_block(
        &self,
        _block: &InvocationReceiver,
        _operation: u32,
        _first: u64,
        _buffer: Option<UserSlice>,
    ) -> Result<(), crate::kernel::block::service::TransferError> {
        Err(crate::kernel::block::service::TransferError::Status(
            hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        ))
    }
    fn mount_filesystem(
        &self,
        _channel: HandleValue,
        _buffer: HandleValue,
        _directory: &InvocationReceiver,
        _path: UserSlice,
        _read_only: bool,
    ) -> Result<(), crate::kernel::vfs::VfsServiceError> {
        Err(crate::kernel::vfs::VfsServiceError::InvalidInput)
    }

    fn unmount_filesystem(
        &self,
        _directory: &InvocationReceiver,
        _path: UserSlice,
    ) -> Result<(), crate::kernel::vfs::VfsServiceError> {
        Err(crate::kernel::vfs::VfsServiceError::InvalidInput)
    }

    fn create_guest_mailbox(
        &self,
        machine: &InvocationReceiver,
        base: u64,
        irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn send_guest_mailbox(
        &self,
        mailbox: &InvocationReceiver,
        bytes: &[u8],
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn receive_guest_mailbox(
        &self,
        mailbox: &InvocationReceiver,
        copy: &mut dyn FnMut(&[u8]) -> Result<(), crate::kernel::vm::service::Error>,
    ) -> Result<usize, crate::kernel::vm::service::Error>;
    fn create_guest_notification(
        &self,
        frontend: &InvocationReceiver,
        backend: HandleValue,
        frontend_base: u64,
        backend_base: u64,
        frontend_irq: u32,
        backend_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn access_guest_notification(
        &self,
        machine: &InvocationReceiver,
        original: u64,
        operation: u32,
        value: u32,
    ) -> Result<u32, crate::kernel::vm::service::Error>;
    fn configure_guest_notification(
        &self,
        machine: &InvocationReceiver,
        original: u64,
        base: u64,
        flags: u32,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn control_guest_notification(
        &self,
        notification: &InvocationReceiver,
        operation: u32,
    ) -> Result<u32, crate::kernel::vm::service::Error>;
}

pub(in crate::kernel) trait VmServices: UserMemoryServices {
    fn virtual_machine_platform_info(
        &self,
        lease: &InvocationReceiver,
        profile: u32,
    ) -> Result<
        crate::kernel::vm::service::VirtualMachinePlatformInfo,
        crate::kernel::vm::service::Error,
    >;
    fn derive_virtual_machine_creation_lease(
        &self,
        authority: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn create_pending_virtual_machine(
        &self,
        lease: &InvocationReceiver,
        configuration: crate::kernel::vm::objects::VirtualMachineConfiguration,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn set_pending_virtual_machine_memory(
        &self,
        pending: &InvocationReceiver,
        vmo: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn set_pending_virtual_machine_bootstrap(
        &self,
        pending: &InvocationReceiver,
        bootstrap: crate::kernel::vm::objects::VirtualCpuBootstrap,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn set_pending_virtual_machine_virtual_serial(
        &self,
        pending: &InvocationReceiver,
        console: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn create_virtual_serial(&self) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn register_virtual_serial_output(
        &self,
        serial: &InvocationReceiver,
        buffer: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn acknowledge_virtual_serial_output(
        &self,
        serial: &InvocationReceiver,
        consumed: u64,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn write_virtual_serial(
        &self,
        serial: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<usize, crate::kernel::vm::service::Error>;
    fn seal_pending_virtual_machine(
        &self,
        pending: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn install_pending_virtual_machine(
        &self,
        pending: &InvocationReceiver,
    ) -> Result<[HandleValue; 2], crate::kernel::vm::service::Error>;
    fn abort_pending_virtual_machine(
        &self,
        pending: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn request_virtual_machine_stop(
        &self,
        machine: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn register_mmio(
        &self,
        machine: &InvocationReceiver,
        base: u64,
        length: u64,
        device: u64,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn pending_mmio(
        &self,
        vcpu: &InvocationReceiver,
    ) -> Result<Option<hyper::vm::device::mmio::Request>, crate::kernel::vm::service::Error>;
    fn complete_mmio(
        &self,
        vcpu: &InvocationReceiver,
        id: u64,
        action: hyper::vm::exit::MmioAction,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn create_guest_memory(
        &self,
        vmo: &InvocationReceiver,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn map_guest_memory(
        &self,
        pending: &InvocationReceiver,
        memory: HandleValue,
        guest_offset: u64,
        source_offset: u64,
        length: u64,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn pending_power_request(
        &self,
        machine: &InvocationReceiver,
    ) -> Result<Option<hyper::vm::arm::psci::Request>, crate::kernel::vm::service::Error>;
    fn complete_power_request(
        &self,
        machine: &InvocationReceiver,
        request_id: u64,
        accept: bool,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn open_vcpu(
        &self,
        machine: &InvocationReceiver,
        vcpu: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error>;
    fn virtual_machine_info(
        &self,
        machine: &InvocationReceiver,
    ) -> Result<
        (
            crate::kernel::vm::objects::VirtualMachineConfiguration,
            crate::kernel::vm::objects::VirtualMachineSnapshot,
        ),
        crate::kernel::vm::service::Error,
    >;
    fn virtual_cpu_info(
        &self,
        vcpu: &InvocationReceiver,
    ) -> Result<crate::kernel::vm::objects::VirtualCpuSnapshot, crate::kernel::vm::service::Error>;
    fn set_virtual_cpu_affinity(
        &self,
        vcpu: &InvocationReceiver,
        words: Option<UserSlice>,
        word_count: usize,
    ) -> Result<(), crate::kernel::vm::service::Error>;
    fn start_virtual_cpu(
        &self,
        vcpu: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error>;
}

pub(in crate::kernel) trait SystemInspectServices: UserMemoryServices {
    fn profile_start(
        &self,
        _receiver: &InvocationReceiver,
        _period: u64,
        _seconds: u64,
    ) -> crate::kernel::profile::Result<u64> {
        Err(hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    }
    fn profile_stop(
        &self,
        _receiver: &InvocationReceiver,
        _generation: u64,
    ) -> crate::kernel::profile::Result<()> {
        Err(hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    }
    fn profile_read(
        &self,
        _receiver: &InvocationReceiver,
        _generation: u64,
        _cpu: u64,
        _cursor: u64,
        _output: &mut [u8],
    ) -> crate::kernel::profile::Result<usize> {
        Err(hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    }

    fn trace_start(
        &self,
        _receiver: &InvocationReceiver,
        _config: &[u8; 56],
    ) -> crate::kernel::trace::Result<u64> {
        Err(hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    }
    fn trace_stop(
        &self,
        _receiver: &InvocationReceiver,
        _generation: u64,
    ) -> crate::kernel::trace::Result<()> {
        Err(hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    }
    fn trace_read(
        &self,
        _receiver: &InvocationReceiver,
        _generation: u64,
        _cpu: u64,
        _cursor: u64,
        _output: &mut [u8],
    ) -> crate::kernel::trace::Result<usize> {
        Err(hyper::abi::native::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    }

    fn memory_statistics(
        &self,
        inspector: &InvocationReceiver,
        query: crate::kernel::inspect::memory::Query,
    ) -> Result<crate::kernel::inspect::memory::Statistics, crate::kernel::inspect::Error>;
    fn memory_observation(
        &self,
        inspector: &InvocationReceiver,
    ) -> Result<crate::kernel::inspect::MemoryObservation, crate::kernel::inspect::Error>;
    fn cpu_observation(
        &self,
        inspector: &InvocationReceiver,
    ) -> Result<crate::kernel::task::scheduler::CpuTimeSnapshot, crate::kernel::inspect::Error>;
}

pub(in crate::kernel) trait InspectServices: UserMemoryServices {
    fn read_object_details(
        &self,
        inspector: &InvocationReceiver,
        process: u64,
        target: u64,
        cursor: u64,
    ) -> Result<crate::kernel::object::diagnostics::ObjectDetails, crate::kernel::inspect::Error>;

    fn scan_processes(
        &self,
        inspector: &InvocationReceiver,
        cursor: u64,
        output: &mut Page<ProcessSnapshot, { crate::kernel::inspect::PROCESS_PAGE_CAPACITY }>,
    ) -> Result<(), crate::kernel::inspect::Error>;
    /// Fills caller-owned snapshot storage. Contents are publishable only on success.
    fn scan_threads(
        &self,
        inspector: &InvocationReceiver,
        cursor: u64,
        output: &mut Page<
            crate::kernel::inspect::TaskThreadSnapshot,
            { crate::kernel::inspect::THREAD_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error>;
    fn scan_objects(
        &self,
        inspector: &InvocationReceiver,
        cursor: u64,
        output: &mut Page<crate::kernel::object::ObjectSnapshot, OBJECT_PAGE_CAPACITY>,
    ) -> Result<(), crate::kernel::inspect::Error>;
    fn scan_process_handles(
        &self,
        inspector: &InvocationReceiver,
        process_koid: u64,
        cursor: u64,
    ) -> Result<
        Page<crate::kernel::inspect::ProcessHandleSnapshot, HANDLE_PAGE_CAPACITY>,
        crate::kernel::inspect::Error,
    >;
    fn derive_task_inspector(
        &self,
        inspector: &InvocationReceiver,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_object_inspector(
        &self,
        inspector: &InvocationReceiver,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_task_inspector_for_task_group(
        &self,
        inspector: &InvocationReceiver,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_object_inspector_for_task_group(
        &self,
        inspector: &InvocationReceiver,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_task_inspector_for_resource_domain(
        &self,
        inspector: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
    fn derive_object_inspector_for_resource_domain(
        &self,
        inspector: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error>;
}

pub(in crate::kernel) trait ConsoleServices {
    fn discard_console_input(
        &self,
        console: &InvocationReceiver,
    ) -> Result<usize, ConsoleServiceError>;
    fn read_console(
        &self,
        console: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<usize, ConsoleServiceError>;
    fn write_console(
        &self,
        console: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<usize, ConsoleServiceError>;
}

pub(in crate::kernel) trait VfsServices: UserMemoryServices {
    fn mount_ramfs(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<(), VfsServiceError>;
    fn switch_root(
        &self,
        directory: &InvocationReceiver,
        request: UserSlice,
    ) -> Result<(), VfsServiceError>;

    fn begin_root_switch(
        &self,
        directory: &InvocationReceiver,
        request: UserSlice,
    ) -> Result<HandleValue, VfsServiceError>;
    fn finish_root_switch(
        &self,
        token: &InvocationReceiver,
        commit: bool,
    ) -> Result<(), VfsServiceError>;
    fn directory_scope_create(
        &self,
        root: &InvocationReceiver,
        start: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError>;
    fn directory_get_metadata(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        follow: bool,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError>;
    fn file_get_metadata(
        &self,
        file: &InvocationReceiver,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError>;
    fn directory_get_self_metadata(
        &self,
        directory: &InvocationReceiver,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError>;
    fn directory_set_metadata(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        follow: bool,
        update: crate::kernel::vfs::MetadataUpdate,
    ) -> Result<(), VfsServiceError>;
    fn file_set_metadata(
        &self,
        file: &InvocationReceiver,
        update: crate::kernel::vfs::MetadataUpdate,
    ) -> Result<(), VfsServiceError>;
    fn directory_rename(
        &self,
        source: &InvocationReceiver,
        path: UserSlice,
        destination: HandleValue,
        new_path: UserSlice,
    ) -> Result<(), VfsServiceError>;
    fn directory_link(
        &self,
        source: &InvocationReceiver,
        path: UserSlice,
        destination: HandleValue,
        new_path: UserSlice,
    ) -> Result<(), VfsServiceError>;
    fn directory_symlink(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        target: UserSlice,
    ) -> Result<(), VfsServiceError>;
    fn directory_read_link(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<crate::kernel::vfs::ScratchVec<u8>, VfsServiceError>;
    fn directory_canonicalize(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<crate::kernel::vfs::ScratchString, VfsServiceError>;
    fn directory_remove_if(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        is_directory: bool,
        expected: u64,
    ) -> Result<(), VfsServiceError>;
    fn directory_open_directory_nofollow(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError>;
    fn file_sync(&self, file: &InvocationReceiver, scope: u64) -> Result<(), VfsServiceError>;
    fn file_lock(
        &self,
        file: &InvocationReceiver,
        mode: crate::kernel::vfs::locks::LockMode,
        deadline: u64,
    ) -> Result<(), VfsServiceError>;
    fn file_unlock(&self, file: &InvocationReceiver) -> Result<(), VfsServiceError>;
    fn directory_open_file_with_options(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
        options: u64,
        mode: u32,
    ) -> Result<HandleValue, VfsServiceError>;

    fn create_file(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
        mode: u32,
    ) -> Result<HandleValue, VfsServiceError>;
    fn create_directory(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        mode: u32,
    ) -> Result<(), VfsServiceError>;
    fn remove_entry(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
        is_directory: bool,
    ) -> Result<(), VfsServiceError>;
    fn resize_file(&self, file: &InvocationReceiver, length: u64) -> Result<(), VfsServiceError>;
    fn write_file_at(
        &self,
        file: &InvocationReceiver,
        offset: Option<u64>,
        input: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError>;

    fn open_file(
        &self,
        root: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError>;
    fn open_directory(
        &self,
        root: &InvocationReceiver,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError>;
    fn begin_directory_read(
        &self,
        directory: &InvocationReceiver,
    ) -> Result<HandleValue, VfsServiceError>;
    fn read_directory(
        &self,
        directory: &InvocationReceiver,
        cookie: u64,
        page: &mut DirectoryPage,
    ) -> Result<(), VfsServiceError>;
    fn read_file_at(
        &self,
        file: &InvocationReceiver,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError>;
    fn protect_file(
        &self,
        file: &InvocationReceiver,
    ) -> Result<(HandleValue, u64), VfsServiceError>;
    fn file_info(&self, file: &InvocationReceiver) -> Result<FileInfo, VfsServiceError>;
    fn directory_info(
        &self,
        directory: &InvocationReceiver,
    ) -> Result<DirectoryInfo, VfsServiceError>;
}

pub(in crate::kernel) trait MemoryServices {
    fn create_contiguous_vmo(&self, size: u64) -> Result<HandleValue, MemoryServiceError>;
    fn create_vmo(&self, size: u64) -> Result<HandleValue, MemoryServiceError>;
    fn create_vmo_snapshot(
        &self,
        vmo: &InvocationReceiver,
    ) -> Result<(HandleValue, u64), MemoryServiceError>;
    fn map_file(
        &self,
        vmar: &InvocationReceiver,
        file: HandleValue,
        protection: HandleValue,
        request: crate::kernel::mm::user_space::PrivateMappingRequest,
    ) -> Result<(), MemoryServiceError>;
    fn map_private(
        &self,
        vmar: &InvocationReceiver,
        snapshot: HandleValue,
        request: crate::kernel::mm::user_space::PrivateMappingRequest,
    ) -> Result<(), MemoryServiceError>;
    fn read_vmo(
        &self,
        vmo: &InvocationReceiver,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(), MemoryServiceError>;
    fn write_vmo(
        &self,
        vmo: &InvocationReceiver,
        offset: u64,
        input: Option<UserSlice>,
    ) -> Result<(), MemoryServiceError>;
    #[allow(clippy::too_many_arguments)]
    fn map_vmo(
        &self,
        vmar: &InvocationReceiver,
        vmo: HandleValue,
        vmo_offset: u64,
        address: u64,
        size: u64,
        permissions: Permissions,
    ) -> Result<(), MemoryServiceError>;
    fn set_memory_residency(
        &self,
        vmar: &InvocationReceiver,
        address: u64,
        size: u64,
        current: bool,
        lock: bool,
    ) -> Result<(), MemoryServiceError>;
    fn allocate_vmar(
        &self,
        parent: &InvocationReceiver,
        address: u64,
        size: u64,
        exact: bool,
    ) -> Result<(HandleValue, u64), MemoryServiceError>;
    fn protect_vmar(
        &self,
        vmar: &InvocationReceiver,
        address: u64,
        size: u64,
        permissions: Permissions,
    ) -> Result<(), MemoryServiceError>;
    fn unmap_vmar(
        &self,
        vmar: &InvocationReceiver,
        address: u64,
        size: u64,
    ) -> Result<(), MemoryServiceError>;
    fn destroy_vmar(&self, vmar: &InvocationReceiver) -> Result<(), MemoryServiceError>;
}

pub(in crate::kernel) trait ProcessBuilderServices {
    fn create_process_builder(
        &self,
        factory: &InvocationReceiver,
        group: HandleValue,
        domain: HandleValue,
        executable: HandleValue,
        loader: HandleValue,
    ) -> Result<HandleValue, ProcessBuilderServiceError>;
    fn set_process_builder_name(
        &self,
        builder: &InvocationReceiver,
        name: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError>;
    fn set_process_builder_data(
        &self,
        builder: &InvocationReceiver,
        data: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError>;
    fn set_process_builder_affinity(
        &self,
        builder: &InvocationReceiver,
        words: Option<UserSlice>,
        word_count: usize,
    ) -> Result<(), ProcessBuilderServiceError>;
    fn set_process_builder_memory_lock(
        &self,
        builder: &InvocationReceiver,
        enabled: bool,
    ) -> Result<(), ProcessBuilderServiceError>;
    fn add_process_builder_handle(
        &self,
        builder: &InvocationReceiver,
        source: HandleValue,
        purpose: u32,
        expected_kind: crate::kernel::object::ObjectKind,
        requested_rights: Option<Rights>,
        operation: crate::kernel::capability::HandleTransferOperation,
    ) -> Result<(), ProcessBuilderServiceError>;
    fn seal_process_builder(
        &self,
        builder: &InvocationReceiver,
    ) -> Result<(), ProcessBuilderServiceError>;
    fn start_process_builder(
        &self,
        builder: &InvocationReceiver,
    ) -> Result<[HandleValue; 2], ProcessBuilderServiceError>;
    fn abort_process_builder(
        &self,
        builder: &InvocationReceiver,
    ) -> Result<(), ProcessBuilderServiceError>;
}

pub(in crate::kernel) trait TaskServices: UserMemoryServices {
    fn get_scheduling(
        &self,
        _receiver: &InvocationReceiver,
    ) -> Result<crate::kernel::task::policy::SchedulingParameters, ObjectServiceError> {
        Err(ObjectServiceError::InvalidInput)
    }
    fn set_scheduling(
        &self,
        _receiver: &InvocationReceiver,
        _parameters: crate::kernel::task::policy::SchedulingParameters,
        _delegated: Option<Rights>,
    ) -> Result<(), ObjectServiceError> {
        Err(ObjectServiceError::InvalidInput)
    }
    fn set_current_thread_name(&self, name: &str) -> Result<(), ProcessError>;
    #[allow(
        clippy::too_many_arguments,
        reason = "validated Native entry registers plus captured scheduling authority"
    )]
    fn create_thread(
        &self,
        scheduling_rights: Rights,
        entry: u64,
        stack: u64,
        tls: u64,
        argument: u64,
        affinity_words: Option<UserSlice>,
        affinity_word_count: usize,
    ) -> Result<HandleValue, ObjectServiceError>;
    fn start_thread(&self, thread: &InvocationReceiver) -> Result<(), ProcessError>;
    fn stop_thread(&self, thread: &InvocationReceiver) -> Result<(), ProcessError>;
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
    fn process_info(&self, process: &InvocationReceiver) -> Result<ProcessSnapshot, ProcessError>;
    fn request_process_stop(&self, process: &InvocationReceiver) -> Result<(), ProcessError>;
    fn derive_process_observer(
        &self,
        process: &InvocationReceiver,
    ) -> Result<HandleValue, ProcessError>;
    fn request_task_group_stop(&self, group: &InvocationReceiver) -> Result<(), ProcessError>;
}

pub(in crate::kernel) trait RetentionServices: UserMemoryServices {
    fn retain_process_builder(
        &self,
        _builder: &InvocationReceiver,
        _bytes: u64,
        _timeout: u64,
    ) -> Result<HandleValue, ProcessBuilderServiceError> {
        Err(ProcessBuilderServiceError::InvalidInput)
    }
    fn retention_info(
        &self,
        _handle: &InvocationReceiver,
        _view: bool,
    ) -> Result<crate::kernel::process::retention::Info, crate::kernel::process::retention::Error>
    {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
    fn retention_open_view(
        &self,
        _handle: &InvocationReceiver,
    ) -> Result<HandleValue, crate::kernel::process::retention::Error> {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
    fn retention_release(
        &self,
        _handle: &InvocationReceiver,
    ) -> Result<(), crate::kernel::process::retention::Error> {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
    fn crash_thread(
        &self,
        _handle: &InvocationReceiver,
        _index: usize,
    ) -> Result<
        (crate::kernel::process::retention::ThreadEvidence, usize),
        crate::kernel::process::retention::Error,
    > {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
    fn crash_mapping(
        &self,
        _handle: &InvocationReceiver,
        _index: usize,
    ) -> Result<
        (crate::kernel::mm::user_space::CrashMapping, usize),
        crate::kernel::process::retention::Error,
    > {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
    fn crash_handle(
        &self,
        _handle: &InvocationReceiver,
        _index: usize,
    ) -> Result<
        (crate::kernel::capability::HandleSnapshot, usize),
        crate::kernel::process::retention::Error,
    > {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
    fn crash_read(
        &self,
        _handle: &InvocationReceiver,
        _address: u64,
        _output: UserSlice,
    ) -> Result<usize, crate::kernel::process::retention::Error> {
        Err(crate::kernel::process::retention::Error::Invalid)
    }
}

pub(in crate::kernel) trait NativeServices:
    UserMemoryServices
    + HandleServices
    + ObjectServices
    + IpcServices
    + HierarchyServices
    + VmServices
    + DeviceServices
    + GuestIoServices
    + SystemInspectServices
    + InspectServices
    + ConsoleServices
    + VfsServices
    + MemoryServices
    + ProcessBuilderServices
    + RetentionServices
    + TaskServices
{
}

impl<T> NativeServices for T where
    T: UserMemoryServices
        + HandleServices
        + ObjectServices
        + IpcServices
        + HierarchyServices
        + VmServices
        + DeviceServices
        + GuestIoServices
        + SystemInspectServices
        + InspectServices
        + ConsoleServices
        + VfsServices
        + MemoryServices
        + ProcessBuilderServices
        + RetentionServices
        + TaskServices
{
}

/// Policy action produced by an ordinary Native syscall.
///
/// This value owns no architecture frame or CPU-affine state. The Thread
/// continuation applies the scheduling or lifecycle effect before user return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel) enum NativeAction {
    Return(NativeResult),
    Yield(NativeResult),
    ExitThread { status: i64 },
    ExitProcess { status: i64 },
}
