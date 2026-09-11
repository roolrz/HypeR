// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-facing VFS operations and user-memory validation.

use crate::kernel::authority::Rights;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::{UserSlice, UserWriteReservation};
use crate::kernel::process::{Process, ProcessError};

use super::{
    DirectoryObject, DirectoryPage, Error as VfsError, FileObject, ScratchBudget, ScratchString,
    ScratchVec,
};

const MAX_PATH_BYTES: usize = hyper::abi::native::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES as usize;
const MAX_READ_BYTES: usize = hyper::abi::native::HYPER_NATIVE_FILE_MAX_READ_BYTES as usize;
const TRANSFER_BATCH_BYTES: usize = 1024;
const READ_BATCH_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub(crate) enum ServiceError {
    FileLock(super::locks::LockError),
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
    directory.object().info().map_err(Into::into)
}

pub(crate) fn file_info(
    process: &Process,
    file: HandleValue,
) -> Result<super::FileInfo, ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::INSPECT)?;
    file.object().info().map_err(Into::into)
}

fn copy_path(process: &Process, path: UserSlice) -> Result<ScratchString, ServiceError> {
    let length = usize::try_from(path.length()).map_err(|_| ServiceError::InvalidInput)?;
    if length == 0 || length > MAX_PATH_BYTES {
        return Err(ServiceError::InvalidInput);
    }
    let mut path_bytes = ScratchVec::new(ScratchBudget::new(&process.resource_domain()));
    path_bytes.resize(length, 0).map_err(VfsError::from)?;
    process.copy_from_user(path, &mut path_bytes)?;
    if path_bytes.contains(&0) {
        return Err(ServiceError::InvalidInput);
    }
    ScratchString::from_utf8(path_bytes).map_err(|_| ServiceError::InvalidInput)
}

pub(crate) fn read_file_at(
    process: &Process,
    file: HandleValue,
    offset: u64,
    output: Option<UserSlice>,
) -> Result<(u64, u64), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::READ)?;
    let file_size = file.object().len()?;
    let Some(output) = output else {
        return Ok((0, file_size));
    };
    let capacity = usize::try_from(output.length()).map_err(|_| ServiceError::InvalidInput)?;
    if capacity > MAX_READ_BYTES {
        return Err(ServiceError::InvalidInput);
    }
    // Bound work by the observed EOF, but recheck through the backend on each
    // batch: concurrent truncation may make a later batch short.
    let capacity = capacity.min(file_size.saturating_sub(offset).min(capacity as u64) as usize);
    if capacity == 0 {
        return Ok((0, file_size));
    }
    // Small reads remain allocation-free apart from user-memory preparation.
    // Bulk scratch is bounded independently of the ABI request limit and is
    // charged to the caller, never placed on the kernel stack.
    let mut small = [0_u8; TRANSFER_BATCH_BYTES];
    let mut large = ScratchVec::new(ScratchBudget::new(&process.resource_domain()));
    let bytes = if capacity <= small.len() {
        &mut small[..capacity]
    } else {
        large
            .resize(capacity.min(READ_BATCH_BYTES), 0)
            .map_err(VfsError::from)?;
        &mut large[..]
    };
    let completed = super::read_contract::read_batches(
        offset,
        capacity,
        bytes.len(),
        |file_offset, completed, length| -> Result<usize, ServiceError> {
            let actual = file.object().read(file_offset, &mut bytes[..length])?;
            if actual == 0 {
                return Ok(0);
            }
            let source = bytes
                .get(..actual)
                .filter(|_| actual <= length)
                .ok_or(ServiceError::InvalidInput)?;
            let base = output
                .base()
                .checked_add(completed as u64)
                .ok_or(ServiceError::InvalidInput)?;
            let destination = UserSlice::new(base, actual as u64)
                .map_err(|error| ProcessError::UserMemory(error.into()))?;
            // The backend has released its file lock before COW/page preparation.
            // Pin only this batch; mapping changes elsewhere remain possible.
            let write: UserWriteReservation = process.reserve_user_write(destination)?;
            write.copy_from(source).map_err(ProcessError::UserMemory)?;
            write.complete();
            Ok(actual)
        },
    )
    .map_err(|error| match error {
        super::read_contract::BatchError::Transfer(error) => error,
        super::read_contract::BatchError::Contract(_) => ServiceError::InvalidInput,
    })?;
    Ok((completed as u64, file_size))
}

impl From<super::instance::Error> for ServiceError {
    fn from(error: super::instance::Error) -> Self {
        Self::FileSystem(error.into())
    }
}

pub(crate) fn create_file(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    rights: Rights,
    mode: u32,
) -> Result<HandleValue, ServiceError> {
    let path = copy_path(process, path)?;
    if mode & !0o777 != 0 {
        return Err(ServiceError::InvalidInput);
    }
    let required = super::rights_contract::directory_rights_for_file(rights.bits())
        .and_then(Rights::from_bits)
        .ok_or(ServiceError::InvalidInput)?
        .union(Rights::WRITE);
    let directory = process.resolve_handle::<DirectoryObject>(directory, required)?;
    directory
        .object()
        .create_file(&path, mode, &process.resource_domain(), |file| {
            process.create_object(file, rights).map_err(Into::into)
        })
}

pub(crate) fn create_directory(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    mode: u32,
) -> Result<(), ServiceError> {
    let path = copy_path(process, path)?;
    if mode & !0o777 != 0 {
        return Err(ServiceError::InvalidInput);
    }
    let directory =
        process.resolve_handle::<DirectoryObject>(directory, Rights::READ.union(Rights::WRITE))?;
    directory
        .object()
        .create(
            &path,
            hyper::fs::NodeKind::Directory,
            mode,
            &ScratchBudget::new(&process.resource_domain()),
        )
        .map_err(Into::into)
}

pub(crate) fn remove_entry(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    is_directory: bool,
) -> Result<(), ServiceError> {
    let path = copy_path(process, path)?;
    let directory =
        process.resolve_handle::<DirectoryObject>(directory, Rights::READ.union(Rights::WRITE))?;
    directory
        .object()
        .remove(
            &path,
            if is_directory {
                hyper::fs::NodeKind::Directory
            } else {
                hyper::fs::NodeKind::File
            },
            &ScratchBudget::new(&process.resource_domain()),
        )
        .map_err(Into::into)
}

pub(crate) fn resize_file(
    process: &Process,
    file: HandleValue,
    length: u64,
) -> Result<(), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::WRITE)?;
    file.object().resize(length).map_err(Into::into)
}

pub(crate) fn write_file_at(
    process: &Process,
    file: HandleValue,
    offset: Option<u64>,
    input: Option<UserSlice>,
) -> Result<(u64, u64), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::WRITE)?;
    let Some(input) = input else {
        return Ok((0, offset.unwrap_or(file.object().len()?)));
    };
    let size = usize::try_from(input.length()).map_err(|_| ServiceError::InvalidInput)?;
    if size > MAX_READ_BYTES {
        return Err(ServiceError::InvalidInput);
    }
    let size = size.min(TRANSFER_BATCH_BYTES);
    let mut bytes = [0; TRANSFER_BATCH_BYTES];
    let source =
        UserSlice::new(input.base(), size as u64).map_err(|_| ServiceError::InvalidInput)?;
    process.copy_from_user(source, &mut bytes[..size])?;
    let (actual, end) = file.object().write(offset, &bytes[..size])?;
    Ok((actual as u64, end))
}

pub(crate) fn directory_scope_create(
    process: &Process,
    root: HandleValue,
    start: HandleValue,
    rights: Rights,
) -> Result<HandleValue, ServiceError> {
    let required = super::rights_contract::directory_rights_for_directory(rights.bits())
        .and_then(Rights::from_bits)
        .ok_or(ServiceError::InvalidInput)?;
    let root = process.resolve_handle::<DirectoryObject>(root, required)?;
    let start = process.resolve_handle::<DirectoryObject>(start, required)?;
    let scope = DirectoryObject::scope(root.object(), start.object(), &process.resource_domain())?;
    Ok(process.create_object(scope, rights)?)
}

pub(crate) fn directory_get_metadata(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    follow: bool,
) -> Result<super::Metadata, ServiceError> {
    let path = copy_path(process, path)?;
    let directory = process
        .resolve_handle::<DirectoryObject>(directory, Rights::READ.union(Rights::INSPECT))?;
    Ok(directory.object().metadata(
        &path,
        follow,
        &ScratchBudget::new(&process.resource_domain()),
    )?)
}

pub(crate) fn file_get_metadata(
    process: &Process,
    file: HandleValue,
) -> Result<super::Metadata, ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::INSPECT)?;
    Ok(file.object().metadata()?)
}

pub(crate) fn directory_get_self_metadata(
    process: &Process,
    directory: HandleValue,
) -> Result<super::Metadata, ServiceError> {
    let directory = process.resolve_handle::<DirectoryObject>(directory, Rights::INSPECT)?;
    Ok(directory.object().self_metadata()?)
}

pub(crate) fn directory_set_metadata(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    follow: bool,
    update: super::MetadataUpdate,
) -> Result<(), ServiceError> {
    let path = copy_path(process, path)?;
    let directory = process
        .resolve_handle::<DirectoryObject>(directory, Rights::READ.union(Rights::SET_ATTRIBUTES))?;
    Ok(directory.object().set_metadata(
        &path,
        follow,
        update,
        &ScratchBudget::new(&process.resource_domain()),
    )?)
}

pub(crate) fn file_set_metadata(
    process: &Process,
    file: HandleValue,
    update: super::MetadataUpdate,
) -> Result<(), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::SET_ATTRIBUTES)?;
    Ok(file.object().set_metadata(update)?)
}

pub(crate) fn directory_rename(
    process: &Process,
    source: HandleValue,
    path: UserSlice,
    destination: HandleValue,
    new_path: UserSlice,
) -> Result<(), ServiceError> {
    let path = copy_path(process, path)?;
    let new_path = copy_path(process, new_path)?;
    let rights = Rights::READ.union(Rights::WRITE);
    let source = process.resolve_handle::<DirectoryObject>(source, rights)?;
    let destination = process.resolve_handle::<DirectoryObject>(destination, rights)?;
    Ok(source.object().rename(
        &path,
        destination.object(),
        &new_path,
        &ScratchBudget::new(&process.resource_domain()),
    )?)
}

pub(crate) fn directory_link(
    process: &Process,
    source: HandleValue,
    path: UserSlice,
    destination: HandleValue,
    new_path: UserSlice,
) -> Result<(), ServiceError> {
    let path = copy_path(process, path)?;
    let new_path = copy_path(process, new_path)?;
    let rights = Rights::READ.union(Rights::WRITE);
    let source = process.resolve_handle::<DirectoryObject>(source, rights)?;
    let destination = process.resolve_handle::<DirectoryObject>(destination, rights)?;
    Ok(source.object().link(
        &path,
        destination.object(),
        &new_path,
        &ScratchBudget::new(&process.resource_domain()),
    )?)
}

pub(crate) fn directory_symlink(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    target: UserSlice,
) -> Result<(), ServiceError> {
    let path = copy_path(process, path)?;
    let target = copy_path(process, target)?;
    let directory =
        process.resolve_handle::<DirectoryObject>(directory, Rights::READ.union(Rights::WRITE))?;
    Ok(directory.object().symlink(
        &path,
        &target,
        &ScratchBudget::new(&process.resource_domain()),
    )?)
}

pub(crate) fn directory_read_link(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
) -> Result<ScratchVec<u8>, ServiceError> {
    let path = copy_path(process, path)?;
    let directory = process.resolve_handle::<DirectoryObject>(directory, Rights::READ)?;
    Ok(directory
        .object()
        .read_link(&path, &ScratchBudget::new(&process.resource_domain()))?)
}

pub(crate) fn directory_canonicalize(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
) -> Result<ScratchString, ServiceError> {
    let path = copy_path(process, path)?;
    let directory = process.resolve_handle::<DirectoryObject>(directory, Rights::READ)?;
    Ok(directory
        .object()
        .canonicalize(&path, &ScratchBudget::new(&process.resource_domain()))?)
}

pub(crate) fn directory_remove_if(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    is_directory: bool,
    expected: u64,
) -> Result<(), ServiceError> {
    if expected == 0 {
        return Err(ServiceError::InvalidInput);
    }
    let path = copy_path(process, path)?;
    let directory =
        process.resolve_handle::<DirectoryObject>(directory, Rights::READ.union(Rights::WRITE))?;
    Ok(directory.object().remove_if(
        &path,
        is_directory,
        expected,
        &ScratchBudget::new(&process.resource_domain()),
    )?)
}

pub(crate) fn directory_open_directory_nofollow(
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
        .open_directory_nofollow(&path, &process.resource_domain())?;
    Ok(process.create_object(child, rights)?)
}

pub(crate) fn file_sync(
    process: &Process,
    file: HandleValue,
    scope: u64,
) -> Result<(), ServiceError> {
    if scope > 1 {
        return Err(ServiceError::InvalidInput);
    }
    let file = process.resolve_handle::<FileObject>(file, Rights::WRITE)?;
    Ok(file.object().sync(scope)?)
}

pub(crate) fn file_lock(
    process: &Process,
    file: HandleValue,
    mode: super::locks::LockMode,
    deadline: u64,
    cancelled: impl Fn() -> bool,
) -> Result<(), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::LOCK_FILE)?;
    file.object()
        .lock(mode, deadline, &process.resource_domain(), cancelled)
        .map_err(ServiceError::FileLock)
}

pub(crate) fn file_unlock(process: &Process, file: HandleValue) -> Result<(), ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::LOCK_FILE)?;
    file.object().unlock().map_err(ServiceError::FileLock)
}

pub(crate) fn directory_open_file_with_options(
    process: &Process,
    directory: HandleValue,
    path: UserSlice,
    rights: Rights,
    options: u64,
    mode: u32,
) -> Result<HandleValue, ServiceError> {
    if options & !7 != 0
        || options & 3 == 3
        || mode & !0o777 != 0
        || (options != 0 && !rights.contains(Rights::WRITE))
    {
        return Err(ServiceError::InvalidInput);
    }
    let path = copy_path(process, path)?;
    let mut required = super::rights_contract::directory_rights_for_file(rights.bits())
        .and_then(Rights::from_bits)
        .ok_or(ServiceError::InvalidInput)?;
    if options & 3 != 0 {
        required = required.union(Rights::WRITE);
    }
    let directory = process.resolve_handle::<DirectoryObject>(directory, required)?;
    let options = super::FileOpenOptions::new(rights, options, mode)?;
    directory
        .object()
        .open_file_with_options(&path, &options, &process.resource_domain(), |file| {
            process
                .create_object(file, rights)
                .map_err(ServiceError::Process)
        })
}
