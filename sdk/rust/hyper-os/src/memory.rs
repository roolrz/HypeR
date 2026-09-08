// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Safe ownership and bounded transfer operations for Native VMOs.

use crate::handle::{AnyObject, HandleRef, OwnedHandle, Rights, VmoObject};
use crate::{Error, Result, Status};

const WRITABLE_RIGHTS: Rights = Rights::DUPLICATE
    .union(Rights::TRANSFER)
    .union(Rights::INSPECT)
    .union(Rights::READ)
    .union(Rights::WRITE)
    .union(Rights::MAP);
const _: () = assert!(hyper_abi::HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES <= usize::MAX as u64);
pub const PAGE_SIZE: u64 = hyper_abi::HYPER_NATIVE_PAGE_SIZE;
pub const MAX_TRANSFER_BYTES: usize = hyper_abi::HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES as usize;

/// Exclusive process-local owner of one writable virtual memory object.
pub struct WritableVmo {
    handle: OwnedHandle<VmoObject>,
    size: u64,
}

impl WritableVmo {
    /// Creates a page-aligned sparse VMO.
    pub fn create(size: u64) -> Result<Self> {
        if size == 0 || !size.is_multiple_of(PAGE_SIZE) {
            return Err(Error::InvalidMemoryRange);
        }
        // SAFETY: successful creation publishes one unique output owner.
        let result = unsafe { hyper_sys::vmo_create(size) };
        Status::from_raw(result.status).into_result()?;
        // SAFETY: this no-input creation call transfers its nonzero result.
        let owner = unsafe {
            crate::handle::adopt_produced_handle_excluding::<AnyObject>(result.value0, &[])?
        };
        let info = owner.info()?;
        if info.rights != WRITABLE_RIGHTS {
            return Err(Error::InvalidResponse);
        }
        let handle = owner
            .downcast::<VmoObject>()
            .map_err(|failure| failure.error())?;
        Ok(Self { handle, size })
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn as_handle_ref(&self) -> HandleRef<'_, VmoObject> {
        self.handle.as_handle_ref()
    }

    /// Copies one complete byte range into the VMO using ABI-bounded calls.
    pub fn write_all_at(&self, offset: u64, bytes: &[u8]) -> Result<()> {
        self.validate_range(offset, bytes.len())?;
        let mut completed = 0usize;
        while completed < bytes.len() {
            let end = completed
                .saturating_add(MAX_TRANSFER_BYTES)
                .min(bytes.len());
            let chunk = bytes.get(completed..end).ok_or(Error::InvalidResponse)?;
            let chunk_offset = offset
                .checked_add(u64::try_from(completed).map_err(|_| Error::OffsetOverflow)?)
                .ok_or(Error::OffsetOverflow)?;
            // SAFETY: the handle is borrowed and `chunk` remains readable for
            // the complete non-retaining syscall.
            Status::from_raw(unsafe {
                hyper_sys::vmo_write(
                    self.handle.as_handle_ref().raw().get(),
                    chunk_offset,
                    chunk.as_ptr(),
                    chunk.len(),
                )
            })
            .into_result()?;
            completed = end;
        }
        Ok(())
    }

    /// Reads one complete byte range from the VMO using ABI-bounded calls.
    pub fn read_exact_at(&self, offset: u64, bytes: &mut [u8]) -> Result<()> {
        self.validate_range(offset, bytes.len())?;
        let mut completed = 0usize;
        while completed < bytes.len() {
            let end = completed
                .saturating_add(MAX_TRANSFER_BYTES)
                .min(bytes.len());
            let chunk = bytes
                .get_mut(completed..end)
                .ok_or(Error::InvalidResponse)?;
            let chunk_offset = offset
                .checked_add(u64::try_from(completed).map_err(|_| Error::OffsetOverflow)?)
                .ok_or(Error::OffsetOverflow)?;
            // SAFETY: the handle is borrowed and `chunk` remains writable for
            // the complete non-retaining syscall.
            Status::from_raw(unsafe {
                hyper_sys::vmo_read(
                    self.handle.as_handle_ref().raw().get(),
                    chunk_offset,
                    chunk.as_mut_ptr(),
                    chunk.len(),
                )
            })
            .into_result()?;
            completed = end;
        }
        Ok(())
    }

    #[must_use]
    pub fn into_handle(self) -> OwnedHandle<VmoObject> {
        self.handle
    }

    fn validate_range(&self, offset: u64, length: usize) -> Result<()> {
        let length = u64::try_from(length).map_err(|_| Error::OffsetOverflow)?;
        let end = offset.checked_add(length).ok_or(Error::OffsetOverflow)?;
        if end > self.size {
            Err(Error::InvalidMemoryRange)
        } else {
            Ok(())
        }
    }
}
