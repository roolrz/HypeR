// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native filesystem wire decoding. Each operation remains a separate leaf frame.

use super::Arguments;
use super::services::{DeferredAction, UserMemoryServices, VfsServices};
use super::status::{
    failure, handle_result, info_result, status_from_address_error, status_from_process_error,
    status_from_vfs_service_error, status_only, success,
};
use super::wire::{
    InfoRequest, copy_extensible_input_record, copy_info_record, parse_handle, prepare_info_request,
};
use crate::kernel::authority::Rights;
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::vfs::{Metadata, MetadataUpdate};
use hyper::abi::native as abi;
use hyper::time::Timestamp;

type Status = abi::HyperNativeStatus;
type MetadataRecord = abi::HyperNativeFileMetadata;
type UpdateRecord = abi::HyperNativeFileMetadataUpdate;
const METADATA_SIZE: usize = core::mem::size_of::<MetadataRecord>();
const UPDATE_SIZE: usize = core::mem::size_of::<UpdateRecord>();
const INVALID: Status = abi::HYPER_NATIVE_STATUS_INVALID_ARGUMENT;

#[cfg(feature = "kernel-self-test")]
#[path = "fs_wire_self_test.rs"]
mod wire_self_test;
#[cfg(feature = "kernel-self-test")]
pub(super) use wire_self_test::run as run_wire_self_test;

fn trailing(arguments: &Arguments, count: usize) -> Result<(), Status> {
    if arguments[count..].iter().any(|value| *value != 0) {
        Err(INVALID)
    } else {
        Ok(())
    }
}
fn path(address: u64, length: u64) -> Result<UserSlice, Status> {
    if length == 0 || length > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
        return Err(INVALID);
    }
    UserSlice::new(UserAddress::new(address), length).map_err(status_from_address_error)
}
fn rights(value: u64) -> Result<Rights, Status> {
    Rights::from_bits(value).ok_or(INVALID)
}
fn follow(options: u64) -> Result<bool, Status> {
    match options {
        0 => Ok(true),
        1 => Ok(false),
        _ => Err(INVALID),
    }
}
fn metadata_request(handle: u64, address: u64, capacity: u64) -> Result<InfoRequest, Status> {
    prepare_info_request(
        &[handle, address, capacity, 0, 0, 0],
        abi::HYPER_NATIVE_FILE_METADATA_MIN_SIZE,
        METADATA_SIZE,
    )
}
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn metadata_bytes(value: Metadata) -> [u8; METADATA_SIZE] {
    let mut bytes = [0; METADATA_SIZE];
    put_u64(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, filesystem_id),
        value.location.filesystem_id,
    );
    put_u64(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, mount_id),
        value.location.mount_id,
    );
    put_u64(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, node_id),
        value.location.node_id,
    );
    put_u64(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, size),
        value.attributes.size(),
    );
    put_u32(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, mode),
        value.attributes.mode(),
    );
    let kind = match value.attributes.kind() {
        hyper::fs::NodeKind::File => abi::HYPER_NATIVE_DIRECTORY_ENTRY_KIND_FILE as u32,
        hyper::fs::NodeKind::Directory => abi::HYPER_NATIVE_DIRECTORY_ENTRY_KIND_DIRECTORY as u32,
        hyper::fs::NodeKind::Symlink => abi::HYPER_NATIVE_DIRECTORY_ENTRY_KIND_SYMLINK as u32,
        hyper::fs::NodeKind::Other => abi::HYPER_NATIVE_DIRECTORY_ENTRY_KIND_OTHER as u32,
    };
    put_u32(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, kind),
        kind,
    );
    let mut valid = 0;
    let offsets = [
        (
            core::mem::offset_of!(MetadataRecord, accessed_seconds),
            core::mem::offset_of!(MetadataRecord, accessed_nanoseconds),
        ),
        (
            core::mem::offset_of!(MetadataRecord, modified_seconds),
            core::mem::offset_of!(MetadataRecord, modified_nanoseconds),
        ),
        (
            core::mem::offset_of!(MetadataRecord, created_seconds),
            core::mem::offset_of!(MetadataRecord, created_nanoseconds),
        ),
        (
            core::mem::offset_of!(MetadataRecord, changed_seconds),
            core::mem::offset_of!(MetadataRecord, changed_nanoseconds),
        ),
    ];
    for (index, timestamp) in [value.accessed, value.modified, value.created, value.changed]
        .into_iter()
        .enumerate()
    {
        if let Some(timestamp) = timestamp {
            valid |= 1 << index;
            put_u64(&mut bytes, offsets[index].0, timestamp.seconds() as u64);
            put_u32(&mut bytes, offsets[index].1, timestamp.nanoseconds());
        }
    }
    put_u32(
        &mut bytes,
        core::mem::offset_of!(MetadataRecord, valid_times),
        valid,
    );
    bytes
}
fn read_update(
    services: &impl UserMemoryServices,
    address: u64,
    size: u64,
) -> Result<MetadataUpdate, Status> {
    let bytes = copy_extensible_input_record::<UPDATE_SIZE>(
        services,
        &[0, address, size, 0, 0, 0],
        abi::HYPER_NATIVE_FILE_METADATA_UPDATE_MIN_SIZE,
    )?;
    let word = |offset: usize| -> Result<u32, Status> {
        Ok(u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().map_err(|_| INVALID)?,
        ))
    };
    let mask = word(core::mem::offset_of!(UpdateRecord, mask))?;
    let mode = word(core::mem::offset_of!(UpdateRecord, mode))?;
    if mask & !7 != 0
        || mode & !0o777 != 0
        || word(core::mem::offset_of!(UpdateRecord, accessed_reserved))? != 0
        || word(core::mem::offset_of!(UpdateRecord, modified_reserved))? != 0
        || (mask & 1 == 0 && mode != 0)
    {
        return Err(INVALID);
    }
    let time =
        |offset: usize, nanos_offset: usize, selected: bool| -> Result<Option<Timestamp>, Status> {
            let seconds =
                i64::from_le_bytes(bytes[offset..offset + 8].try_into().map_err(|_| INVALID)?);
            let nanos = word(nanos_offset)?;
            if !selected {
                return if seconds == 0 && nanos == 0 {
                    Ok(None)
                } else {
                    Err(INVALID)
                };
            }
            Timestamp::new(seconds, nanos).map(Some).ok_or(INVALID)
        };
    Ok(MetadataUpdate {
        mode: (mask & 1 != 0).then_some(mode),
        accessed: time(
            core::mem::offset_of!(UpdateRecord, accessed_seconds),
            core::mem::offset_of!(UpdateRecord, accessed_nanoseconds),
            mask & 2 != 0,
        )?,
        modified: time(
            core::mem::offset_of!(UpdateRecord, modified_seconds),
            core::mem::offset_of!(UpdateRecord, modified_nanoseconds),
            mask & 4 != 0,
        )?,
    })
}

#[inline(never)]
pub(super) fn sys_directory_scope_create(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        trailing(a, 3)?;
        services
            .directory_scope_create(parse_handle(a[0])?, parse_handle(a[1])?, rights(a[2])?)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}
#[inline(never)]
pub(super) fn sys_directory_get_metadata(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let request = metadata_request(a[0], a[4], a[5])?;
        let value = services
            .directory_get_metadata(request.value, path(a[1], a[2])?, follow(a[3])?)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &metadata_bytes(value))
    })();
    DeferredAction::Return(info_result(result))
}
#[inline(never)]
pub(super) fn sys_directory_set_metadata(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let update = read_update(services, a[4], a[5])?;
        services
            .directory_set_metadata(
                parse_handle(a[0])?,
                path(a[1], a[2])?,
                follow(a[3])?,
                update,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_file_set_metadata(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 3)?;
        let update = read_update(services, a[1], a[2])?;
        services
            .file_set_metadata(parse_handle(a[0])?, update)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_directory_symlink(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 5)?;
        services
            .directory_symlink(parse_handle(a[0])?, path(a[1], a[2])?, path(a[3], a[4])?)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_directory_remove_if(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        trailing(a, 5)?;
        let is_directory = !follow(a[3])?;
        services
            .directory_remove_if(parse_handle(a[0])?, path(a[1], a[2])?, is_directory, a[4])
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_directory_open_directory_nofollow(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        trailing(a, 4)?;
        services
            .directory_open_directory_nofollow(
                parse_handle(a[0])?,
                path(a[1], a[2])?,
                rights(a[3])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}
#[inline(never)]
pub(super) fn sys_file_sync(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 2)?;
        if a[1] > 1 {
            return Err(INVALID);
        }
        services
            .file_sync(parse_handle(a[0])?, a[1])
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_file_lock(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 3)?;
        let mode = match a[1] {
            0 => crate::kernel::vfs::locks::LockMode::Shared,
            1 => crate::kernel::vfs::locks::LockMode::Exclusive,
            _ => return Err(INVALID),
        };
        services
            .file_lock(parse_handle(a[0])?, mode, a[2])
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_file_unlock(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 1)?;
        services
            .file_unlock(parse_handle(a[0])?)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(super) fn sys_clock_get_realtime(a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 0)?;
        crate::kernel::time::realtime().ok_or(abi::HYPER_NATIVE_STATUS_NOT_SUPPORTED)
    })();
    DeferredAction::Return(match result {
        Ok(time) => success([time.seconds() as u64, u64::from(time.nanoseconds())]),
        Err(status) => failure(status),
    })
}
#[inline(never)]
pub(super) fn sys_directory_open_file_with_options(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let mode = u32::try_from(a[5]).map_err(|_| INVALID)?;
        services
            .directory_open_file_with_options(
                parse_handle(a[0])?,
                path(a[1], a[2])?,
                rights(a[3])?,
                a[4],
                mode,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_file_get_metadata(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        trailing(a, 3)?;
        let request = metadata_request(a[0], a[1], a[2])?;
        let value = services
            .file_get_metadata(request.value)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &metadata_bytes(value))
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_directory_get_self_metadata(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        trailing(a, 3)?;
        let request = metadata_request(a[0], a[1], a[2])?;
        let value = services
            .directory_get_self_metadata(request.value)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &metadata_bytes(value))
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(super) fn sys_directory_rename(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        services
            .directory_rename(
                parse_handle(a[0])?,
                path(a[1], a[2])?,
                parse_handle(a[3])?,
                path(a[4], a[5])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_directory_link(services: &impl VfsServices, a: &Arguments) -> DeferredAction {
    let result = (|| {
        services
            .directory_link(
                parse_handle(a[0])?,
                path(a[1], a[2])?,
                parse_handle(a[3])?,
                path(a[4], a[5])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(super) fn sys_directory_read_link(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        trailing(a, 5)?;
        if a[4] > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
            return Err(INVALID);
        }
        let value = services
            .directory_read_link(parse_handle(a[0])?, path(a[1], a[2])?)
            .map_err(status_from_vfs_service_error)?;
        let bytes = &value[..];
        let length = bytes.len() as u64;
        if a[4] < length {
            return Err(abi::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL);
        }
        if length != 0 {
            let destination = UserSlice::new(UserAddress::new(a[3]), length)
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
pub(super) fn sys_directory_canonicalize(
    services: &impl VfsServices,
    a: &Arguments,
) -> DeferredAction {
    let result = (|| {
        trailing(a, 5)?;
        if a[4] > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
            return Err(INVALID);
        }
        let value = services
            .directory_canonicalize(parse_handle(a[0])?, path(a[1], a[2])?)
            .map_err(status_from_vfs_service_error)?;
        let bytes = value.as_bytes();
        let length = bytes.len() as u64;
        if a[4] < length {
            return Err(abi::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL);
        }
        if length != 0 {
            let destination = UserSlice::new(UserAddress::new(a[3]), length)
                .map_err(status_from_address_error)?;
            services
                .copy_to_user(destination, bytes)
                .map_err(status_from_process_error)?;
        }
        Ok(length)
    })();
    DeferredAction::Return(info_result(result))
}
