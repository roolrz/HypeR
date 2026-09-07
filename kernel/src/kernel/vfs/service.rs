// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-facing VFS operations and user-memory validation.

use alloc::vec::Vec;

use crate::kernel::authority::Rights;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::{UserSlice, UserWriteReservation};
use crate::kernel::process::{Process, ProcessError};

use super::{DirectoryObject, DirectoryPage, Error as VfsError, FileObject};

const MAX_PATH_BYTES: usize = hyper::abi::native::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES as usize;
const MAX_READ_BYTES: usize = hyper::abi::native::HYPER_NATIVE_FILE_MAX_READ_BYTES as usize;
const TRANSFER_BATCH_BYTES: usize = 1024;

#[derive(Debug)]
pub(crate) enum ServiceError {
    FileSystem(VfsError),
    InvalidInput,
    Process(ProcessError),
}

impl From<VfsError> for ServiceError {
    fn from(error: VfsError) -> Self {
        Self::FileSystem(error)
    }
}

impl From<ProcessError> for ServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

pub(crate) fn open_file(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    rights: Rights,
) -> Result<HandleValue, ServiceError> {
    let path = copy_path(process, path)?;

    let required = super::rights_contract::directory_rights_for_file(rights.bits())
        .and_then(Rights::from_bits)
        .ok_or(ServiceError::InvalidInput)?;
    let directory = process.resolve_handle::<DirectoryObject>(directory, required)?;
    let file = directory
        .object()
        .open_file(&path, &process.resource_domain())?;
    Ok(process.create_object(file, rights)?)
}

pub(crate) fn open_directory(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    rights: Rights,
) -> Result<HandleValue, ServiceError> {
    let path = copy_path(process, path)?;
    let required = super::rights_contract::directory_rights_for_directory(rights.bits())
        .and_then(Rights::from_bits)
        .ok_or(ServiceError::InvalidInput)?;
    let directory = process.resolve_handle::<DirectoryObject>(directory, required)?;
    let child = directory
        .object()
        .open_directory(&path, &process.resource_domain())?;
    Ok(process.create_object(child, rights)?)
}

pub(crate) fn read_directory(
    process: &Process,
    directory: HandleValue,
    cookie: u64,
) -> Result<DirectoryPage, ServiceError> {
    let directory = process.resolve_handle::<DirectoryObject>(directory, Rights::READ)?;
    directory.object().read_page(cookie).map_err(Into::into)
}

pub(crate) fn directory_info(
    process: &Process,
    directory: HandleValue,
) -> Result<super::DirectoryInfo, ServiceError> {
    let directory = process.resolve_handle::<DirectoryObject>(directory, Rights::INSPECT)?;
    Ok(directory.object().info())
}

pub(crate) fn file_info(
    process: &Process,
    file: HandleValue,
) -> Result<super::FileInfo, ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::INSPECT)?;
    Ok(file.object().info())
}

fn copy_path(process: &Process, path: UserSlice) -> Result<alloc::string::String, ServiceError> {
    let length = usize::try_from(path.length()).map_err(|_| ServiceError::InvalidInput)?;
    if length == 0 || length > MAX_PATH_BYTES {
        return Err(ServiceError::InvalidInput);
    }
    let mut path_bytes = Vec::new();
    path_bytes
        .try_reserve_exact(length)
        .map_err(|_| ProcessError::Allocation)?;
    path_bytes.resize(length, 0);
    process.copy_from_user(path, &mut path_bytes)?;
    let path = core::str::from_utf8(&path_bytes).map_err(|_| ServiceError::InvalidInput)?;
    if path.as_bytes().contains(&0) {
        return Err(ServiceError::InvalidInput);
    }
    let mut owned = alloc::string::String::new();
    owned
        .try_reserve_exact(path.len())
        .map_err(|_| ProcessError::Allocation)?;
    owned.push_str(path);
    Ok(owned)
}

pub(crate) fn read_file_at(
    process: &Process,
    file: HandleValue,
    offset: u64,
    output: Option<UserSlice>,
) -> Result<(u64, u64), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::READ)?;
    let file_size = file.object().len();
    let Some(output) = output else {
        return Ok((0, file_size));
    };
    let capacity = usize::try_from(output.length()).map_err(|_| ServiceError::InvalidInput)?;
    if capacity > MAX_READ_BYTES {
        return Err(ServiceError::InvalidInput);
    }
    let capacity = capacity.min(TRANSFER_BATCH_BYTES);
    let mut bytes = [0_u8; TRANSFER_BATCH_BYTES];
    let actual = file.object().read(offset, &mut bytes[..capacity])?;
    let actual_u64 = u64::try_from(actual).map_err(|_| ServiceError::InvalidInput)?;
    if actual == 0 {
        return Ok((0, file_size));
    }
    let destination = UserSlice::new(output.base(), actual_u64)
        .map_err(|error| ProcessError::UserMemory(error.into()))?;
    let write: UserWriteReservation = process.reserve_user_write(destination)?;
    let Some(bytes) = bytes.get(..actual) else {
        return Err(ServiceError::InvalidInput);
    };
    write.copy_from(bytes).map_err(ProcessError::UserMemory)?;
    write.complete();
    Ok((actual_u64, file_size))
}
