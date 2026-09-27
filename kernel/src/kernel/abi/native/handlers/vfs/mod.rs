// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native directory traversal and file I/O syscalls.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, VfsServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, info_result, scan_result, status_from_address_error,
    status_from_process_error, status_from_vfs_service_error, status_only, success,
};
use crate::kernel::abi::native::wire::{
    copy_directory_page, copy_info_record, encode_directory_info, encode_file_info,
    optional_user_slice, parse_handle, prepare_info_request, require_zero,
};
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use hyper::abi::native as abi;
use hyper::abi::native::{
    HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY, HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE,
    HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES, HYPER_NATIVE_FILE_INFO_MIN_SIZE,
    HYPER_NATIVE_FILE_MAX_READ_BYTES, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HyperNativeDirectoryEntry, HyperNativeDirectoryInfo, HyperNativeFileInfo, HyperNativeStatus,
};
mod metadata;

type Status = abi::HyperNativeStatus;
const INVALID: Status = abi::HYPER_NATIVE_STATUS_INVALID_ARGUMENT;

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_open_file(
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
pub(in crate::kernel::abi::native) fn sys_directory_open_directory(
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

fn parse_directory_open(
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
pub(in crate::kernel::abi::native) fn sys_directory_read(
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

fn parse_directory_read(
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
pub(in crate::kernel::abi::native) fn sys_file_read_at(
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
pub(in crate::kernel::abi::native) fn sys_file_get_info(
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
pub(in crate::kernel::abi::native) fn sys_directory_get_info(
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
pub(in crate::kernel::abi::native) fn sys_file_write_at(
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
pub(in crate::kernel::abi::native) fn sys_file_resize(
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

fn parse_mutation_path(
    arguments: &Arguments,
) -> Result<(HandleValue, UserSlice), HyperNativeStatus> {
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
pub(in crate::kernel::abi::native) fn sys_directory_create_file(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[5] != 0 || arguments[4] & !0o777 != 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let (directory, path) = parse_mutation_path(arguments)?;
        let rights = Rights::from_bits(arguments[3]).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .create_file(directory, path, rights, arguments[4] as u32)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_create_directory(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[4] != 0 || arguments[5] != 0 || arguments[3] & !0o777 != 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let (directory, path) = parse_mutation_path(arguments)?;
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
pub(in crate::kernel::abi::native) fn sys_directory_remove(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[4] != 0 || arguments[5] != 0 || arguments[3] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let (directory, path) = parse_mutation_path(arguments)?;
        services
            .remove_entry(directory, path, arguments[3] == 1)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    })
}

fn parse_path(address: u64, length: u64) -> Result<UserSlice, Status> {
    if length == 0 || length > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
        return Err(INVALID);
    }
    UserSlice::new(UserAddress::new(address), length).map_err(status_from_address_error)
}

fn parse_rights(value: u64) -> Result<Rights, Status> {
    Rights::from_bits(value).ok_or(INVALID)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_scope_create(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        services
            .directory_scope_create(
                parse_handle(arguments[0])?,
                parse_handle(arguments[1])?,
                parse_rights(arguments[2])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_symlink(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        services
            .directory_symlink(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                parse_path(arguments[3], arguments[4])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_remove_if(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        let is_directory = match arguments[3] {
            0 => false,
            1 => true,
            _ => return Err(INVALID),
        };
        services
            .directory_remove_if(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                is_directory,
                arguments[4],
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_open_directory_nofollow(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[4..])?;
        services
            .directory_open_directory_nofollow(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                parse_rights(arguments[3])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_file_sync(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        if arguments[1] > 1 {
            return Err(INVALID);
        }
        services
            .file_sync(parse_handle(arguments[0])?, arguments[1])
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_file_lock(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let mode = match arguments[1] {
            0 => crate::kernel::vfs::locks::LockMode::Shared,
            1 => crate::kernel::vfs::locks::LockMode::Exclusive,
            _ => return Err(INVALID),
        };
        services
            .file_lock(parse_handle(arguments[0])?, mode, arguments[2])
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_file_unlock(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[1..])?;
        services
            .file_unlock(parse_handle(arguments[0])?)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_open_file_with_options(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let mode = u32::try_from(arguments[5]).map_err(|_| INVALID)?;
        services
            .directory_open_file_with_options(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                parse_rights(arguments[3])?,
                arguments[4],
                mode,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_rename(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        services
            .directory_rename(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                parse_handle(arguments[3])?,
                parse_path(arguments[4], arguments[5])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_link(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        services
            .directory_link(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                parse_handle(arguments[3])?,
                parse_path(arguments[4], arguments[5])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_read_link(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        if arguments[4] > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
            return Err(INVALID);
        }
        let value = services
            .directory_read_link(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
            )
            .map_err(status_from_vfs_service_error)?;
        let bytes = &value[..];
        let length = bytes.len() as u64;
        if arguments[4] < length {
            return Err(abi::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL);
        }
        if length != 0 {
            let destination = UserSlice::new(UserAddress::new(arguments[3]), length)
                .map_err(status_from_address_error)?;
            services
                .copy_to_user(destination, bytes)
                .map_err(status_from_process_error)?;
        }
        Ok(length)
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_canonicalize(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        if arguments[4] > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
            return Err(INVALID);
        }
        let value = services
            .directory_canonicalize(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
            )
            .map_err(status_from_vfs_service_error)?;
        let bytes = value.as_bytes();
        let length = bytes.len() as u64;
        if arguments[4] < length {
            return Err(abi::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL);
        }
        if length != 0 {
            let destination = UserSlice::new(UserAddress::new(arguments[3]), length)
                .map_err(status_from_address_error)?;
            services
                .copy_to_user(destination, bytes)
                .map_err(status_from_process_error)?;
        }
        Ok(length)
    })();
    DeferredAction::Return(info_result(result))
}

pub(in crate::kernel::abi::native) use metadata::{
    sys_directory_get_metadata, sys_directory_get_self_metadata, sys_directory_set_metadata,
    sys_file_get_metadata, sys_file_set_metadata,
};

#[cfg(feature = "kernel-self-test")]
pub(in crate::kernel::abi::native) use metadata::run_wire_self_test;
