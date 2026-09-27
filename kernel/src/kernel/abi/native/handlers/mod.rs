// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native syscall leaf handlers.

mod console;
mod handles;
mod inspect;
mod ipc;
mod object;
mod system;
mod task;

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
pub(super) use object::{
    sys_event_create, sys_event_signal, sys_object_wait_many, sys_object_wait_one,
    sys_wait_set_add, sys_wait_set_create, sys_wait_set_rearm, sys_wait_set_remove,
    sys_wait_set_wait,
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

#[cfg(feature = "kernel-self-test")]
pub(super) use ipc::capability_receive_result;

use super::Arguments;
use super::services::{
    DeferredAction, MemoryServices, ProcessBuilderServices, VfsServices, VmServices,
};
use super::status::{
    console_io_result, failure, handle_result, info_result, scan_result, status_from_address_error,
    status_from_memory_service_error, status_from_process_builder_service_error,
    status_from_vfs_service_error, status_from_vm_service_error, status_only, success,
};
use super::wire::{
    copy_directory_page, copy_info_record, decode_virtual_cpu_bootstrap,
    decode_virtual_machine_configuration, encode_directory_info, encode_file_info,
    encode_virtual_cpu_info, encode_virtual_machine_info, optional_user_slice,
    parse_affinity_request, parse_builder_create, parse_builder_handle, parse_builder_text,
    parse_handle, parse_single_handle, parse_two_handles, parse_virtual_serial_io,
    prepare_info_request, require_zero,
};
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use hyper::abi::native::{
    HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY, HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE,
    HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES, HYPER_NATIVE_FILE_INFO_MIN_SIZE,
    HYPER_NATIVE_FILE_MAX_READ_BYTES, HYPER_NATIVE_PROCESS_ARGUMENT_MAX_BYTES,
    HYPER_NATIVE_PROCESS_ENVIRONMENT_MAX_BYTES, HYPER_NATIVE_PROCESS_NAME_MAX_BYTES,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE,
    HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE, HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE,
    HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES, HyperNativeDirectoryEntry, HyperNativeDirectoryInfo,
    HyperNativeFileInfo, HyperNativeStatus, HyperNativeVirtualCpuInfo,
    HyperNativeVirtualMachineInfo,
};

// Keep each syscall as a distinct machine frame. The routing match must not
// inherit the largest handler's stack requirement, and crash traces should
// identify the operation which was active at the fault boundary.

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
    let result = parse_affinity_request(arguments).and_then(|(builder, words, word_count)| {
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
pub(super) fn sys_virtual_serial_register_output(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_two_handles(arguments).and_then(
        |[serial, buffer]| {
            services
                .register_virtual_serial_output(serial, buffer)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(super) fn sys_virtual_serial_acknowledge_output(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = require_zero(&arguments[2..]).and_then(|()| {
        let serial = super::wire::parse_handle(arguments[0])?;
        services
            .acknowledge_virtual_serial_output(serial, arguments[1])
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
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
pub(super) fn sys_virtual_cpu_set_affinity(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_affinity_request(arguments).and_then(|(vcpu, words, count)| {
        services
            .set_virtual_cpu_affinity(vcpu, words, count)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
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
    DeferredAction::Return(match result {
        Ok((handle, size)) => success([handle.get(), size]),
        Err(error) => failure(error),
    })
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
    let result = if arguments[4..].iter().any(|value| *value != 0)
        || arguments[3] & !hyper::abi::native::HYPER_NATIVE_VMAR_ALLOCATE_EXACT != 0
    {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        parse_handle(arguments[0]).and_then(|parent| {
            services
                .allocate_vmar(parent, arguments[1], arguments[2], arguments[3] != 0)
                .map_err(status_from_memory_service_error)
        })
    };
    DeferredAction::Return(match result {
        Ok((handle, address)) => success([handle.get(), address]),
        Err(status) => failure(status),
    })
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

#[inline(never)]
pub(super) fn sys_virtual_machine_creation_lease_get_platform_info(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::abi::native::{
        HYPER_NATIVE_VIRTUAL_MACHINE_PLATFORM_INFO_MIN_SIZE, HyperNativeVirtualMachinePlatformInfo,
    };
    let result = (|| {
        require_zero(&arguments[4..])?;
        let profile =
            u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let request = prepare_info_request(
            &[arguments[0], arguments[2], arguments[3], 0, 0, 0],
            HYPER_NATIVE_VIRTUAL_MACHINE_PLATFORM_INFO_MIN_SIZE,
            core::mem::size_of::<HyperNativeVirtualMachinePlatformInfo>(),
        )?;
        let info = services
            .virtual_machine_platform_info(request.value, profile)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(
            services,
            request,
            &super::wire::encode_virtual_machine_platform_info(info),
        )
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_vmo_create_snapshot(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[1..])?;
        let value = parse_handle(arguments[0])?;
        services
            .create_vmo_snapshot(value)
            .map_err(status_from_memory_service_error)
    })();
    DeferredAction::Return(match result {
        Ok((handle, size)) => success([handle.get(), size]),
        Err(error) => failure(error),
    })
}

#[inline(never)]
pub(super) fn sys_file_create_snapshot(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[1..])?;
        let value = parse_handle(arguments[0])?;
        services
            .create_file_snapshot(value)
            .map_err(status_from_memory_service_error)
    })();
    DeferredAction::Return(match result {
        Ok((handle, size)) => success([handle.get(), size]),
        Err(error) => failure(error),
    })
}

#[inline(never)]
pub(super) fn sys_vmar_map_private(
    services: &(impl MemoryServices + super::services::UserMemoryServices),
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[4..])?;
        let vmar = parse_handle(arguments[0])?;
        let snapshot = parse_handle(arguments[1])?;
        let record = super::wire::copy_extensible_input_record::<48>(
            services,
            &[0, arguments[2], arguments[3], 0, 0, 0],
            48,
        )?;
        use super::wire::{read_record_u32, read_record_u64};
        let request = crate::kernel::mm::user_space::PrivateMappingRequest {
            source_offset: read_record_u64(&record, 0),
            source_length: read_record_u64(&record, 8),
            address: read_record_u64(&record, 16),
            size: read_record_u64(&record, 24),
            data_offset: read_record_u64(&record, 32),
            permissions: read_record_u32(&record, 40),
            mode: read_record_u32(&record, 44),
        };
        services
            .map_private(vmar, snapshot, request)
            .map_err(status_from_memory_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_get_power_request(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::abi::native::{
        HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_VIRTUAL_MACHINE_POWER_REQUEST_MIN_SIZE,
        HyperNativeVirtualMachinePowerRequest,
    };
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_MACHINE_POWER_REQUEST_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualMachinePowerRequest>(),
    )
    .and_then(|output| {
        // Inspection is non-consuming. A failed copyout leaves this exact
        // request pending; only explicit ID completion advances ownership.
        let request = services
            .pending_power_request(output.value)
            .map_err(status_from_vm_service_error)?
            .ok_or(HYPER_NATIVE_STATUS_WOULD_BLOCK)?;
        copy_info_record(
            services,
            output,
            &super::wire::encode_virtual_machine_power_request(request),
        )
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_complete_power_request(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let machine = parse_handle(arguments[0])?;
        if arguments[1] == 0 || arguments[2] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .complete_power_request(machine, arguments[1], arguments[2] == 1)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_open_vcpu(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        let machine = parse_handle(arguments[0])?;
        let vcpu = u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .open_vcpu(machine, vcpu)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_virtual_machine_register_mmio(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[4..])?;
        let machine = parse_handle(arguments[0])?;
        if arguments[2] == 0
            || arguments[3] == 0
            || arguments[1].checked_add(arguments[2]).is_none()
        {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .register_mmio(machine, arguments[1], arguments[2], arguments[3])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_virtual_cpu_get_mmio_request(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::abi::native::{
        HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_VIRTUAL_CPU_MMIO_REQUEST_MIN_SIZE,
        HyperNativeVirtualCpuMmioRequest,
    };
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_CPU_MMIO_REQUEST_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualCpuMmioRequest>(),
    )
    .and_then(|output| {
        let request = services
            .pending_mmio(output.value)
            .map_err(status_from_vm_service_error)?
            .ok_or(HYPER_NATIVE_STATUS_WOULD_BLOCK)?;
        copy_info_record(
            services,
            output,
            &super::wire::encode_virtual_cpu_mmio_request(request),
        )
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_virtual_cpu_complete_mmio(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::vm::exit::MmioAction;
    let result = (|| {
        require_zero(&arguments[4..])?;
        let vcpu = parse_handle(arguments[0])?;
        if arguments[1] == 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let action = match (arguments[2], arguments[3]) {
            (0, value) => MmioAction::CompleteRead(value),
            (1, 0) => MmioAction::CompleteWrite,
            (2, 0) => MmioAction::Stop,
            _ => return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
        };
        services
            .complete_mmio(vcpu, arguments[1], action)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_guest_memory_create(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_single_handle(arguments).and_then(|vmo| {
        services
            .create_guest_memory(vmo)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_pending_virtual_machine_map_memory(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        let pending = parse_handle(arguments[0])?;
        let memory = parse_handle(arguments[1])?;
        services
            .map_guest_memory(pending, memory, arguments[2], arguments[3], arguments[4])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_vmo_create_contiguous(
    services: &impl MemoryServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = if arguments[1..].iter().any(|value| *value != 0) {
        Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        services
            .create_contiguous_vmo(arguments[0])
            .map_err(status_from_memory_service_error)
    };
    DeferredAction::Return(handle_result(result))
}
