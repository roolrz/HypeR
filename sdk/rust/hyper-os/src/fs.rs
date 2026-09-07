// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Owned access to a capability-relative filesystem directory.

use core::num::NonZeroU64;

use crate::handle::{DirectoryObject, FileObject, HandleRef, OwnedHandle, Rights};
use crate::{Error, Result, Status};

const _: () = assert!(hyper_abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES <= usize::MAX as u64);
const _: () = assert!(hyper_abi::HYPER_NATIVE_FILE_MAX_READ_BYTES <= usize::MAX as u64);
const MAX_PATH_BYTES: usize = hyper_abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES as usize;
const MAX_READ_BYTES: usize = hyper_abi::HYPER_NATIVE_FILE_MAX_READ_BYTES as usize;

/// Rights which may be requested for a newly opened file.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileRights(Rights);

impl FileRights {
    pub const NONE: Self = Self(Rights::NONE);
    pub const READ: Self = Self(Rights::READ);
    pub const INSPECT: Self = Self(Rights::INSPECT);
    pub const DUPLICATE: Self = Self(Rights::DUPLICATE);
    pub const TRANSFER: Self = Self(Rights::TRANSFER);
    pub const EXECUTE: Self = Self(Rights::EXECUTE);

    const ALLOWED: Rights = Rights::READ
        .union(Rights::INSPECT)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER)
        .union(Rights::EXECUTE);

    /// Narrows generic rights to those meaningful for files.
    #[must_use]
    pub const fn from_rights(rights: Rights) -> Option<Self> {
        if Self::ALLOWED.contains(rights) {
            Some(Self(rights))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0.union(other.0))
    }

    #[must_use]
    pub const fn as_rights(self) -> Rights {
        self.0
    }
}

/// Exclusive authority to resolve paths relative to one directory capability.
pub struct Directory {
    handle: OwnedHandle<DirectoryObject>,
}

impl Directory {
    /// Restores directory operations from an exclusively owned typed handle.
    ///
    /// This consumes the owner and is therefore safe for handles delegated at
    /// startup or received through a capability channel.
    #[must_use]
    pub const fn from_handle(handle: OwnedHandle<DirectoryObject>) -> Self {
        Self { handle }
    }

    /// Borrows the underlying directory authority for delegation.
    #[must_use]
    pub fn as_handle_ref(&self) -> HandleRef<'_, DirectoryObject> {
        self.handle.as_handle_ref()
    }

    /// Opens one UTF-8 path and returns the unique new `File` owner.
    ///
    /// The source Directory must carry `READ` and every requested File right;
    /// the kernel then applies the resolved node's immutable rights ceiling.
    pub fn open(&self, path: &str, requested_rights: FileRights) -> Result<File> {
        validate_path(path)?;
        let result = raw_ops::open(
            self.handle.as_handle_ref(),
            path,
            requested_rights.as_rights(),
        );
        Status::from_raw(result.status).into_result()?;
        let raw = NonZeroU64::new(result.value0).ok_or(Error::InvalidResponse)?;
        // SAFETY: a successful DIRECTORY_OPEN_FILE publishes exactly one File
        // handle owner to this process.
        let handle = unsafe { OwnedHandle::from_raw_owned(raw) };
        Ok(File { handle })
    }

    /// Recovers the generic typed owner for delegation or explicit close.
    #[must_use]
    pub fn into_handle(self) -> OwnedHandle<DirectoryObject> {
        self.handle
    }
}

/// Exclusive ownership of one filesystem file.
pub struct File {
    handle: OwnedHandle<FileObject>,
}

impl File {
    /// Restores file operations from an exclusively owned typed handle.
    ///
    /// This consumes the owner and is therefore safe for a delegated file.
    #[must_use]
    pub const fn from_handle(handle: OwnedHandle<FileObject>) -> Self {
        Self { handle }
    }

    /// Borrows the underlying file capability.
    #[must_use]
    pub fn as_handle_ref(&self) -> HandleRef<'_, FileObject> {
        self.handle.as_handle_ref()
    }

    /// Returns the immutable file size reported by the kernel.
    pub fn size(&self) -> Result<u64> {
        self.read_at(0, &mut []).map(|outcome| outcome.file_size)
    }

    /// Reads at most one ABI-bounded chunk from `offset`.
    ///
    /// When `output` is larger than the ABI limit, only its first bounded
    /// prefix is considered. The returned file size is immutable for the life
    /// of this `File` object.
    pub fn read_at(&self, offset: u64, output: &mut [u8]) -> Result<ReadOutcome> {
        let capacity = output.len().min(MAX_READ_BYTES);
        let output = output.get_mut(..capacity).ok_or(Error::InvalidResponse)?;
        let result = raw_ops::read(self.handle.as_handle_ref(), offset, output);
        Status::from_raw(result.status).into_result()?;
        let actual = usize::try_from(result.value0).map_err(|_| Error::InvalidResponse)?;
        if actual > capacity {
            return Err(Error::InvalidResponse);
        }
        let actual_u64 = u64::try_from(actual).map_err(|_| Error::InvalidResponse)?;
        let end = offset
            .checked_add(actual_u64)
            .ok_or(Error::InvalidResponse)?;
        if actual != 0 && end > result.value1 {
            return Err(Error::InvalidResponse);
        }
        Ok(ReadOutcome {
            bytes_read: actual,
            file_size: result.value1,
        })
    }

    /// Fills `output`, issuing as many bounded reads as necessary.
    ///
    /// On an unexpected end of file, the completed prefix remains initialized
    /// and the error reports its exact length.
    pub fn read_exact_at(&self, offset: u64, output: &mut [u8]) -> Result<()> {
        if output.is_empty() {
            return Ok(());
        }
        let expected = output.len();
        let expected_u64 = u64::try_from(expected).map_err(|_| Error::OffsetOverflow)?;
        let end = offset
            .checked_add(expected_u64)
            .ok_or(Error::OffsetOverflow)?;
        let file_size = self.size()?;
        if end > file_size {
            return Err(Error::UnexpectedEndOfFile {
                completed: 0,
                expected,
                file_size,
            });
        }

        let mut completed = 0;
        while completed < expected {
            let completed_u64 = u64::try_from(completed).map_err(|_| Error::OffsetOverflow)?;
            let chunk_offset = offset
                .checked_add(completed_u64)
                .ok_or(Error::OffsetOverflow)?;
            let remaining = output.get_mut(completed..).ok_or(Error::InvalidResponse)?;
            let outcome = self.read_at(chunk_offset, remaining)?;
            if outcome.file_size != file_size {
                return Err(Error::InvalidResponse);
            }
            if outcome.bytes_read == 0 {
                return Err(Error::UnexpectedEndOfFile {
                    completed,
                    expected,
                    file_size,
                });
            }
            completed = completed
                .checked_add(outcome.bytes_read)
                .ok_or(Error::InvalidResponse)?;
        }
        Ok(())
    }

    /// Recovers the generic typed owner for delegation or explicit close.
    #[must_use]
    pub fn into_handle(self) -> OwnedHandle<FileObject> {
        self.handle
    }
}

/// Result metadata from one bounded `File` read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadOutcome {
    pub bytes_read: usize,
    pub file_size: u64,
}

fn validate_path(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.as_bytes().contains(&0) {
        Err(Error::InvalidPath)
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
mod raw_ops {
    use super::{DirectoryObject, HandleRef, Rights};

    pub(super) fn open(
        root: HandleRef<'_, DirectoryObject>,
        path: &str,
        rights: Rights,
    ) -> hyper_sys::CallResult {
        // SAFETY: the typed borrow keeps the directory root live, and the UTF-8
        // path remains readable for its complete length during the syscall.
        unsafe {
            hyper_sys::directory_open_file(
                root.raw().get(),
                path.as_ptr(),
                path.len(),
                rights.bits(),
            )
        }
    }

    pub(super) fn read(
        file: HandleRef<'_, super::FileObject>,
        offset: u64,
        output: &mut [u8],
    ) -> hyper_sys::CallResult {
        // SAFETY: the typed borrow keeps the file live and the unique output
        // slice remains writable for its complete length during the syscall.
        unsafe {
            hyper_sys::file_read_at(file.raw().get(), offset, output.as_mut_ptr(), output.len())
        }
    }
}

#[cfg(test)]
mod raw_ops {
    use super::{DirectoryObject, HandleRef, Rights};

    const CONTENT: &[u8] = b"HypeR VFS test image";

    pub(super) fn open(
        _root: HandleRef<'_, DirectoryObject>,
        path: &str,
        _rights: Rights,
    ) -> hyper_sys::CallResult {
        if path == "bin/init" {
            hyper_sys::CallResult {
                status: hyper_abi::HYPER_NATIVE_STATUS_OK,
                value0: hyper_abi::HYPER_NATIVE_OBJECT_FILE.into(),
                value1: 0,
            }
        } else {
            hyper_sys::CallResult {
                status: hyper_abi::HYPER_NATIVE_STATUS_NOT_FOUND,
                value0: 0,
                value1: 0,
            }
        }
    }

    pub(super) fn read(
        _file: HandleRef<'_, super::FileObject>,
        offset: u64,
        output: &mut [u8],
    ) -> hyper_sys::CallResult {
        let start = match usize::try_from(offset) {
            Ok(start) => start,
            Err(_) => usize::MAX,
        };
        let source = CONTENT.get(start..).unwrap_or(&[]);
        let actual = source.len().min(output.len());
        if let (Some(source), Some(destination)) = (source.get(..actual), output.get_mut(..actual))
        {
            destination.copy_from_slice(source);
        }
        hyper_sys::CallResult {
            status: hyper_abi::HYPER_NATIVE_STATUS_OK,
            value0: actual as u64,
            value1: CONTENT.len() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use super::{Directory, FileRights};
    use crate::Error;
    use crate::handle::{AnyObject, DirectoryObject, FileObject, OwnedHandle, Rights};

    fn root_directory() -> Result<Directory, Error> {
        let raw = NonZeroU64::new(hyper_abi::HYPER_NATIVE_OBJECT_DIRECTORY.into())
            .ok_or(Error::InvalidResponse)?;
        // SAFETY: the host backend treats this one nonzero value as a unique
        // directory owner for the duration of the test.
        Ok(Directory::from_handle(unsafe {
            OwnedHandle::<DirectoryObject>::from_raw_owned(raw)
        }))
    }

    #[test]
    fn rights_narrowing_rejects_non_file_authority() {
        assert!(FileRights::from_rights(Rights::READ.union(Rights::TRANSFER)).is_some());
        assert!(FileRights::from_rights(Rights::WRITE).is_none());
    }

    #[test]
    fn open_and_exact_read_are_typed_and_bounded() -> Result<(), Error> {
        let file =
            root_directory()?.open("bin/init", FileRights::READ.union(FileRights::TRANSFER))?;
        let mut bytes = [0_u8; 20];
        file.read_exact_at(0, &mut bytes)?;
        assert_eq!(&bytes, b"HypeR VFS test image");
        assert_eq!(file.size()?, bytes.len() as u64);
        Ok(())
    }

    #[test]
    fn delegated_typed_owners_reenter_safe_filesystem_apis() -> Result<(), Error> {
        let directory_raw = NonZeroU64::new(hyper_abi::HYPER_NATIVE_OBJECT_DIRECTORY.into())
            .ok_or(Error::InvalidResponse)?;
        // SAFETY: this models one type-erased owner received from the kernel.
        let directory = unsafe { OwnedHandle::<AnyObject>::from_raw_owned(directory_raw) }
            .downcast::<DirectoryObject>()
            .map_err(|failure| failure.error())?;
        let directory = Directory::from_handle(directory);
        let _ = directory.as_handle_ref();

        let file_raw = NonZeroU64::new(hyper_abi::HYPER_NATIVE_OBJECT_FILE.into())
            .ok_or(Error::InvalidResponse)?;
        // SAFETY: this models a distinct type-erased owner received from the
        // kernel for the duration of this host test.
        let file = unsafe { OwnedHandle::<AnyObject>::from_raw_owned(file_raw) }
            .downcast::<FileObject>()
            .map_err(|failure| failure.error())?;
        let file = super::File::from_handle(file);
        assert_eq!(file.size()?, 20);
        Ok(())
    }

    #[test]
    fn invalid_path_and_short_exact_read_fail_before_partial_copy() -> Result<(), Error> {
        assert!(matches!(
            root_directory()?.open("bad\0path", FileRights::READ),
            Err(Error::InvalidPath)
        ));
        let file = root_directory()?.open("bin/init", FileRights::READ)?;
        let mut bytes = [0xaa_u8; 21];
        assert!(matches!(
            file.read_exact_at(0, &mut bytes),
            Err(Error::UnexpectedEndOfFile {
                completed: 0,
                expected: 21,
                file_size: 20,
            })
        ));
        assert_eq!(bytes, [0xaa; 21]);
        file.read_exact_at(u64::MAX, &mut [])?;
        Ok(())
    }
}
