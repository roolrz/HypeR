// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Raw bindings to the `HypeR` Native userspace ABI.
//!
//! This crate deliberately exposes the ownership and pointer hazards of the
//! machine ABI. Native applications should use `hyper-os`; language runtimes
//! are the expected direct consumers of this crate.

#![no_std]

pub mod allocator;

mod console;
mod ffi;
mod handle;
mod inspect;
mod ipc;
mod memory;
mod startup;
mod system;
mod task;
mod vm;
mod wait;

pub use console::{
    console_read, console_write, virtual_serial_acknowledge_output, virtual_serial_create,
    virtual_serial_register_output, virtual_serial_write,
};
pub use handle::{
    handle_close, handle_duplicate, handle_get_info, handle_replace, object_get_basic_info,
};
pub use hyper_abi as abi;
pub use inspect::{
    cpu_inspector_read, memory_inspector_read, object_inspector_derive_process,
    object_inspector_derive_resource_domain, object_inspector_derive_task_group,
    object_inspector_scan_handles, object_inspector_scan_objects, task_inspector_derive_process,
    task_inspector_derive_resource_domain, task_inspector_derive_task_group,
    task_inspector_scan_processes, task_inspector_scan_threads,
};
pub use ipc::{
    byte_channel_create, byte_channel_read, byte_channel_write, capability_channel_create,
    capability_channel_receive, capability_channel_try_send,
};
pub use memory::{
    vmar_allocate, vmar_destroy, vmar_map, vmar_map_private, vmar_protect, vmar_unmap, vmo_create,
    vmo_create_contiguous, vmo_create_snapshot, vmo_get_dma_extent, vmo_read, vmo_write,
};
pub use startup::{AuxiliaryEntry, RawStartup, startup_find_handle};
pub use system::{abi_query, clock_get_monotonic, clock_get_realtime, system_config};
pub use task::{
    process_builder_abort, process_builder_add_argument, process_builder_add_environment,
    process_builder_add_handle, process_builder_create, process_builder_seal,
    process_builder_set_affinity, process_builder_set_name, process_builder_start, process_exit,
    process_get_current_id, process_get_info, process_request_stop, resource_domain_create,
    task_group_create, thread_create, thread_exit, thread_request_stop, thread_sleep, thread_start,
    thread_yield,
};
pub use vm::{
    guest_memory_create, pending_virtual_machine_abort, pending_virtual_machine_assign_device,
    pending_virtual_machine_install, pending_virtual_machine_map_memory,
    pending_virtual_machine_seal, pending_virtual_machine_set_bootstrap,
    pending_virtual_machine_set_memory, pending_virtual_machine_set_virtual_serial,
    virtual_cpu_complete_mmio, virtual_cpu_get_info, virtual_cpu_get_mmio_request,
    virtual_cpu_set_affinity, virtual_cpu_start, virtual_machine_complete_power_request,
    virtual_machine_create, virtual_machine_creation_lease_create,
    virtual_machine_creation_lease_get_platform_info, virtual_machine_get_info,
    virtual_machine_get_power_request, virtual_machine_open_vcpu, virtual_machine_register_mmio,
    virtual_machine_request_stop,
};
pub use wait::{
    atomic_wait, atomic_wake, object_wait_many, object_wait_one, wait_set_add, wait_set_create,
    wait_set_rearm, wait_set_remove, wait_set_wait,
};

use ffi::ffi_native_call6;

/// Register result returned by one `HypeR` Native syscall.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallResult {
    pub status: abi::HyperNativeStatus,
    pub value0: u64,
    pub value1: u64,
}

const _: () = assert!(core::mem::size_of::<CallResult>() == 24);
const _: () = assert!(core::mem::align_of::<CallResult>() == 8);

/// Opens one file relative to a `Directory` capability.
///
/// # Safety
///
/// `directory` must remain live with read rights and every right named by
/// `requested_rights`. `path` must be readable for `path_size` bytes. On `OK`,
/// the caller assumes exclusive ownership of the nonzero `File` handle
/// returned in `value0`.
#[inline]
pub unsafe fn directory_open_file(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_size: usize,
    requested_rights: u64,
) -> CallResult {
    // SAFETY: the caller establishes the handle, input-buffer, and ownership
    // contracts of the raw Native operation.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE,
            directory,
            path.addr() as u64,
            path_size as u64,
            requested_rights,
            0,
            0,
        )
    }
}

/// Opens one child directory relative to a `Directory` capability.
///
/// # Safety
///
/// `directory` must remain live with read rights and every requested right.
/// `path` must be readable for `path_size` bytes. On `OK`, the caller assumes
/// exclusive ownership of the nonzero Directory handle in `value0`.
#[inline]
pub unsafe fn directory_open_directory(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_size: usize,
    requested_rights: u64,
) -> CallResult {
    // SAFETY: the caller establishes all raw handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY,
            directory,
            path.addr() as u64,
            path_size as u64,
            requested_rights,
            0,
            0,
        )
    }
}

/// Reads one fixed-capacity page from a Directory enumeration.
///
/// # Safety
///
/// `directory` must remain live with read rights. `records` must identify
/// writable storage for `capacity` directory-entry records for the complete
/// syscall. Callers must pass the exact capacity published by the Native ABI
/// and treat the returned continuation cookie as opaque.
#[inline]
pub unsafe fn directory_read(
    directory: abi::HyperNativeHandle,
    cookie: u64,
    records: *mut abi::HyperNativeDirectoryEntry,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the borrowed handle, output-buffer, and
    // exact-capacity contracts of the raw Native operation.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_READ,
            directory,
            cookie,
            records.addr() as u64,
            capacity as u64,
            0,
            0,
        )
    }
}

/// Retrieves immutable attributes for one Directory.
///
/// # Safety
///
/// `directory` must remain live with inspect rights. `info` must be aligned
/// and writable for one complete [`abi::HyperNativeDirectoryInfo`] record.
#[inline]
pub unsafe fn directory_get_info(
    directory: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeDirectoryInfo,
) -> CallResult {
    // SAFETY: the caller establishes both handle and output-pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_GET_INFO,
            directory,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeDirectoryInfo>() as u64,
            0,
            0,
            0,
        )
    }
}

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

/// Creates a file and transfers exclusive ownership of the returned handle.
///
/// # Safety
///
/// Handles must remain live with the operation's required rights. Input
/// pointers must be readable for their stated lengths.
pub unsafe fn directory_create_file(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_size: usize,
    rights: u64,
    mode: u32,
) -> CallResult {
    // SAFETY: the caller establishes the Native handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_CREATE_FILE,
            directory,
            path.addr() as u64,
            path_size as u64,
            rights,
            mode as u64,
            0,
        )
    }
}

/// Invokes Native `directory_create_directory`.
///
/// # Safety
///
/// Handles must remain live with the operation's required rights. Input
/// pointers must be readable for their stated lengths.
pub unsafe fn directory_create_directory(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_size: usize,
    mode: u32,
) -> CallResult {
    // SAFETY: the caller establishes the Native handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_CREATE_DIRECTORY,
            directory,
            path.addr() as u64,
            path_size as u64,
            mode as u64,
            0,
            0,
        )
    }
}

/// Invokes Native `directory_remove`.
///
/// # Safety
///
/// Handles must remain live with the operation's required rights. Input
/// pointers must be readable for their stated lengths.
pub unsafe fn directory_remove(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_size: usize,
    options: u32,
) -> CallResult {
    // SAFETY: the caller establishes the Native handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_REMOVE,
            directory,
            path.addr() as u64,
            path_size as u64,
            options as u64,
            0,
            0,
        )
    }
}

/// Invokes Native `directory_scope_create`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_scope_create(
    root: abi::HyperNativeHandle,
    start: abi::HyperNativeHandle,
    rights: u64,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_SCOPE_CREATE,
            root,
            start,
            rights,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `directory_get_metadata`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_get_metadata(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    options: u32,
    output: *mut abi::HyperNativeFileMetadata,
    output_size: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_GET_METADATA,
            directory,
            path as u64,
            path_length as u64,
            options as u64,
            output as u64,
            output_size as u64,
        )
    }
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

/// Invokes Native `directory_get_self_metadata`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_get_self_metadata(
    directory: abi::HyperNativeHandle,
    output: *mut abi::HyperNativeFileMetadata,
    output_size: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_GET_SELF_METADATA,
            directory,
            output as u64,
            output_size as u64,
            0,
            0,
            0,
        )
    }
}

/// Invokes Native `directory_set_metadata`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_set_metadata(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    options: u32,
    input: *const abi::HyperNativeFileMetadataUpdate,
    input_size: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_SET_METADATA,
            directory,
            path as u64,
            path_length as u64,
            options as u64,
            input as u64,
            input_size as u64,
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

/// Invokes Native `directory_rename`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_rename(
    source: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    destination: abi::HyperNativeHandle,
    new_path: *const u8,
    new_path_length: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_RENAME,
            source,
            path as u64,
            path_length as u64,
            destination,
            new_path as u64,
            new_path_length as u64,
        )
    }
}

/// Invokes Native `directory_link`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_link(
    source: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    destination: abi::HyperNativeHandle,
    new_path: *const u8,
    new_path_length: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_LINK,
            source,
            path as u64,
            path_length as u64,
            destination,
            new_path as u64,
            new_path_length as u64,
        )
    }
}

/// Invokes Native `directory_symlink`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_symlink(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    target: *const u8,
    target_length: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_SYMLINK,
            directory,
            path as u64,
            path_length as u64,
            target as u64,
            target_length as u64,
            0,
        )
    }
}

/// Invokes Native `directory_read_link`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_read_link(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    output: *mut u8,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_READ_LINK,
            directory,
            path as u64,
            path_length as u64,
            output as u64,
            capacity as u64,
            0,
        )
    }
}

/// Invokes Native `directory_canonicalize`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_canonicalize(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    output: *mut u8,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_CANONICALIZE,
            directory,
            path as u64,
            path_length as u64,
            output as u64,
            capacity as u64,
            0,
        )
    }
}

/// Invokes Native `directory_remove_if`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_remove_if(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    options: u32,
    expected_node_id: u64,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_REMOVE_IF,
            directory,
            path as u64,
            path_length as u64,
            options as u64,
            expected_node_id,
            0,
        )
    }
}

/// Invokes Native `directory_open_directory_nofollow`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_open_directory_nofollow(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    rights: u64,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW,
            directory,
            path as u64,
            path_length as u64,
            rights,
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

/// Invokes Native `directory_open_file_with_options`.
///
/// # Safety
/// Borrowed handles must stay live with required rights. Input pointers must
/// reference readable initialized records or the specified byte ranges; output
/// pointers must exclusively reference writable records or byte ranges.
pub unsafe fn directory_open_file_with_options(
    directory: abi::HyperNativeHandle,
    path: *const u8,
    path_length: usize,
    rights: u64,
    options: u32,
    mode: u32,
) -> CallResult {
    // SAFETY: the caller establishes handle lifetime and buffer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE_WITH_OPTIONS,
            directory,
            path as u64,
            path_length as u64,
            rights,
            options as u64,
            mode as u64,
        )
    }
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

/// Executes the Native `device_claim` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn device_claim(authority: u64, index: u32) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_CLAIM,
            authority,
            u64::from(index),
            0,
            0,
            0,
            0,
        )
    }
}

/// Claims exactly one device matching an explicit firmware identity and profile.
///
/// # Safety
/// The identity pointer must remain readable for `length` bytes during this call.
pub unsafe fn device_claim_matching(
    authority: u64,
    profile: u32,
    identity_kind: u32,
    identity: *const u8,
    length: usize,
) -> CallResult {
    // SAFETY: Caller retains the authority and readable identity bytes.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_CLAIM_MATCHING,
            authority,
            u64::from(profile),
            u64::from(identity_kind),
            identity as u64,
            length as u64,
            0,
        )
    }
}

/// Queries the immutable validated assignment profile.
///
/// # Safety
/// Output must be writable for `size` bytes and the handle must stay live.
pub unsafe fn device_profile_info(
    device: u64,
    output: *mut abi::HyperNativeDeviceProfileInfo,
    size: usize,
) -> CallResult {
    // SAFETY: Caller establishes the handle and output buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_PROFILE_INFO,
            device,
            output as u64,
            size as u64,
            0,
            0,
            0,
        )
    }
}
/// Queries one named, guest-relative register window.
///
/// # Safety
/// Output must be writable for `size` bytes and the handle must stay live.
pub unsafe fn device_resource_info(
    device: u64,
    index: u32,
    output: *mut abi::HyperNativeDeviceResourceInfo,
    size: usize,
) -> CallResult {
    // SAFETY: Caller establishes the handle and output buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_RESOURCE_INFO,
            device,
            u64::from(index),
            output as u64,
            size as u64,
            0,
            0,
        )
    }
}

/// Executes the Native `physical_device_info` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn physical_device_info(
    device: u64,
    output: *mut abi::HyperNativePhysicalDeviceInfo,
    size: usize,
) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PHYSICAL_DEVICE_INFO,
            device,
            output as u64,
            size as u64,
            0,
            0,
            0,
        )
    }
}

/// Executes the Native `guest_mailbox_create` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn guest_mailbox_create(machine: u64, base: u64, irq: u32) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_MAILBOX_CREATE,
            machine,
            base,
            u64::from(irq),
            0,
            0,
            0,
        )
    }
}

/// Executes the Native `guest_mailbox_send` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn guest_mailbox_send(mailbox: u64, bytes: *const u8, length: usize) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_MAILBOX_SEND,
            mailbox,
            bytes as u64,
            length as u64,
            0,
            0,
            0,
        )
    }
}

/// Executes the Native `guest_mailbox_receive` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn guest_mailbox_receive(mailbox: u64, bytes: *mut u8, capacity: usize) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_MAILBOX_RECEIVE,
            mailbox,
            bytes as u64,
            capacity as u64,
            0,
            0,
            0,
        )
    }
}

/// Executes the Native `guest_notification_create` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn guest_notification_create(
    frontend: u64,
    backend: u64,
    frontend_base: u64,
    backend_base: u64,
    frontend_irq: u32,
    backend_irq: u32,
) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_NOTIFICATION_CREATE,
            frontend,
            backend,
            frontend_base,
            backend_base,
            u64::from(frontend_irq),
            u64::from(backend_irq),
        )
    }
}

/// Executes the Native `guest_notification_control` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn guest_notification_control(notification: u64, operation: u32) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_NOTIFICATION_CONTROL,
            notification,
            u64::from(operation),
            0,
            0,
            0,
            0,
        )
    }
}

/// Creates a Native virtio-scsi initiator over a dedicated shared memory grant.
///
/// # Safety
/// Input handles must remain valid throughout this call.
#[inline]
pub unsafe fn native_block_create(
    memory: u64,
    backend: u64,
    guest_base: u64,
    notification_base: u64,
    notification_irq: u32,
) -> CallResult {
    // SAFETY: The caller retains the input capabilities.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_NATIVE_BLOCK_CREATE,
            memory,
            backend,
            guest_base,
            notification_base,
            u64::from(notification_irq),
            0,
        )
    }
}

/// Activates a negotiated initiator and discovers its SCSI capacity.
///
/// # Safety
/// The block handle must remain valid throughout this blocking call.
#[inline]
pub unsafe fn native_block_activate(block: u64, readonly: bool) -> CallResult {
    // SAFETY: The caller retains the input capability.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_NATIVE_BLOCK_ACTIVATE,
            block,
            u64::from(readonly),
            0,
            0,
            0,
            0,
        )
    }
}

/// Mounts a Native block volume at a directory-relative path.
///
/// # Safety
/// Both handles and the readable path must remain valid throughout the call.
#[inline]
pub unsafe fn native_block_mount(
    block: u64,
    directory: u64,
    path: *const u8,
    length: usize,
) -> CallResult {
    // SAFETY: The caller supplies valid capabilities and a borrowed path range.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_NATIVE_BLOCK_MOUNT,
            block,
            directory,
            path as u64,
            length as u64,
            0,
            0,
        )
    }
}

/// Creates a retained sparse DMA mapping in an installed backend VM.
/// # Safety
/// Input handles must remain valid throughout the call.
pub unsafe fn guest_mapping_create(backend: u64, memory: u64, frontend: u64) -> CallResult {
    // SAFETY: caller retains the borrowed input capabilities.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_MAPPING_CREATE,
            backend,
            memory,
            frontend,
            0,
            0,
            0,
        )
    }
}
/// Releases a mapping after backend-certified DMA quiescence.
/// # Safety
/// The mapping handle must remain valid throughout the call.
pub unsafe fn guest_mapping_release(mapping: u64) -> CallResult {
    // SAFETY: caller retains the borrowed mapping capability.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_MAPPING_RELEASE,
            mapping,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Executes the corresponding Native physical-device operation.
///
/// # Safety
/// Handles and pointer ranges must satisfy the Native ABI for the full call.
pub unsafe fn device_firmware_read(
    authority: u64,
    query: *const abi::HyperNativeDeviceFirmwareQuery,
    output: *mut u8,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller provides valid handles and complete memory ranges.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_FIRMWARE_READ,
            authority,
            query as u64,
            output as u64,
            capacity as u64,
            0,
            0,
        )
    }
}

/// Executes the corresponding Native physical-device operation.
///
/// # Safety
/// Handles and pointer ranges must satisfy the Native ABI for the full call.
pub unsafe fn device_claim_bundle(
    authority: u64,
    entries: *const abi::HyperNativeDeviceBundleEntry,
    count: usize,
    irq_node: u32,
) -> CallResult {
    // SAFETY: the caller provides valid handles and complete memory ranges.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_CLAIM_BUNDLE,
            authority,
            entries as u64,
            count as u64,
            u64::from(irq_node),
            0,
            0,
        )
    }
}

/// Executes the corresponding Native physical-device operation.
///
/// # Safety
/// Handles and pointer ranges must satisfy the Native ABI for the full call.
pub unsafe fn device_mmio(
    device: u64,
    offset: u64,
    width: u32,
    operation: u32,
    value: u64,
) -> CallResult {
    // SAFETY: the caller provides valid handles and complete memory ranges.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_MMIO,
            device,
            offset,
            u64::from(width),
            u64::from(operation),
            value,
            0,
        )
    }
}

/// Executes the corresponding Native physical-device operation.
///
/// # Safety
/// Handles and pointer ranges must satisfy the Native ABI for the full call.
pub unsafe fn device_irq_pending(device: u64) -> CallResult {
    // SAFETY: the caller provides valid handles and complete memory ranges.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_IRQ_PENDING,
            device,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Executes the corresponding Native physical-device operation.
///
/// # Safety
/// Handles and pointer ranges must satisfy the Native ABI for the full call.
pub unsafe fn device_irq_complete(device: u64, sequence: u64, asserted: bool) -> CallResult {
    // SAFETY: the caller provides valid handles and complete memory ranges.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_DEVICE_IRQ_COMPLETE,
            device,
            sequence,
            u64::from(asserted),
            0,
            0,
            0,
        )
    }
}
