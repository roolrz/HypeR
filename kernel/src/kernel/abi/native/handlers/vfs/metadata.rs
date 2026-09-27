// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native filesystem metadata records and syscall leaves.

use super::{parse_follow_symlinks, parse_path};
use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, UserMemoryServices, VfsServices};
use crate::kernel::abi::native::status::{info_result, status_from_vfs_service_error, status_only};
use crate::kernel::abi::native::wire::{
    InfoRequest, copy_extensible_input_record, copy_info_record, parse_handle,
    prepare_info_request, require_zero,
};
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
mod self_test;
#[cfg(feature = "kernel-self-test")]
pub(in crate::kernel::abi::native) use self_test::run as run_wire_self_test;

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

fn encode_metadata(value: Metadata) -> [u8; METADATA_SIZE] {
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

fn decode_metadata_update(
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
pub(in crate::kernel::abi::native) fn sys_directory_get_metadata(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let request = metadata_request(arguments[0], arguments[4], arguments[5])?;
        let value = services
            .directory_get_metadata(
                request.value,
                parse_path(arguments[1], arguments[2])?,
                parse_follow_symlinks(arguments[3])?,
            )
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &encode_metadata(value))
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_set_metadata(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let update = decode_metadata_update(services, arguments[4], arguments[5])?;
        services
            .directory_set_metadata(
                parse_handle(arguments[0])?,
                parse_path(arguments[1], arguments[2])?,
                parse_follow_symlinks(arguments[3])?,
                update,
            )
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_file_set_metadata(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let update = decode_metadata_update(services, arguments[1], arguments[2])?;
        services
            .file_set_metadata(parse_handle(arguments[0])?, update)
            .map_err(status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_file_get_metadata(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let request = metadata_request(arguments[0], arguments[1], arguments[2])?;
        let value = services
            .file_get_metadata(request.value)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &encode_metadata(value))
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_directory_get_self_metadata(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let request = metadata_request(arguments[0], arguments[1], arguments[2])?;
        let value = services
            .directory_get_self_metadata(request.value)
            .map_err(status_from_vfs_service_error)?;
        copy_info_record(services, request, &encode_metadata(value))
    })();
    DeferredAction::Return(info_result(result))
}
