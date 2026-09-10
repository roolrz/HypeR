// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Boot-time Native ABI contract self-test.

use hyper::abi::native::{
    HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES,
    HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES, HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES,
    HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE, HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS,
    HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES, HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY,
    HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES, HYPER_NATIVE_FEATURE_CORE,
    HYPER_NATIVE_HANDLE_INFO_MIN_SIZE, HYPER_NATIVE_OBJECT_EVENT,
    HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS, HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS,
    HYPER_NATIVE_PROCESS_NAME_MAX_BYTES, HYPER_NATIVE_STATUS_BAD_HANDLE,
    HYPER_NATIVE_STATUS_BAD_STATE, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, HYPER_NATIVE_STATUS_BUSY,
    HYPER_NATIVE_STATUS_CANCELLED, HYPER_NATIVE_STATUS_FAULT, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HYPER_NATIVE_STATUS_NO_MEMORY, HYPER_NATIVE_STATUS_NOT_SUPPORTED, HYPER_NATIVE_STATUS_OK,
    HYPER_NATIVE_STATUS_PEER_CLOSED, HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
    HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_SYS_ABI_QUERY,
    HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE, HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
    HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE, HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE,
    HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE, HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
    HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC, HYPER_NATIVE_SYS_CONSOLE_READ,
    HYPER_NATIVE_SYS_CONSOLE_WRITE, HYPER_NATIVE_SYS_DIRECTORY_GET_INFO,
    HYPER_NATIVE_SYS_DIRECTORY_READ, HYPER_NATIVE_SYS_FILE_GET_INFO, HYPER_NATIVE_SYS_HANDLE_CLOSE,
    HYPER_NATIVE_SYS_HANDLE_DUPLICATE, HYPER_NATIVE_SYS_HANDLE_GET_INFO,
    HYPER_NATIVE_SYS_OBJECT_WAIT_MANY, HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY, HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME,
    HYPER_NATIVE_SYS_PROCESS_EXIT, HYPER_NATIVE_SYS_PROCESS_GET_INFO, HYPER_NATIVE_SYS_THREAD_EXIT,
    HYPER_NATIVE_SYS_THREAD_YIELD, HyperNativeCapabilityDisposition,
    HyperNativeCapabilityReceiveSlot, HyperNativeDirectoryInfo, HyperNativeFileInfo,
    HyperNativeObjectInspection, HyperNativeResourceLimits, NativeInvocation, NativeResult,
};

use crate::kernel::capability::{HandleInfo, HandleValue, Rights};
use crate::kernel::inspect::{
    HANDLE_PAGE_CAPACITY, OBJECT_PAGE_CAPACITY, Page, ProcessHandleSnapshot, TaskThreadSnapshot,
};
use crate::kernel::ipc::{
    ByteChannelError, ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelError,
    CapabilityChannelServiceError, CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::{MemoryServiceError, Permissions, UserSlice};
use crate::kernel::object::{
    Event, ObjectWaitError, PublishableRef, SignalWaitManyOutcome, SignalWaitOutcome,
};
use crate::kernel::process::{ProcessError, ProcessSnapshot};
use crate::kernel::task::TimedWaitError;
use crate::kernel::vfs::{DirectoryInfo, DirectoryPage, FileInfo, VfsServiceError};

use super::dispatch::{dispatch_deferred, dispatch_immediate};
use super::handlers::capability_receive_result;
use super::services::{
    ConsoleServiceError, ConsoleServices, DeferredAction, HandleServices, HierarchyServices,
    InspectServices, IpcServices, MemoryServices, ObjectServiceError, ObjectServices,
    ProcessBuilderServiceError, ProcessBuilderServices, SystemInspectServices, TaskServices,
    UserMemoryServices, VfsServices, VmServices,
};
use super::status::{
    console_io_result, failure, status_from_byte_channel_error,
    status_from_capability_channel_error, status_from_console_service_error,
    status_from_object_wait_error, status_from_vm_service_error, success,
};
use super::wire::{
    HANDLE_INFO_SIZE, PROCESS_INFO_SIZE, capability_disposition_bytes,
    capability_receive_slot_bytes, capability_record_bytes, copy_extensible_input_record,
    copy_info_record, decode_resource_limits, encode_directory_info_fields,
    encode_file_info_fields, encode_handle_info_fields, encode_object_basic_info_fields,
    encode_object_inspection, encode_process_info_fields, parse_builder_handle,
    prepare_info_request,
};

#[cfg(feature = "kernel-self-test")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelfTestError {
    AbiQuery,
    Clock,
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
    VmErrorMapping,
    ChannelErrorMapping,
    RecordEncoding,
    ValidationReachedService,
    DeferredDispatch,
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn run_self_test() -> Result<(), SelfTestError> {
    super::fs_handlers::run_wire_self_test().map_err(|_| SelfTestError::RecordEncoding)?;
    use core::cell::Cell;

    struct RejectingServices {
        calls: Cell<usize>,
    }

    struct RecordInput {
        nonzero_at: Option<u64>,
    }

    struct RecordOutput {
        written: Cell<usize>,
    }

    impl UserMemoryServices for RecordInput {
        fn copy_to_user(&self, _: UserSlice, _: &[u8]) -> Result<(), ProcessError> {
            Err(ProcessError::Allocation)
        }

        fn copy_from_user(
            &self,
            source: UserSlice,
            destination: &mut [u8],
        ) -> Result<(), ProcessError> {
            destination.fill(0);
            if let Some(nonzero_at) = self.nonzero_at {
                let base = source.base().get();
                let end = source.end().get();
                if nonzero_at >= base && nonzero_at < end {
                    destination[(nonzero_at - base) as usize] = 1;
                }
            }
            Ok(())
        }
    }

    impl UserMemoryServices for RecordOutput {
        fn copy_to_user(&self, _: UserSlice, source: &[u8]) -> Result<(), ProcessError> {
            self.written.set(source.len());
            Ok(())
        }

        fn copy_from_user(&self, _: UserSlice, _: &mut [u8]) -> Result<(), ProcessError> {
            Err(ProcessError::Allocation)
        }
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

    impl UserMemoryServices for RejectingServices {
        fn copy_to_user(&self, _: UserSlice, _: &[u8]) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn copy_from_user(&self, _: UserSlice, _: &mut [u8]) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
    }

    impl HandleServices for RejectingServices {
        fn close_handle(&self, _: HandleValue) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn handle_info(&self, _: HandleValue, _: Rights) -> Result<HandleInfo, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn duplicate_handle(&self, _: HandleValue, _: Rights) -> Result<HandleValue, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
        fn replace_handle(&self, _: HandleValue, _: Rights) -> Result<HandleValue, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
    }

    impl ObjectServices for RejectingServices {
        fn current_process_id(&self) -> u64 {
            1
        }
        fn wait_set_create(&self, _capacity: usize) -> Result<HandleValue, ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }

        fn wait_set_add(
            &self,
            _set: HandleValue,
            _source: HandleValue,
            _signals: u64,
        ) -> Result<u64, ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }

        fn wait_set_rearm(
            &self,
            _set: HandleValue,
            _registration: u64,
        ) -> Result<(), ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }

        fn wait_set_remove(
            &self,
            _set: HandleValue,
            _registration: u64,
        ) -> Result<(), ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }

        fn wait_set_wait(
            &self,
            _set: HandleValue,
            _deadline: u64,
            _output: UserSlice,
        ) -> Result<(), ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }

        fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

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
    }

    impl IpcServices for RejectingServices {
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
    }

    impl ConsoleServices for RejectingServices {
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
    }

    impl VfsServices for RejectingServices {
        fn directory_scope_create(
            &self,
            _root: HandleValue,
            _start: HandleValue,
            _rights: Rights,
        ) -> Result<HandleValue, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_get_metadata(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _follow: bool,
        ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn file_get_metadata(
            &self,
            _file: HandleValue,
        ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_get_self_metadata(
            &self,
            _directory: HandleValue,
        ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_set_metadata(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _follow: bool,
            _update: crate::kernel::vfs::MetadataUpdate,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn file_set_metadata(
            &self,
            _file: HandleValue,
            _update: crate::kernel::vfs::MetadataUpdate,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_rename(
            &self,
            _source: HandleValue,
            _path: UserSlice,
            _destination: HandleValue,
            _new_path: UserSlice,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_link(
            &self,
            _source: HandleValue,
            _path: UserSlice,
            _destination: HandleValue,
            _new_path: UserSlice,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_symlink(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _target: UserSlice,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_read_link(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
        ) -> Result<crate::kernel::vfs::ScratchVec<u8>, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_canonicalize(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
        ) -> Result<crate::kernel::vfs::ScratchString, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_remove_if(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _is_directory: bool,
            _expected: u64,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_open_directory_nofollow(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _rights: Rights,
        ) -> Result<HandleValue, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn file_sync(&self, _file: HandleValue, _scope: u64) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn file_lock(
            &self,
            _file: HandleValue,
            _mode: crate::kernel::vfs::locks::LockMode,
            _deadline: u64,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn file_unlock(&self, _file: HandleValue) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }
        fn directory_open_file_with_options(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _rights: Rights,
            _options: u64,
            _mode: u32,
        ) -> Result<HandleValue, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }

        fn create_file(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _rights: Rights,
            _mode: u32,
        ) -> Result<HandleValue, VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }

        fn create_directory(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _mode: u32,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }

        fn remove_entry(
            &self,
            _directory: HandleValue,
            _path: UserSlice,
            _is_directory: bool,
        ) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }

        fn resize_file(&self, _file: HandleValue, _length: u64) -> Result<(), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
        }

        fn write_file_at(
            &self,
            _file: HandleValue,
            _offset: Option<u64>,
            _input: Option<UserSlice>,
        ) -> Result<(u64, u64), VfsServiceError> {
            Err(VfsServiceError::InvalidInput)
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

        fn read_directory(&self, _: HandleValue, _: u64) -> Result<DirectoryPage, VfsServiceError> {
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

        fn file_info(&self, _: HandleValue) -> Result<FileInfo, VfsServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(VfsServiceError::Process(ProcessError::Allocation))
        }

        fn directory_info(&self, _: HandleValue) -> Result<DirectoryInfo, VfsServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(VfsServiceError::Process(ProcessError::Allocation))
        }
    }

    impl MemoryServices for RejectingServices {
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
    }

    impl ProcessBuilderServices for RejectingServices {
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
    }

    impl TaskServices for RejectingServices {
        fn create_thread(
            &self,
            _: u64,
            _: u64,
            _: u64,
            _: u64,
        ) -> Result<HandleValue, ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }
        fn start_thread(&self, _: HandleValue) -> Result<(), ProcessError> {
            Err(ProcessError::Allocation)
        }
        fn stop_thread(&self, _: HandleValue) -> Result<(), ProcessError> {
            Err(ProcessError::Allocation)
        }
        fn atomic_wait(
            &self,
            _: u64,
            _: u32,
            _: u64,
        ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }
        fn atomic_wake(&self, _: u64, _: u32) -> Result<u64, ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }
        fn sleep_thread(
            &self,
            _: u64,
        ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
            Err(ObjectServiceError::InvalidInput)
        }

        fn process_info(&self, _: HandleValue) -> Result<ProcessSnapshot, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn request_process_stop(&self, _: HandleValue) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }
    }

    impl HierarchyServices for RejectingServices {
        fn create_resource_domain(
            &self,
            _: HandleValue,
            _: crate::kernel::accounting::ResourceLimits,
        ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::process::hierarchy::Error::NotSupported)
        }

        fn create_task_group(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::process::hierarchy::Error::NotSupported)
        }
    }

    impl VmServices for RejectingServices {
        fn derive_virtual_machine_creation_lease(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn create_pending_virtual_machine(
            &self,
            _: HandleValue,
            _: crate::kernel::vm::objects::VirtualMachineConfiguration,
        ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn set_pending_virtual_machine_memory(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn set_pending_virtual_machine_bootstrap(
            &self,
            _: HandleValue,
            _: crate::kernel::vm::objects::VirtualCpuBootstrap,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn set_pending_virtual_machine_virtual_serial(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn create_virtual_serial(&self) -> Result<HandleValue, crate::kernel::vm::service::Error> {
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn register_virtual_serial_output(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            Err(crate::kernel::vm::service::Error::NotSupported)
        }
        fn acknowledge_virtual_serial_output(
            &self,
            _: HandleValue,
            _: u64,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            Err(crate::kernel::vm::service::Error::NotSupported)
        }
        fn write_virtual_serial(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
        ) -> Result<usize, crate::kernel::vm::service::Error> {
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn seal_pending_virtual_machine(
            &self,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn install_pending_virtual_machine(
            &self,
            _: HandleValue,
        ) -> Result<[HandleValue; 2], crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn abort_pending_virtual_machine(
            &self,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn request_virtual_machine_stop(
            &self,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn virtual_machine_info(
            &self,
            _: HandleValue,
        ) -> Result<
            (
                crate::kernel::vm::objects::VirtualMachineConfiguration,
                crate::kernel::vm::objects::VirtualMachineSnapshot,
            ),
            crate::kernel::vm::service::Error,
        > {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn virtual_cpu_info(
            &self,
            _: HandleValue,
        ) -> Result<crate::kernel::vm::objects::VirtualCpuSnapshot, crate::kernel::vm::service::Error>
        {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }

        fn start_virtual_cpu(
            &self,
            _: HandleValue,
        ) -> Result<(), crate::kernel::vm::service::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::vm::service::Error::NotSupported)
        }
    }

    impl SystemInspectServices for RejectingServices {
        fn memory_observation(
            &self,
            _: HandleValue,
        ) -> Result<crate::kernel::inspect::MemoryObservation, crate::kernel::inspect::Error>
        {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn cpu_observation(
            &self,
            _: HandleValue,
        ) -> Result<crate::kernel::task::scheduler::CpuTimeSnapshot, crate::kernel::inspect::Error>
        {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }
    }

    impl InspectServices for RejectingServices {
        fn scan_processes(
            &self,
            _: HandleValue,
            _: u64,
        ) -> Result<
            Page<ProcessSnapshot, { crate::kernel::inspect::PROCESS_PAGE_CAPACITY }>,
            crate::kernel::inspect::Error,
        > {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn scan_threads(
            &self,
            _: HandleValue,
            _: u64,
        ) -> Result<
            Page<TaskThreadSnapshot, { crate::kernel::inspect::THREAD_PAGE_CAPACITY }>,
            crate::kernel::inspect::Error,
        > {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn scan_objects(
            &self,
            _: HandleValue,
            _: u64,
        ) -> Result<
            Page<crate::kernel::object::ObjectSnapshot, OBJECT_PAGE_CAPACITY>,
            crate::kernel::inspect::Error,
        > {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn scan_process_handles(
            &self,
            _: HandleValue,
            _: u64,
            _: u64,
        ) -> Result<Page<ProcessHandleSnapshot, HANDLE_PAGE_CAPACITY>, crate::kernel::inspect::Error>
        {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn derive_task_inspector(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::inspect::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn derive_object_inspector(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::inspect::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn derive_task_inspector_for_task_group(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::inspect::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn derive_object_inspector_for_task_group(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::inspect::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn derive_task_inspector_for_resource_domain(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::inspect::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }

        fn derive_object_inspector_for_resource_domain(
            &self,
            _: HandleValue,
            _: HandleValue,
        ) -> Result<HandleValue, crate::kernel::inspect::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(crate::kernel::inspect::Error::AccessDenied)
        }
    }

    fn invoke(number: u64, arguments: [u64; 6]) -> NativeInvocation {
        NativeInvocation::new(number, arguments, 0x1000)
    }

    let services = RejectingServices {
        calls: Cell::new(0),
    };
    let query = dispatch_immediate(&services, invoke(HYPER_NATIVE_SYS_ABI_QUERY, [u64::MAX; 6]));
    if query.status() != HYPER_NATIVE_STATUS_OK
        || query.values() != &[HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE]
    {
        return Err(SelfTestError::AbiQuery);
    }
    let first_clock = dispatch_immediate(
        &services,
        invoke(HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC, [0; 6]),
    );
    let second_clock = dispatch_immediate(
        &services,
        invoke(HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC, [0; 6]),
    );
    let malformed_clock = dispatch_immediate(
        &services,
        invoke(HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC, [1, 0, 0, 0, 0, 0]),
    );
    if first_clock.status() != HYPER_NATIVE_STATUS_OK
        || first_clock.values()[1] != 0
        || second_clock.status() != HYPER_NATIVE_STATUS_OK
        || second_clock.values()[0] < first_clock.values()[0]
        || second_clock.values()[1] != 0
        || malformed_clock != failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    {
        return Err(SelfTestError::Clock);
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
    let quota_domain = crate::kernel::accounting::ResourceDomain::try_new_root(
        crate::kernel::accounting::ResourceLimits::UNLIMITED
            .with(crate::kernel::accounting::ResourceKind::Threads, 0),
    )
    .map_err(|_| SelfTestError::VmErrorMapping)?;
    let quota_error = match quota_domain.reserve(
        crate::kernel::accounting::ResourceAmount::ZERO
            .with(crate::kernel::accounting::ResourceKind::Threads, 1),
    ) {
        Err(error) => error,
        Ok(reservation) => {
            drop(reservation);
            return Err(SelfTestError::VmErrorMapping);
        }
    };
    if status_from_vm_service_error(crate::kernel::vm::service::Error::from(
        crate::kernel::vm::objects::Error::Registry(crate::kernel::vm::registry::Error::Resource(
            quota_error,
        )),
    )) != HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        || status_from_vm_service_error(crate::kernel::vm::service::Error::from(
            crate::kernel::vm::objects::Error::Scheduler(
                crate::kernel::task::scheduler::Error::Thread(
                    crate::kernel::task::thread::Error::Allocation,
                ),
            ),
        )) != HYPER_NATIVE_STATUS_NO_MEMORY
        || status_from_vm_service_error(crate::kernel::vm::service::Error::from(
            crate::kernel::vm::objects::Error::MemoryLayout(
                crate::kernel::vm::memory::Error::Resource(quota_error),
            ),
        )) != HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        || status_from_vm_service_error(crate::kernel::vm::service::Error::from(
            crate::kernel::vm::objects::Error::MemoryLayout(
                crate::kernel::vm::memory::Error::MetadataAllocation,
            ),
        )) != HYPER_NATIVE_STATUS_NO_MEMORY
        || status_from_vm_service_error(crate::kernel::vm::service::Error::from(
            crate::kernel::vm::objects::Error::MemoryLayout(
                crate::kernel::vm::memory::Error::InvalidRange,
            ),
        )) != HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        || status_from_vm_service_error(crate::kernel::vm::service::Error::from(
            crate::kernel::vm::objects::Error::Memory(
                crate::kernel::mm::user_space::MemoryObjectError::Vmo(
                    crate::kernel::mm::user_space::VmoError::Busy,
                ),
            ),
        )) != HYPER_NATIVE_STATUS_BUSY
        || status_from_vm_service_error(crate::kernel::vm::service::Error::from(
            crate::kernel::vm::objects::Error::BadState,
        )) != HYPER_NATIVE_STATUS_BAD_STATE
    {
        return Err(SelfTestError::VmErrorMapping);
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
    let compatible_info = prepare_info_request(
        &[
            1_u64 << 24 | 1,
            0x2000,
            HANDLE_INFO_SIZE as u64 + 8,
            0,
            0,
            0,
        ],
        HYPER_NATIVE_HANDLE_INFO_MIN_SIZE,
        HANDLE_INFO_SIZE,
    )
    .map_err(|_| SelfTestError::InvalidRecordSize)?;
    if compatible_info.destination.length() != HANDLE_INFO_SIZE as u64
        || compatible_info.supported_size != HANDLE_INFO_SIZE
        || prepare_info_request(
            &[1_u64 << 24 | 1, 0x2000, HANDLE_INFO_SIZE as u64, 1, 0, 0],
            HYPER_NATIVE_HANDLE_INFO_MIN_SIZE,
            HANDLE_INFO_SIZE,
        )
        .err()
            != Some(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
        || prepare_info_request(
            &[
                1_u64 << 24 | 1,
                0x2000,
                HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES + 1,
                0,
                0,
                0,
            ],
            HYPER_NATIVE_HANDLE_INFO_MIN_SIZE,
            HANDLE_INFO_SIZE,
        )
        .err()
            != Some(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let prefix_info =
        prepare_info_request(&[1_u64 << 24 | 1, 0x3000, 8, 0, 0, 0], 8, HANDLE_INFO_SIZE)
            .map_err(|_| SelfTestError::InvalidRecordSize)?;
    let output = RecordOutput {
        written: Cell::new(0),
    };
    if copy_info_record(&output, prefix_info, &[0; HANDLE_INFO_SIZE]) != Ok(HANDLE_INFO_SIZE as u64)
        || output.written.get() != 8
    {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let resource_record_size = core::mem::size_of::<HyperNativeResourceLimits>() as u64;
    let extended_resource_arguments = [1_u64 << 24 | 1, 0x4000, resource_record_size + 8, 0, 0, 0];
    if decode_resource_limits(
        &RecordInput { nonzero_at: None },
        &extended_resource_arguments,
    )
    .is_err()
        || decode_resource_limits(
            &RecordInput {
                nonzero_at: Some(0x4000 + resource_record_size),
            },
            &extended_resource_arguments,
        )
        .err()
            != Some(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
        || decode_resource_limits(
            &RecordInput { nonzero_at: None },
            &[1_u64 << 24 | 1, 0x4000, resource_record_size, 1, 0, 0],
        )
        .err()
            != Some(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let prefix_arguments = [1_u64 << 24 | 1, 0x5000, 8, 0, 0, 0];
    let prefix = copy_extensible_input_record::<16>(
        &RecordInput {
            nonzero_at: Some(0x5003),
        },
        &prefix_arguments,
        8,
    )
    .map_err(|_| SelfTestError::InvalidRecordSize)?;
    if prefix[3] != 1
        || prefix[..3].iter().any(|byte| *byte != 0)
        || prefix[4..].iter().any(|byte| *byte != 0)
    {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let extended_arguments = [1_u64 << 24 | 1, 0x5000, 24, 0, 0, 0];
    if copy_extensible_input_record::<16>(&RecordInput { nonzero_at: None }, &extended_arguments, 8)
        .is_err()
        || copy_extensible_input_record::<16>(
            &RecordInput {
                nonzero_at: Some(0x5000 + 16),
            },
            &extended_arguments,
            8,
        )
        .err()
            != Some(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    {
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
                HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS + 1,
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
    let bad_directory_page_capacity = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_DIRECTORY_READ,
            [
                1_u64 << 24 | 1,
                0,
                0x2000,
                HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY - 1,
                0,
                0,
            ],
        ),
    );
    let bad_file_info_size = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_FILE_GET_INFO,
            [
                1_u64 << 24 | 1,
                0x2000,
                core::mem::size_of::<HyperNativeFileInfo>() as u64 - 1,
                0,
                0,
                0,
            ],
        ),
    );
    let bad_directory_info_size = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_DIRECTORY_GET_INFO,
            [
                1_u64 << 24 | 1,
                0x2000,
                core::mem::size_of::<HyperNativeDirectoryInfo>() as u64 - 1,
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
        || bad_directory_page_capacity
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_file_info_size
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_directory_info_size
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
                HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES + 1,
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
                HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES + 1,
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
                HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES + 1,
            ],
        ),
    );
    let disposition_bytes = capability_disposition_bytes(3);
    let receive_slot_bytes = capability_receive_slot_bytes(5);
    // Unequal synthetic layouts prove the shared calculation remains generic;
    // send and receive records may evolve to different ABI sizes.
    let narrow_record_bytes = capability_record_bytes::<[u8; 7]>(3);
    let wide_record_bytes = capability_record_bytes::<[u8; 13]>(5);
    if bad_capability_create
        != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_capability_send_options
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_capability_send
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || oversized_capability_receive
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || disposition_bytes
            != Ok(3 * core::mem::size_of::<HyperNativeCapabilityDisposition>() as u64)
        || receive_slot_bytes
            != Ok(5 * core::mem::size_of::<HyperNativeCapabilityReceiveSlot>() as u64)
        || narrow_record_bytes != Ok(21)
        || wide_record_bytes != Ok(65)
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
                HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES + 1,
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
                HYPER_NATIVE_PROCESS_NAME_MAX_BYTES + 1,
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
                HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS + 1,
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
                HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE,
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
                u64::from(HYPER_NATIVE_OBJECT_EVENT),
                0,
                u64::MAX,
            ],
        ),
    );
    let same_rights_builder_handle = parse_builder_handle(&[
        1_u64 << 24 | 1,
        1_u64 << 24 | 2,
        1,
        u64::from(HYPER_NATIVE_OBJECT_EVENT),
        HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS,
        HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE,
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
    let file_record = encode_file_info_fields(11, 17, 23, 29, 0o100_755);
    if file_record[0..8] != 11_u64.to_ne_bytes()
        || file_record[8..16] != 17_u64.to_ne_bytes()
        || file_record[16..24] != 23_u64.to_ne_bytes()
        || file_record[24..32] != 29_u64.to_ne_bytes()
        || file_record[32..36] != 0o100_755_u32.to_ne_bytes()
        || file_record[36..40] != [0; 4]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    let directory_record = encode_directory_info_fields(31, 37, 0, 0o040_755);
    if directory_record[0..8] != 31_u64.to_ne_bytes()
        || directory_record[8..16] != 37_u64.to_ne_bytes()
        || directory_record[16..24] != 0_u64.to_ne_bytes()
        || directory_record[24..28] != 0o040_755_u32.to_ne_bytes()
        || directory_record[28..32] != [0; 4]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    let inspection_domain = crate::kernel::accounting::ResourceDomain::try_new_root(
        crate::kernel::accounting::ResourceLimits::UNLIMITED,
    )
    .map_err(|_| SelfTestError::RecordEncoding)?;
    crate::kernel::object::test_delivery_rollback(&inspection_domain)
        .map_err(|_| SelfTestError::RecordEncoding)?;
    let event = Event::try_new(&inspection_domain).map_err(|_| SelfTestError::RecordEncoding)?;
    let service = PublishableRef::try_new(event).map_err(|_| SelfTestError::RecordEncoding)?;
    let active = service
        .publication()
        .activate()
        .map_err(|_| SelfTestError::RecordEncoding)?;
    let operation = active.pin::<Event>().ok_or(SelfTestError::RecordEncoding)?;
    let binding = operation.into_vm_device_binding();
    let inspection_record = encode_object_inspection(binding.snapshot());
    let binding_offset =
        core::mem::offset_of!(HyperNativeObjectInspection, vm_device_binding_references);
    let operation_offset = core::mem::offset_of!(HyperNativeObjectInspection, operation_references);
    if inspection_record[binding_offset..binding_offset + 8] != 1_u64.to_ne_bytes()
        || inspection_record[operation_offset..operation_offset + 8] != 0_u64.to_ne_bytes()
    {
        return Err(SelfTestError::RecordEncoding);
    }
    services.reached()
}
