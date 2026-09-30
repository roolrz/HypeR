// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! File contents, metadata, snapshots, and locking.

use crate::ffi::ffi_native_call6;
use crate::{CallResult, abi};

/// Creates an immutable executable VMO snapshot of one file.
///
/// # Safety
///
/// `file` must remain live with read and execute authority. On `OK`, the caller
/// assumes exclusive ownership of the VMO in `value0`.
#[inline]
pub unsafe fn file_create_executable_vmo(file: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: the caller establishes the borrowed file and result ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO,
            file,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Reads one bounded range from a `File`.
///
/// # Safety
///
/// `file` must remain live with read rights. For nonzero `output_capacity`,
/// `output` must be writable for that many bytes for the duration of the call.
#[inline]
pub unsafe fn file_read_at(
    file: abi::HyperNativeHandle,
    offset: u64,
    output: *mut u8,
    output_capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_READ_AT,
            file,
            0,
            offset,
            output.addr() as u64,
            output_capacity as u64,
            0,
        )
    }
}

/// Retrieves immutable attributes for one File.
///
/// # Safety
///
/// `file` must remain live with inspect rights. `info` must be aligned and
/// writable for one complete [`abi::HyperNativeFileInfo`] record.
#[inline]
pub unsafe fn file_get_info(
    file: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeFileInfo,
) -> CallResult {
    // SAFETY: the caller establishes both handle and output-pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_GET_INFO,
            file,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeFileInfo>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `file_write_at`.
///
/// # Safety
///
/// Handles must remain live with the operation's required rights. Input
/// pointers must be readable for their stated lengths.
pub unsafe fn file_write_at(
    file: abi::HyperNativeHandle,
    options: u32,
    offset: u64,
    input: *const u8,
    size: usize,
) -> CallResult {
    // SAFETY: the caller establishes the Native handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_WRITE_AT,
            file,
            options as u64,
            offset,
            input.addr() as u64,
            size as u64,
            0,
        )
    }
}

/// Invokes Native `file_resize`.
///
/// # Safety
///
/// Handles must remain live with the operation's required rights. Input
/// pointers must be readable for their stated lengths.
pub unsafe fn file_resize(file: abi::HyperNativeHandle, size: u64) -> CallResult {
    // SAFETY: the caller establishes the Native handle and buffer contracts.
    unsafe { ffi_native_call6(abi::HYPER_NATIVE_SYS_FILE_RESIZE, file, size, 0, 0, 0, 0) }
}

/// Invokes Native `file_get_metadata`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn file_get_metadata(
    file: abi::HyperNativeHandle,
    output: *mut abi::HyperNativeFileMetadata,
    output_size: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_GET_METADATA,
            file,
            output as u64,
            output_size as u64,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `file_set_metadata`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn file_set_metadata(
    file: abi::HyperNativeHandle,
    input: *const abi::HyperNativeFileMetadataUpdate,
    input_size: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_SET_METADATA,
            file,
            input as u64,
            input_size as u64,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `file_sync`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn file_sync(file: abi::HyperNativeHandle, scope: u32) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_SYNC,
            file,
            scope as u64,
            0,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `file_lock`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn file_lock(file: abi::HyperNativeHandle, mode: u32, deadline: u64) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_LOCK,
            file,
            mode as u64,
            deadline,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `file_unlock`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn file_unlock(file: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe { ffi_native_call6(abi::HYPER_NATIVE_SYS_FILE_UNLOCK, file, 0, 0, 0, 0, 0) }
}

/// Captures one immutable file-content generation.
///
/// # Safety
/// `file` must name a live readable file; the caller adopts the produced handle.
pub unsafe fn file_create_snapshot(file: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: the caller supplies a live borrowed file and adopts the result.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_FILE_CREATE_SNAPSHOT,
            file,
            0,
            0,
            0,
            0,
            0,
        )
    }
}
