// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native memory syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, MemoryServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, status_from_memory_service_error, status_only, success,
};
use crate::kernel::abi::native::wire::{optional_user_slice, parse_handle, require_zero};
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::UserSlice;
use hyper::abi::native::{
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES, HyperNativeStatus,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_vmo_create(
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
pub(in crate::kernel::abi::native) fn sys_file_create_executable_vmo(
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
pub(in crate::kernel::abi::native) fn sys_vmo_read(
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
pub(in crate::kernel::abi::native) fn sys_vmo_write(
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

fn parse_vmo_transfer(
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
pub(in crate::kernel::abi::native) fn sys_vmar_allocate(
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
pub(in crate::kernel::abi::native) fn sys_vmar_map(
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
pub(in crate::kernel::abi::native) fn sys_vmar_protect(
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
pub(in crate::kernel::abi::native) fn sys_vmar_unmap(
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
pub(in crate::kernel::abi::native) fn sys_vmar_destroy(
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
pub(in crate::kernel::abi::native) fn sys_vmo_create_snapshot(
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
pub(in crate::kernel::abi::native) fn sys_file_create_snapshot(
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
pub(in crate::kernel::abi::native) fn sys_vmar_map_private(
    services: &(impl MemoryServices + crate::kernel::abi::native::services::UserMemoryServices),
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[4..])?;
        let vmar = parse_handle(arguments[0])?;
        let snapshot = parse_handle(arguments[1])?;
        let record = crate::kernel::abi::native::wire::copy_extensible_input_record::<48>(
            services,
            &[0, arguments[2], arguments[3], 0, 0, 0],
            48,
        )?;
        use crate::kernel::abi::native::wire::{read_record_u32, read_record_u64};
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
pub(in crate::kernel::abi::native) fn sys_vmo_create_contiguous(
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
