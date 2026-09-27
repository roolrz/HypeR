// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Raw bindings to the `HypeR` Native userspace ABI.
//!
//! This crate deliberately exposes the ownership and pointer hazards of the
//! machine ABI. Native applications should use `hyper-os`; language runtimes
//! are the expected direct consumers of this crate.

#![no_std]

pub mod allocator;

mod ffi;
mod handle;
mod inspect;
mod ipc;
mod startup;
mod system;
mod wait;

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
pub use startup::{AuxiliaryEntry, RawStartup, startup_find_handle};
pub use system::{abi_query, clock_get_monotonic, clock_get_realtime, system_config};
pub use wait::{
    atomic_wait, atomic_wake, object_wait_many, object_wait_one, wait_set_add, wait_set_create,
    wait_set_rearm, wait_set_remove, wait_set_wait,
};

use ffi::{
    ffi_console_read, ffi_console_write, ffi_native_call6, ffi_process_exit, ffi_thread_create,
    ffi_thread_exit, ffi_thread_request_stop, ffi_thread_sleep, ffi_thread_start, ffi_thread_yield,
};

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

/// Derives one resource-domain-bound VM creation lease.
///
/// # Safety
///
/// Both input handles must remain live. On success, the caller owns the
/// returned nonzero handle in `value0`.
#[inline]
pub unsafe fn virtual_machine_creation_lease_create(
    authority: abi::HyperNativeHandle,
    resource_domain: abi::HyperNativeHandle,
) -> CallResult {
    // SAFETY: the caller establishes the borrowed input and output ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_CREATE,
            authority,
            resource_domain,
            0,
            0,
            0,
            0,
        )
    }
}

/// Creates an independently accounted child resource domain.
///
/// # Safety
///
/// `parent` must remain live with create-resource-domain rights and `limits`
/// must identify one readable ABI limits record. On success, the caller owns
/// the returned nonzero handle in `value0`.
#[inline]
pub unsafe fn resource_domain_create(
    parent: abi::HyperNativeHandle,
    limits: *const abi::HyperNativeResourceLimits,
) -> CallResult {
    // SAFETY: the caller establishes the borrowed input and output ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_RESOURCE_DOMAIN_CREATE,
            parent,
            limits as u64,
            core::mem::size_of::<abi::HyperNativeResourceLimits>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Creates a task group charged to `resource_domain`.
///
/// # Safety
///
/// Both input handles must remain live with their required rights. On
/// success, the caller owns the returned nonzero handle in `value0`.
#[inline]
pub unsafe fn task_group_create(
    factory: abi::HyperNativeHandle,
    resource_domain: abi::HyperNativeHandle,
) -> CallResult {
    // SAFETY: the caller establishes borrowed inputs and output ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_TASK_GROUP_CREATE,
            factory,
            resource_domain,
            0,
            0,
            0,
            0,
        )
    }
}

/// Creates a pending VM and consumes `lease` only on success.
///
/// # Safety
///
/// `configuration` must remain readable for the complete call. The caller
/// must honor the consume-on-success contract for `lease` and adopt the
/// returned handle in `value0` only on success.
#[inline]
pub unsafe fn virtual_machine_create(
    lease: abi::HyperNativeHandle,
    configuration: *const abi::HyperNativeVirtualMachineConfiguration,
) -> CallResult {
    // SAFETY: the caller establishes input pointer and handle ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATE,
            lease,
            configuration.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualMachineConfiguration>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Attaches the writable guest-memory VMO to a pending VM.
///
/// # Safety
///
/// Both handles must remain live for the complete call.
#[inline]
pub unsafe fn pending_virtual_machine_set_memory(
    pending: abi::HyperNativeHandle,
    vmo: abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes both borrowed handle lifetimes.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_MEMORY,
            pending,
            vmo,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Commits a virtual-serial device capability into a pending VM.
///
/// # Safety
///
/// Both handles must remain live for the complete call. The caller must honor
/// the consume-on-success contract for `serial`.
#[inline]
pub unsafe fn pending_virtual_machine_set_virtual_serial(
    pending: abi::HyperNativeHandle,
    serial: abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes both handle lifetimes and ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL,
            pending,
            serial,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Creates one unbound buffered virtual serial port.
///
/// # Safety
///
/// The caller must adopt the returned handle exactly once on success.
#[inline]
pub unsafe fn virtual_serial_create() -> CallResult {
    // SAFETY: result ownership is delegated to the caller.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_CREATE,
            0,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Registers caller-owned whole pages for virtual serial output.
///
/// # Safety
///
/// Both handles must remain live with WRITE authority (and READ|MAP for the
/// VMO). Do not access its contents during registration. After success use
/// the shared-ring atomic protocol until all kernel producers are quiescent.
#[inline]
pub unsafe fn virtual_serial_register_output(
    serial: abi::HyperNativeHandle,
    buffer: abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: caller retains both handles and suspends buffer access during registration.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_REGISTER_OUTPUT,
            serial,
            buffer,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Acknowledges consumed output and atomically reconciles READABLE.
///
/// # Safety
/// The serial handle must retain READ authority. Complete all reads of the
/// acknowledged prefix before this call; those slots may immediately be reused.
#[inline]
pub unsafe fn virtual_serial_acknowledge_output(
    serial: abi::HyperNativeHandle,
    consumed: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller retains the handle and has completed the prefix reads.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT,
            serial,
            consumed,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Writes guest input to a virtual serial port.
///
/// # Safety
///
/// `bytes` must be readable for `length` bytes and `serial` must remain live.
#[inline]
pub unsafe fn virtual_serial_write(
    serial: abi::HyperNativeHandle,
    bytes: *const u8,
    length: usize,
) -> CallResult {
    // SAFETY: the caller establishes the pointer and handle contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE,
            serial,
            bytes as u64,
            length as u64,
            0,
            0,
            0,
        )
    }
}

/// Sets the boot-vCPU initial machine state.
///
/// # Safety
///
/// `bootstrap` must remain readable and `pending` live for the complete call.
#[inline]
pub unsafe fn pending_virtual_machine_set_bootstrap(
    pending: abi::HyperNativeHandle,
    bootstrap: *const abi::HyperNativeVirtualCpuBootstrap,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes pointer validity and handle lifetime.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP,
            pending,
            bootstrap.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualCpuBootstrap>() as u64,
            0,
            0,
            0,
        )
        .status
    }
}

/// Seals a fully configured pending VM.
///
/// # Safety
///
/// `pending` must remain live for the complete call.
#[inline]
pub unsafe fn pending_virtual_machine_seal(
    pending: abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the borrowed handle lifetime.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SEAL,
            pending,
            0,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Installs a sealed VM in a dormant state and consumes `pending` only on
/// success.
///
/// # Safety
///
/// The caller must honor the consume-on-success contract and adopt both
/// returned handles only when the result status is `OK`.
#[inline]
pub unsafe fn pending_virtual_machine_install(pending: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: the caller establishes input and returned-handle ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_INSTALL,
            pending,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Makes one installed dormant vCPU scheduler-runnable.
///
/// # Safety
///
/// `vcpu` must remain live with start rights for the complete call.
#[inline]
pub unsafe fn virtual_cpu_start(vcpu: abi::HyperNativeHandle) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the borrowed handle lifetime.
    unsafe { ffi_native_call6(abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_START, vcpu, 0, 0, 0, 0, 0).status }
}

/// Replaces a vCPU's host-CPU affinity mask.
///
/// # Safety
/// `vcpu` must remain live with write rights and `words` must remain readable
/// for `word_count` little-endian `u64` values throughout the call.
#[inline]
pub unsafe fn virtual_cpu_set_affinity(
    vcpu: abi::HyperNativeHandle,
    words: *const u64,
    word_count: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the handle and input buffer lifetimes.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_SET_AFFINITY,
            vcpu,
            words.addr() as u64,
            word_count as u64,
            0,
            0,
            0,
        )
        .status
    }
}

/// Aborts and consumes one pending VM only on success.
///
/// # Safety
///
/// The caller must honor the consume-on-success contract for `pending`.
#[inline]
pub unsafe fn pending_virtual_machine_abort(
    pending: abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes consuming handle ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ABORT,
            pending,
            0,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Requests asynchronous VM stop.
///
/// # Safety
///
/// `machine` must remain live for the complete call.
#[inline]
pub unsafe fn virtual_machine_request_stop(
    machine: abi::HyperNativeHandle,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the borrowed handle lifetime.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REQUEST_STOP,
            machine,
            0,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Reads installed VM metadata.
///
/// # Safety
///
/// `machine` must remain live and `info` writable for the complete call.
#[inline]
pub unsafe fn virtual_machine_get_info(
    machine: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeVirtualMachineInfo,
) -> CallResult {
    // SAFETY: the caller establishes handle and pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_INFO,
            machine,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualMachineInfo>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Reads installed vCPU metadata.
///
/// # Safety
///
/// `vcpu` must remain live and `info` writable for the complete call.
#[inline]
pub unsafe fn virtual_cpu_get_info(
    vcpu: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeVirtualCpuInfo,
) -> CallResult {
    // SAFETY: the caller establishes handle and pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_INFO,
            vcpu,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualCpuInfo>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Retrieves terminal and lifecycle information for one raw Process handle.
///
/// # Safety
///
/// `process` must remain live with inspect rights. `info` must be aligned and
/// writable for one complete process-info record.
#[inline]
pub unsafe fn process_get_info(
    process: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeProcessInfo,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-pointer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_GET_INFO,
            process,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeProcessInfo>() as u64,
            0,
            0,
            0,
        )
    }
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

/// Creates one zero-filled writable VMO.
///
/// # Safety
///
/// On `OK`, the caller assumes exclusive ownership of the VMO in `value0`.
#[inline]
pub unsafe fn vmo_create(size: u64) -> CallResult {
    // SAFETY: ownership of the successful raw result transfers to the caller.
    unsafe { ffi_native_call6(abi::HYPER_NATIVE_SYS_VMO_CREATE, size, 0, 0, 0, 0, 0) }
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

/// Reads bytes from a VMO.
///
/// # Safety
///
/// `vmo` must remain live with read rights and `output` must be writable for
/// `length` bytes.
#[inline]
pub unsafe fn vmo_read(
    vmo: abi::HyperNativeHandle,
    offset: u64,
    output: *mut u8,
    length: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the raw handle and output range.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMO_READ,
            vmo,
            offset,
            output.addr() as u64,
            length as u64,
            0,
            0,
        )
        .status
    }
}

/// Writes bytes into a writable VMO.
///
/// # Safety
///
/// `vmo` must remain live with write rights and `input` must be readable for
/// `length` bytes.
#[inline]
pub unsafe fn vmo_write(
    vmo: abi::HyperNativeHandle,
    offset: u64,
    input: *const u8,
    length: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the raw handle and input range.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMO_WRITE,
            vmo,
            offset,
            input.addr() as u64,
            length as u64,
            0,
            0,
        )
        .status
    }
}

/// Allocates a child VMAR. Options zero treats address as a low-end hint;
/// address zero selects the lowest free range. `VMAR_ALLOCATE_EXACT` forbids
/// relocation, including for address zero. Success returns its base in value1.
///
/// # Safety
///
/// `parent` must remain live with map rights. On `OK`, the caller assumes
/// exclusive ownership of the child VMAR in `value0`.
#[inline]
pub unsafe fn vmar_allocate(
    parent: abi::HyperNativeHandle,
    address: u64,
    size: u64,
    options: u64,
) -> CallResult {
    // SAFETY: the caller establishes the parent and result ownership contract.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMAR_ALLOCATE,
            parent,
            address,
            size,
            options,
            0,
            0,
        )
    }
}

/// Maps a VMO into an exact VMAR range.
///
/// # Safety
///
/// Both handles must remain live with map rights and all offsets, ranges, and
/// permissions must satisfy the Native VMAR contract.
#[inline]
pub unsafe fn vmar_map(
    vmar: abi::HyperNativeHandle,
    vmo: abi::HyperNativeHandle,
    object_offset: u64,
    address: u64,
    size: u64,
    permissions: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes both handles and the complete map range.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMAR_MAP,
            vmar,
            vmo,
            object_offset,
            address,
            size,
            permissions,
        )
        .status
    }
}

/// Changes permissions on a mapped VMAR range.
///
/// # Safety
///
/// `vmar` must remain live with map rights and the range must be fully owned
/// by it.
#[inline]
pub unsafe fn vmar_protect(
    vmar: abi::HyperNativeHandle,
    address: u64,
    size: u64,
    permissions: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the authority and exact range contract.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMAR_PROTECT,
            vmar,
            address,
            size,
            permissions,
            0,
            0,
        )
        .status
    }
}

/// Removes mappings from a VMAR range.
///
/// # Safety
///
/// `vmar` must remain live with map rights and the range must be fully owned
/// by it.
#[inline]
pub unsafe fn vmar_unmap(
    vmar: abi::HyperNativeHandle,
    address: u64,
    size: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the authority and exact range contract.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMAR_UNMAP,
            vmar,
            address,
            size,
            0,
            0,
            0,
        )
        .status
    }
}

/// Destroys and consumes one empty child VMAR.
///
/// # Safety
///
/// The caller must exclusively own `vmar`. `OK` consumes it; every failure
/// preserves ownership.
#[inline]
pub unsafe fn vmar_destroy(vmar: abi::HyperNativeHandle) -> abi::HyperNativeStatus {
    // SAFETY: the caller owns the consume-on-success transition.
    unsafe { ffi_native_call6(abi::HYPER_NATIVE_SYS_VMAR_DESTROY, vmar, 0, 0, 0, 0, 0).status }
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

/// Creates a mutable process builder from four borrowed authorities.
///
/// # Safety
///
/// Every input handle must remain live for the call and satisfy its exact ABI
/// kind and rights contract. On `OK`, the caller exclusively owns the nonzero
/// builder handle returned in `value0`.
#[inline]
pub unsafe fn process_builder_create(
    factory: abi::HyperNativeHandle,
    group: abi::HyperNativeHandle,
    domain: abi::HyperNativeHandle,
    executable: abi::HyperNativeHandle,
) -> CallResult {
    // SAFETY: the caller establishes all borrowed authority and result-owner
    // contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE,
            factory,
            group,
            domain,
            executable,
            0,
            0,
        )
    }
}

/// Sets the builder's process/thread name.
///
/// # Safety
///
/// `builder` must remain live with write rights and `name` must remain readable
/// for `name_size` bytes.
#[inline]
pub unsafe fn process_builder_set_name(
    builder: abi::HyperNativeHandle,
    name: *const u8,
    name_size: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME,
            builder,
            name.addr() as u64,
            name_size as u64,
            0,
            0,
            0,
        )
        .status
    }
}

/// Appends one argv entry to a process builder.
///
/// # Safety
///
/// `builder` must remain live with write rights and `argument` must remain
/// readable for `argument_size` bytes.
#[inline]
pub unsafe fn process_builder_add_argument(
    builder: abi::HyperNativeHandle,
    argument: *const u8,
    argument_size: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT,
            builder,
            argument.addr() as u64,
            argument_size as u64,
            0,
            0,
            0,
        )
        .status
    }
}

/// Appends one `name=value` environment entry to a process builder.
///
/// # Safety
///
/// `builder` must remain live with write rights and `environment` must remain
/// readable for `environment_size` bytes.
#[inline]
pub unsafe fn process_builder_add_environment(
    builder: abi::HyperNativeHandle,
    environment: *const u8,
    environment_size: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT,
            builder,
            environment.addr() as u64,
            environment_size as u64,
            0,
            0,
            0,
        )
        .status
    }
}

/// Replaces the process builder's CPU affinity mask.
///
/// # Safety
///
/// `builder` must remain live with write rights and `words` must remain
/// readable for `word_count` `u64` values.
#[inline]
pub unsafe fn process_builder_set_affinity(
    builder: abi::HyperNativeHandle,
    words: *const u64,
    word_count: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the handle and buffer contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY,
            builder,
            words.addr() as u64,
            word_count as u64,
            0,
            0,
            0,
        )
        .status
    }
}

/// Adds one capability disposition to a process builder.
///
/// # Safety
///
/// `builder` and `source` must satisfy the ABI kind, rights, and lifetime
/// contracts. MOVE consumes `source` only on `OK`; DUPLICATE preserves it on
/// every result.
#[inline]
pub unsafe fn process_builder_add_handle(
    builder: abi::HyperNativeHandle,
    source: abi::HyperNativeHandle,
    purpose: u32,
    expected_kind: u32,
    rights: u64,
    operation: u32,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes both handle contracts and owns the
    // operation-dependent commit transition.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
            builder,
            source,
            u64::from(purpose),
            u64::from(expected_kind),
            rights,
            u64::from(operation),
        )
        .status
    }
}

/// Irreversibly seals a process builder.
///
/// # Safety
///
/// `builder` must remain live with write rights for the call.
#[inline]
pub unsafe fn process_builder_seal(builder: abi::HyperNativeHandle) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the borrowed builder contract.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL,
            builder,
            0,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Starts and consumes one sealed process builder.
///
/// # Safety
///
/// The caller must exclusively own `builder`. `OK` consumes it and publishes
/// one nonzero Process handle in `value0`; every failure preserves `builder`.
#[inline]
pub unsafe fn process_builder_start(builder: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: the caller owns the builder's consume-on-success transition.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_START,
            builder,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Aborts and consumes one process builder.
///
/// # Safety
///
/// The caller must exclusively own `builder`. `OK` consumes it; every failure
/// preserves ownership.
#[inline]
pub unsafe fn process_builder_abort(builder: abi::HyperNativeHandle) -> abi::HyperNativeStatus {
    // SAFETY: the caller owns the builder's consume-on-success transition.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT,
            builder,
            0,
            0,
            0,
            0,
            0,
        )
        .status
    }
}

/// Requests asynchronous termination of a Process.
///
/// # Safety
///
/// `process` must remain live with request-stop rights for the call.
#[inline]
pub unsafe fn process_request_stop(process: abi::HyperNativeHandle) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the borrowed Process contract.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP,
            process,
            0,
            0,
            0,
            0,
            0,
        )
        .status
    }
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

/// Invokes Native `thread_create`.
///
/// # Safety
/// Handles, addresses and thread entry state must satisfy the Native ABI;
/// referenced memory must remain live through the operation or thread lifetime.
pub unsafe fn thread_create(
    entry: u64,
    stack: u64,
    tls: u64,
    argument: u64,
    affinity_words: *const u64,
    affinity_word_count: usize,
) -> CallResult {
    // SAFETY: the caller upholds the raw syscall contract, including the
    // borrowed little-endian affinity array (or null/zero for inheritance).
    unsafe {
        ffi_thread_create(
            entry,
            stack,
            tls,
            argument,
            affinity_words,
            affinity_word_count,
        )
    }
}

/// Invokes Native `thread_start`.
///
/// # Safety
/// Handles, addresses and thread entry state must satisfy the Native ABI;
/// referenced memory must remain live through the operation or thread lifetime.
pub unsafe fn thread_start(thread: u64) -> abi::HyperNativeStatus {
    // SAFETY: the caller upholds the raw syscall contract.
    unsafe { ffi_thread_start(thread) }
}

/// Invokes Native `thread_request_stop`.
///
/// # Safety
/// Handles, addresses and thread entry state must satisfy the Native ABI;
/// referenced memory must remain live through the operation or thread lifetime.
pub unsafe fn thread_request_stop(thread: u64) -> abi::HyperNativeStatus {
    // SAFETY: the caller upholds the raw syscall contract.
    unsafe { ffi_thread_request_stop(thread) }
}

/// Invokes Native `thread_sleep`.
///
/// # Safety
/// Handles, addresses and thread entry state must satisfy the Native ABI;
/// referenced memory must remain live through the operation or thread lifetime.
pub unsafe fn thread_sleep(deadline: u64) -> abi::HyperNativeStatus {
    // SAFETY: the caller upholds the raw syscall contract.
    unsafe { ffi_thread_sleep(deadline) }
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

/// Returns the calling process's observation-only KOID.
///
/// # Safety
/// The caller must execute in a Native process using the matching SDK.
pub unsafe fn process_get_current_id() -> CallResult {
    // SAFETY: this call has no pointers or capability arguments.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PROCESS_GET_CURRENT_ID,
            0,
            0,
            0,
            0,
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

/// Reads immutable board metadata without consuming the creation lease.
///
/// # Safety
/// `lease` must remain live and `info` writable for the complete call.
#[inline]
pub unsafe fn virtual_machine_creation_lease_get_platform_info(
    lease: abi::HyperNativeHandle,
    profile: u32,
    info: *mut abi::HyperNativeVirtualMachinePlatformInfo,
) -> CallResult {
    // SAFETY: the caller establishes handle and pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO,
            lease,
            u64::from(profile),
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualMachinePlatformInfo>() as u64,
            0,
            0,
        )
    }
}

/// Creates an immutable snapshot of borrowed VMO bytes.
///
/// # Safety
/// `vmo` must name a live readable VMO; the caller adopts the produced handle.
pub unsafe fn vmo_create_snapshot(vmo: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: the caller supplies a live borrowed VMO and adopts the result.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMO_CREATE_SNAPSHOT,
            vmo,
            0,
            0,
            0,
            0,
            0,
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

/// Installs a private mapping of immutable snapshot bytes.
///
/// # Safety
/// Handles and the input record must be live for this call. The caller must
/// own the destination virtual range and uphold Rust aliasing for its mappings.
pub unsafe fn vmar_map_private(
    vmar: abi::HyperNativeHandle,
    snapshot: abi::HyperNativeHandle,
    mapping: *const abi::HyperNativePrivateMapping,
    mapping_size: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller supplies live handles, request bytes, and destination ownership.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMAR_MAP_PRIVATE,
            vmar,
            snapshot,
            mapping as usize as u64,
            mapping_size as u64,
            0,
            0,
        )
        .status
    }
}

/// Snapshots one pending guest power request without consuming it.
///
/// # Safety
///
/// `machine` must remain live and `request` writable for the complete call.
#[inline]
pub unsafe fn virtual_machine_get_power_request(
    machine: abi::HyperNativeHandle,
    request: *mut abi::HyperNativeVirtualMachinePowerRequest,
) -> CallResult {
    // SAFETY: The caller establishes handle and output validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_POWER_REQUEST,
            machine,
            request.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualMachinePowerRequest>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Completes the exact pending power request.
///
/// # Safety
///
/// `machine` must remain live throughout the call.
#[inline]
pub unsafe fn virtual_machine_complete_power_request(
    machine: abi::HyperNativeHandle,
    request_id: u64,
    accept: u32,
) -> abi::HyperNativeStatus {
    // SAFETY: The caller retains the handle; scalar arguments borrow no memory.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST,
            machine,
            request_id,
            u64::from(accept),
            0,
            0,
            0,
        )
        .status
    }
}

/// Opens one configured vCPU control handle.
///
/// # Safety
///
/// `machine` must remain live; a successfully returned handle must be adopted once.
#[inline]
pub unsafe fn virtual_machine_open_vcpu(
    machine: abi::HyperNativeHandle,
    vcpu_id: u32,
) -> CallResult {
    // SAFETY: The caller retains the input and owns the returned capability.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_OPEN_VCPU,
            machine,
            u64::from(vcpu_id),
            0,
            0,
            0,
            0,
        )
    }
}

/// Registers one device aperture before a VM starts.
///
/// # Safety
/// The machine handle must remain live throughout the call.
#[inline]
pub unsafe fn virtual_machine_register_mmio(
    machine: abi::HyperNativeHandle,
    base: u64,
    length: u64,
    device: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: The caller retains the handle; all remaining arguments are scalars.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REGISTER_MMIO,
            machine,
            base,
            length,
            device,
            0,
            0,
        )
        .status
    }
}

/// Snapshots a pending MMIO instruction without consuming its continuation.
///
/// # Safety
/// The vCPU handle must remain live and request must be writable for the call.
#[inline]
pub unsafe fn virtual_cpu_get_mmio_request(
    vcpu: abi::HyperNativeHandle,
    request: *mut abi::HyperNativeVirtualCpuMmioRequest,
) -> CallResult {
    // SAFETY: The caller establishes handle and output validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_MMIO_REQUEST,
            vcpu,
            request.addr() as u64,
            core::mem::size_of::<abi::HyperNativeVirtualCpuMmioRequest>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Completes an exact pending MMIO instruction, or aborts its VM.
///
/// # Safety
/// The vCPU handle must remain live throughout the call.
#[inline]
pub unsafe fn virtual_cpu_complete_mmio(
    vcpu: abi::HyperNativeHandle,
    id: u64,
    operation: u32,
    value: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: The caller retains the handle; all remaining arguments are scalars.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_COMPLETE_MMIO,
            vcpu,
            id,
            u64::from(operation),
            value,
            0,
            0,
        )
        .status
    }
}

/// Freezes a writable VMO for explicitly shared guest use.
///
/// # Safety
/// The VMO handle remains live; adopt the returned memory handle exactly once.
#[inline]
pub unsafe fn guest_memory_create(vmo: abi::HyperNativeHandle) -> CallResult {
    // SAFETY: The caller retains the sole borrowed handle.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_GUEST_MEMORY_CREATE,
            vmo,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Adds a page-aligned region to a pending VM's immutable RAM layout.
///
/// # Safety
/// Both handles must remain live for the call.
#[inline]
pub unsafe fn pending_virtual_machine_map_memory(
    pending: abi::HyperNativeHandle,
    memory: abi::HyperNativeHandle,
    guest_offset: u64,
    source_offset: u64,
    length: u64,
) -> abi::HyperNativeStatus {
    // SAFETY: The caller retains both handles; remaining arguments are scalars.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_MAP_MEMORY,
            pending,
            memory,
            guest_offset,
            source_offset,
            length,
            0,
        )
        .status
    }
}

/// Creates an eagerly allocated physically contiguous writable VMO.
///
/// # Safety
/// Adopt a successfully returned handle exactly once.
#[inline]
pub unsafe fn vmo_create_contiguous(size: u64) -> CallResult {
    // SAFETY: Scalar request; no borrowed memory or input capability.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMO_CREATE_CONTIGUOUS,
            size,
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

/// Executes the Native `vmo_get_dma_extent` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn vmo_get_dma_extent(
    authority: u64,
    vmo: u64,
    offset: u64,
    length: u64,
    output: *mut abi::HyperNativeDmaExtent,
    size: usize,
) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMO_GET_DMA_EXTENT,
            authority,
            vmo,
            offset,
            length,
            output as u64,
            size as u64,
        )
    }
}

/// Executes the Native `pending_virtual_machine_assign_device` operation.
///
/// # Safety
/// Retain all input handles and provide valid readable/writable pointer ranges
/// as specified by the Native ABI for the complete non-retaining call.
#[inline]
pub unsafe fn pending_virtual_machine_assign_device(
    pending: u64,
    device: u64,
    base: u64,
    irq: u32,
) -> CallResult {
    // SAFETY: The caller establishes the ABI handle and memory contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE,
            pending,
            device,
            base,
            u64::from(irq),
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
