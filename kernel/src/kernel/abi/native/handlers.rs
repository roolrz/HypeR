// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native syscall leaf handlers.

use hyper::abi::native::{
    HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE,
    HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY, HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE,
    HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES, HYPER_NATIVE_FEATURE_CORE,
    HYPER_NATIVE_FILE_INFO_MIN_SIZE, HYPER_NATIVE_FILE_MAX_READ_BYTES,
    HYPER_NATIVE_HANDLE_INFO_MIN_SIZE, HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE,
    HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE, HYPER_NATIVE_PROCESS_ARGUMENT_MAX_BYTES,
    HYPER_NATIVE_PROCESS_ENVIRONMENT_MAX_BYTES, HYPER_NATIVE_PROCESS_INFO_MIN_SIZE,
    HYPER_NATIVE_PROCESS_NAME_MAX_BYTES, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
    HYPER_NATIVE_STATUS_CANCELLED, HYPER_NATIVE_STATUS_INTERNAL,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_STATUS_NOT_SUPPORTED,
    HYPER_NATIVE_STATUS_TIMED_OUT, HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
    HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE, HYPER_NATIVE_SYS_CONSOLE_READ,
    HYPER_NATIVE_SYS_CONSOLE_WRITE, HYPER_NATIVE_SYS_VIRTUAL_SERIAL_READ,
    HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE, HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE,
    HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE, HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES,
    HyperNativeCpuObservation, HyperNativeDirectoryEntry, HyperNativeDirectoryInfo,
    HyperNativeFileInfo, HyperNativeMemoryObservation, HyperNativeObjectInspection,
    HyperNativeStatus, HyperNativeTaskProcess, HyperNativeTaskThread, HyperNativeVirtualCpuInfo,
    HyperNativeVirtualMachineInfo, NativeResult,
};

use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::inspect::OBJECT_PAGE_CAPACITY;
use crate::kernel::ipc::{
    ByteChannelReadOutcome, CapabilityChannelError, CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::object::{SignalWaitManyOutcome, SignalWaitOutcome};

use super::Arguments;
use super::services::{
    ConsoleServices, DeferredAction, HandleServices, HierarchyServices, ImmediateServices,
    InspectServices, IpcServices, MemoryServices, ObjectServices, ProcessBuilderServices,
    SystemInspectServices, TaskServices, VfsServices, VmServices,
};
use super::status::{
    console_io_result, failure, handle_result, info_result, scan_result, status_from_address_error,
    status_from_byte_channel_service_error, status_from_capability_channel_error,
    status_from_capability_channel_service_error, status_from_console_service_error,
    status_from_hierarchy_error, status_from_inspection_error, status_from_memory_service_error,
    status_from_object_service_error, status_from_process_builder_service_error,
    status_from_process_error, status_from_vfs_service_error, status_from_vm_service_error,
    status_only, success,
};
use super::wire::{
    HANDLE_INFO_SIZE, OBJECT_BASIC_INFO_SIZE, PROCESS_INFO_SIZE, copy_directory_page,
    copy_encoded_page, copy_info_record, decode_resource_limits, decode_virtual_cpu_bootstrap,
    decode_virtual_machine_configuration, encode_cpu_observation, encode_directory_info,
    encode_file_info, encode_handle_info, encode_handle_inspection, encode_memory_observation,
    encode_object_basic_info, encode_object_inspection, encode_process_info, encode_task_process,
    encode_task_thread, encode_virtual_cpu_info, encode_virtual_machine_info, optional_user_slice,
    parse_builder_affinity, parse_builder_create, parse_builder_handle, parse_builder_text,
    parse_byte_channel_io, parse_capability_channel_receive, parse_capability_channel_send,
    parse_console_io, parse_handle, parse_handle_and_rights, parse_handle_inspector_scan,
    parse_inspector_derivation, parse_inspector_scan, parse_single_handle, parse_two_handles,
    parse_virtual_serial_io, parse_wait_many, prepare_info_request, require_zero,
};

// Keep each syscall as a distinct machine frame. The routing match must not
// inherit the largest handler's stack requirement, and crash traces should
// identify the operation which was active at the fault boundary.

#[inline(never)]
pub(super) fn sys_abi_query(
    _services: &impl ImmediateServices,
    _arguments: &Arguments,
) -> NativeResult {
    success([HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE])
}

#[inline(never)]
pub(super) fn sys_clock_get_monotonic(
    _services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = require_zero(arguments).and_then(|()| {
        crate::kernel::time::monotonic_nanoseconds().map_err(|_| HYPER_NATIVE_STATUS_INTERNAL)
    });
    match result {
        Ok(nanoseconds) => success([nanoseconds, 0]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
pub(super) fn sys_handle_close(
    services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .close_handle(value)
            .map_err(status_from_process_error)
    });
    status_only(result)
}

#[inline(never)]
pub(super) fn sys_handle_duplicate(
    services: &impl HandleServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = parse_handle_and_rights(arguments[0], arguments[1]).and_then(|(value, rights)| {
        services
            .duplicate_handle(value, rights)
            .map_err(status_from_process_error)
    });
    handle_result(result)
}

#[inline(never)]
pub(super) fn sys_handle_replace(
    services: &impl HandleServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = parse_handle_and_rights(arguments[0], arguments[1]).and_then(|(value, rights)| {
        services
            .replace_handle(value, rights)
            .map_err(status_from_process_error)
    });
    handle_result(result)
}

#[inline(never)]
pub(super) fn sys_handle_get_info(
    services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_HANDLE_INFO_MIN_SIZE,
        HANDLE_INFO_SIZE,
    )
    .and_then(|request| {
        let value = request.value;
        let info = services
            .handle_info(value, Rights::NONE)
            .map_err(status_from_process_error)?;
        let record = encode_handle_info(info);
        copy_info_record(services, request, &record)
    });
    info_result(result)
}

#[inline(never)]
pub(super) fn sys_object_get_basic_info(
    services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE,
        OBJECT_BASIC_INFO_SIZE,
    )
    .and_then(|request| {
        let value = request.value;
        let info = services
            .handle_info(value, Rights::INSPECT)
            .map_err(status_from_process_error)?;
        let record = encode_object_basic_info(info);
        copy_info_record(services, request, &record)
    });
    info_result(result)
}

#[inline(never)]
pub(super) fn sys_resource_domain_create(
    services: &impl HierarchyServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|parent| {
        let limits = decode_resource_limits(services, arguments)?;
        services
            .create_resource_domain(parent, limits)
            .map_err(status_from_hierarchy_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_task_group_create(
    services: &impl HierarchyServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|factory| {
        let domain = parse_handle(arguments[1])?;
        require_zero(&arguments[2..])?;
        services
            .create_task_group(factory, domain)
            .map_err(status_from_hierarchy_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_event_create(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> NativeResult {
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
pub(super) fn sys_byte_channel_create(
    services: &impl IpcServices,
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
pub(super) fn sys_capability_channel_create(
    services: &impl IpcServices,
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
pub(super) fn sys_event_signal(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .signal_event(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_not_supported() -> NativeResult {
    failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED)
}

#[inline(never)]
pub(super) fn sys_thread_yield() -> DeferredAction {
    DeferredAction::Yield(success([0, 0]))
}

#[inline(never)]
pub(super) fn sys_thread_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitThread {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
pub(super) fn sys_process_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitProcess {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
pub(super) fn sys_object_wait_one(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_object_wait_many(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_byte_channel_write(
    services: &impl IpcServices,
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
pub(super) fn sys_byte_channel_read(
    services: &impl IpcServices,
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
pub(super) fn sys_capability_channel_try_send(
    services: &impl IpcServices,
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
pub(super) fn sys_capability_channel_receive(
    services: &impl IpcServices,
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

pub(super) fn capability_receive_result(
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
pub(super) fn sys_process_builder_create(
    services: &impl ProcessBuilderServices,
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
pub(super) fn sys_process_builder_set_name(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(arguments, HYPER_NATIVE_PROCESS_NAME_MAX_BYTES).and_then(
        |(builder, text)| {
            services
                .set_process_builder_name(builder, text)
                .map_err(status_from_process_builder_service_error)
        },
    );
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_process_builder_add_argument(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(arguments, HYPER_NATIVE_PROCESS_ARGUMENT_MAX_BYTES).and_then(
        |(builder, text)| {
            services
                .add_process_builder_argument(builder, text)
                .map_err(status_from_process_builder_service_error)
        },
    );
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_process_builder_add_environment(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(arguments, HYPER_NATIVE_PROCESS_ENVIRONMENT_MAX_BYTES)
        .and_then(|(builder, text)| {
            services
                .add_process_builder_environment(builder, text)
                .map_err(status_from_process_builder_service_error)
        });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_process_builder_set_affinity(
    services: &impl ProcessBuilderServices,
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
pub(super) fn sys_process_builder_add_handle(
    services: &impl ProcessBuilderServices,
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
pub(super) fn sys_process_builder_seal(
    services: &impl ProcessBuilderServices,
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
pub(super) fn sys_process_builder_start(
    services: &impl ProcessBuilderServices,
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
pub(super) fn sys_process_builder_abort(
    services: &impl ProcessBuilderServices,
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
pub(super) fn sys_process_request_stop(
    services: &impl TaskServices,
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
pub(super) fn sys_process_get_info(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_PROCESS_INFO_MIN_SIZE,
        PROCESS_INFO_SIZE,
    )
    .and_then(|request| {
        let process = request.value;
        let snapshot = services
            .process_info(process)
            .map_err(status_from_process_error)?;
        let record = encode_process_info(snapshot);
        copy_info_record(services, request, &record)
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_task_inspector_scan_processes(
    services: &impl InspectServices,
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
pub(super) fn sys_task_inspector_scan_threads(
    services: &impl InspectServices,
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
pub(super) fn sys_object_inspector_scan_objects(
    services: &impl InspectServices,
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
pub(super) fn sys_object_inspector_scan_handles(
    services: &impl InspectServices,
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
pub(super) fn sys_task_inspector_derive_process(
    services: &impl InspectServices,
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
pub(super) fn sys_object_inspector_derive_process(
    services: &impl InspectServices,
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
pub(super) fn sys_task_inspector_derive_task_group(
    services: &impl InspectServices,
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
pub(super) fn sys_object_inspector_derive_task_group(
    services: &impl InspectServices,
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
pub(super) fn sys_task_inspector_derive_resource_domain(
    services: &impl InspectServices,
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
pub(super) fn sys_object_inspector_derive_resource_domain(
    services: &impl InspectServices,
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
pub(super) fn sys_memory_inspector_read(
    services: &impl SystemInspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE,
        core::mem::size_of::<HyperNativeMemoryObservation>(),
    )
    .and_then(|request| {
        let inspector = request.value;
        let observation = services
            .memory_observation(inspector)
            .map_err(status_from_inspection_error)?;
        copy_info_record(services, request, &encode_memory_observation(observation))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_cpu_inspector_read(
    services: &impl SystemInspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE,
        core::mem::size_of::<HyperNativeCpuObservation>(),
    )
    .and_then(|request| {
        let inspector = request.value;
        let observation = services
            .cpu_observation(inspector)
            .map_err(status_from_inspection_error)?;
        copy_info_record(services, request, &encode_cpu_observation(observation))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_console_read(
    services: &impl ConsoleServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_console_io(arguments).and_then(|(console, bytes)| {
        services
            .read_console(console, bytes)
            .map_err(status_from_console_service_error)
    });
    DeferredAction::Return(console_io_result(HYPER_NATIVE_SYS_CONSOLE_READ, result))
}

#[inline(never)]
pub(super) fn sys_console_write(
    services: &impl ConsoleServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_console_io(arguments).and_then(|(console, bytes)| {
        services
            .write_console(console, bytes)
            .map_err(status_from_console_service_error)
    });
    DeferredAction::Return(console_io_result(HYPER_NATIVE_SYS_CONSOLE_WRITE, result))
}

#[inline(never)]
pub(super) fn sys_directory_open_file(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|root| {
        if arguments[4] != 0
            || arguments[5] != 0
            || arguments[2] == 0
            || arguments[2] > HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES
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
pub(super) fn sys_directory_open_directory(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_directory_open(arguments).and_then(|(root, path, rights)| {
        services
            .open_directory(root, path, rights)
            .map_err(status_from_vfs_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

pub(super) fn parse_directory_open(
    arguments: &Arguments,
) -> Result<(HandleValue, UserSlice, Rights), HyperNativeStatus> {
    let root = parse_handle(arguments[0])?;
    if arguments[4] != 0
        || arguments[5] != 0
        || arguments[2] == 0
        || arguments[2] > HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let path = UserSlice::new(UserAddress::new(arguments[1]), arguments[2])
        .map_err(status_from_address_error)?;
    let rights = Rights::from_bits(arguments[3]).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    Ok((root, path, rights))
}

#[inline(never)]
pub(super) fn sys_directory_read(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_directory_read(arguments).and_then(|(directory, cookie, destination)| {
        let page = services
            .read_directory(directory, cookie)
            .map_err(status_from_vfs_service_error)?;
        copy_directory_page(services, destination, &page)
    });
    DeferredAction::Return(scan_result(result))
}

pub(super) fn parse_directory_read(
    arguments: &Arguments,
) -> Result<(HandleValue, u64, UserSlice), HyperNativeStatus> {
    if arguments[3] != HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY
        || arguments[4] != 0
        || arguments[5] != 0
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let bytes = arguments[3]
        .checked_mul(core::mem::size_of::<HyperNativeDirectoryEntry>() as u64)
        .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let destination =
        UserSlice::new(UserAddress::new(arguments[2]), bytes).map_err(status_from_address_error)?;
    Ok((parse_handle(arguments[0])?, arguments[1], destination))
}

#[inline(never)]
pub(super) fn sys_file_read_at(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|file| {
        if arguments[1] != 0 || arguments[4] > HYPER_NATIVE_FILE_MAX_READ_BYTES {
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
pub(super) fn sys_file_get_info(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_FILE_INFO_MIN_SIZE,
        core::mem::size_of::<HyperNativeFileInfo>(),
    )
    .and_then(|request| {
        let file = request.value;
        let info = services
            .file_info(file)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &encode_file_info(info))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_directory_get_info(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE,
        core::mem::size_of::<HyperNativeDirectoryInfo>(),
    )
    .and_then(|request| {
        let directory = request.value;
        let info = services
            .directory_info(directory)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &encode_directory_info(info))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_creation_lease_create(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_two_handles(arguments).and_then(|[authority, domain]| {
        services
            .derive_virtual_machine_creation_lease(authority, domain)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_create(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|lease| {
        let configuration = decode_virtual_machine_configuration(services, arguments)?;
        services
            .create_pending_virtual_machine(lease, configuration)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_set_memory(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_two_handles(arguments).and_then(|[pending, vmo]| {
        services
            .set_pending_virtual_machine_memory(pending, vmo)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_set_bootstrap(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|pending| {
        let bootstrap = decode_virtual_cpu_bootstrap(services, arguments)?;
        services
            .set_pending_virtual_machine_bootstrap(pending, bootstrap)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_set_virtual_serial(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_two_handles(arguments).and_then(|[pending, serial]| {
        services
            .set_pending_virtual_machine_virtual_serial(pending, serial)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_virtual_serial_create(
    services: &impl VmServices,
    _arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(handle_result(
        services
            .create_virtual_serial()
            .map_err(status_from_vm_service_error),
    ))
}

#[inline(never)]
pub(super) fn sys_virtual_serial_read(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_virtual_serial_io(arguments).and_then(|(serial, bytes)| {
        services
            .read_virtual_serial(serial, bytes)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(console_io_result(
        HYPER_NATIVE_SYS_VIRTUAL_SERIAL_READ,
        result,
    ))
}

#[inline(never)]
pub(super) fn sys_virtual_serial_write(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_virtual_serial_io(arguments).and_then(|(serial, bytes)| {
        services
            .write_virtual_serial(serial, bytes)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(console_io_result(
        HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE,
        result,
    ))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_seal(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |pending| {
            services
                .seal_pending_virtual_machine(pending)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_install(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_single_handle(arguments).and_then(|pending| {
        services
            .install_pending_virtual_machine(pending)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(match result {
        Ok([machine, vcpu]) => success([machine.get(), vcpu.get()]),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(super) fn sys_virtual_cpu_start(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |vcpu| {
            services
                .start_virtual_cpu(vcpu)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_abort(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |pending| {
            services
                .abort_pending_virtual_machine(pending)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_request_stop(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |machine| {
            services
                .request_virtual_machine_stop(machine)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_get_info(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualMachineInfo>(),
    )
    .and_then(|request| {
        let machine = request.value;
        let (configuration, snapshot) = services
            .virtual_machine_info(machine)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(
            services,
            request,
            &encode_virtual_machine_info(configuration, snapshot),
        )
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_virtual_cpu_get_info(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualCpuInfo>(),
    )
    .and_then(|request| {
        let vcpu = request.value;
        let snapshot = services
            .virtual_cpu_info(vcpu)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(services, request, &encode_virtual_cpu_info(snapshot))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_vmo_create(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_file_create_executable_vmo(
    services: &impl MemoryServices,
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
pub(super) fn sys_vmo_read(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_vmo_transfer(arguments).and_then(|(vmo, offset, bytes)| {
        services
            .read_vmo(vmo, offset, bytes)
            .map_err(status_from_memory_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_vmo_write(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_vmo_transfer(arguments).and_then(|(vmo, offset, bytes)| {
        services
            .write_vmo(vmo, offset, bytes)
            .map_err(status_from_memory_service_error)
    });
    DeferredAction::Return(status_only(result))
}

pub(super) fn parse_vmo_transfer(
    arguments: &Arguments,
) -> Result<(HandleValue, u64, Option<UserSlice>), HyperNativeStatus> {
    if arguments[4] != 0 || arguments[5] != 0 || arguments[3] > HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES
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
pub(super) fn sys_vmar_allocate(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_vmar_map(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_vmar_protect(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_vmar_unmap(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
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
pub(super) fn sys_vmar_destroy(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
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
