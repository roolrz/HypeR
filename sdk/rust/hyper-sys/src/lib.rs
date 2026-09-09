// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Raw bindings to the `HypeR` Native userspace ABI.
//!
//! This crate deliberately exposes the ownership and pointer hazards of the
//! machine ABI. Native applications should use `hyper-os`; language runtimes
//! are the expected direct consumers of this crate.

#![no_std]

pub mod allocator;

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
    #[link_name = "hyper_native_call6"]
    fn ffi_native_call6(
        number: u64,
        argument0: u64,
        argument1: u64,
        argument2: u64,
        argument3: u64,
        argument4: u64,
        argument5: u64,
    ) -> CallResult;

    #[link_name = "hyper_abi_query"]
    fn ffi_abi_query() -> CallResult;

    #[link_name = "hyper_clock_get_monotonic"]
    fn ffi_clock_get_monotonic() -> CallResult;

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

    #[link_name = "hyper_byte_channel_write"]
    fn ffi_byte_channel_write(
        endpoint: abi::HyperNativeHandle,
        bytes: *const u8,
        byte_count: usize,
    ) -> abi::HyperNativeStatus;

    #[link_name = "hyper_byte_channel_read"]
    fn ffi_byte_channel_read(
        endpoint: abi::HyperNativeHandle,
        bytes: *mut u8,
        byte_capacity: usize,
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

/// Reads absolute nanoseconds from the kernel monotonic clock domain.
///
/// # Safety
///
/// The caller must be executing as a `HypeR` Native process through the runtime
/// and syscall veneer installed with this crate.
#[inline]
pub unsafe fn clock_get_monotonic() -> CallResult {
    // SAFETY: the caller establishes the Native runtime and syscall contract.
    unsafe { ffi_clock_get_monotonic() }
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

/// Duplicates one raw process handle with attenuated rights.
///
/// # Safety
///
/// `source` must remain live for the call. The caller assumes exclusive
/// ownership of a nonzero handle returned in `value0` only when the status is
/// `OK`.
#[inline]
pub unsafe fn handle_duplicate(source: abi::HyperNativeHandle, rights: u64) -> CallResult {
    // SAFETY: the caller establishes the source-handle lifetime and ownership
    // contract for the returned value.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_HANDLE_DUPLICATE,
            source,
            rights,
            0,
            0,
            0,
            0,
        )
    }
}

/// Replaces one raw process handle with an attenuated value.
///
/// # Safety
///
/// The caller must exclusively own `source`. An `OK` result consumes it and
/// transfers exclusive ownership of the nonzero `value0` handle to the caller;
/// every failure preserves ownership of `source`.
#[inline]
pub unsafe fn handle_replace(source: abi::HyperNativeHandle, rights: u64) -> CallResult {
    // SAFETY: the caller owns the source's consume-on-success transition.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_HANDLE_REPLACE,
            source,
            rights,
            0,
            0,
            0,
            0,
        )
    }
}

/// Retrieves handle-local metadata into one ABI record.
///
/// # Safety
///
/// `handle` must remain live during the call and `info` must be aligned and
/// writable for one complete [`abi::HyperNativeHandleInfo`] record.
#[inline]
pub unsafe fn handle_get_info(
    handle: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeHandleInfo,
) -> CallResult {
    // SAFETY: the caller establishes both handle and output-pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_HANDLE_GET_INFO,
            handle,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeHandleInfo>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Retrieves object identity and kind into one ABI record.
///
/// # Safety
///
/// `handle` must remain live during the call and carry `INSPECT` rights.
/// `info` must be aligned and writable for one complete
/// [`abi::HyperNativeObjectBasicInfo`] record.
#[inline]
pub unsafe fn object_get_basic_info(
    handle: abi::HyperNativeHandle,
    info: *mut abi::HyperNativeObjectBasicInfo,
) -> CallResult {
    // SAFETY: the caller establishes both handle and output-pointer validity.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO,
            handle,
            info.addr() as u64,
            core::mem::size_of::<abi::HyperNativeObjectBasicInfo>() as u64,
            0,
            0,
            0,
        )
    }
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

/// Waits for one member of a raw object-wait array.
///
/// # Safety
///
/// Every record must contain a live handle with wait rights and a valid signal
/// mask for that object's kind. `items` must remain readable for `item_count`
/// complete records throughout the call.
#[inline]
pub unsafe fn object_wait_many(
    items: *const abi::HyperNativeObjectWaitItem,
    item_count: usize,
    deadline: u64,
) -> CallResult {
    // SAFETY: the caller establishes the array and borrowed-handle contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_OBJECT_WAIT_MANY,
            items.addr() as u64,
            item_count as u64,
            deadline,
            0,
            0,
            0,
        )
    }
}

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

/// Reads retained guest output from a virtual serial port.
///
/// # Safety
///
/// `output` must be writable for `capacity` bytes and `serial` must remain live.
#[inline]
pub unsafe fn virtual_serial_read(
    serial: abi::HyperNativeHandle,
    output: *mut u8,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the pointer and handle contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_READ,
            serial,
            output as u64,
            capacity as u64,
            0,
            0,
            0,
        )
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

/// Creates one raw `ByteChannel` endpoint pair.
///
/// # Safety
///
/// On `OK`, the caller assumes exclusive ownership of both nonzero handles in
/// `value0` and `value1`. Every failure publishes no handle.
#[inline]
pub unsafe fn byte_channel_create() -> CallResult {
    // SAFETY: the caller accepts ownership of both successful raw results.
    unsafe { ffi_native_call6(abi::HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE, 0, 0, 0, 0, 0, 0) }
}

/// Sends one handle-free message through a raw `ByteChannel` endpoint.
///
/// # Safety
///
/// `endpoint` must remain live with write rights. For a nonzero `byte_count`,
/// `bytes` must remain readable for that extent during the call.
#[inline]
pub unsafe fn byte_channel_write(
    endpoint: abi::HyperNativeHandle,
    bytes: *const u8,
    byte_count: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes the handle and input-buffer contracts.
    unsafe { ffi_byte_channel_write(endpoint, bytes, byte_count) }
}

/// Receives one handle-free message through a raw `ByteChannel` endpoint.
///
/// # Safety
///
/// `endpoint` must remain live with read rights. For a nonzero
/// `byte_capacity`, `bytes` must remain writable for that extent during the
/// call. Messages carrying handles are reported as too large by this veneer.
#[inline]
pub unsafe fn byte_channel_read(
    endpoint: abi::HyperNativeHandle,
    bytes: *mut u8,
    byte_capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-buffer contracts.
    unsafe { ffi_byte_channel_read(endpoint, bytes, byte_capacity) }
}

/// Creates one raw `CapabilityChannel` endpoint pair.
///
/// # Safety
///
/// On `OK`, the caller assumes exclusive ownership of both nonzero handles in
/// `value0` and `value1`. Every failure publishes no handle.
#[inline]
pub unsafe fn capability_channel_create() -> CallResult {
    // SAFETY: the caller accepts ownership of both successful raw results.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE,
            0,
            0,
            0,
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

/// Scans one fixed-capacity page of Processes visible through a task inspector.
///
/// # Safety
///
/// `inspector` must remain live with inspect rights. `records` must be writable
/// for `capacity` complete records, and capacity must equal the ABI page size.
#[inline]
pub unsafe fn task_inspector_scan_processes(
    inspector: abi::HyperNativeHandle,
    cursor: u64,
    records: *mut abi::HyperNativeTaskProcess,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-array contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES,
            inspector,
            cursor,
            records.addr() as u64,
            capacity as u64,
            0,
            0,
        )
    }
}

/// Scans one fixed-capacity page of Threads visible through a task inspector.
///
/// # Safety
///
/// The typed handle and complete writable record-array contracts must hold.
#[inline]
pub unsafe fn task_inspector_scan_threads(
    inspector: abi::HyperNativeHandle,
    cursor: u64,
    records: *mut abi::HyperNativeTaskThread,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-array contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS,
            inspector,
            cursor,
            records.addr() as u64,
            capacity as u64,
            0,
            0,
        )
    }
}

/// Scans one fixed-capacity page of global object observations.
///
/// # Safety
///
/// The typed handle and complete writable record-array contracts must hold.
#[inline]
pub unsafe fn object_inspector_scan_objects(
    inspector: abi::HyperNativeHandle,
    cursor: u64,
    records: *mut abi::HyperNativeObjectInspection,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-array contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS,
            inspector,
            cursor,
            records.addr() as u64,
            capacity as u64,
            0,
            0,
        )
    }
}

/// Scans handles owned by one visible Process KOID.
///
/// # Safety
///
/// The typed handle and complete writable record-array contracts must hold.
#[inline]
pub unsafe fn object_inspector_scan_handles(
    inspector: abi::HyperNativeHandle,
    process_koid: u64,
    cursor: u64,
    records: *mut abi::HyperNativeHandleInspection,
    capacity: usize,
) -> CallResult {
    // SAFETY: the caller establishes the handle and output-array contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES,
            inspector,
            process_koid,
            cursor,
            records.addr() as u64,
            capacity as u64,
            0,
        )
    }
}

/// Derives a Process-scoped inspector from a wider inspector.
///
/// # Safety
///
/// Both handles must remain live with the ABI-required rights. On `OK`, the
/// caller assumes exclusive ownership of the returned nonzero handle.
#[inline]
unsafe fn inspector_derive(
    syscall: u64,
    inspector: abi::HyperNativeHandle,
    scope: abi::HyperNativeHandle,
) -> CallResult {
    // SAFETY: the caller owns successful output-handle adoption.
    unsafe { ffi_native_call6(syscall, inspector, scope, 0, 0, 0, 0) }
}

macro_rules! inspector_derivation {
    ($name:ident, $syscall:ident, $scope:literal) => {
        #[doc = concat!("Derives an inspector scoped to one ", $scope, ".")]
        ///
        /// # Safety
        ///
        /// Both handles must remain live with the ABI-required rights. On
        /// `OK`, the caller assumes exclusive ownership of the returned handle.
        #[inline]
        pub unsafe fn $name(
            inspector: abi::HyperNativeHandle,
            scope: abi::HyperNativeHandle,
        ) -> CallResult {
            // SAFETY: the caller owns successful output-handle adoption.
            unsafe { inspector_derive(abi::$syscall, inspector, scope) }
        }
    };
}

inspector_derivation!(
    task_inspector_derive_process,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS,
    "Process"
);
inspector_derivation!(
    task_inspector_derive_task_group,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP,
    "TaskGroup"
);
inspector_derivation!(
    task_inspector_derive_resource_domain,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
    "ResourceDomain"
);
inspector_derivation!(
    object_inspector_derive_process,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS,
    "Process"
);
inspector_derivation!(
    object_inspector_derive_task_group,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP,
    "TaskGroup"
);
inspector_derivation!(
    object_inspector_derive_resource_domain,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
    "ResourceDomain"
);

/// Attempts one transactional capability rendezvous.
///
/// # Safety
///
/// `endpoint` must remain live with write rights. The byte and disposition
/// arrays must remain readable for their complete extents. Every disposition
/// must describe an exclusively owned or validly borrowed handle according to
/// its operation. `OK` consumes every MOVE source and creates the corresponding
/// destination owners; every non-`OK` result preserves all source ownership.
#[inline]
pub unsafe fn capability_channel_try_send(
    endpoint: abi::HyperNativeHandle,
    bytes: *const u8,
    byte_count: usize,
    dispositions: *const abi::HyperNativeCapabilityDisposition,
    disposition_count: usize,
) -> abi::HyperNativeStatus {
    // SAFETY: the caller establishes all pointer, handle, and transactional
    // ownership contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
            endpoint,
            0,
            bytes.addr() as u64,
            byte_count as u64,
            dispositions.addr() as u64,
            disposition_count as u64,
        )
        .status
    }
}

/// Receives one transactional capability rendezvous.
///
/// # Safety
///
/// `endpoint` must remain live with read rights. `bytes` and `slots` must be
/// writable for their declared extents; slots must also contain initialized
/// receive requests. On `OK`, the caller owns each nonzero handle installed in
/// the first `value1` slots. On non-`OK`, no output handle is live. A `FAULT`
/// may have partially modified output memory, which the caller must ignore.
#[inline]
pub unsafe fn capability_channel_receive(
    endpoint: abi::HyperNativeHandle,
    deadline: u64,
    bytes: *mut u8,
    byte_capacity: usize,
    slots: *mut abi::HyperNativeCapabilityReceiveSlot,
    slot_count: usize,
) -> CallResult {
    // SAFETY: the caller establishes all pointer, handle, initialization, and
    // successful-result ownership contracts.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE,
            endpoint,
            deadline,
            bytes.addr() as u64,
            byte_capacity as u64,
            slots.addr() as u64,
            slot_count as u64,
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

/// Reads one immutable physical-memory accounting snapshot.
///
/// # Safety
///
/// `observation` must designate one writable ABI observation record.
pub unsafe fn memory_inspector_read(
    inspector: abi::HyperNativeHandle,
    observation: *mut abi::HyperNativeMemoryObservation,
) -> CallResult {
    // SAFETY: the caller owns the pointer contract stated above.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_MEMORY_INSPECTOR_READ,
            inspector,
            observation.addr() as u64,
            core::mem::size_of::<abi::HyperNativeMemoryObservation>() as u64,
            0,
            0,
            0,
        )
    }
}

/// Reads one immutable scheduler CPU-time snapshot.
///
/// # Safety
///
/// `observation` must designate one writable ABI observation record.
pub unsafe fn cpu_inspector_read(
    inspector: abi::HyperNativeHandle,
    observation: *mut abi::HyperNativeCpuObservation,
) -> CallResult {
    // SAFETY: the caller owns the pointer contract stated above.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_CPU_INSPECTOR_READ,
            inspector,
            observation.addr() as u64,
            core::mem::size_of::<abi::HyperNativeCpuObservation>() as u64,
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

/// Allocates an exact child VMAR.
///
/// # Safety
///
/// `parent` must remain live with map rights. On `OK`, the caller assumes
/// exclusive ownership of the child VMAR in `value0`.
#[inline]
pub unsafe fn vmar_allocate(parent: abi::HyperNativeHandle, address: u64, size: u64) -> CallResult {
    // SAFETY: the caller establishes the parent and result ownership contract.
    unsafe {
        ffi_native_call6(
            abi::HYPER_NATIVE_SYS_VMAR_ALLOCATE,
            parent,
            address,
            size,
            0,
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
