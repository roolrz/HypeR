// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Physical console transport.

use crate::ffi::{ffi_console_read, ffi_console_write};
use crate::{CallResult, abi};

/// Reads bytes through one raw Console handle.
///
/// # Safety
///
/// `console` must remain live and identify a Console with read rights. For a
/// nonzero `capacity`, `bytes` must identify writable memory of that extent
/// for the complete syscall.
#[inline]
pub unsafe fn console_read(
    console: abi::HyperNativeHandle,
    bytes: *mut u8,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle and output-buffer validity.
    unsafe { ffi_console_read(console, bytes, capacity) }
}

/// Writes bytes through one raw Console handle.
///
/// # Safety
///
/// `console` must remain live and identify a Console with write rights. For a
/// nonzero `count`, `bytes` must identify readable memory of that extent for
/// the complete syscall.
#[inline]
pub unsafe fn console_write(
    console: abi::HyperNativeHandle,
    bytes: *const u8,
    count: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle and input-buffer validity.
    unsafe { ffi_console_write(console, bytes, count) }
}
