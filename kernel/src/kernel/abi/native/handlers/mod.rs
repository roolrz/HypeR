// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native syscall leaf handlers.

mod console;
mod handles;
mod inspect;
mod ipc;
mod memory;
mod object;
mod process_builder;
mod system;
mod task;
mod vm;

pub(super) use console::{sys_console_read, sys_console_write};
pub(super) use handles::{
    sys_handle_close, sys_handle_duplicate, sys_handle_get_info, sys_handle_replace,
    sys_object_get_basic_info,
};
pub(super) use inspect::{
    sys_cpu_inspector_read, sys_memory_inspector_read, sys_object_inspector_derive_process,
    sys_object_inspector_derive_resource_domain, sys_object_inspector_derive_task_group,
    sys_object_inspector_scan_handles, sys_object_inspector_scan_objects,
    sys_task_inspector_derive_process, sys_task_inspector_derive_resource_domain,
    sys_task_inspector_derive_task_group, sys_task_inspector_scan_processes,
    sys_task_inspector_scan_threads,
};
pub(super) use ipc::{
    sys_byte_channel_create, sys_byte_channel_read, sys_byte_channel_write,
    sys_capability_channel_create, sys_capability_channel_receive, sys_capability_channel_try_send,
};
pub(super) use memory::{
    sys_file_create_executable_vmo, sys_file_create_snapshot, sys_vmar_allocate, sys_vmar_destroy,
    sys_vmar_map, sys_vmar_map_private, sys_vmar_protect, sys_vmar_unmap, sys_vmo_create,
    sys_vmo_create_contiguous, sys_vmo_create_snapshot, sys_vmo_read, sys_vmo_write,
};
pub(super) use object::{
    sys_event_create, sys_event_signal, sys_object_wait_many, sys_object_wait_one,
    sys_wait_set_add, sys_wait_set_create, sys_wait_set_rearm, sys_wait_set_remove,
    sys_wait_set_wait,
};
pub(super) use process_builder::{
    sys_process_builder_abort, sys_process_builder_add_argument,
    sys_process_builder_add_environment, sys_process_builder_add_handle,
    sys_process_builder_create, sys_process_builder_seal, sys_process_builder_set_affinity,
    sys_process_builder_set_name, sys_process_builder_start,
};
pub(super) use system::{
    sys_abi_query, sys_clock_get_monotonic, sys_not_supported, sys_system_config,
};
pub(super) use task::{
    sys_atomic_wait, sys_atomic_wake, sys_process_exit, sys_process_get_current_id,
    sys_process_get_info, sys_process_request_stop, sys_resource_domain_create,
    sys_task_group_create, sys_thread_create, sys_thread_exit, sys_thread_request_stop,
    sys_thread_sleep, sys_thread_start, sys_thread_yield,
};
pub(super) use vm::{
    sys_guest_memory_create, sys_pending_virtual_machine_abort,
    sys_pending_virtual_machine_install, sys_pending_virtual_machine_map_memory,
    sys_pending_virtual_machine_seal, sys_pending_virtual_machine_set_bootstrap,
    sys_pending_virtual_machine_set_memory, sys_pending_virtual_machine_set_virtual_serial,
    sys_virtual_cpu_complete_mmio, sys_virtual_cpu_get_info, sys_virtual_cpu_get_mmio_request,
    sys_virtual_cpu_set_affinity, sys_virtual_cpu_start,
    sys_virtual_machine_complete_power_request, sys_virtual_machine_create,
    sys_virtual_machine_creation_lease_create,
    sys_virtual_machine_creation_lease_get_platform_info, sys_virtual_machine_get_info,
    sys_virtual_machine_get_power_request, sys_virtual_machine_open_vcpu,
    sys_virtual_machine_register_mmio, sys_virtual_machine_request_stop,
    sys_virtual_serial_acknowledge_output, sys_virtual_serial_create,
    sys_virtual_serial_register_output, sys_virtual_serial_write,
};

#[cfg(feature = "kernel-self-test")]
pub(super) use ipc::capability_receive_result;

use super::Arguments;
use super::services::{DeferredAction, VfsServices};
use super::status::{
    failure, handle_result, info_result, scan_result, status_from_address_error,
    status_from_vfs_service_error, success,
};
use super::wire::{
    copy_directory_page, copy_info_record, encode_directory_info, encode_file_info,
    optional_user_slice, parse_handle, prepare_info_request,
};
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use hyper::abi::native::{
    HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY, HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE,
    HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES, HYPER_NATIVE_FILE_INFO_MIN_SIZE,
    HYPER_NATIVE_FILE_MAX_READ_BYTES, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HyperNativeDirectoryEntry, HyperNativeDirectoryInfo, HyperNativeFileInfo, HyperNativeStatus,
};

// Keep each syscall as a distinct machine frame. The routing match must not
// inherit the largest handler's stack requirement, and crash traces should
// identify the operation which was active at the fault boundary.

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
        let mut page = crate::kernel::vfs::DirectoryPage::empty();
        services
            .read_directory(directory, cookie, &mut page)
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
pub(super) fn sys_file_write_at(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[1] > 1
            || (arguments[1] == 1 && arguments[2] != 0)
            || arguments[4] > HYPER_NATIVE_FILE_MAX_READ_BYTES
            || arguments[5] != 0
        {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let file = parse_handle(arguments[0])?;
        let input = optional_user_slice(arguments[3], arguments[4])?;
        services
            .write_file_at(file, (arguments[1] == 0).then_some(arguments[2]), input)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(match result {
        Ok((actual, end)) => success([actual, end]),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(super) fn sys_file_resize(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[2..].iter().any(|value| *value != 0) {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .resize_file(parse_handle(arguments[0])?, arguments[1])
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    })
}

fn mutation_path(arguments: &Arguments) -> Result<(HandleValue, UserSlice), HyperNativeStatus> {
    if arguments[2] == 0 || arguments[2] > HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    Ok((
        parse_handle(arguments[0])?,
        UserSlice::new(UserAddress::new(arguments[1]), arguments[2])
            .map_err(status_from_address_error)?,
    ))
}

#[inline(never)]
pub(super) fn sys_directory_create_file(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[5] != 0 || arguments[4] & !0o777 != 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let (directory, path) = mutation_path(arguments)?;
        let rights = Rights::from_bits(arguments[3]).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .create_file(directory, path, rights, arguments[4] as u32)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_directory_create_directory(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[4] != 0 || arguments[5] != 0 || arguments[3] & !0o777 != 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let (directory, path) = mutation_path(arguments)?;
        services
            .create_directory(directory, path, arguments[3] as u32)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(super) fn sys_directory_remove(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[4] != 0 || arguments[5] != 0 || arguments[3] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let (directory, path) = mutation_path(arguments)?;
        services
            .remove_entry(directory, path, arguments[3] == 1)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    })
}
