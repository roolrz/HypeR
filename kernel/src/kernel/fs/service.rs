// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-facing boot-filesystem operations and user-memory validation.

use alloc::vec::Vec;

use crate::kernel::authority::Rights;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::{UserSlice, UserWriteReservation};
use crate::kernel::process::{Process, ProcessError};

use super::{BootFile, BootFs, BootFsError};

const MAX_PATH_BYTES: usize = hyper::abi::native::HYPER_NATIVE_BOOTFS_MAX_PATH_BYTES as usize;
const MAX_READ_BYTES: usize = hyper::abi::native::HYPER_NATIVE_BOOTFS_MAX_READ_BYTES as usize;

#[derive(Debug)]
pub(crate) enum BootFsServiceError {
    FileSystem(BootFsError),
    InvalidPath,
    Process(ProcessError),
}

impl From<BootFsError> for BootFsServiceError {
    fn from(error: BootFsError) -> Self {
        Self::FileSystem(error)
    }
}

impl From<ProcessError> for BootFsServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

pub(crate) fn open(
    process: &Process,
    root: HandleValue,
    path: UserSlice,
    rights: Rights,
) -> Result<HandleValue, BootFsServiceError> {
    let length = usize::try_from(path.length()).map_err(|_| BootFsServiceError::InvalidPath)?;
    if length == 0 || length > MAX_PATH_BYTES {
        return Err(BootFsServiceError::InvalidPath);
    }
    let root = process.resolve_handle::<BootFs>(root, Rights::READ)?;
    let mut path_bytes = Vec::new();
    path_bytes
        .try_reserve_exact(length)
        .map_err(|_| ProcessError::Allocation)?;
    path_bytes.resize(length, 0);
    process.copy_from_user(path, &mut path_bytes)?;
    let path = core::str::from_utf8(&path_bytes).map_err(|_| BootFsServiceError::InvalidPath)?;
    if path.as_bytes().contains(&0) {
        return Err(BootFsServiceError::InvalidPath);
    }
    let file = root.object().open(path, &process.resource_domain())?;
    Ok(process.create_object(file, rights)?)
}

pub(crate) fn read(
    process: &Process,
    file: HandleValue,
    offset: u64,
    output: Option<UserSlice>,
) -> Result<(u64, u64), BootFsServiceError> {
    let file = process.resolve_handle::<BootFile>(file, Rights::READ)?;
    let file_size = file.object().len();
    let Some(output) = output else {
        return Ok((0, file_size));
    };
    let capacity = usize::try_from(output.length())
        .map_err(|_| BootFsServiceError::Process(ProcessError::Allocation))?;
    if capacity > MAX_READ_BYTES {
        return Err(BootFsServiceError::InvalidPath);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| ProcessError::Allocation)?;
    bytes.resize(capacity, 0);
    let actual = file.object().read(offset, &mut bytes);
    let actual_u64 = u64::try_from(actual).map_err(|_| ProcessError::Allocation)?;
    if actual == 0 {
        return Ok((0, file_size));
    }
    let destination = UserSlice::new(output.base(), actual_u64)
        .map_err(|error| ProcessError::UserMemory(error.into()))?;
    let write: UserWriteReservation = process.reserve_user_write(destination)?;
    let Some(bytes) = bytes.get(..actual) else {
        return Err(BootFsServiceError::Process(ProcessError::Allocation));
    };
    write.copy_from(bytes).map_err(ProcessError::UserMemory)?;
    write.complete();
    Ok((actual_u64, file_size))
}
