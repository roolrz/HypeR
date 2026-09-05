// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Raw bindings to the `HypeR` Native userspace ABI.
//!
//! This crate deliberately exposes the ownership and pointer hazards of the
//! machine ABI. Native applications should use `hyper-os`; language runtimes
//! are the expected direct consumers of this crate.

#![no_std]

pub use hyper_abi as abi;

use core::ffi::c_char;

/// Register result returned by one `HypeR` Native syscall.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallResult {
    pub status: abi::HyperNativeStatus,
    pub value0: u64,
    pub value1: u64,
}

/// One architecture-width auxiliary-vector entry.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuxiliaryEntry {
    pub key: usize,
    pub value: usize,
}

/// Parsed process-startup view produced by the Native C runtime.
#[repr(C)]
#[derive(Debug)]
pub struct RawStartup {
    pub argument_count: usize,
    pub arguments: *const *const c_char,
    pub environment_count: usize,
    pub environment: *const *const c_char,
    pub auxiliary_count: usize,
    pub auxiliary: *const AuxiliaryEntry,
    pub handle_count: usize,
    pub handles: *const abi::HyperNativeStartupHandle,
}

const _: () = assert!(core::mem::size_of::<CallResult>() == 24);
const _: () = assert!(core::mem::align_of::<CallResult>() == 8);
const _: () = assert!(core::mem::size_of::<AuxiliaryEntry>() == 2 * core::mem::size_of::<usize>());
const _: () = assert!(core::mem::align_of::<AuxiliaryEntry>() == core::mem::align_of::<usize>());
const _: () = assert!(core::mem::size_of::<RawStartup>() == 8 * core::mem::size_of::<usize>());
const _: () = assert!(core::mem::align_of::<RawStartup>() == core::mem::align_of::<usize>());
const _: () = assert!(core::mem::offset_of!(RawStartup, argument_count) == 0);
const _: () =
    assert!(core::mem::offset_of!(RawStartup, arguments) == core::mem::size_of::<usize>());
const _: () =
    assert!(core::mem::offset_of!(RawStartup, handles) == 7 * core::mem::size_of::<usize>());

unsafe extern "C" {
    #[link_name = "hyper_abi_query"]
    fn ffi_abi_query() -> CallResult;

    #[link_name = "hyper_startup_find_handle"]
    fn ffi_startup_find_handle(
        startup: *const RawStartup,
        purpose: u32,
        handle: *mut abi::HyperNativeHandle,
    ) -> abi::HyperNativeStatus;

    #[link_name = "hyper_handle_close"]
    fn ffi_handle_close(handle: abi::HyperNativeHandle) -> abi::HyperNativeStatus;

    #[link_name = "hyper_object_wait_one"]
    fn ffi_object_wait_one(
        object: abi::HyperNativeHandle,
        signals: u64,
        deadline: u64,
    ) -> CallResult;

    #[link_name = "hyper_console_read"]
    fn ffi_console_read(
        console: abi::HyperNativeHandle,
        bytes: *mut u8,
        capacity: usize,
    ) -> CallResult;

    #[link_name = "hyper_console_write"]
    fn ffi_console_write(
        console: abi::HyperNativeHandle,
        bytes: *const u8,
        count: usize,
    ) -> CallResult;

    #[link_name = "hyper_thread_yield"]
    fn ffi_thread_yield() -> abi::HyperNativeStatus;

    #[link_name = "hyper_thread_exit"]
    fn ffi_thread_exit(status: i64) -> !;

    #[link_name = "hyper_process_exit"]
    fn ffi_process_exit(status: i64) -> !;
}

/// Queries the Native ABI revision and feature mask.
///
/// # Safety
///
/// The caller must be executing as a `HypeR` Native process through the runtime
/// and syscall veneer installed with this crate.
#[inline]
pub unsafe fn abi_query() -> CallResult {
    // SAFETY: the caller establishes the Native runtime and syscall contract.
    unsafe { ffi_abi_query() }
}

/// Finds one handle in a C-runtime-validated startup record.
///
/// # Safety
///
/// `startup` must point to a live `RawStartup` produced by the matching Native
/// runtime. `handle` must be valid and writable for one handle value. The
/// returned raw value remains owned by the process handle table.
#[inline]
pub unsafe fn startup_find_handle(
    startup: *const RawStartup,
    purpose: u32,
    handle: *mut abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller supplies both pointer validity contracts.
    unsafe { ffi_startup_find_handle(startup, purpose, handle) }
}

/// Closes one raw process handle.
///
/// # Safety
///
/// The caller must exclusively own `handle` and must prevent every subsequent
/// use of that value, including use through safe wrappers.
#[inline]
pub unsafe fn handle_close(handle: abi::HyperNativeHandle) -> abi::HyperNativeStatus {
    // SAFETY: the caller owns the raw capability and its close transition.
    unsafe { ffi_handle_close(handle) }
}

/// Waits for signals on one raw process handle.
///
/// # Safety
///
/// `object` must remain a live waitable handle for the duration of the call.
#[inline]
pub unsafe fn object_wait_one(
    object: abi::HyperNativeHandle,
    signals: u64,
    deadline: u64,
) -> CallResult {
    // SAFETY: the caller keeps the raw handle live across the syscall.
    unsafe { ffi_object_wait_one(object, signals, deadline) }
}

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

/// Yields the calling Native Thread.
///
/// # Safety
///
/// The caller must be executing through the `HypeR` Native runtime.
#[inline]
pub unsafe fn thread_yield() -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the Native execution contract.
    unsafe { ffi_thread_yield() }
}

/// Terminates the calling Native Thread.
///
/// # Safety
///
/// The caller must be executing through the `HypeR` Native runtime and must not
/// rely on destructors after this terminal transition.
#[inline]
pub unsafe fn thread_exit(status: i64) -> ! {
    // SAFETY: the caller authorizes the non-returning Thread transition.
    unsafe { ffi_thread_exit(status) }
}

/// Terminates the calling Native Process.
///
/// # Safety
///
/// The caller must be executing through the `HypeR` Native runtime and must not
/// rely on destructors after this terminal transition.
#[inline]
pub unsafe fn process_exit(status: i64) -> ! {
    // SAFETY: the caller authorizes the non-returning Process transition.
    unsafe { ffi_process_exit(status) }
}
