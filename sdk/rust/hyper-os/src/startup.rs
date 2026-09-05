// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use crate::console::Console;
use crate::{Error, Result, Status};

/// Borrowed view of the process-startup record owned by the language runtime.
///
/// Startup capabilities remain owned by the process handle table. Returned
/// object views cannot outlive this record and do not close the underlying
/// handle, so repeated read-only lookup cannot create duplicate ownership.
pub struct Startup<'runtime> {
    raw: NonNull<hyper_sys::RawStartup>,
    _runtime: PhantomData<&'runtime hyper_sys::RawStartup>,
}

impl<'runtime> Startup<'runtime> {
    /// Constructs the safe startup view at the language-runtime boundary.
    ///
    /// # Safety
    ///
    /// `raw` must point to a live, immutable startup record produced by the
    /// matching Native C runtime. All pointees referenced by that record and
    /// every process handle it names must remain live for `'runtime`.
    pub unsafe fn from_raw(raw: *const hyper_sys::RawStartup) -> Result<Self> {
        let raw = NonNull::new(raw.cast_mut()).ok_or(Error::InvalidStartup)?;
        Ok(Self {
            raw,
            _runtime: PhantomData,
        })
    }

    /// Borrows the Console capability assigned to this process at startup.
    pub fn console(&self) -> Result<Console<'_>> {
        let mut raw_handle = 0;
        const PURPOSE: u64 = hyper_abi::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE;
        const _: () = assert!(PURPOSE <= u32::MAX as u64);

        // SAFETY: construction proves the startup record remains live, the C
        // runtime validated its referenced arrays, and `raw_handle` is a live
        // writable output for the complete call. This safe API never closes
        // or exposes the returned raw handle.
        let status = unsafe {
            hyper_sys::startup_find_handle(self.raw.as_ptr(), PURPOSE as u32, &mut raw_handle)
        };
        Status::from_raw(status).into_result()?;
        let raw_handle = NonZeroU64::new(raw_handle).ok_or(Error::InvalidResponse)?;
        Ok(Console::from_startup(raw_handle))
    }
}
