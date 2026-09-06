// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Owned access to the immutable process-startup `BootFS` namespace.

use core::num::NonZeroU64;

use crate::handle::{BootFileObject, BootFsObject, HandleRef, OwnedHandle, Rights};
use crate::{Error, Result, Status};

const _: () = assert!(hyper_abi::HYPER_NATIVE_BOOTFS_MAX_PATH_BYTES <= usize::MAX as u64);
const _: () = assert!(hyper_abi::HYPER_NATIVE_BOOTFS_MAX_READ_BYTES <= usize::MAX as u64);
const MAX_PATH_BYTES: usize = hyper_abi::HYPER_NATIVE_BOOTFS_MAX_PATH_BYTES as usize;
const MAX_READ_BYTES: usize = hyper_abi::HYPER_NATIVE_BOOTFS_MAX_READ_BYTES as usize;

/// Rights which may be requested for a newly opened immutable `BootFile`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootFileRights(Rights);

impl BootFileRights {
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

    /// Narrows generic rights to those meaningful for immutable `BootFile`s.
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

/// Exclusive authority to resolve paths in the immutable boot namespace.
pub struct BootFs {
    handle: OwnedHandle<BootFsObject>,
}

impl BootFs {
    /// Restores `BootFS` operations from an exclusively owned typed handle.
    ///
    /// This consumes the owner and is therefore safe for handles delegated at
    /// startup or received through a capability channel.
    #[must_use]
    pub const fn from_handle(handle: OwnedHandle<BootFsObject>) -> Self {
        Self { handle }
    }

    /// Borrows the underlying `BootFS` authority for delegation.
    #[must_use]
    pub fn as_handle_ref(&self) -> HandleRef<'_, BootFsObject> {
        self.handle.as_handle_ref()
    }

    /// Opens one UTF-8 path and returns the unique new `BootFile` owner.
    pub fn open(&self, path: &str, requested_rights: BootFileRights) -> Result<BootFile> {
        validate_path(path)?;
        let result = raw_ops::open(
            self.handle.as_handle_ref(),
            path,
            requested_rights.as_rights(),
        );
        Status::from_raw(result.status).into_result()?;
        let raw = NonZeroU64::new(result.value0).ok_or(Error::InvalidResponse)?;
        // SAFETY: a successful BOOTFS_OPEN publishes exactly one BootFile
        // handle owner to this process.
        let handle = unsafe { OwnedHandle::from_raw_owned(raw) };
        Ok(BootFile { handle })
    }

    /// Recovers the generic typed owner for delegation or explicit close.
    #[must_use]
    pub fn into_handle(self) -> OwnedHandle<BootFsObject> {
        self.handle
    }
}

/// Exclusive ownership of one immutable `BootFS` file.
pub struct BootFile {
    handle: OwnedHandle<BootFileObject>,
}

impl BootFile {
    /// Restores file operations from an exclusively owned typed handle.
    ///
    /// This consumes the owner and is therefore safe for a delegated file.
    #[must_use]
    pub const fn from_handle(handle: OwnedHandle<BootFileObject>) -> Self {
        Self { handle }
    }

    /// Borrows the underlying immutable file capability.
    #[must_use]
    pub fn as_handle_ref(&self) -> HandleRef<'_, BootFileObject> {
        self.handle.as_handle_ref()
    }

    /// Returns the immutable file size reported by the kernel.
    pub fn size(&self) -> Result<u64> {
        self.read(0, &mut []).map(|outcome| outcome.file_size)
    }

    /// Reads at most one ABI-bounded chunk from `offset`.
    ///
    /// When `output` is larger than the ABI limit, only its first bounded
    /// prefix is considered. The returned file size is immutable for the life
    /// of this `BootFile` object.
    pub fn read(&self, offset: u64, output: &mut [u8]) -> Result<ReadOutcome> {
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
    pub fn read_exact(&self, offset: u64, output: &mut [u8]) -> Result<()> {
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
            let outcome = self.read(chunk_offset, remaining)?;
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
    pub fn into_handle(self) -> OwnedHandle<BootFileObject> {
        self.handle
    }
}

/// Result metadata from one bounded `BootFile` read.
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
    use super::{BootFsObject, HandleRef, Rights};

    pub(super) fn open(
        root: HandleRef<'_, BootFsObject>,
        path: &str,
        rights: Rights,
    ) -> hyper_sys::CallResult {
        // SAFETY: the typed borrow keeps the BootFS root live, and the UTF-8
        // path remains readable for its complete length during the syscall.
        unsafe {
            hyper_sys::bootfs_open(root.raw().get(), path.as_ptr(), path.len(), rights.bits())
        }
    }

    pub(super) fn read(
        file: HandleRef<'_, super::BootFileObject>,
        offset: u64,
        output: &mut [u8],
    ) -> hyper_sys::CallResult {
        // SAFETY: the typed borrow keeps the file live and the unique output
        // slice remains writable for its complete length during the syscall.
        unsafe {
            hyper_sys::boot_file_read(file.raw().get(), offset, output.as_mut_ptr(), output.len())
        }
    }
}

#[cfg(test)]
mod raw_ops {
    use super::{BootFsObject, HandleRef, Rights};

    const CONTENT: &[u8] = b"HypeR BootFS test image";

    pub(super) fn open(
        _root: HandleRef<'_, BootFsObject>,
        path: &str,
        _rights: Rights,
    ) -> hyper_sys::CallResult {
        if path == "bin/init" {
            hyper_sys::CallResult {
                status: hyper_abi::HYPER_NATIVE_STATUS_OK,
                value0: hyper_abi::HYPER_NATIVE_OBJECT_BOOT_FILE.into(),
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
        _file: HandleRef<'_, super::BootFileObject>,
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

    use super::{BootFileRights, BootFs};
    use crate::Error;
    use crate::handle::{AnyObject, BootFileObject, BootFsObject, OwnedHandle, Rights};

    fn boot_fs() -> Result<BootFs, Error> {
        let raw = NonZeroU64::new(hyper_abi::HYPER_NATIVE_OBJECT_BOOT_FS.into())
            .ok_or(Error::InvalidResponse)?;
        // SAFETY: the host backend treats this one nonzero value as a unique
        // BootFS owner for the duration of the test.
        Ok(BootFs::from_handle(unsafe {
            OwnedHandle::<BootFsObject>::from_raw_owned(raw)
        }))
    }

    #[test]
    fn rights_narrowing_rejects_non_file_authority() {
        assert!(BootFileRights::from_rights(Rights::READ.union(Rights::TRANSFER)).is_some());
        assert!(BootFileRights::from_rights(Rights::WRITE).is_none());
    }

    #[test]
    fn open_and_exact_read_are_typed_and_bounded() -> Result<(), Error> {
        let file = boot_fs()?.open(
            "bin/init",
            BootFileRights::READ.union(BootFileRights::TRANSFER),
        )?;
        let mut bytes = [0_u8; 23];
        file.read_exact(0, &mut bytes)?;
        assert_eq!(&bytes, b"HypeR BootFS test image");
        assert_eq!(file.size()?, bytes.len() as u64);
        Ok(())
    }

    #[test]
    fn delegated_typed_owners_reenter_safe_bootfs_apis() -> Result<(), Error> {
        let fs_raw = NonZeroU64::new(hyper_abi::HYPER_NATIVE_OBJECT_BOOT_FS.into())
            .ok_or(Error::InvalidResponse)?;
        // SAFETY: this models one type-erased owner received from the kernel.
        let fs = unsafe { OwnedHandle::<AnyObject>::from_raw_owned(fs_raw) }
            .downcast::<BootFsObject>()
            .map_err(|failure| failure.error())?;
        let fs = BootFs::from_handle(fs);
        let _ = fs.as_handle_ref();

        let file_raw = NonZeroU64::new(hyper_abi::HYPER_NATIVE_OBJECT_BOOT_FILE.into())
            .ok_or(Error::InvalidResponse)?;
        // SAFETY: this models a distinct type-erased owner received from the
        // kernel for the duration of this host test.
        let file = unsafe { OwnedHandle::<AnyObject>::from_raw_owned(file_raw) }
            .downcast::<BootFileObject>()
            .map_err(|failure| failure.error())?;
        let file = super::BootFile::from_handle(file);
        assert_eq!(file.size()?, 23);
        Ok(())
    }

    #[test]
    fn invalid_path_and_short_exact_read_fail_before_partial_copy() -> Result<(), Error> {
        assert!(matches!(
            boot_fs()?.open("bad\0path", BootFileRights::READ),
            Err(Error::InvalidPath)
        ));
        let file = boot_fs()?.open("bin/init", BootFileRights::READ)?;
        let mut bytes = [0xaa_u8; 24];
        assert!(matches!(
            file.read_exact(0, &mut bytes),
            Err(Error::UnexpectedEndOfFile {
                completed: 0,
                expected: 24,
                file_size: 23,
            })
        ));
        assert_eq!(bytes, [0xaa; 24]);
        file.read_exact(u64::MAX, &mut [])?;
        Ok(())
    }
}
