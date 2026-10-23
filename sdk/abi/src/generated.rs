// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Generated from schema/native.rs. Do not edit.

/// Pre-release schema revision; zero does not promise a stable published ABI.
pub const HYPER_NATIVE_ABI_REVISION: u64 = 0;
/// Business argument registers, excluding the receiver and operation selectors.
pub const HYPER_NATIVE_INVOCATION_ARGUMENT_REGISTERS: usize = 6;
/// Auxiliary result registers, excluding the signed status register.
pub const HYPER_NATIVE_INVOCATION_RESULT_REGISTERS: usize = 2;
/// Process-local, generation-qualified handle; not a KOID or pointer.
pub type HyperNativeHandle = u64;
/// Signed invocation status: zero for OK, negative for failure.
pub type HyperNativeStatus = i64;

/// Advertised ABI feature bit: `core`.
pub const HYPER_NATIVE_FEATURE_CORE: u64 = 1_u64 << 0;

/// Invocation status `ok`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_OK: HyperNativeStatus = 0;
/// Invocation status `invalid_argument`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_INVALID_ARGUMENT: HyperNativeStatus = -1;
/// Invocation status `bad_handle`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_BAD_HANDLE: HyperNativeStatus = -2;
/// Invocation status `access_denied`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_ACCESS_DENIED: HyperNativeStatus = -3;
/// Invocation status `not_supported`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_NOT_SUPPORTED: HyperNativeStatus = -4;
/// Invocation status `no_memory`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_NO_MEMORY: HyperNativeStatus = -5;
/// Invocation status `bad_state`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_BAD_STATE: HyperNativeStatus = -6;
/// Invocation status `fault`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_FAULT: HyperNativeStatus = -7;
/// Invocation status `resource_limit`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_RESOURCE_LIMIT: HyperNativeStatus = -8;
/// Invocation status `busy`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_BUSY: HyperNativeStatus = -9;
/// Invocation status `internal`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_INTERNAL: HyperNativeStatus = -10;
/// Invocation status `timed_out`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_TIMED_OUT: HyperNativeStatus = -11;
/// Invocation status `cancelled`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_CANCELLED: HyperNativeStatus = -12;
/// Invocation status `would_block`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_WOULD_BLOCK: HyperNativeStatus = -13;
/// Invocation status `buffer_too_small`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL: HyperNativeStatus = -14;
/// Invocation status `peer_closed`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_PEER_CLOSED: HyperNativeStatus = -15;
/// Invocation status `not_found`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_NOT_FOUND: HyperNativeStatus = -16;
/// Invocation status `already_exists`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_ALREADY_EXISTS: HyperNativeStatus = -17;
/// Invocation status `not_empty`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_NOT_EMPTY: HyperNativeStatus = -18;
/// Invocation status `not_directory`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_NOT_DIRECTORY: HyperNativeStatus = -19;
/// Invocation status `is_directory`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_IS_DIRECTORY: HyperNativeStatus = -20;
/// Invocation status `symlink_loop`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_SYMLINK_LOOP: HyperNativeStatus = -21;
/// Invocation status `cross_device`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_CROSS_DEVICE: HyperNativeStatus = -22;
/// Invocation status `io_error`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_IO_ERROR: HyperNativeStatus = -23;
/// Invocation status `no_space`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_NO_SPACE: HyperNativeStatus = -24;
/// Invocation status `read_only`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_READ_ONLY: HyperNativeStatus = -25;
/// Invocation status `revoked`. Auxiliary results on failure require an explicit method contract.
pub const HYPER_NATIVE_STATUS_REVOKED: HyperNativeStatus = -26;

/// Object-kind tag `none` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_NONE: u32 = 0;
/// Object-kind tag `event` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_EVENT: u32 = 1;
/// Object-kind tag `byte_channel` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_BYTE_CHANNEL: u32 = 2;
/// Object-kind tag `thread` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_THREAD: u32 = 3;
/// Object-kind tag `process` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_PROCESS: u32 = 4;
/// Object-kind tag `task_group` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_TASK_GROUP: u32 = 5;
/// Object-kind tag `resource_domain` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN: u32 = 6;
/// Object-kind tag `task_factory` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_TASK_FACTORY: u32 = 7;
/// Object-kind tag `executable_authority` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY: u32 = 8;
/// Object-kind tag `vmo` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VMO: u32 = 9;
/// Object-kind tag `vmar` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VMAR: u32 = 10;
/// Object-kind tag `console` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_CONSOLE: u32 = 11;
/// Object-kind tag `directory` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_DIRECTORY: u32 = 12;
/// Object-kind tag `file` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_FILE: u32 = 13;
/// Object-kind tag `capability_channel` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL: u32 = 14;
/// Object-kind tag `process_builder` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_PROCESS_BUILDER: u32 = 15;
/// Object-kind tag `task_inspector` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_TASK_INSPECTOR: u32 = 16;
/// Object-kind tag `object_inspector` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR: u32 = 17;
/// Object-kind tag `memory_inspector` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR: u32 = 18;
/// Object-kind tag `cpu_inspector` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_CPU_INSPECTOR: u32 = 19;
/// Object-kind tag `virtual_machine_creation_authority` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY: u32 = 20;
/// Object-kind tag `virtual_machine_creation_lease` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE: u32 = 21;
/// Object-kind tag `pending_virtual_machine` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE: u32 = 22;
/// Object-kind tag `virtual_machine` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE: u32 = 23;
/// Object-kind tag `virtual_cpu` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VIRTUAL_CPU: u32 = 24;
/// Object-kind tag `virtual_serial` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL: u32 = 25;
/// Object-kind tag `wait_set` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_WAIT_SET: u32 = 26;
/// Object-kind tag `guest_memory` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_GUEST_MEMORY: u32 = 27;
/// Object-kind tag `device_assignment_authority` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY: u32 = 28;
/// Object-kind tag `physical_device` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE: u32 = 29;
/// Object-kind tag `guest_mailbox` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_GUEST_MAILBOX: u32 = 30;
/// Object-kind tag `guest_notification` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION: u32 = 31;
/// Object-kind tag `native_block` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_NATIVE_BLOCK: u32 = 32;
/// Object-kind tag `guest_mapping` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_GUEST_MAPPING: u32 = 33;
/// Object-kind tag `file_protection` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_FILE_PROTECTION: u32 = 34;
/// Object-kind tag `directory_stream` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_DIRECTORY_STREAM: u32 = 35;
/// Object-kind tag `root_transition` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_ROOT_TRANSITION: u32 = 36;
/// Object-kind tag `guest_interrupt` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_GUEST_INTERRUPT: u32 = 37;
/// Object-kind tag `process_retirement` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT: u32 = 38;
/// Object-kind tag `crash_view` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_CRASH_VIEW: u32 = 39;
/// Object-kind tag `backend_memory_lease` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_BACKEND_MEMORY_LEASE: u32 = 40;
/// Object-kind tag `execution_context` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT: u32 = 41;
/// Object-kind tag `object_factory` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_OBJECT_FACTORY: u32 = 42;
/// Object-kind tag `system_info` used with an object-local operation number.
pub const HYPER_NATIVE_OBJECT_SYSTEM_INFO: u32 = 43;

/// Handle transport is forbidden for this object kind.
pub const HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN: u32 = 0;
/// Buffered capability transfer and rendezvous are permitted with transfer authority.
pub const HYPER_NATIVE_TRANSFER_CLASS_GENERAL: u32 = 1;
/// Transfer requires rendezvous or staged startup; buffered transfer is forbidden.
pub const HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY: u32 = 2;

/// Return the permitted transport class for an object kind.
/// Unknown kinds return FORBIDDEN. This classification does not grant TRANSFER authority.
pub const fn hyper_native_object_transfer_class(object_kind: u32) -> u32 {
    match object_kind {
        HYPER_NATIVE_OBJECT_NONE => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
        HYPER_NATIVE_OBJECT_EVENT => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_BYTE_CHANNEL => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_THREAD => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_PROCESS => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_TASK_GROUP => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_TASK_FACTORY => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_VMO => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_VMAR => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_CONSOLE => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_DIRECTORY => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_FILE => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_PROCESS_BUILDER => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_TASK_INSPECTOR => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_CPU_INSPECTOR => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY => {
            HYPER_NATIVE_TRANSFER_CLASS_GENERAL
        }
        HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE => {
            HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY
        }
        HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_VIRTUAL_CPU => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_WAIT_SET => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
        HYPER_NATIVE_OBJECT_GUEST_MEMORY => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_GUEST_MAILBOX => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_NATIVE_BLOCK => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_GUEST_MAPPING => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_FILE_PROTECTION => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
        HYPER_NATIVE_OBJECT_DIRECTORY_STREAM => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_ROOT_TRANSITION => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_GUEST_INTERRUPT => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT => HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY,
        HYPER_NATIVE_OBJECT_CRASH_VIEW => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        HYPER_NATIVE_OBJECT_BACKEND_MEMORY_LEASE => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
        HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
        HYPER_NATIVE_OBJECT_OBJECT_FACTORY => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
        HYPER_NATIVE_OBJECT_SYSTEM_INFO => HYPER_NATIVE_TRANSFER_CLASS_GENERAL,
        _ => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
    }
}

/// Capability right; the receiving object's method determines the required rights: `duplicate`.
pub const HYPER_NATIVE_RIGHT_DUPLICATE: u64 = 1_u64 << 0;
/// Capability right; the receiving object's method determines the required rights: `transfer`.
pub const HYPER_NATIVE_RIGHT_TRANSFER: u64 = 1_u64 << 1;
/// Capability right; the receiving object's method determines the required rights: `wait`.
pub const HYPER_NATIVE_RIGHT_WAIT: u64 = 1_u64 << 2;
/// Capability right; the receiving object's method determines the required rights: `inspect`.
pub const HYPER_NATIVE_RIGHT_INSPECT: u64 = 1_u64 << 3;
/// Capability right; the receiving object's method determines the required rights: `read`.
pub const HYPER_NATIVE_RIGHT_READ: u64 = 1_u64 << 4;
/// Capability right; the receiving object's method determines the required rights: `write`.
pub const HYPER_NATIVE_RIGHT_WRITE: u64 = 1_u64 << 5;
/// Capability right; the receiving object's method determines the required rights: `map`.
pub const HYPER_NATIVE_RIGHT_MAP: u64 = 1_u64 << 6;
/// Capability right; the receiving object's method determines the required rights: `execute`.
pub const HYPER_NATIVE_RIGHT_EXECUTE: u64 = 1_u64 << 7;
/// Capability right; the receiving object's method determines the required rights: `resize`.
pub const HYPER_NATIVE_RIGHT_RESIZE: u64 = 1_u64 << 8;
/// Capability right; the receiving object's method determines the required rights: `pin`.
pub const HYPER_NATIVE_RIGHT_PIN: u64 = 1_u64 << 9;
/// Capability right; the receiving object's method determines the required rights: `start`.
pub const HYPER_NATIVE_RIGHT_START: u64 = 1_u64 << 10;
/// Capability right; the receiving object's method determines the required rights: `request_stop`.
pub const HYPER_NATIVE_RIGHT_REQUEST_STOP: u64 = 1_u64 << 11;
/// Capability right; the receiving object's method determines the required rights: `run_vcpu`.
pub const HYPER_NATIVE_RIGHT_RUN_VCPU: u64 = 1_u64 << 12;
/// Capability right; the receiving object's method determines the required rights: `inject_interrupt`.
pub const HYPER_NATIVE_RIGHT_INJECT_INTERRUPT: u64 = 1_u64 << 13;
/// Capability right; the receiving object's method determines the required rights: `grant_memory`.
pub const HYPER_NATIVE_RIGHT_GRANT_MEMORY: u64 = 1_u64 << 14;
/// Capability right; the receiving object's method determines the required rights: `assign_device`.
pub const HYPER_NATIVE_RIGHT_ASSIGN_DEVICE: u64 = 1_u64 << 15;
/// Capability right; the receiving object's method determines the required rights: `map_dma`.
pub const HYPER_NATIVE_RIGHT_MAP_DMA: u64 = 1_u64 << 16;
/// Capability right; the receiving object's method determines the required rights: `ack_interrupt`.
pub const HYPER_NATIVE_RIGHT_ACK_INTERRUPT: u64 = 1_u64 << 17;
/// Capability right; the receiving object's method determines the required rights: `revoke`.
pub const HYPER_NATIVE_RIGHT_REVOKE: u64 = 1_u64 << 18;
/// Capability right; the receiving object's method determines the required rights: `signal`.
pub const HYPER_NATIVE_RIGHT_SIGNAL: u64 = 1_u64 << 19;
/// Capability right; the receiving object's method determines the required rights: `create_process`.
pub const HYPER_NATIVE_RIGHT_CREATE_PROCESS: u64 = 1_u64 << 20;
/// Capability right; the receiving object's method determines the required rights: `create_thread`.
pub const HYPER_NATIVE_RIGHT_CREATE_THREAD: u64 = 1_u64 << 21;
/// Capability right; the receiving object's method determines the required rights: `create_task_group`.
pub const HYPER_NATIVE_RIGHT_CREATE_TASK_GROUP: u64 = 1_u64 << 22;
/// Capability right; the receiving object's method determines the required rights: `create_resource_domain`.
pub const HYPER_NATIVE_RIGHT_CREATE_RESOURCE_DOMAIN: u64 = 1_u64 << 23;
/// Capability right; the receiving object's method determines the required rights: `set_limits`.
pub const HYPER_NATIVE_RIGHT_SET_LIMITS: u64 = 1_u64 << 24;
/// Capability right; the receiving object's method determines the required rights: `create_executable`.
pub const HYPER_NATIVE_RIGHT_CREATE_EXECUTABLE: u64 = 1_u64 << 25;
/// Capability right; the receiving object's method determines the required rights: `task_group_attach_process`.
pub const HYPER_NATIVE_RIGHT_TASK_GROUP_ATTACH_PROCESS: u64 = 1_u64 << 26;
/// Capability right; the receiving object's method determines the required rights: `resource_domain_sponsor`.
pub const HYPER_NATIVE_RIGHT_RESOURCE_DOMAIN_SPONSOR: u64 = 1_u64 << 27;
/// Capability right; the receiving object's method determines the required rights: `derive`.
pub const HYPER_NATIVE_RIGHT_DERIVE: u64 = 1_u64 << 28;
/// Capability right; the receiving object's method determines the required rights: `create_virtual_machine`.
pub const HYPER_NATIVE_RIGHT_CREATE_VIRTUAL_MACHINE: u64 = 1_u64 << 29;
/// Capability right; the receiving object's method determines the required rights: `bind_wait`.
pub const HYPER_NATIVE_RIGHT_BIND_WAIT: u64 = 1_u64 << 30;
/// Capability right; the receiving object's method determines the required rights: `set_attributes`.
pub const HYPER_NATIVE_RIGHT_SET_ATTRIBUTES: u64 = 1_u64 << 31;
/// Capability right; the receiving object's method determines the required rights: `lock_file`.
pub const HYPER_NATIVE_RIGHT_LOCK_FILE: u64 = 1_u64 << 32;
/// Capability right; the receiving object's method determines the required rights: `inspect_details`.
pub const HYPER_NATIVE_RIGHT_INSPECT_DETAILS: u64 = 1_u64 << 33;
/// Capability right; the receiving object's method determines the required rights: `manage_namespace`.
pub const HYPER_NATIVE_RIGHT_MANAGE_NAMESPACE: u64 = 1_u64 << 34;
/// Capability right; the receiving object's method determines the required rights: `protect`.
pub const HYPER_NATIVE_RIGHT_PROTECT: u64 = 1_u64 << 35;
/// Capability right; the receiving object's method determines the required rights: `lock_memory`.
pub const HYPER_NATIVE_RIGHT_LOCK_MEMORY: u64 = 1_u64 << 36;
/// Capability right; the receiving object's method determines the required rights: `retain_process`.
pub const HYPER_NATIVE_RIGHT_RETAIN_PROCESS: u64 = 1_u64 << 37;
/// Capability right; the receiving object's method determines the required rights: `retire`.
pub const HYPER_NATIVE_RIGHT_RETIRE: u64 = 1_u64 << 38;
/// Capability right; the receiving object's method determines the required rights: `create_object`.
pub const HYPER_NATIVE_RIGHT_CREATE_OBJECT: u64 = 1_u64 << 39;
/// Capability right; the receiving object's method determines the required rights: `sched_lower`.
pub const HYPER_NATIVE_RIGHT_SCHED_LOWER: u64 = 1_u64 << 40;
/// Capability right; the receiving object's method determines the required rights: `sched_priority`.
pub const HYPER_NATIVE_RIGHT_SCHED_PRIORITY: u64 = 1_u64 << 41;
/// Capability right; the receiving object's method determines the required rights: `sched_class`.
pub const HYPER_NATIVE_RIGHT_SCHED_CLASS: u64 = 1_u64 << 42;
/// Capability right; the receiving object's method determines the required rights: `sched_budget`.
pub const HYPER_NATIVE_RIGHT_SCHED_BUDGET: u64 = 1_u64 << 43;

/// All declared right bits; this mask does not grant authority to a handle.
pub const HYPER_NATIVE_RIGHTS_MASK: u64 = 0xfffffffffff;

/// Wait signal `ready` for `process_retirement` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_PROCESS_RETIREMENT_READY: u64 = 1_u64 << 0;
/// Wait signal `released` for `process_retirement` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_PROCESS_RETIREMENT_RELEASED: u64 = 1_u64 << 1;
/// Wait signal `readable` for `physical_device` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_PHYSICAL_DEVICE_READABLE: u64 = 1_u64 << 0;
/// Wait signal `dma_fault` for `physical_device` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_PHYSICAL_DEVICE_DMA_FAULT: u64 = 1_u64 << 1;
/// Wait signal `peer_closed` for `native_block` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_NATIVE_BLOCK_PEER_CLOSED: u64 = 1_u64 << 0;
/// Wait signal `readable` for `virtual_serial` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_READABLE: u64 = 1_u64 << 0;
/// Wait signal `writable` for `virtual_serial` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_WRITABLE: u64 = 1_u64 << 1;
/// Wait signal `peer_closed` for `virtual_serial` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_PEER_CLOSED: u64 = 1_u64 << 2;
/// Wait signal `readable` for `wait_set` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_WAIT_SET_READABLE: u64 = 1_u64 << 0;
/// Wait signal `signaled` for `event` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_EVENT_SIGNALED: u64 = 1_u64 << 0;
/// Wait signal `readable` for `byte_channel` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE: u64 = 1_u64 << 0;
/// Wait signal `writable` for `byte_channel` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE: u64 = 1_u64 << 1;
/// Wait signal `peer_closed` for `byte_channel` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED: u64 = 1_u64 << 2;
/// Wait signal `peer_receiving` for `capability_channel` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_RECEIVING: u64 = 1_u64 << 0;
/// Wait signal `peer_closed` for `capability_channel` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_CLOSED: u64 = 1_u64 << 1;
/// Wait signal `terminated` for `thread` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_THREAD_TERMINATED: u64 = 1_u64 << 0;
/// Wait signal `terminated` for `task_group` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_TASK_GROUP_TERMINATED: u64 = 1_u64 << 0;
/// Wait signal `terminated` for `process` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED: u64 = 1_u64 << 0;
/// Wait signal `readable` for `console` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_CONSOLE_READABLE: u64 = 1_u64 << 0;
/// Wait signal `writable` for `console` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE: u64 = 1_u64 << 1;
/// Wait signal `terminated` for `virtual_machine` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_TERMINATED: u64 = 1_u64 << 0;
/// Wait signal `power_request` for `virtual_machine` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_POWER_REQUEST: u64 = 1_u64 << 1;
/// Wait signal `vcpu_terminated` for `virtual_machine` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_VCPU_TERMINATED: u64 = 1_u64 << 2;
/// Wait signal `terminated` for `virtual_cpu` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_TERMINATED: u64 = 1_u64 << 0;
/// Wait signal `mmio_request` for `virtual_cpu` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_MMIO_REQUEST: u64 = 1_u64 << 1;
/// Wait signal `peer_closed` for `guest_notification` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_GUEST_NOTIFICATION_PEER_CLOSED: u64 = 1_u64 << 0;
/// Wait signal `readable` for `guest_mailbox` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_GUEST_MAILBOX_READABLE: u64 = 1_u64 << 0;
/// Wait signal `writable` for `guest_mailbox` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_GUEST_MAILBOX_WRITABLE: u64 = 1_u64 << 1;
/// Wait signal `peer_closed` for `guest_mailbox` objects; bit positions are object-local.
pub const HYPER_NATIVE_SIGNAL_GUEST_MAILBOX_PEER_CLOSED: u64 = 1_u64 << 2;

/// `handle_invalid` in the `handle_encoding` constants domain.
///
/// Handle value classes; the remaining bits are opaque process-local data.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// No handle is present. This value never identifies a live handle.
pub const HYPER_NATIVE_HANDLE_INVALID: u64 = 0;
/// `handle_prefix_mask` in the `handle_encoding` constants domain.
///
/// Handle value classes; the remaining bits are opaque process-local data.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Selects the most significant byte of a wire handle.
pub const HYPER_NATIVE_HANDLE_PREFIX_MASK: u64 = 18374686479671623680;
/// `handle_prefix` in the `handle_encoding` constants domain.
///
/// Handle value classes; the remaining bits are opaque process-local data.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Live handles have prefix 0x0F; all other prefixes are reserved.
pub const HYPER_NATIVE_HANDLE_PREFIX: u64 = 1080863910568919040;

/// `page_size` in the `core_limits` constants domain.
///
/// Page geometry, record bounds, identity encoding and wait limits.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Native memory-management granule in bytes.
pub const HYPER_NATIVE_PAGE_SIZE: u64 = 4096;
/// `extensible_record_max_bytes` in the `core_limits` constants domain.
///
/// Page geometry, record bounds, identity encoding and wait limits.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Upper bound on a caller-supplied extensible record, including its tail.
pub const HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES: u64 = 4096;
/// `koid_slot_mask` in the `core_limits` constants domain.
///
/// Page geometry, record bounds, identity encoding and wait limits.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Low slot bits of a KOID; the generation is required for identity.
pub const HYPER_NATIVE_KOID_SLOT_MASK: u64 = 4294967295;
/// `deadline_infinite` in the `core_limits` constants domain.
///
/// Page geometry, record bounds, identity encoding and wait limits.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Wait without a deadline; finite deadlines use absolute monotonic nanoseconds.
pub const HYPER_NATIVE_DEADLINE_INFINITE: u64 = 18446744073709551615;
/// `object_wait_many_max_items` in the `core_limits` constants domain.
///
/// Page geometry, record bounds, identity encoding and wait limits.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS: u64 = 64;

/// `system_config_page_size` in the `system_config` enum domain.
///
/// Selectors for system configuration queries.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SYSTEM_CONFIG_PAGE_SIZE: u64 = 1;
/// `system_config_application_address_limit` in the `system_config` enum domain.
///
/// Selectors for system configuration queries.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SYSTEM_CONFIG_APPLICATION_ADDRESS_LIMIT: u64 = 2;

/// `elf_osabi` in the `startup_limits` constants domain.
///
/// Executable identifiers and kernel-to-loader transport bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// `HypeR`'s ELF OSABI byte; this is not a Linux or generic System V image.
pub const HYPER_NATIVE_ELF_OSABI: u64 = 63;
/// `elf_abi_version` in the `startup_limits` constants domain.
///
/// Executable identifiers and kernel-to-loader transport bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_ELF_ABI_VERSION: u64 = 0;
/// `process_startup_data_max_bytes` in the `startup_limits` constants domain.
///
/// Executable identifiers and kernel-to-loader transport bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES: u64 = 16384;
/// `startup_max_handles` in the `startup_limits` constants domain.
///
/// Executable identifiers and kernel-to-loader transport bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_STARTUP_MAX_HANDLES: u64 = 256;
/// `boot_command_line_max_bytes` in the `startup_limits` constants domain.
///
/// Executable identifiers and kernel-to-loader transport bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Maximum UTF-8 boot-argument payload, excluding its length prefix.
pub const HYPER_NATIVE_BOOT_COMMAND_LINE_MAX_BYTES: u64 = 512;

/// `loader_startup_reply` in the `loader_startup` enum domain.
///
/// Kernel-to-loader startup message tags; payload semantics belong to userspace.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_LOADER_STARTUP_REPLY: u64 = 1;

/// `startup_handle_purpose_resource_domain` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN: u64 = 1;
/// `startup_handle_purpose_task_group` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP: u64 = 2;
/// `startup_handle_purpose_task_factory` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY: u64 = 3;
/// `startup_handle_purpose_executable_authority` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY: u64 = 4;
/// `startup_handle_purpose_root_vmar` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR: u64 = 5;
/// `startup_handle_purpose_console` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE: u64 = 6;
/// `startup_handle_purpose_root_directory` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY: u64 = 7;
/// `startup_handle_purpose_task_inspector` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_INSPECTOR: u64 = 8;
/// `startup_handle_purpose_object_inspector` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_INSPECTOR: u64 = 9;
/// `startup_handle_purpose_dynamic_library_directory` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DYNAMIC_LIBRARY_DIRECTORY: u64 = 10;
/// `startup_handle_purpose_memory_inspector` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_MEMORY_INSPECTOR: u64 = 11;
/// `startup_handle_purpose_cpu_inspector` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CPU_INSPECTOR: u64 = 12;
/// `startup_handle_purpose_virtual_machine_creation_authority` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_VIRTUAL_MACHINE_CREATION_AUTHORITY: u64 = 13;
/// `startup_handle_purpose_device_assignment_authority` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DEVICE_ASSIGNMENT_AUTHORITY: u64 = 14;
/// `startup_handle_purpose_initial_stack_vmar` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR: u64 = 15;
/// `startup_handle_purpose_execution_context` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTION_CONTEXT: u64 = 16;
/// `startup_handle_purpose_object_factory` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_FACTORY: u64 = 17;
/// `startup_handle_purpose_system_info` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_SYSTEM_INFO: u64 = 18;
/// `startup_handle_purpose_boot_command_line` in the `startup_handle_purpose` enum domain.
///
/// Purposes assigned to capabilities in the process startup table.
/// Select one value; this is not a bit mask.
///
/// Unconsumed boot arguments: immutable VMO with READ | INSPECT rights.
///  Contains a little-endian u64 byte length followed by that many UTF-8
///  bytes (no NUL), bounded by `BOOT_COMMAND_LINE_MAX_BYTES`. The kernel
///  passes unrecognized arguments to its initial process. Each userspace
///  boot stage may pass its own remainder to the successor under this purpose.
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_BOOT_COMMAND_LINE: u64 = 19;

/// `process_phase_prepared` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_PREPARED: u64 = 0;
/// `process_phase_created` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_CREATED: u64 = 1;
/// `process_phase_running` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_RUNNING: u64 = 2;
/// `process_phase_stopping` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_STOPPING: u64 = 3;
/// `process_phase_stopped` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_STOPPED: u64 = 4;
/// `process_phase_retiring` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_RETIRING: u64 = 5;
/// `process_phase_retired` in the `process_phase` enum domain.
///
/// Process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_PHASE_RETIRED: u64 = 6;

/// `process_terminal_none` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_NONE: u64 = 0;
/// `process_terminal_requested` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_REQUESTED: u64 = 1;
/// `process_terminal_thread_exited` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_THREAD_EXITED: u64 = 2;
/// `process_terminal_process_exited` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_PROCESS_EXITED: u64 = 3;
/// `process_terminal_last_thread_exited` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_LAST_THREAD_EXITED: u64 = 4;
/// `process_terminal_fault` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_FAULT: u64 = 5;
/// `process_terminal_task_group_stop` in the `process_terminal` enum domain.
///
/// Reasons a process stopped.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_TERMINAL_TASK_GROUP_STOP: u64 = 6;

/// `thread_role_bootstrap` in the `thread_role` enum domain.
///
/// Scheduler thread roles reported by inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_ROLE_BOOTSTRAP: u64 = 1;
/// `thread_role_idle` in the `thread_role` enum domain.
///
/// Scheduler thread roles reported by inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_ROLE_IDLE: u64 = 2;
/// `thread_role_kernel` in the `thread_role` enum domain.
///
/// Scheduler thread roles reported by inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_ROLE_KERNEL: u64 = 3;
/// `thread_role_user` in the `thread_role` enum domain.
///
/// Scheduler thread roles reported by inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_ROLE_USER: u64 = 4;
/// `thread_role_vcpu` in the `thread_role` enum domain.
///
/// Scheduler thread roles reported by inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_ROLE_VCPU: u64 = 5;

/// `scheduling_policy_fair` in the `scheduling_policy` enum domain.
///
/// Scheduling algorithms; these do not identify the thread execution payload.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_POLICY_FAIR: u64 = 1;
/// `scheduling_policy_budget` in the `scheduling_policy` enum domain.
///
/// Scheduling algorithms; these do not identify the thread execution payload.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_POLICY_BUDGET: u64 = 2;
/// `scheduling_policy_fifo` in the `scheduling_policy` enum domain.
///
/// Scheduling algorithms; these do not identify the thread execution payload.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_POLICY_FIFO: u64 = 3;

/// `scheduling_role_ordinary` in the `scheduling_role` enum domain.
///
/// Eligibility for CPU-local priority and minimum-progress allowances.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_ROLE_ORDINARY: u64 = 0;
/// `scheduling_role_priority` in the `scheduling_role` enum domain.
///
/// Eligibility for CPU-local priority and minimum-progress allowances.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_ROLE_PRIORITY: u64 = 1;
/// `scheduling_role_protected` in the `scheduling_role` enum domain.
///
/// Eligibility for CPU-local priority and minimum-progress allowances.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_ROLE_PROTECTED: u64 = 2;

/// `scheduling_fair_backend_stride` in the `scheduling_fair_backend` enum domain.
///
/// Fair queue implementations supported by the selected kernel image.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_FAIR_BACKEND_STRIDE: u64 = 0;
/// `scheduling_fair_backend_round_robin` in the `scheduling_fair_backend` enum domain.
///
/// Fair queue implementations supported by the selected kernel image.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_FAIR_BACKEND_ROUND_ROBIN: u64 = 1;
/// `scheduling_fair_backend_default` in the `scheduling_fair_backend` enum domain.
///
/// Fair queue implementations supported by the selected kernel image.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_SCHEDULING_FAIR_BACKEND_DEFAULT: u64 = 2;

/// `thread_registry_resident` in the `thread_registry` enum domain.
///
/// Thread registry membership states.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_REGISTRY_RESIDENT: u64 = 1;
/// `thread_registry_retiring` in the `thread_registry` enum domain.
///
/// Thread registry membership states.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_THREAD_REGISTRY_RETIRING: u64 = 2;

/// `thread_name_max_bytes` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// UTF-8 bytes excluding any NUL terminator; control characters are rejected.
pub const HYPER_NATIVE_THREAD_NAME_MAX_BYTES: u64 = 64;
/// `process_name_max_bytes` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_PROCESS_NAME_MAX_BYTES: u64 = 64;
/// `process_affinity_max_words` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS: u64 = 4;
/// `process_affinity_max_cpus` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_PROCESS_AFFINITY_MAX_CPUS: u64 = 256;
/// `task_inspector_process_page_capacity` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_TASK_INSPECTOR_PROCESS_PAGE_CAPACITY: u64 = 8;
/// `task_inspector_thread_page_capacity` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_TASK_INSPECTOR_THREAD_PAGE_CAPACITY: u64 = 8;
/// `object_inspector_object_page_capacity` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_OBJECT_INSPECTOR_OBJECT_PAGE_CAPACITY: u64 = 8;
/// `object_inspector_handle_page_capacity` in the `task_limits` constants domain.
///
/// Task names, affinity bounds and inspection page sizes.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_OBJECT_INSPECTOR_HANDLE_PAGE_CAPACITY: u64 = 8;

/// `object_detail_empty` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_EMPTY: u64 = 0;
/// `object_detail_thread` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_THREAD: u64 = 1;
/// `object_detail_vmar` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_VMAR: u64 = 2;
/// `object_detail_mapping` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_MAPPING: u64 = 3;
/// `object_detail_channel` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_CHANNEL: u64 = 4;
/// `object_detail_device` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_DEVICE: u64 = 5;
/// `object_detail_device_resource` in the `object_detail` enum domain.
///
/// Payload tags for detailed object inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_DETAIL_DEVICE_RESOURCE: u64 = 6;

/// `object_handle_state_unpublished` in the `object_handle_state` enum domain.
///
/// Userspace handle publication states.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_HANDLE_STATE_UNPUBLISHED: u64 = 1;
/// `object_handle_state_active` in the `object_handle_state` enum domain.
///
/// Userspace handle publication states.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_HANDLE_STATE_ACTIVE: u64 = 2;
/// `object_handle_state_retired` in the `object_handle_state` enum domain.
///
/// Userspace handle publication states.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_OBJECT_HANDLE_STATE_RETIRED: u64 = 3;

/// `process_retirement_prepared` in the `process_retirement` enum domain.
///
/// Retained process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_PREPARED: u64 = 0;
/// `process_retirement_running` in the `process_retirement` enum domain.
///
/// Retained process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RUNNING: u64 = 1;
/// `process_retirement_stopping` in the `process_retirement` enum domain.
///
/// Retained process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_STOPPING: u64 = 2;
/// `process_retirement_ready` in the `process_retirement` enum domain.
///
/// Retained process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_READY: u64 = 3;
/// `process_retirement_released` in the `process_retirement` enum domain.
///
/// Retained process lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RELEASED: u64 = 4;

/// `process_retirement_release_none` in the `process_retirement_release` enum domain.
///
/// Reasons retained process state was released.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_NONE: u64 = 0;
/// `process_retirement_release_explicit` in the `process_retirement_release` enum domain.
///
/// Reasons retained process state was released.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_EXPLICIT: u64 = 1;
/// `process_retirement_release_owner_lost` in the `process_retirement_release` enum domain.
///
/// Reasons retained process state was released.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_OWNER_LOST: u64 = 2;
/// `process_retirement_release_deadline` in the `process_retirement_release` enum domain.
///
/// Reasons retained process state was released.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_DEADLINE: u64 = 3;
/// `process_retirement_release_aborted` in the `process_retirement_release` enum domain.
///
/// Reasons retained process state was released.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_ABORTED: u64 = 4;

/// `crash_partial_memory` in the `crash_partial` flags domain.
///
/// Incomplete sections of retained crash state.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_PARTIAL_MEMORY: u64 = 1;
/// `crash_partial_threads` in the `crash_partial` flags domain.
///
/// Incomplete sections of retained crash state.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_PARTIAL_THREADS: u64 = 2;
/// `crash_partial_handles` in the `crash_partial` flags domain.
///
/// Incomplete sections of retained crash state.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_PARTIAL_HANDLES: u64 = 4;
/// `crash_partial_mappings` in the `crash_partial` flags domain.
///
/// Incomplete sections of retained crash state.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_PARTIAL_MAPPINGS: u64 = 8;

/// `crash_mapping_retained` in the `crash_mapping_status` flags domain.
///
/// Retention and exclusion flags for a crash mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_MAPPING_RETAINED: u64 = 1;
/// `crash_mapping_partial` in the `crash_mapping_status` flags domain.
///
/// Retention and exclusion flags for a crash mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_MAPPING_PARTIAL: u64 = 2;
/// `crash_mapping_excluded_shared` in the `crash_mapping_status` flags domain.
///
/// Retention and exclusion flags for a crash mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_MAPPING_EXCLUDED_SHARED: u64 = 4;
/// `crash_mapping_excluded_file` in the `crash_mapping_status` flags domain.
///
/// Retention and exclusion flags for a crash mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_CRASH_MAPPING_EXCLUDED_FILE: u64 = 8;

/// `crash_mapping_anonymous` in the `crash_mapping_kind` enum domain.
///
/// Backing kinds reported for a crash mapping.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_CRASH_MAPPING_ANONYMOUS: u64 = 1;
/// `crash_mapping_private_file` in the `crash_mapping_kind` enum domain.
///
/// Backing kinds reported for a crash mapping.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_CRASH_MAPPING_PRIVATE_FILE: u64 = 2;
/// `crash_mapping_shared` in the `crash_mapping_kind` enum domain.
///
/// Backing kinds reported for a crash mapping.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_CRASH_MAPPING_SHARED: u64 = 3;

/// `crash_read_max_bytes` in the `crash_limits` constants domain.
///
/// Maximum bytes returned by one crash-memory read.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_CRASH_READ_MAX_BYTES: u64 = 65536;

/// `memory_statistics_heap` in the `memory_statistics` enum domain.
///
/// Selectors for detailed memory statistics.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_MEMORY_STATISTICS_HEAP: u64 = 1;
/// `memory_statistics_buddy` in the `memory_statistics` enum domain.
///
/// Selectors for detailed memory statistics.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_MEMORY_STATISTICS_BUDDY: u64 = 2;
/// `memory_statistics_owner` in the `memory_statistics` enum domain.
///
/// Selectors for detailed memory statistics.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_MEMORY_STATISTICS_OWNER: u64 = 3;
/// `memory_statistics_file_cache` in the `memory_statistics` enum domain.
///
/// Selectors for detailed memory statistics.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_MEMORY_STATISTICS_FILE_CACHE: u64 = 4;
/// `memory_statistics_heap_class` in the `memory_statistics` enum domain.
///
/// Selectors for detailed memory statistics.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_MEMORY_STATISTICS_HEAP_CLASS: u64 = 5;

/// `memory_statistics_buddy_order_count` in the `memory_statistics_dimensions` constants domain.
///
/// Dimensions of memory statistics records.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_MEMORY_STATISTICS_BUDDY_ORDER_COUNT: u64 = 19;
/// `memory_statistics_owner_count` in the `memory_statistics_dimensions` constants domain.
///
/// Dimensions of memory statistics records.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_MEMORY_STATISTICS_OWNER_COUNT: u64 = 5;
/// `memory_statistics_heap_class_count` in the `memory_statistics_dimensions` constants domain.
///
/// Dimensions of memory statistics records.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_MEMORY_STATISTICS_HEAP_CLASS_COUNT: u64 = 18;

/// `private_mapping_copy_on_write` in the `private_mapping` enum domain.
///
/// Private mapping population policies.
/// Select one value; this is not a bit mask.
///
/// Share source pages until the mapping is written; modifications remain private.
pub const HYPER_NATIVE_PRIVATE_MAPPING_COPY_ON_WRITE: u64 = 0;
/// `private_mapping_eager` in the `private_mapping` enum domain.
///
/// Private mapping population policies.
/// Select one value; this is not a bit mask.
///
/// Populate private pages immediately instead of deferring copying to write faults.
pub const HYPER_NATIVE_PRIVATE_MAPPING_EAGER: u64 = 1;

/// `vmar_permission_read` in the `vmar_permission` flags domain.
///
/// Permissions which may be combined for a mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_VMAR_PERMISSION_READ: u64 = 1;
/// `vmar_permission_write` in the `vmar_permission` flags domain.
///
/// Permissions which may be combined for a mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_VMAR_PERMISSION_WRITE: u64 = 2;
/// `vmar_permission_execute` in the `vmar_permission` flags domain.
///
/// Permissions which may be combined for a mapping.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_VMAR_PERMISSION_EXECUTE: u64 = 4;

/// `vmar_allocate_exact` in the `vmar_allocate` flags domain.
///
/// Options for address-space allocation.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_VMAR_ALLOCATE_EXACT: u64 = 1;

/// `vmar_memory_current` in the `vmar_memory` flags domain.
///
/// Options for locking currently mapped memory.
/// Independent flag bit; combine permitted members with bitwise OR.
///
/// Lock mappings that exist now; future mappings do not inherit the lock.
///  Unmapping and process teardown still release the corresponding residency.
pub const HYPER_NATIVE_VMAR_MEMORY_CURRENT: u64 = 1;

/// `vmo_max_size_bytes` in the `vmo_limits` constants domain.
///
/// VMO size and transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VMO_MAX_SIZE_BYTES: u64 = 4294967296;
/// `vmo_max_transfer_bytes` in the `vmo_limits` constants domain.
///
/// VMO size and transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES: u64 = 65536;
/// `vmo_max_contiguous_size_bytes` in the `vmo_limits` constants domain.
///
/// VMO size and transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VMO_MAX_CONTIGUOUS_SIZE_BYTES: u64 = 134217728;

/// `capability_disposition_move` in the `capability_disposition` enum domain.
///
/// Ownership operations for a transferred capability.
/// Select one value; this is not a bit mask.
///
/// Transfer ownership only when the operation commits; failures preserve the source.
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE: u64 = 0;
/// `capability_disposition_duplicate` in the `capability_disposition` enum domain.
///
/// Ownership operations for a transferred capability.
/// Select one value; this is not a bit mask.
///
/// Keep the source handle; requires DUPLICATE in addition to transfer authority.
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE: u64 = 1;

/// `byte_channel_max_message_bytes` in the `channel_limits` constants domain.
///
/// Channel message, queue and capability-transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES: u64 = 65536;
/// `byte_channel_max_queued_messages` in the `channel_limits` constants domain.
///
/// Channel message, queue and capability-transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_MESSAGES: u64 = 16;
/// `byte_channel_max_queued_bytes` in the `channel_limits` constants domain.
///
/// Channel message, queue and capability-transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_BYTES: u64 = 1048576;
/// `capability_channel_max_message_bytes` in the `channel_limits` constants domain.
///
/// Channel message, queue and capability-transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES: u64 = 4096;
/// `capability_channel_max_handles` in the `channel_limits` constants domain.
///
/// Channel message, queue and capability-transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES: u64 = 64;
/// `capability_disposition_same_rights` in the `channel_limits` constants domain.
///
/// Channel message, queue and capability-transfer bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Request the source's current rights, not every right defined by the ABI.
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS: u64 = 18446744073709551615;

/// `console_max_transfer_bytes` in the `console_limits` constants domain.
///
/// Console transfer bounds and virtual serial shared-memory layout.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES: u64 = 4096;
/// `virtual_serial_max_transfer_bytes` in the `console_limits` constants domain.
///
/// Console transfer bounds and virtual serial shared-memory layout.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_SERIAL_MAX_TRANSFER_BYTES: u64 = 4096;
/// `virtual_serial_output_capacity` in the `console_limits` constants domain.
///
/// Console transfer bounds and virtual serial shared-memory layout.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Byte slots in the producer ring; unread bytes are never overwritten.
pub const HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_CAPACITY: u64 = 65536;
/// `virtual_serial_output_header_bytes` in the `console_limits` constants domain.
///
/// Console transfer bounds and virtual serial shared-memory layout.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Byte offset of ring data after its shared atomic counter header.
pub const HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_HEADER_BYTES: u64 = 4096;
/// `virtual_serial_output_bytes` in the `console_limits` constants domain.
///
/// Console transfer bounds and virtual serial shared-memory layout.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Exact VMO size required by `virtual_serial_register_output`.
pub const HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_BYTES: u64 = 69632;

/// `directory_entry_kind_file` in the `directory_entry_kind` enum domain.
///
/// Kinds reported by directory enumeration.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_KIND_FILE: u64 = 1;
/// `directory_entry_kind_directory` in the `directory_entry_kind` enum domain.
///
/// Kinds reported by directory enumeration.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_KIND_DIRECTORY: u64 = 2;
/// `directory_entry_kind_symlink` in the `directory_entry_kind` enum domain.
///
/// Kinds reported by directory enumeration.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_KIND_SYMLINK: u64 = 3;
/// `directory_entry_kind_other` in the `directory_entry_kind` enum domain.
///
/// Kinds reported by directory enumeration.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_KIND_OTHER: u64 = 4;

/// `directory_entry_page_capacity` in the `filesystem_limits` constants domain.
///
/// Path, directory-page, transfer and root-switch bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY: u64 = 4;
/// `directory_entry_name_max_bytes` in the `filesystem_limits` constants domain.
///
/// Path, directory-page, transfer and root-switch bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_NAME_MAX_BYTES: u64 = 255;
/// `directory_max_path_bytes` in the `filesystem_limits` constants domain.
///
/// Path, directory-page, transfer and root-switch bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES: u64 = 4096;
/// `file_max_read_bytes` in the `filesystem_limits` constants domain.
///
/// Path, directory-page, transfer and root-switch bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_FILE_MAX_READ_BYTES: u64 = 2097152;
/// `filesystem_switch_root_max_moves` in the `filesystem_limits` constants domain.
///
/// Path, directory-page, transfer and root-switch bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_FILESYSTEM_SWITCH_ROOT_MAX_MOVES: u64 = 16;

/// `device_profile_virtio_mmio_scsi` in the `device_profile` enum domain.
///
/// Physical-device assignment profiles.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_PROFILE_VIRTIO_MMIO_SCSI: u64 = 1;
/// `device_profile_userspace` in the `device_profile` enum domain.
///
/// Physical-device assignment profiles.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_PROFILE_USERSPACE: u64 = 2;
/// `device_profile_virtio_mmio_net` in the `device_profile` enum domain.
///
/// Physical-device assignment profiles.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_PROFILE_VIRTIO_MMIO_NET: u64 = 3;
/// `device_profile_pci_function` in the `device_profile` enum domain.
///
/// Physical-device assignment profiles.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_PROFILE_PCI_FUNCTION: u64 = 4;

/// `device_identity_compatible` in the `device_identity` enum domain.
///
/// Selectors for device identity matching.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_IDENTITY_COMPATIBLE: u64 = 1;
/// `device_identity_fdt_path` in the `device_identity` enum domain.
///
/// Selectors for device identity matching.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_IDENTITY_FDT_PATH: u64 = 2;
/// `device_identity_pci_id` in the `device_identity` enum domain.
///
/// Selectors for device identity matching.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_IDENTITY_PCI_ID: u64 = 3;

/// `device_firmware_field_info` in the `device_firmware_field` enum domain.
///
/// Fields exposed by firmware inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_INFO: u64 = 0;
/// `device_firmware_field_path` in the `device_firmware_field` enum domain.
///
/// Fields exposed by firmware inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_PATH: u64 = 1;
/// `device_firmware_field_compatible` in the `device_firmware_field` enum domain.
///
/// Fields exposed by firmware inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_COMPATIBLE: u64 = 2;
/// `device_firmware_field_registers` in the `device_firmware_field` enum domain.
///
/// Fields exposed by firmware inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_REGISTERS: u64 = 3;
/// `device_firmware_field_property` in the `device_firmware_field` enum domain.
///
/// Fields exposed by firmware inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_PROPERTY: u64 = 4;
/// `device_firmware_field_property_names` in the `device_firmware_field` enum domain.
///
/// Fields exposed by firmware inspection.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_PROPERTY_NAMES: u64 = 5;

/// `device_resource_pci_ecam` in the `device_resource_ids` constants domain.
///
/// PCI resource identifiers; BAR0 is the base of the BAR index range.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_RESOURCE_PCI_ECAM: u64 = 256;
/// `device_resource_pci_msi` in the `device_resource_ids` constants domain.
///
/// PCI resource identifiers; BAR0 is the base of the BAR index range.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_RESOURCE_PCI_MSI: u64 = 257;
/// `device_resource_pci_bar0` in the `device_resource_ids` constants domain.
///
/// PCI resource identifiers; BAR0 is the base of the BAR index range.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// Add the zero-based BAR index to identify a PCI BAR resource.
pub const HYPER_NATIVE_DEVICE_RESOURCE_PCI_BAR0: u64 = 512;
/// `device_resource_pci_intx` in the `device_resource_ids` constants domain.
///
/// PCI resource identifiers; BAR0 is the base of the BAR index range.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_RESOURCE_PCI_INTX: u64 = 258;

/// `device_resource_memory_64` in the `device_resource_flags` flags domain.
///
/// PCI resource attributes.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_DEVICE_RESOURCE_MEMORY_64: u64 = 1;
/// `device_resource_prefetchable` in the `device_resource_flags` flags domain.
///
/// PCI resource attributes.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_DEVICE_RESOURCE_PREFETCHABLE: u64 = 2;

/// `device_firmware_max_bytes` in the `device_limits` constants domain.
///
/// Device discovery, assignment and PCI aperture bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_MAX_BYTES: u64 = 65536;
/// `device_firmware_name_max_bytes` in the `device_limits` constants domain.
///
/// Device discovery, assignment and PCI aperture bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_NAME_MAX_BYTES: u64 = 128;
/// `device_bundle_max_entries` in the `device_limits` constants domain.
///
/// Device discovery, assignment and PCI aperture bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_BUNDLE_MAX_ENTRIES: u64 = 8;
/// `device_pci_aperture_size` in the `device_limits` constants domain.
///
/// Device discovery, assignment and PCI aperture bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_PCI_APERTURE_SIZE: u64 = 8388608;
/// `device_assignment_max_devices` in the `device_limits` constants domain.
///
/// Device discovery, assignment and PCI aperture bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_DEVICE_ASSIGNMENT_MAX_DEVICES: u64 = 4;

/// `dma_fault_source_smmuv3` in the `dma_fault_source` enum domain.
///
/// DMA isolation mechanisms reporting a fault.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DMA_FAULT_SOURCE_SMMUV3: u64 = 1;

/// `dma_fault_scope_stream` in the `dma_fault_scope` enum domain.
///
/// Scope of a DMA fault report.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DMA_FAULT_SCOPE_STREAM: u64 = 1;
/// `dma_fault_scope_controller` in the `dma_fault_scope` enum domain.
///
/// Scope of a DMA fault report.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_DMA_FAULT_SCOPE_CONTROLLER: u64 = 2;

/// `dma_fault_address_valid` in the `dma_fault_flags` flags domain.
///
/// Valid-address and access-direction flags in DMA fault reports.
/// Independent flag bit; combine permitted members with bitwise OR.
///
/// The reported address is meaningful; a controller-wide report may omit it.
pub const HYPER_NATIVE_DMA_FAULT_ADDRESS_VALID: u64 = 1;
/// `dma_fault_read` in the `dma_fault_flags` flags domain.
///
/// Valid-address and access-direction flags in DMA fault reports.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_DMA_FAULT_READ: u64 = 2;
/// `dma_fault_write` in the `dma_fault_flags` flags domain.
///
/// Valid-address and access-direction flags in DMA fault reports.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_DMA_FAULT_WRITE: u64 = 4;

/// `native_block_transfer_bytes` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_TRANSFER_BYTES: u64 = 524288;
/// `native_block_batch_max` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_BATCH_MAX: u64 = 4;
/// `native_block_batch_record_bytes` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_BATCH_RECORD_BYTES: u64 = 16;
/// `native_block_transfer_frame_bytes` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_TRANSFER_FRAME_BYTES: u64 = 524352;
/// `native_block_memory_bytes` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_MEMORY_BYTES: u64 = 1048576;
/// `native_block_queue_count` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_QUEUE_COUNT: u64 = 6;
/// `native_block_queue_size` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_QUEUE_SIZE: u64 = 8;
/// `native_block_queue_stride` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_QUEUE_STRIDE: u64 = 4096;
/// `native_block_available_offset` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_AVAILABLE_OFFSET: u64 = 256;
/// `native_block_used_offset` in the `native_block_layout` constants domain.
///
/// Shared-memory geometry and transfer bounds for the Native block initiator.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_NATIVE_BLOCK_USED_OFFSET: u64 = 512;

/// `guest_notification_disable` in the `guest_notification` enum domain.
///
/// Guest notification control operations.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_GUEST_NOTIFICATION_DISABLE: u64 = 0;
/// `guest_notification_enable` in the `guest_notification` enum domain.
///
/// Guest notification control operations.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_GUEST_NOTIFICATION_ENABLE: u64 = 1;
/// `guest_notification_raise_config` in the `guest_notification` enum domain.
///
/// Guest notification control operations.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_GUEST_NOTIFICATION_RAISE_CONFIG: u64 = 2;
/// `guest_notification_disconnect` in the `guest_notification` enum domain.
///
/// Guest notification control operations.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_GUEST_NOTIFICATION_DISCONNECT: u64 = 3;

/// `io_max_clients` in the `io_transport_limits` constants domain.
///
/// I/O client, mailbox and guest alias-window bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_IO_MAX_CLIENTS: u64 = 9;
/// `guest_mailbox_max_message_bytes` in the `io_transport_limits` constants domain.
///
/// I/O client, mailbox and guest alias-window bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_GUEST_MAILBOX_MAX_MESSAGE_BYTES: u64 = 256;
/// `guest_dynamic_alias_offset` in the `io_transport_limits` constants domain.
///
/// I/O client, mailbox and guest alias-window bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_GUEST_DYNAMIC_ALIAS_OFFSET: u64 = 68719476736;
/// `guest_dynamic_physical_limit` in the `io_transport_limits` constants domain.
///
/// I/O client, mailbox and guest alias-window bounds.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_GUEST_DYNAMIC_PHYSICAL_LIMIT: u64 = 274877906944;

/// `virtual_machine_architecture_aarch64` in the `virtual_machine_architecture` enum domain.
///
/// Architecture identifiers for guest construction.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_AARCH64: u64 = 1;
/// `virtual_machine_architecture_riscv64` in the `virtual_machine_architecture` enum domain.
///
/// Architecture identifiers for guest construction.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_RISCV64: u64 = 2;
/// `virtual_machine_architecture_x86_64` in the `virtual_machine_architecture` enum domain.
///
/// Architecture identifiers for guest construction.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_X86_64: u64 = 3;

/// `virtual_machine_power_cpu_on` in the `virtual_machine_power` enum domain.
///
/// Guest power-management requests.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_POWER_CPU_ON: u64 = 1;
/// `virtual_machine_power_cpu_off` in the `virtual_machine_power` enum domain.
///
/// Guest power-management requests.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_POWER_CPU_OFF: u64 = 2;
/// `virtual_machine_power_system_off` in the `virtual_machine_power` enum domain.
///
/// Guest power-management requests.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_POWER_SYSTEM_OFF: u64 = 3;
/// `virtual_machine_power_system_reset` in the `virtual_machine_power` enum domain.
///
/// Guest power-management requests.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_POWER_SYSTEM_RESET: u64 = 4;

/// `virtual_machine_phase_installed` in the `virtual_machine_phase` enum domain.
///
/// Installed guest lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_INSTALLED: u64 = 1;
/// `virtual_machine_phase_running` in the `virtual_machine_phase` enum domain.
///
/// Installed guest lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_RUNNING: u64 = 2;
/// `virtual_machine_phase_stopping` in the `virtual_machine_phase` enum domain.
///
/// Installed guest lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPING: u64 = 3;
/// `virtual_machine_phase_stopped` in the `virtual_machine_phase` enum domain.
///
/// Installed guest lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPED: u64 = 4;

/// `virtual_cpu_phase_dormant` in the `virtual_cpu_phase` enum domain.
///
/// Virtual CPU lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_PHASE_DORMANT: u64 = 1;
/// `virtual_cpu_phase_started` in the `virtual_cpu_phase` enum domain.
///
/// Virtual CPU lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_PHASE_STARTED: u64 = 2;
/// `virtual_cpu_phase_stopped` in the `virtual_cpu_phase` enum domain.
///
/// Virtual CPU lifecycle phases.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_PHASE_STOPPED: u64 = 3;

/// `virtual_cpu_terminal_none` in the `virtual_cpu_terminal` enum domain.
///
/// Virtual CPU termination reasons.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_NONE: u64 = 0;
/// `virtual_cpu_terminal_memory_fault` in the `virtual_cpu_terminal` enum domain.
///
/// Virtual CPU termination reasons.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MEMORY_FAULT: u64 = 1;
/// `virtual_cpu_terminal_mmio` in the `virtual_cpu_terminal` enum domain.
///
/// Virtual CPU termination reasons.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MMIO: u64 = 2;
/// `virtual_cpu_terminal_synchronous` in the `virtual_cpu_terminal` enum domain.
///
/// Virtual CPU termination reasons.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_SYNCHRONOUS: u64 = 3;
/// `virtual_cpu_terminal_administrative` in the `virtual_cpu_terminal` enum domain.
///
/// Virtual CPU termination reasons.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_ADMINISTRATIVE: u64 = 4;

/// `vcpu_host_cpu_unknown` in the `virtual_cpu_sentinels` constants domain.
///
/// Sentinel for an unknown host CPU assignment.
/// Numeric parameter or sentinel, not an enum discriminant.
///
/// No host CPU assignment is available; do not treat this as a CPU index.
pub const HYPER_NATIVE_VCPU_HOST_CPU_UNKNOWN: u64 = 4294967295;

/// `virtual_platform_aarch64_reference` in the `virtual_platform` enum domain.
///
/// Reference platform identifiers.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE: u64 = 1;
/// `virtual_platform_riscv64_reference` in the `virtual_platform` enum domain.
///
/// Reference platform identifiers.
/// Select one value; this is not a bit mask.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE: u64 = 2;

/// `virtual_platform_aarch64_reference_max_vcpus` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_MAX_VCPUS: u64 = 8;
/// `virtual_platform_aarch64_reference_guest_ram_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GUEST_RAM_BASE: u64 = 1073741824;
/// `virtual_platform_aarch64_reference_dtb_offset` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_DTB_OFFSET: u64 = 65536;
/// `virtual_platform_aarch64_reference_gic_distributor_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_BASE: u64 = 134217728;
/// `virtual_platform_aarch64_reference_gic_distributor_size` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_SIZE: u64 = 65536;
/// `virtual_platform_aarch64_reference_gic_redistributor_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_BASE: u64 = 134873088;
/// `virtual_platform_aarch64_reference_gic_redistributor_size` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_SIZE: u64 = 131072;
/// `virtual_platform_aarch64_reference_gicv2_distributor_size` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GICV2_DISTRIBUTOR_SIZE: u64 = 4096;
/// `virtual_platform_aarch64_reference_gicv2_cpu_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GICV2_CPU_BASE: u64 = 134283264;
/// `virtual_platform_aarch64_reference_gicv2_cpu_size` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GICV2_CPU_SIZE: u64 = 8192;
/// `virtual_platform_aarch64_reference_uart_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_BASE: u64 = 150994944;
/// `virtual_platform_aarch64_reference_user_mmio_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_USER_MMIO_BASE: u64 = 167772160;
/// `virtual_platform_aarch64_reference_user_mmio_size` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_USER_MMIO_SIZE: u64 = 16777216;
/// `virtual_platform_aarch64_reference_uart_size` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_SIZE: u64 = 4096;
/// `virtual_platform_aarch64_reference_uart_baud` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_BAUD: u64 = 115200;
/// `virtual_platform_aarch64_reference_uart_interrupt` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_INTERRUPT: u64 = 33;
/// `virtual_platform_aarch64_reference_timer_interrupt` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_TIMER_INTERRUPT: u64 = 27;
/// `virtual_platform_aarch64_reference_interrupt_count` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_INTERRUPT_COUNT: u64 = 256;
/// `virtual_platform_aarch64_reference_device_interrupt_base` in the `virtual_platform_aarch64_layout` constants domain.
///
/// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_DEVICE_INTERRUPT_BASE: u64 = 40;

/// `virtual_platform_riscv64_reference_guest_ram_base` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_GUEST_RAM_BASE: u64 = 2147483648;
/// `virtual_platform_riscv64_reference_dtb_offset` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_DTB_OFFSET: u64 = 65536;
/// `virtual_platform_riscv64_reference_uart_base` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_BASE: u64 = 268435456;
/// `virtual_platform_riscv64_reference_uart_size` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_SIZE: u64 = 4096;
/// `virtual_platform_riscv64_reference_uart_interrupt` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_INTERRUPT: u64 = 10;
/// `virtual_platform_riscv64_reference_uart_clock_frequency` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_CLOCK_FREQUENCY: u64 = 3686400;
/// `virtual_platform_riscv64_reference_plic_base` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_BASE: u64 = 201326592;
/// `virtual_platform_riscv64_reference_plic_size` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_SIZE: u64 = 4194304;
/// `virtual_platform_riscv64_reference_plic_num_sources` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_NUM_SOURCES: u64 = 31;
/// `virtual_platform_riscv64_reference_plic_supervisor_context` in the `virtual_platform_riscv64_layout` constants domain.
///
/// Immutable RISC-V reference platform geometry shared by kernel and userspace.
/// Numeric parameter or sentinel, not an enum discriminant.
pub const HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_SUPERVISOR_CONTEXT: u64 = 1;

/// `riscv_isa_i` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_I: u64 = 1;
/// `riscv_isa_m` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_M: u64 = 2;
/// `riscv_isa_a` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_A: u64 = 4;
/// `riscv_isa_f` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_F: u64 = 8;
/// `riscv_isa_d` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_D: u64 = 16;
/// `riscv_isa_c` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_C: u64 = 32;
/// `riscv_isa_zicsr` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_ZICSR: u64 = 64;
/// `riscv_isa_zifencei` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_ZIFENCEI: u64 = 128;
/// `riscv_isa_sstc` in the `riscv_isa` flags domain.
///
/// RISC-V ISA extensions accepted in guest configuration.
/// Independent flag bit; combine permitted members with bitwise OR.
pub const HYPER_NATIVE_RISCV_ISA_SSTC: u64 = 256;

/// Object-local method `abi_query`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `revision: u64`, `features: u64` |
/// | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Abi` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_ABI_QUERY: u64 = 256;

/// Object-local method `handle_close`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `handle: ConsumeOnCommit, any, rights=0x0` |
/// | Operation | 0 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `handle: ConsumeOnCommit, any, rights=0x0` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_HANDLE_CLOSE: u64 = 0;

/// Object-local method `handle_duplicate`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `source: Borrow, any, rights=0x1` |
/// | Operation | 1 |
/// | Feature | `Core` |
/// | Arguments | `requested_rights: rights` |
/// | Results | `handle: handle` |
/// | Capability effects | `source: Borrow, any, rights=0x1`, `handle: produce, same-as(source), subset-from(requested_rights)` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_HANDLE_DUPLICATE: u64 = 1;

/// Object-local method `handle_replace`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `source: ConsumeOnCommit, any, rights=0x0` |
/// | Operation | 2 |
/// | Feature | `Core` |
/// | Arguments | `requested_rights: rights` |
/// | Results | `handle: handle` |
/// | Capability effects | `source: ConsumeOnCommit, any, rights=0x0`, `handle: produce, same-as(source), subset-from(requested_rights)` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_HANDLE_REPLACE: u64 = 2;

/// Object-local method `handle_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `handle: Borrow, any, rights=0x0` |
/// | Operation | 3 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `handle: Borrow, any, rights=0x0` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=handle_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_HANDLE_GET_INFO: u64 = 3;

/// Object-local method `object_get_basic_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `handle: Borrow, any, rights=0x8` |
/// | Operation | 4 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `handle: Borrow, any, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=object_basic_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_GET_BASIC_INFO: u64 = 4;

/// Object-local method `thread_yield`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_YIELD: u64 = 256;

/// Object-local method `thread_exit`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x800` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `status: i64` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_EXIT: u64 = 257;

/// Object-local method `process_exit`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x800` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `status: i64` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_EXIT: u64 = 258;

/// Object-local method `event_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `options: u32` |
/// | Results | `handle: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `handle: produce, kind=event, fixed=0x8000f` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_EVENT_CREATE: u64 = 256;

/// Object-local method `event_signal`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `event: Borrow, kind=event, rights=0x80000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `clear_mask: u64`, `set_mask: u64` |
/// | Results | — |
/// | Capability effects | `event: Borrow, kind=event, rights=0x80000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_EVENT_SIGNAL: u64 = 256;

/// Object-local method `object_wait_one`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, any, rights=0x4` |
/// | Operation | 5 |
/// | Feature | `Core` |
/// | Arguments | `signals: u64`, `deadline: u64` |
/// | Results | `observed: u64` |
/// | Capability effects | `object: Borrow, any, rights=0x4` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_WAIT_ONE: u64 = 5;

/// Object-local method `byte_channel_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `options: u32` |
/// | Results | `endpoint0: handle`, `endpoint1: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `endpoint0: produce, kind=byte_channel, fixed=0x3f`, `endpoint1: produce, kind=byte_channel, fixed=0x3f` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_BYTE_CHANNEL_CREATE: u64 = 257;

/// Object-local method `byte_channel_write`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `endpoint: Borrow, kind=byte_channel, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count` |
/// | Results | — |
/// | Capability effects | `endpoint: Borrow, kind=byte_channel, rights=0x20` |
/// | User memory | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_BYTE_CHANNEL_WRITE: u64 = 256;

/// Object-local method `byte_channel_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `endpoint: Borrow, kind=byte_channel, rights=0x10` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` |
/// | Results | `actual_bytes: byte_count; also-on=buffer_too_small` |
/// | Capability effects | `endpoint: Borrow, kind=byte_channel, rights=0x10` |
/// | User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=65536; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_BYTE_CHANNEL_READ: u64 = 257;

/// Object-local method `console_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `console: Borrow, kind=console, rights=0x10` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` |
/// | Results | `actual_bytes: byte_count; also-on=would_block` |
/// | Capability effects | `console: Borrow, kind=console, rights=0x10` |
/// | User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CONSOLE_READ: u64 = 256;

/// Object-local method `console_write`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `console: Borrow, kind=console, rights=0x20` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count` |
/// | Results | `actual_bytes: byte_count; also-on=would_block` |
/// | Capability effects | `console: Borrow, kind=console, rights=0x20` |
/// | User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CONSOLE_WRITE: u64 = 257;

/// Object-local method `directory_open_file`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` |
/// | Results | `file: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
/// | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE: u64 = 256;

/// Object-local method `file_read_at`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x10` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `offset: u64`, `output: user_address`, `output_capacity: byte_count` |
/// | Results | `actual_bytes: byte_count`, `file_size: byte_count` |
/// | Capability effects | `file: Borrow, kind=file, rights=0x10` |
/// | User memory | `output: Write, len=output_capacity bytes, max-bytes=2097152; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_READ_AT: u64 = 256;

/// Object-local method `capability_channel_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `options: u32` |
/// | Results | `endpoint0: handle`, `endpoint1: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `endpoint0: produce, kind=capability_channel, fixed=0x3f`, `endpoint1: produce, kind=capability_channel, fixed=0x3f` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CAPABILITY_CHANNEL_CREATE: u64 = 258;

/// Object-local method `capability_channel_try_send`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `endpoint: Borrow, kind=capability_channel, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count`, `dispositions: user_address`, `disposition_count: element_count` |
/// | Results | — |
/// | Capability effects | `endpoint: Borrow, kind=capability_channel, rights=0x20` |
/// | User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0`, `dispositions: Read, len=disposition_count elements, max-elements=16, element-size=24, record=capability_disposition, transactional-handles=(handle, rights, expected_kind, operation), common-rights=0x2, operations=[move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3], commit=AtomicOnOk; order=1` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CAPABILITY_CHANNEL_TRY_SEND: u64 = 256;

/// Object-local method `capability_channel_receive`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `endpoint: Borrow, kind=capability_channel, rights=0x10` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `deadline: u64`, `bytes: user_address`, `byte_capacity: byte_count`, `capability_slots: user_address`, `slot_count: element_count` |
/// | Results | `actual_bytes: byte_count; also-on=buffer_too_small`, `actual_capabilities: element_count; also-on=buffer_too_small` |
/// | Capability effects | `endpoint: Borrow, kind=capability_channel, rights=0x10` |
/// | User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0`, `capability_slots: ReadWrite, len=slot_count elements, max-elements=16, element-size=24, record=capability_receive_slot, typed-receive-slots=(handle, rights, expected_kind, flags), produce-transferred-handles, commit=AtomicOnOk; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE: u64 = 257;

/// Object-local method `process_builder_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `factory: Borrow, kind=task_factory, rights=0x100000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `group: handle`, `domain: handle`, `executable: handle`, `loader: handle` |
/// | Results | `builder: handle` |
/// | Capability effects | `factory: Borrow, kind=task_factory, rights=0x100000`, `group: Borrow, kind=task_group, rights=0x4000000`, `domain: Borrow, kind=resource_domain, rights=0x8000000`, `executable: Borrow, kind=file, rights=0x800000093`, `loader: Borrow, kind=file, rights=0x90`, `builder: produce, kind=process_builder, fixed=0x10000000c2a, inherited-from(factory), mask=0xe3000000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_CREATE: u64 = 256;

/// Object-local method `process_builder_set_name`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `name: user_address`, `name_size: byte_count` |
/// | Results | — |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | User memory | `name: Read, len=name_size bytes, max-bytes=64; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_SET_NAME: u64 = 256;

/// Object-local method `process_builder_set_data`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `data: user_address`, `data_size: byte_count` |
/// | Results | — |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | User memory | `data: Read, len=data_size bytes, max-bytes=16384; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_SET_DATA: u64 = 257;

/// Object-local method `process_builder_set_affinity`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `affinity_words: user_address`, `word_count: element_count` |
/// | Results | — |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_SET_AFFINITY: u64 = 258;

/// Object-local method `process_builder_add_handle`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `source: handle`, `purpose: u32`, `expected_kind: u32`, `rights: rights`, `operation: u32` |
/// | Results | — |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20`, `source: ByOperation(operation: move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3), any, common-rights=0x2` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_ADD_HANDLE: u64 = 259;

/// Object-local method `process_builder_seal`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_SEAL: u64 = 260;

/// Object-local method `process_builder_start`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `process: handle`, `startup_channel: handle` |
/// | Capability effects | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400`, `process: produce, kind=process, fixed=0x80e`, `startup_channel: produce, kind=byte_channel, fixed=0x14` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_START: u64 = 261;

/// Object-local method `process_builder_abort`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_ABORT: u64 = 262;

/// Object-local method `process_request_stop`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `process: Borrow, kind=process, rights=0x800` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `process: Borrow, kind=process, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_REQUEST_STOP: u64 = 256;

/// Object-local method `object_wait_many`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `items: user_address`, `item_count: element_count`, `deadline: u64` |
/// | Results | `index: element_count`, `observed: u64` |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | User memory | `items: Read, len=item_count elements, max-elements=64, element-size=16, record=object_wait_item, borrowed-handles=(handle), required-rights=0x4; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_WAIT_MANY: u64 = 259;

/// Object-local method `process_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `process: Borrow, kind=process, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `info: user_address`, `info_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `process: Borrow, kind=process, rights=0x8` |
/// | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=process_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_GET_INFO: u64 = 257;

/// Object-local method `task_inspector_scan_processes`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
/// | Results | `count: element_count`, `next_cursor: u64` |
/// | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x8` |
/// | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=96, record=task_process; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_PROCESSES: u64 = 256;

/// Object-local method `task_inspector_scan_threads`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
/// | Results | `count: element_count`, `next_cursor: u64` |
/// | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x8` |
/// | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=task_thread; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_THREADS: u64 = 257;

/// Object-local method `task_inspector_derive_process`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `process: handle` |
/// | Results | `inspector: handle` |
/// | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_PROCESS: u64 = 258;

/// Object-local method `object_inspector_scan_objects`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
/// | Results | `count: element_count`, `next_cursor: u64` |
/// | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x8` |
/// | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=object_inspection; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_OBJECTS: u64 = 256;

/// Object-local method `object_inspector_scan_handles`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `process_koid: u64`, `cursor: u64`, `records: user_address`, `capacity: element_count` |
/// | Results | `count: element_count`, `next_cursor: u64` |
/// | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x8` |
/// | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=40, record=handle_inspection; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_HANDLES: u64 = 257;

/// Object-local method `object_inspector_derive_process`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `process: handle` |
/// | Results | `inspector: handle` |
/// | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_PROCESS: u64 = 258;

/// Object-local method `task_inspector_derive_task_group`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `task_group: handle` |
/// | Results | `inspector: handle` |
/// | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_TASK_GROUP: u64 = 259;

/// Object-local method `object_inspector_derive_task_group`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `task_group: handle` |
/// | Results | `inspector: handle` |
/// | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_TASK_GROUP: u64 = 259;

/// Object-local method `task_inspector_derive_resource_domain`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `resource_domain: handle` |
/// | Results | `inspector: handle` |
/// | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN: u64 = 260;

/// Object-local method `object_inspector_derive_resource_domain`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `resource_domain: handle` |
/// | Results | `inspector: handle` |
/// | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN: u64 = 260;

/// Object-local method `directory_open_directory`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` |
/// | Results | `child: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `child: produce, kind=directory, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
/// | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY: u64 = 257;

/// Object-local method `vmo_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `size: byte_count` |
/// | Results | `vmo: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `vmo: produce, kind=vmo, fixed=0x7b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMO_CREATE: u64 = 259;

/// Object-local method `vmo_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmo: Borrow, kind=vmo, rights=0x10` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `offset: u64`, `bytes: user_address`, `byte_count: byte_count` |
/// | Results | — |
/// | Capability effects | `vmo: Borrow, kind=vmo, rights=0x10` |
/// | User memory | `bytes: Write, len=byte_count bytes, max-bytes=65536; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMO_READ: u64 = 256;

/// Object-local method `vmo_write`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmo: Borrow, kind=vmo, rights=0x20` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `offset: u64`, `bytes: user_address`, `byte_count: byte_count` |
/// | Results | — |
/// | Capability effects | `vmo: Borrow, kind=vmo, rights=0x20` |
/// | User memory | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMO_WRITE: u64 = 257;

/// Object-local method `vmar_allocate`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `size: byte_count`, `options: u64` |
/// | Results | `child: handle`, `address: u64` |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `child: produce, kind=vmar, fixed=0x4b, inherited-from(vmar), mask=0x1000000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_ALLOCATE: u64 = 256;

/// Object-local method `vmar_map`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `vmo: handle`, `vmo_offset: u64`, `address: u64`, `size: byte_count`, `permissions: u32` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x40` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_MAP: u64 = 257;

/// Object-local method `vmar_protect`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `size: byte_count`, `permissions: u32` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_PROTECT: u64 = 258;

/// Object-local method `vmar_unmap`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `size: byte_count` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_UNMAP: u64 = 259;

/// Object-local method `vmar_destroy`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_DESTROY: u64 = 260;

/// Object-local method `directory_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `stream: Borrow, kind=directory_stream, rights=0x10` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `cookie: u64`, `records: user_address`, `capacity: element_count`, `options: u32` |
/// | Results | `count: element_count`, `next_cookie: u64` |
/// | Capability effects | `stream: Borrow, kind=directory_stream, rights=0x10` |
/// | User memory | `records: Write, len=capacity elements, max-elements=4, element-size=280, record=directory_entry; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_READ: u64 = 256;

/// Object-local method `memory_inspector_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `observation: user_address`, `observation_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
/// | User memory | `observation: Write, len=observation_size bytes, max-bytes=4096, record=memory_observation; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ: u64 = 256;

/// Object-local method `cpu_inspector_profile_start`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `period: u64`, `seconds: u64` |
/// | Results | `generation: u64` |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_START: u64 = 257;

/// Object-local method `cpu_inspector_profile_stop`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `generation: u64` |
/// | Results | — |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_STOP: u64 = 258;

/// Object-local method `cpu_inspector_profile_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `generation: u64`, `cpu: u64`, `cursor: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=192; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_READ: u64 = 259;

/// Object-local method `cpu_inspector_trace_start`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | `generation: u64` |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | User memory | `configuration: Read, fixed-bytes=56, record=trace_config; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_START: u64 = 260;

/// Object-local method `cpu_inspector_trace_stop`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `generation: u64` |
/// | Results | — |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_STOP: u64 = 261;

/// Object-local method `cpu_inspector_trace_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `generation: u64`, `cpu: u64`, `cursor: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=2656; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_READ: u64 = 262;

/// Object-local method `cpu_inspector_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `observation: user_address`, `observation_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x8` |
/// | User memory | `observation: Write, len=observation_size bytes, max-bytes=4096, record=cpu_observation; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CPU_INSPECTOR_READ: u64 = 256;

/// Object-local method `file_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `file: Borrow, kind=file, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_info; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_GET_INFO: u64 = 257;

/// Object-local method `directory_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x8` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=directory_info; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_GET_INFO: u64 = 258;

/// Object-local method `virtual_machine_creation_lease_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `resource_domain: handle` |
/// | Results | `lease: handle` |
/// | Capability effects | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `lease: produce, kind=virtual_machine_creation_lease, fixed=0x2000000a, inherited-from(authority), mask=0xf0000000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_CREATE: u64 = 256;

/// Object-local method `virtual_machine_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address`, `configuration_size: byte_count` |
/// | Results | `pending_virtual_machine: handle` |
/// | Capability effects | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000`, `pending_virtual_machine: produce, kind=pending_virtual_machine, fixed=0xc2a, inherited-from(lease), mask=0xf0000000000` |
/// | User memory | `configuration: Read, len=configuration_size bytes, max-bytes=4096, record=virtual_machine_configuration; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATE: u64 = 256;

/// Object-local method `pending_virtual_machine_set_memory`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `vmo: handle` |
/// | Results | — |
/// | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `vmo: Borrow, kind=vmo, rights=0x70` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_MEMORY: u64 = 256;

/// Object-local method `pending_virtual_machine_set_bootstrap`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `bootstrap: user_address`, `bootstrap_size: byte_count` |
/// | Results | — |
/// | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | User memory | `bootstrap: Read, len=bootstrap_size bytes, max-bytes=4096, record=virtual_cpu_bootstrap; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP: u64 = 257;

/// Object-local method `pending_virtual_machine_seal`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SEAL: u64 = 258;

/// Object-local method `pending_virtual_machine_install`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `virtual_machine: handle`, `boot_virtual_cpu: handle` |
/// | Capability effects | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400`, `virtual_machine: produce, kind=virtual_machine, fixed=0x82e, inherited-from(pending_virtual_machine), mask=0xf0000000000`, `boot_virtual_cpu: produce, kind=virtual_cpu, fixed=0x1000000042e, inherited-from(pending_virtual_machine), mask=0xe0000000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_INSTALL: u64 = 259;

/// Object-local method `pending_virtual_machine_abort`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ABORT: u64 = 260;

/// Object-local method `virtual_machine_request_stop`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_REQUEST_STOP: u64 = 256;

/// Object-local method `virtual_machine_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `info: user_address`, `info_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` |
/// | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_INFO: u64 = 257;

/// Object-local method `virtual_cpu_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `info: user_address`, `info_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` |
/// | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_cpu_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_GET_INFO: u64 = 256;

/// Object-local method `resource_domain_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `parent: Borrow, kind=resource_domain, rights=0x800000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `limits: user_address`, `limits_size: byte_count` |
/// | Results | `child: handle` |
/// | Capability effects | `parent: Borrow, kind=resource_domain, rights=0x800000`, `child: produce, kind=resource_domain, fixed=0x984000b` |
/// | User memory | `limits: Read, len=limits_size bytes, max-bytes=4096, record=resource_limits; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_RESOURCE_DOMAIN_CREATE: u64 = 256;

/// Object-local method `task_group_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `factory: Borrow, kind=task_factory, rights=0x400000` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `resource_domain: handle` |
/// | Results | `group: handle` |
/// | Capability effects | `factory: Borrow, kind=task_factory, rights=0x400000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `group: produce, kind=task_group, fixed=0x400080f` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_GROUP_CREATE: u64 = 257;

/// Object-local method `virtual_cpu_start`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_START: u64 = 257;

/// Object-local method `pending_virtual_machine_set_virtual_serial`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `virtual_serial: handle` |
/// | Results | — |
/// | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `virtual_serial: ConsumeOnCommit, kind=virtual_serial, rights=0x8002` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL: u64 = 261;

/// Object-local method `clock_get_monotonic`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `nanoseconds: u64` |
/// | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Abi` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CLOCK_GET_MONOTONIC: u64 = 257;

/// Object-local method `virtual_serial_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `virtual_serial: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `virtual_serial: produce, kind=virtual_serial, fixed=0x803f` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_SERIAL_CREATE: u64 = 260;

/// Object-local method `virtual_serial_register_output`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `buffer: handle` |
/// | Results | — |
/// | Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20`, `buffer: Borrow, kind=vmo, rights=0x70` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_SERIAL_REGISTER_OUTPUT: u64 = 256;

/// Object-local method `virtual_serial_write`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `bytes: user_address`, `byte_count: byte_count` |
/// | Results | `actual_bytes: byte_count` |
/// | Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
/// | User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_SERIAL_WRITE: u64 = 257;

/// Object-local method `thread_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x200000` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `entry: u64`, `stack: u64`, `tls: u64`, `argument: u64`, `affinity_words: user_address`, `word_count: element_count` |
/// | Results | `thread: handle` |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x200000`, `thread: produce, kind=thread, fixed=0x10000000c0f, inherited-from(receiver), mask=0xe0000000000` |
/// | User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_CREATE: u64 = 260;

/// Object-local method `thread_start`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `thread: Borrow, kind=thread, rights=0x400` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `thread: Borrow, kind=thread, rights=0x400` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_START: u64 = 256;

/// Object-local method `thread_request_stop`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `thread: Borrow, kind=thread, rights=0x800` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `thread: Borrow, kind=thread, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_REQUEST_STOP: u64 = 257;

/// Object-local method `atomic_wait`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `expected: u32`, `deadline: u64` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_ATOMIC_WAIT: u64 = 261;

/// Object-local method `atomic_wake`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `count: u32` |
/// | Results | `woken: u64` |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_ATOMIC_WAKE: u64 = 262;

/// Object-local method `thread_sleep`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `deadline: u64` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_SLEEP: u64 = 263;

/// Object-local method `file_write_at`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x20` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `options: u32`, `offset: u64`, `input: user_address`, `input_size: byte_count` |
/// | Results | `actual_bytes: byte_count`, `end_offset: u64` |
/// | Capability effects | `file: Borrow, kind=file, rights=0x20` |
/// | User memory | `input: Read, len=input_size bytes, max-bytes=2097152; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_WRITE_AT: u64 = 258;

/// Object-local method `file_resize`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x20` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `size: byte_count` |
/// | Results | — |
/// | Capability effects | `file: Borrow, kind=file, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_RESIZE: u64 = 259;

/// Object-local method `directory_create_file`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `mode: u32` |
/// | Results | `file: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x30`, `file: produce, kind=file, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
/// | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_CREATE_FILE: u64 = 259;

/// Object-local method `directory_create_directory`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_size: byte_count`, `mode: u32` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
/// | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_CREATE_DIRECTORY: u64 = 260;

/// Object-local method `directory_remove`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_size: byte_count`, `options: u32` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
/// | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_REMOVE: u64 = 261;

/// Object-local method `wait_set_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `capacity: element_count` |
/// | Results | `wait_set: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `wait_set: produce, kind=wait_set, fixed=0x4000000d` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_WAIT_SET_CREATE: u64 = 261;

/// Object-local method `wait_set_add`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `object: handle`, `signals: u64` |
/// | Results | `registration: u64` |
/// | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000`, `object: Borrow, any, rights=0x4` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_WAIT_SET_ADD: u64 = 256;

/// Object-local method `wait_set_rearm`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `registration: u64` |
/// | Results | — |
/// | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_WAIT_SET_REARM: u64 = 257;

/// Object-local method `wait_set_remove`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `registration: u64` |
/// | Results | — |
/// | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_WAIT_SET_REMOVE: u64 = 258;

/// Object-local method `wait_set_wait`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x4` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `deadline: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | — |
/// | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x4` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=24, record=wait_set_event; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_WAIT_SET_WAIT: u64 = 259;

/// Object-local method `process_get_current_id`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x10` |
/// | Operation | 264 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `koid: u64` |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_GET_CURRENT_ID: u64 = 264;

/// Object-local method `virtual_serial_acknowledge_output`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `consumed: u64` |
/// | Results | — |
/// | Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT: u64 = 258;

/// Object-local method `directory_scope_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `root: Borrow, kind=directory, rights=0x10` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `start: handle`, `rights: rights` |
/// | Results | `directory: handle` |
/// | Capability effects | `root: Borrow, kind=directory, rights=0x10`, `start: Borrow, kind=directory, rights=0x10`, `directory: produce, kind=directory, exact-from(rights), allowed=0xd800000bb, required-from=root` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_SCOPE_CREATE: u64 = 262;

/// Object-local method `directory_get_metadata`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x18` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x18` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_GET_METADATA: u64 = 263;

/// Object-local method `file_get_metadata`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x8` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `file: Borrow, kind=file, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_GET_METADATA: u64 = 260;

/// Object-local method `directory_get_self_metadata`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x8` |
/// | Operation | 264 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_GET_SELF_METADATA: u64 = 264;

/// Object-local method `directory_set_metadata`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x80000010` |
/// | Operation | 265 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `input: user_address`, `input_size: byte_count` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x80000010` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=1`, `input: Read, len=input_size bytes, max-bytes=4096, record=file_metadata_update; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_SET_METADATA: u64 = 265;

/// Object-local method `file_set_metadata`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x80000000` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `input: user_address`, `input_size: byte_count` |
/// | Results | — |
/// | Capability effects | `file: Borrow, kind=file, rights=0x80000000` |
/// | User memory | `input: Read, len=input_size bytes, max-bytes=4096, record=file_metadata_update; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_SET_METADATA: u64 = 261;

/// Object-local method `directory_rename`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `source: Borrow, kind=directory, rights=0x30` |
/// | Operation | 266 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `destination: handle`, `new_path: user_address`, `new_path_length: byte_count` |
/// | Results | — |
/// | Capability effects | `source: Borrow, kind=directory, rights=0x30`, `destination: Borrow, kind=directory, rights=0x30` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `new_path: Read, len=new_path_length bytes, max-bytes=4096; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_RENAME: u64 = 266;

/// Object-local method `directory_link`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `source: Borrow, kind=directory, rights=0x30` |
/// | Operation | 267 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `destination: handle`, `new_path: user_address`, `new_path_length: byte_count` |
/// | Results | — |
/// | Capability effects | `source: Borrow, kind=directory, rights=0x30`, `destination: Borrow, kind=directory, rights=0x30` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `new_path: Read, len=new_path_length bytes, max-bytes=4096; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_LINK: u64 = 267;

/// Object-local method `directory_symlink`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
/// | Operation | 268 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `target: user_address`, `target_length: byte_count` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `target: Read, len=target_length bytes, max-bytes=4096; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_SYMLINK: u64 = 268;

/// Object-local method `directory_read_link`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 269 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `output: user_address`, `capacity: byte_count` |
/// | Results | `actual: byte_count` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=capacity bytes, max-bytes=4096; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_READ_LINK: u64 = 269;

/// Object-local method `directory_canonicalize`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 270 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `output: user_address`, `capacity: byte_count` |
/// | Results | `actual: byte_count` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=capacity bytes, max-bytes=4096; order=1` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_CANONICALIZE: u64 = 270;

/// Object-local method `directory_remove_if`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
/// | Operation | 271 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `expected_node_id: u64` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_REMOVE_IF: u64 = 271;

/// Object-local method `directory_open_directory_nofollow`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 272 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `rights: rights` |
/// | Results | `directory: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `directory: produce, kind=directory, exact-from(rights), allowed=0x9800000bb, required-from=directory` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW: u64 = 272;

/// Object-local method `file_sync`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x20` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `scope: u32` |
/// | Results | — |
/// | Capability effects | `file: Borrow, kind=file, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_SYNC: u64 = 262;

/// Object-local method `file_lock`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x100000000` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `mode: u32`, `deadline: u64` |
/// | Results | — |
/// | Capability effects | `file: Borrow, kind=file, rights=0x100000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_LOCK: u64 = 263;

/// Object-local method `file_unlock`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x100000000` |
/// | Operation | 264 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `file: Borrow, kind=file, rights=0x100000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_UNLOCK: u64 = 264;

/// Object-local method `clock_get_realtime`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `seconds: i64`, `nanoseconds: u32` |
/// | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CLOCK_GET_REALTIME: u64 = 258;

/// Object-local method `directory_open_file_with_options`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 273 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count`, `rights: rights`, `options: u32`, `mode: u32` |
/// | Results | `file: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(rights), allowed=0x9800000bb, required-from=directory` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE_WITH_OPTIONS: u64 = 273;

/// Object-local method `virtual_machine_creation_lease_get_platform_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `lease: Borrow, kind=virtual_machine_creation_lease, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `platform_profile: u32`, `info: user_address`, `info_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `lease: Borrow, kind=virtual_machine_creation_lease, rights=0x8` |
/// | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_platform_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO: u64 = 257;

/// Object-local method `vmo_create_snapshot`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmo: Borrow, kind=vmo, rights=0x10` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `vmo: handle`, `byte_size: byte_count` |
/// | Capability effects | `vmo: Borrow, kind=vmo, rights=0x10`, `vmo: produce, kind=vmo, fixed=0x5b` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMO_CREATE_SNAPSHOT: u64 = 258;

/// Object-local method `vmar_map_private`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `vmo: handle`, `mapping: user_address`, `mapping_size: byte_count` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x50` |
/// | User memory | `mapping: Read, len=mapping_size bytes, max-bytes=4096, record=private_mapping; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_MAP_PRIVATE: u64 = 261;

/// Object-local method `virtual_machine_get_power_request`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `request: user_address`, `request_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | User memory | `request: Write, len=request_size bytes, max-bytes=4096, record=virtual_machine_power_request; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_POWER_REQUEST: u64 = 258;

/// Object-local method `virtual_machine_complete_power_request`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `request_id: u64`, `accept: u32` |
/// | Results | — |
/// | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST: u64 = 259;

/// Object-local method `virtual_machine_open_vcpu`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `vcpu_id: u32` |
/// | Results | `virtual_cpu: handle` |
/// | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20`, `virtual_cpu: produce, kind=virtual_cpu, fixed=0x1000000042e, inherited-from(virtual_machine), mask=0xe0000000000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_OPEN_VCPU: u64 = 260;

/// Object-local method `virtual_machine_register_mmio`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `base: u64`, `length: u64`, `device: u64` |
/// | Results | — |
/// | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_MACHINE_REGISTER_MMIO: u64 = 261;

/// Object-local method `virtual_cpu_get_mmio_request`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `request: user_address`, `request_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
/// | User memory | `request: Write, len=request_size bytes, max-bytes=4096, record=virtual_cpu_mmio_request; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_GET_MMIO_REQUEST: u64 = 258;

/// Object-local method `virtual_cpu_complete_mmio`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `request_id: u64`, `operation: u32`, `value: u64` |
/// | Results | — |
/// | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_COMPLETE_MMIO: u64 = 259;

/// Object-local method `guest_memory_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmo: Borrow, kind=vmo, rights=0x70` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `memory: handle` |
/// | Capability effects | `vmo: Borrow, kind=vmo, rights=0x70`, `memory: produce, kind=guest_memory, fixed=0x7b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MEMORY_CREATE: u64 = 259;

/// Object-local method `pending_virtual_machine_map_memory`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `memory: handle`, `guest_offset: u64`, `source_offset: u64`, `length: u64` |
/// | Results | — |
/// | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `memory: Borrow, kind=guest_memory, rights=0x40` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_MAP_MEMORY: u64 = 262;

/// Object-local method `vmo_create_contiguous`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `size: byte_count` |
/// | Results | `vmo: handle` |
/// | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `vmo: produce, kind=vmo, fixed=0x7b` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMO_CREATE_CONTIGUOUS: u64 = 262;

/// Object-local method `device_claim`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `index: u32` |
/// | Results | `physical_device: handle` |
/// | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x800f` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_CLAIM: u64 = 256;

/// Object-local method `physical_device_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=physical_device_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PHYSICAL_DEVICE_INFO: u64 = 256;

/// Object-local method `vmo_get_dma_extent`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x10000` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `vmo: handle`, `offset: u64`, `length: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x10000`, `vmo: Borrow, kind=vmo, rights=0x50` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=dma_extent; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMO_GET_DMA_EXTENT: u64 = 257;

/// Object-local method `pending_virtual_machine_assign_device`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `pending: Borrow, kind=pending_virtual_machine, rights=0x20` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `device: handle`, `guest_base: u64`, `guest_irq: u32` |
/// | Results | — |
/// | Capability effects | `pending: Borrow, kind=pending_virtual_machine, rights=0x20`, `device: Borrow, kind=physical_device, rights=0x8000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE: u64 = 263;

/// Object-local method `guest_mailbox_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `base: u64`, `irq: u32` |
/// | Results | `guest_mailbox: handle` |
/// | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20`, `guest_mailbox: produce, kind=guest_mailbox, fixed=0x3e` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MAILBOX_CREATE: u64 = 262;

/// Object-local method `guest_mailbox_send`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `mailbox: Borrow, kind=guest_mailbox, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `bytes: user_address`, `length: byte_count` |
/// | Results | — |
/// | Capability effects | `mailbox: Borrow, kind=guest_mailbox, rights=0x20` |
/// | User memory | `bytes: Read, len=length bytes, max-bytes=256; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MAILBOX_SEND: u64 = 256;

/// Object-local method `guest_mailbox_receive`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `mailbox: Borrow, kind=guest_mailbox, rights=0x10` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `bytes: user_address`, `capacity: byte_count` |
/// | Results | `bytes: byte_count` |
/// | Capability effects | `mailbox: Borrow, kind=guest_mailbox, rights=0x10` |
/// | User memory | `bytes: Write, len=capacity bytes, max-bytes=256; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MAILBOX_RECEIVE: u64 = 257;

/// Object-local method `guest_notification_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `frontend: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `backend: handle`, `frontend_base: u64`, `backend_base: u64`, `frontend_irq: u32`, `backend_irq: u32` |
/// | Results | `guest_notification: handle` |
/// | Capability effects | `frontend: Borrow, kind=virtual_machine, rights=0x20`, `backend: Borrow, kind=virtual_machine, rights=0x20`, `guest_notification: produce, kind=guest_notification, fixed=0x2e` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_NOTIFICATION_CREATE: u64 = 263;

/// Object-local method `guest_notification_control`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `notification: Borrow, kind=guest_notification, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `operation: u32` |
/// | Results | `epoch: u32` |
/// | Capability effects | `notification: Borrow, kind=guest_notification, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONTROL: u64 = 256;

/// Object-local method `native_block_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `memory: Borrow, kind=guest_memory, rights=0x40` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `backend: handle`, `guest_base: u64`, `notification_base: u64`, `notification_irq: u32` |
/// | Results | `block: handle` |
/// | Capability effects | `memory: Borrow, kind=guest_memory, rights=0x40`, `backend: Borrow, kind=virtual_machine, rights=0x20`, `block: produce, kind=native_block, fixed=0x7e` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_NATIVE_BLOCK_CREATE: u64 = 256;

/// Object-local method `native_block_activate`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `block: Borrow, kind=native_block, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `readonly: u32` |
/// | Results | `sectors: u64`, `readonly: u32` |
/// | Capability effects | `block: Borrow, kind=native_block, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_NATIVE_BLOCK_ACTIVATE: u64 = 256;

/// Object-local method `filesystem_mount`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0xa0` |
/// | Operation | 274 |
/// | Feature | `Core` |
/// | Arguments | `channel: handle`, `buffer: handle`, `path: user_address`, `path_length: byte_count`, `read_only: u64` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0xa0`, `channel: Borrow, kind=byte_channel, rights=0x34`, `buffer: Borrow, kind=vmo, rights=0x70` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILESYSTEM_MOUNT: u64 = 274;

/// Object-local method `guest_mapping_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `backend: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 264 |
/// | Feature | `Core` |
/// | Arguments | `memory: handle`, `frontend_base: u64` |
/// | Results | `mapping: handle`, `token: u64` |
/// | Capability effects | `backend: Borrow, kind=virtual_machine, rights=0x20`, `memory: Borrow, kind=guest_memory, rights=0x40`, `mapping: produce, kind=guest_mapping, fixed=0x2a` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MAPPING_CREATE: u64 = 264;

/// Object-local method `guest_mapping_release`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `mapping: Borrow, kind=guest_mapping, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `mapping: Borrow, kind=guest_mapping, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MAPPING_RELEASE: u64 = 256;

/// Object-local method `device_claim_matching`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `profile: u32`, `identity_kind: u32`, `identity: user_address`, `length: byte_count` |
/// | Results | `physical_device: handle` |
/// | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x800f` |
/// | User memory | `identity: Read, len=length bytes, max-bytes=512; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_CLAIM_MATCHING: u64 = 258;

/// Object-local method `device_profile_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_profile_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_PROFILE_INFO: u64 = 257;

/// Object-local method `device_resource_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `index: u32`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_resource_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_RESOURCE_INFO: u64 = 258;

/// Object-local method `device_firmware_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `query: user_address`, `output: user_address`, `capacity: byte_count` |
/// | Results | `value: u64` |
/// | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8` |
/// | User memory | `query: Read, fixed-bytes=24, record=device_firmware_query; order=0`, `output: Write, len=capacity bytes, max-bytes=65536; order=1` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_FIRMWARE_READ: u64 = 259;

/// Object-local method `device_claim_bundle`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `entries: user_address`, `count: element_count`, `irq_node: u32` |
/// | Results | `physical_device: handle` |
/// | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x2803f` |
/// | User memory | `entries: Read, len=count elements, max-elements=8, element-size=16, record=device_bundle_entry; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_CLAIM_BUNDLE: u64 = 260;

/// Object-local method `device_mmio`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x0` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `offset: u64`, `width: u32`, `operation: u32`, `value: u64` |
/// | Results | `value: u64` |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x0` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_MMIO: u64 = 259;

/// Object-local method `device_irq_pending`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x4` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `value: u64` |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x4` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_IRQ_PENDING: u64 = 260;

/// Object-local method `device_irq_complete`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x20000` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `sequence: u64`, `asserted: u32` |
/// | Results | — |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x20000` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_IRQ_COMPLETE: u64 = 261;

/// Object-local method `virtual_cpu_set_affinity`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `affinity_words: user_address`, `word_count: element_count` |
/// | Results | — |
/// | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
/// | User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_SET_AFFINITY: u64 = 260;

/// Object-local method `system_config`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `key: u64` |
/// | Results | `value: u64` |
/// | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Abi` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_SYSTEM_CONFIG: u64 = 259;

/// Object-local method `object_inspector_read_details`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x200000008` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `process_koid: u64`, `target: u64`, `cursor: u64`, `record: user_address`, `record_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x200000008` |
/// | User memory | `record: Write, len=record_size bytes, max-bytes=4096, record=object_details; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_OBJECT_INSPECTOR_READ_DETAILS: u64 = 261;

/// Object-local method `filesystem_unmount`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0xa0` |
/// | Operation | 275 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0xa0` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILESYSTEM_UNMOUNT: u64 = 275;

/// Object-local method `filesystem_mount_ramfs`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
/// | Operation | 276 |
/// | Feature | `Core` |
/// | Arguments | `path: user_address`, `path_length: byte_count` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0` |
/// | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILESYSTEM_MOUNT_RAMFS: u64 = 276;

/// Object-local method `filesystem_switch_root`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
/// | Operation | 277 |
/// | Feature | `Core` |
/// | Arguments | `request: user_address` |
/// | Results | — |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0` |
/// | User memory | `request: Read, fixed-bytes=48, record=filesystem_root_switch; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILESYSTEM_SWITCH_ROOT: u64 = 277;

/// Object-local method `file_protect`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `file: Borrow, kind=file, rights=0x800000000` |
/// | Operation | 265 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `protection: handle`, `byte_size: byte_count` |
/// | Capability effects | `file: Borrow, kind=file, rights=0x800000000`, `protection: produce, kind=file_protection, fixed=0x48` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILE_PROTECT: u64 = 265;

/// Object-local method `vmar_map_file`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `file: handle`, `protection: handle`, `mapping: user_address`, `mapping_size: byte_count` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `file: Borrow, kind=file, rights=0x10`, `protection: Borrow, kind=file_protection, rights=0x40` |
/// | User memory | `mapping: Read, len=mapping_size bytes, max-bytes=4096, record=private_mapping; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_MAP_FILE: u64 = 262;

/// Object-local method `directory_begin_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
/// | Operation | 278 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `stream: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `stream: produce, kind=directory_stream, inherited-from(directory), mask=0x1b` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DIRECTORY_BEGIN_READ: u64 = 278;

/// Object-local method `process_derive_observer`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `process: Borrow, kind=process, rights=0x806` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `observer: handle` |
/// | Capability effects | `process: Borrow, kind=process, rights=0x806`, `observer: produce, kind=process, fixed=0x6` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_DERIVE_OBSERVER: u64 = 258;

/// Object-local method `task_group_request_stop`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `task_group: Borrow, kind=task_group, rights=0x800` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `task_group: Borrow, kind=task_group, rights=0x800` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_TASK_GROUP_REQUEST_STOP: u64 = 256;

/// Object-local method `console_discard_input`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `console: Borrow, kind=console, rights=0x10` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `discarded: byte_count` |
/// | Capability effects | `console: Borrow, kind=console, rights=0x10` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CONSOLE_DISCARD_INPUT: u64 = 258;

/// Object-local method `guest_notification_configure`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 265 |
/// | Feature | `Core` |
/// | Arguments | `original_base: u64`, `base: u64`, `flags: u32` |
/// | Results | — |
/// | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONFIGURE: u64 = 265;

/// Object-local method `guest_notification_access`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 266 |
/// | Feature | `Core` |
/// | Arguments | `original_base: u64`, `operation: u32`, `value: u32` |
/// | Results | `value: u32` |
/// | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_NOTIFICATION_ACCESS: u64 = 266;

/// Object-local method `vmar_lock_memory`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `size: byte_count`, `flags: u32` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_LOCK_MEMORY: u64 = 263;

/// Object-local method `vmar_unlock_memory`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
/// | Operation | 264 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `size: byte_count`, `flags: u32` |
/// | Results | — |
/// | Capability effects | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VMAR_UNLOCK_MEMORY: u64 = 264;

/// Object-local method `process_builder_set_memory_lock`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x1000000020` |
/// | Operation | 263 |
/// | Feature | `Core` |
/// | Arguments | `enabled: u32` |
/// | Results | — |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x1000000020` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_SET_MEMORY_LOCK: u64 = 263;

/// Object-local method `filesystem_begin_root_switch`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
/// | Operation | 279 |
/// | Feature | `Core` |
/// | Arguments | `request: user_address` |
/// | Results | `transition: handle` |
/// | Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0`, `transition: produce, kind=root_transition, fixed=0x2a` |
/// | User memory | `request: Read, fixed-bytes=48, record=filesystem_root_switch; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILESYSTEM_BEGIN_ROOT_SWITCH: u64 = 279;

/// Object-local method `filesystem_finish_root_switch`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `transition: Borrow, kind=root_transition, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `commit: u32` |
/// | Results | — |
/// | Capability effects | `transition: Borrow, kind=root_transition, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_FILESYSTEM_FINISH_ROOT_SWITCH: u64 = 256;

/// Object-local method `guest_memory_transfer`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `memory: Borrow, kind=guest_memory, rights=0x0` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `operation: u32`, `offset: u64`, `buffer: user_address`, `length: byte_count` |
/// | Results | — |
/// | Capability effects | `memory: Borrow, kind=guest_memory, rights=0x0` |
/// | User memory | `buffer: ReadWrite, len=length bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MEMORY_TRANSFER: u64 = 257;

/// Object-local method `guest_memory_scalar`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `memory: Borrow, kind=guest_memory, rights=0x0` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `operation: u32`, `offset: u64`, `width: u32`, `value: u64` |
/// | Results | `value: u64` |
/// | Capability effects | `memory: Borrow, kind=guest_memory, rights=0x0` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_MEMORY_SCALAR: u64 = 258;

/// Object-local method `guest_interrupt_create`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
/// | Operation | 267 |
/// | Feature | `Core` |
/// | Arguments | `irq: u32` |
/// | Results | `interrupt: handle` |
/// | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20`, `interrupt: produce, kind=guest_interrupt, fixed=0x2a` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_INTERRUPT_CREATE: u64 = 267;

/// Object-local method `guest_interrupt_set_level`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `interrupt: Borrow, kind=guest_interrupt, rights=0x20` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `asserted: u32` |
/// | Results | — |
/// | Capability effects | `interrupt: Borrow, kind=guest_interrupt, rights=0x20` |
/// | User memory | — |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_GUEST_INTERRUPT_SET_LEVEL: u64 = 256;

/// Object-local method `memory_inspector_read_statistics`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `topic: u32`, `index: u32`, `statistics: user_address`, `statistics_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
/// | User memory | `statistics: Write, len=statistics_size bytes, max-bytes=4096; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Object` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ_STATISTICS: u64 = 257;

/// Object-local method `process_builder_retain`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `builder: Borrow, kind=process_builder, rights=0x2000000020` |
/// | Operation | 264 |
/// | Feature | `Core` |
/// | Arguments | `max_bytes: byte_count`, `timeout_ns: u64` |
/// | Results | `handle: handle` |
/// | Capability effects | `builder: Borrow, kind=process_builder, rights=0x2000000020`, `handle: produce, kind=process_retirement, fixed=0x420000000e` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_RETAIN: u64 = 264;

/// Object-local method `process_retirement_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=process_retirement, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `object: Borrow, kind=process_retirement, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=process_retirement_info; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_RETIREMENT_GET_INFO: u64 = 256;

/// Object-local method `process_retirement_open_view`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=process_retirement, rights=0x200000000` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | `handle: handle` |
/// | Capability effects | `object: Borrow, kind=process_retirement, rights=0x200000000`, `handle: produce, kind=crash_view, fixed=0x1b` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_RETIREMENT_OPEN_VIEW: u64 = 257;

/// Object-local method `process_retirement_release`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=process_retirement, rights=0x4000000000` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | — |
/// | Results | — |
/// | Capability effects | `object: Borrow, kind=process_retirement, rights=0x4000000000` |
/// | User memory | — |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_RETIREMENT_RELEASE: u64 = 258;

/// Object-local method `crash_view_get_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=crash_view, rights=0x8` |
/// | Operation | 256 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `object: Borrow, kind=crash_view, rights=0x8` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=process_retirement_info; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CRASH_VIEW_GET_INFO: u64 = 256;

/// Object-local method `crash_view_read`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `address: u64`, `output: user_address`, `length: byte_count` |
/// | Results | `actual_bytes: byte_count` |
/// | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
/// | User memory | `output: Write, len=length bytes, max-bytes=65536; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CRASH_VIEW_READ: u64 = 257;

/// Object-local method `crash_view_thread`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count`, `next_index: u64` |
/// | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_thread; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CRASH_VIEW_THREAD: u64 = 258;

/// Object-local method `crash_view_mapping`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count`, `next_index: u64` |
/// | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_mapping; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CRASH_VIEW_MAPPING: u64 = 259;

/// Object-local method `crash_view_handle`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
/// | Operation | 260 |
/// | Feature | `Core` |
/// | Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count`, `next_index: u64` |
/// | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_handle; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_CRASH_VIEW_HANDLE: u64 = 260;

/// Object-local method `thread_set_current_name`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | Operation | 265 |
/// | Feature | `Core` |
/// | Arguments | `name: user_address`, `name_size: byte_count` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
/// | User memory | `name: Read, len=name_size bytes, max-bytes=64; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_SET_CURRENT_NAME: u64 = 265;

/// Object-local method `device_dma_fault_info`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `device: Borrow, kind=physical_device, rights=0x8008` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `output: user_address`, `output_size: byte_count` |
/// | Results | `supported_size: byte_count` |
/// | Capability effects | `device: Borrow, kind=physical_device, rights=0x8008` |
/// | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_dma_fault_info; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_DEVICE_DMA_FAULT_INFO: u64 = 262;

/// Object-local method `native_block_transfer`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `block: Borrow, kind=native_block, rights=0x0` |
/// | Operation | 257 |
/// | Feature | `Core` |
/// | Arguments | `operation: u32`, `first_sector: u64`, `buffer: user_address`, `length: byte_count` |
/// | Results | — |
/// | Capability effects | `block: Borrow, kind=native_block, rights=0x0` |
/// | User memory | `buffer: ReadWrite, len=length bytes, max-bytes=524352; order=0` |
/// | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Capability` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_NATIVE_BLOCK_TRANSFER: u64 = 257;

/// Object-local method `thread_get_current_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x8` |
/// | Operation | 266 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x8` |
/// | User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_GET_CURRENT_SCHEDULING: u64 = 266;

/// Object-local method `thread_set_current_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=execution_context, rights=0x0` |
/// | Operation | 267 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x0` |
/// | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_SET_CURRENT_SCHEDULING: u64 = 267;

/// Object-local method `thread_get_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=thread, rights=0x8` |
/// | Operation | 258 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=thread, rights=0x8` |
/// | User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_GET_SCHEDULING: u64 = 258;

/// Object-local method `thread_set_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=thread, rights=0x0` |
/// | Operation | 259 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=thread, rights=0x0` |
/// | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_THREAD_SET_SCHEDULING: u64 = 259;

/// Object-local method `virtual_cpu_get_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=virtual_cpu, rights=0x8` |
/// | Operation | 261 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=virtual_cpu, rights=0x8` |
/// | User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_GET_SCHEDULING: u64 = 261;

/// Object-local method `virtual_cpu_set_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=virtual_cpu, rights=0x0` |
/// | Operation | 262 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=virtual_cpu, rights=0x0` |
/// | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_VIRTUAL_CPU_SET_SCHEDULING: u64 = 262;

/// Object-local method `process_builder_set_scheduling`.
///
/// | Property | Contract |
/// | --- | --- |
/// | Receiver | `receiver: Borrow, kind=process_builder, rights=0x0` |
/// | Operation | 265 |
/// | Feature | `Core` |
/// | Arguments | `configuration: user_address`, `delegated_rights: rights` |
/// | Results | — |
/// | Capability effects | `receiver: Borrow, kind=process_builder, rights=0x0` |
/// | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
/// | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
/// | Audit | `Task` |
///
/// Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
///
/// Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
pub const HYPER_NATIVE_OP_PROCESS_BUILDER_SET_SCHEDULING: u64 = 265;

/// A decoded object method; enum discriminants are not a machine ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOperation {
    /// Decoded method; see [`HYPER_NATIVE_OP_ABI_QUERY`] for its contract.
    AbiQuery,
    /// Decoded method; see [`HYPER_NATIVE_OP_HANDLE_CLOSE`] for its contract.
    HandleClose,
    /// Decoded method; see [`HYPER_NATIVE_OP_HANDLE_DUPLICATE`] for its contract.
    HandleDuplicate,
    /// Decoded method; see [`HYPER_NATIVE_OP_HANDLE_REPLACE`] for its contract.
    HandleReplace,
    /// Decoded method; see [`HYPER_NATIVE_OP_HANDLE_GET_INFO`] for its contract.
    HandleGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_GET_BASIC_INFO`] for its contract.
    ObjectGetBasicInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_YIELD`] for its contract.
    ThreadYield,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_EXIT`] for its contract.
    ThreadExit,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_EXIT`] for its contract.
    ProcessExit,
    /// Decoded method; see [`HYPER_NATIVE_OP_EVENT_CREATE`] for its contract.
    EventCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_EVENT_SIGNAL`] for its contract.
    EventSignal,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_WAIT_ONE`] for its contract.
    ObjectWaitOne,
    /// Decoded method; see [`HYPER_NATIVE_OP_BYTE_CHANNEL_CREATE`] for its contract.
    ByteChannelCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_BYTE_CHANNEL_WRITE`] for its contract.
    ByteChannelWrite,
    /// Decoded method; see [`HYPER_NATIVE_OP_BYTE_CHANNEL_READ`] for its contract.
    ByteChannelRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_CONSOLE_READ`] for its contract.
    ConsoleRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_CONSOLE_WRITE`] for its contract.
    ConsoleWrite,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE`] for its contract.
    DirectoryOpenFile,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_READ_AT`] for its contract.
    FileReadAt,
    /// Decoded method; see [`HYPER_NATIVE_OP_CAPABILITY_CHANNEL_CREATE`] for its contract.
    CapabilityChannelCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_CAPABILITY_CHANNEL_TRY_SEND`] for its contract.
    CapabilityChannelTrySend,
    /// Decoded method; see [`HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE`] for its contract.
    CapabilityChannelReceive,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_CREATE`] for its contract.
    ProcessBuilderCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_SET_NAME`] for its contract.
    ProcessBuilderSetName,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_SET_DATA`] for its contract.
    ProcessBuilderSetData,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_SET_AFFINITY`] for its contract.
    ProcessBuilderSetAffinity,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_ADD_HANDLE`] for its contract.
    ProcessBuilderAddHandle,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_SEAL`] for its contract.
    ProcessBuilderSeal,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_START`] for its contract.
    ProcessBuilderStart,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_ABORT`] for its contract.
    ProcessBuilderAbort,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_REQUEST_STOP`] for its contract.
    ProcessRequestStop,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_WAIT_MANY`] for its contract.
    ObjectWaitMany,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_GET_INFO`] for its contract.
    ProcessGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_PROCESSES`] for its contract.
    TaskInspectorScanProcesses,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_THREADS`] for its contract.
    TaskInspectorScanThreads,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_PROCESS`] for its contract.
    TaskInspectorDeriveProcess,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_OBJECTS`] for its contract.
    ObjectInspectorScanObjects,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_HANDLES`] for its contract.
    ObjectInspectorScanHandles,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_PROCESS`] for its contract.
    ObjectInspectorDeriveProcess,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_TASK_GROUP`] for its contract.
    TaskInspectorDeriveTaskGroup,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_TASK_GROUP`] for its contract.
    ObjectInspectorDeriveTaskGroup,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN`] for its contract.
    TaskInspectorDeriveResourceDomain,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN`] for its contract.
    ObjectInspectorDeriveResourceDomain,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY`] for its contract.
    DirectoryOpenDirectory,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMO_CREATE`] for its contract.
    VmoCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMO_READ`] for its contract.
    VmoRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMO_WRITE`] for its contract.
    VmoWrite,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_ALLOCATE`] for its contract.
    VmarAllocate,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_MAP`] for its contract.
    VmarMap,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_PROTECT`] for its contract.
    VmarProtect,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_UNMAP`] for its contract.
    VmarUnmap,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_DESTROY`] for its contract.
    VmarDestroy,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_READ`] for its contract.
    DirectoryRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ`] for its contract.
    MemoryInspectorRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_START`] for its contract.
    CpuInspectorProfileStart,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_STOP`] for its contract.
    CpuInspectorProfileStop,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_READ`] for its contract.
    CpuInspectorProfileRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_START`] for its contract.
    CpuInspectorTraceStart,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_STOP`] for its contract.
    CpuInspectorTraceStop,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_READ`] for its contract.
    CpuInspectorTraceRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_CPU_INSPECTOR_READ`] for its contract.
    CpuInspectorRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_GET_INFO`] for its contract.
    FileGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_GET_INFO`] for its contract.
    DirectoryGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_CREATE`] for its contract.
    VirtualMachineCreationLeaseCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATE`] for its contract.
    VirtualMachineCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_MEMORY`] for its contract.
    PendingVirtualMachineSetMemory,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP`] for its contract.
    PendingVirtualMachineSetBootstrap,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SEAL`] for its contract.
    PendingVirtualMachineSeal,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_INSTALL`] for its contract.
    PendingVirtualMachineInstall,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ABORT`] for its contract.
    PendingVirtualMachineAbort,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_REQUEST_STOP`] for its contract.
    VirtualMachineRequestStop,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_INFO`] for its contract.
    VirtualMachineGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_GET_INFO`] for its contract.
    VirtualCpuGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_RESOURCE_DOMAIN_CREATE`] for its contract.
    ResourceDomainCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_GROUP_CREATE`] for its contract.
    TaskGroupCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_START`] for its contract.
    VirtualCpuStart,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL`] for its contract.
    PendingVirtualMachineSetVirtualSerial,
    /// Decoded method; see [`HYPER_NATIVE_OP_CLOCK_GET_MONOTONIC`] for its contract.
    ClockGetMonotonic,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_SERIAL_CREATE`] for its contract.
    VirtualSerialCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_SERIAL_REGISTER_OUTPUT`] for its contract.
    VirtualSerialRegisterOutput,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_SERIAL_WRITE`] for its contract.
    VirtualSerialWrite,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_CREATE`] for its contract.
    ThreadCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_START`] for its contract.
    ThreadStart,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_REQUEST_STOP`] for its contract.
    ThreadRequestStop,
    /// Decoded method; see [`HYPER_NATIVE_OP_ATOMIC_WAIT`] for its contract.
    AtomicWait,
    /// Decoded method; see [`HYPER_NATIVE_OP_ATOMIC_WAKE`] for its contract.
    AtomicWake,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_SLEEP`] for its contract.
    ThreadSleep,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_WRITE_AT`] for its contract.
    FileWriteAt,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_RESIZE`] for its contract.
    FileResize,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_CREATE_FILE`] for its contract.
    DirectoryCreateFile,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_CREATE_DIRECTORY`] for its contract.
    DirectoryCreateDirectory,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_REMOVE`] for its contract.
    DirectoryRemove,
    /// Decoded method; see [`HYPER_NATIVE_OP_WAIT_SET_CREATE`] for its contract.
    WaitSetCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_WAIT_SET_ADD`] for its contract.
    WaitSetAdd,
    /// Decoded method; see [`HYPER_NATIVE_OP_WAIT_SET_REARM`] for its contract.
    WaitSetRearm,
    /// Decoded method; see [`HYPER_NATIVE_OP_WAIT_SET_REMOVE`] for its contract.
    WaitSetRemove,
    /// Decoded method; see [`HYPER_NATIVE_OP_WAIT_SET_WAIT`] for its contract.
    WaitSetWait,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_GET_CURRENT_ID`] for its contract.
    ProcessGetCurrentId,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT`] for its contract.
    VirtualSerialAcknowledgeOutput,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_SCOPE_CREATE`] for its contract.
    DirectoryScopeCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_GET_METADATA`] for its contract.
    DirectoryGetMetadata,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_GET_METADATA`] for its contract.
    FileGetMetadata,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_GET_SELF_METADATA`] for its contract.
    DirectoryGetSelfMetadata,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_SET_METADATA`] for its contract.
    DirectorySetMetadata,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_SET_METADATA`] for its contract.
    FileSetMetadata,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_RENAME`] for its contract.
    DirectoryRename,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_LINK`] for its contract.
    DirectoryLink,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_SYMLINK`] for its contract.
    DirectorySymlink,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_READ_LINK`] for its contract.
    DirectoryReadLink,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_CANONICALIZE`] for its contract.
    DirectoryCanonicalize,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_REMOVE_IF`] for its contract.
    DirectoryRemoveIf,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW`] for its contract.
    DirectoryOpenDirectoryNofollow,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_SYNC`] for its contract.
    FileSync,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_LOCK`] for its contract.
    FileLock,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_UNLOCK`] for its contract.
    FileUnlock,
    /// Decoded method; see [`HYPER_NATIVE_OP_CLOCK_GET_REALTIME`] for its contract.
    ClockGetRealtime,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE_WITH_OPTIONS`] for its contract.
    DirectoryOpenFileWithOptions,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO`] for its contract.
    VirtualMachineCreationLeaseGetPlatformInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMO_CREATE_SNAPSHOT`] for its contract.
    VmoCreateSnapshot,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_MAP_PRIVATE`] for its contract.
    VmarMapPrivate,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_POWER_REQUEST`] for its contract.
    VirtualMachineGetPowerRequest,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST`] for its contract.
    VirtualMachineCompletePowerRequest,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_OPEN_VCPU`] for its contract.
    VirtualMachineOpenVcpu,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_MACHINE_REGISTER_MMIO`] for its contract.
    VirtualMachineRegisterMmio,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_GET_MMIO_REQUEST`] for its contract.
    VirtualCpuGetMmioRequest,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_COMPLETE_MMIO`] for its contract.
    VirtualCpuCompleteMmio,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MEMORY_CREATE`] for its contract.
    GuestMemoryCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_MAP_MEMORY`] for its contract.
    PendingVirtualMachineMapMemory,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMO_CREATE_CONTIGUOUS`] for its contract.
    VmoCreateContiguous,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_CLAIM`] for its contract.
    DeviceClaim,
    /// Decoded method; see [`HYPER_NATIVE_OP_PHYSICAL_DEVICE_INFO`] for its contract.
    PhysicalDeviceInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMO_GET_DMA_EXTENT`] for its contract.
    VmoGetDmaExtent,
    /// Decoded method; see [`HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE`] for its contract.
    PendingVirtualMachineAssignDevice,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MAILBOX_CREATE`] for its contract.
    GuestMailboxCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MAILBOX_SEND`] for its contract.
    GuestMailboxSend,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MAILBOX_RECEIVE`] for its contract.
    GuestMailboxReceive,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_NOTIFICATION_CREATE`] for its contract.
    GuestNotificationCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONTROL`] for its contract.
    GuestNotificationControl,
    /// Decoded method; see [`HYPER_NATIVE_OP_NATIVE_BLOCK_CREATE`] for its contract.
    NativeBlockCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_NATIVE_BLOCK_ACTIVATE`] for its contract.
    NativeBlockActivate,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILESYSTEM_MOUNT`] for its contract.
    FilesystemMount,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MAPPING_CREATE`] for its contract.
    GuestMappingCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MAPPING_RELEASE`] for its contract.
    GuestMappingRelease,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_CLAIM_MATCHING`] for its contract.
    DeviceClaimMatching,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_PROFILE_INFO`] for its contract.
    DeviceProfileInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_RESOURCE_INFO`] for its contract.
    DeviceResourceInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_FIRMWARE_READ`] for its contract.
    DeviceFirmwareRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_CLAIM_BUNDLE`] for its contract.
    DeviceClaimBundle,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_MMIO`] for its contract.
    DeviceMmio,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_IRQ_PENDING`] for its contract.
    DeviceIrqPending,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_IRQ_COMPLETE`] for its contract.
    DeviceIrqComplete,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_SET_AFFINITY`] for its contract.
    VirtualCpuSetAffinity,
    /// Decoded method; see [`HYPER_NATIVE_OP_SYSTEM_CONFIG`] for its contract.
    SystemConfig,
    /// Decoded method; see [`HYPER_NATIVE_OP_OBJECT_INSPECTOR_READ_DETAILS`] for its contract.
    ObjectInspectorReadDetails,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILESYSTEM_UNMOUNT`] for its contract.
    FilesystemUnmount,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILESYSTEM_MOUNT_RAMFS`] for its contract.
    FilesystemMountRamfs,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILESYSTEM_SWITCH_ROOT`] for its contract.
    FilesystemSwitchRoot,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILE_PROTECT`] for its contract.
    FileProtect,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_MAP_FILE`] for its contract.
    VmarMapFile,
    /// Decoded method; see [`HYPER_NATIVE_OP_DIRECTORY_BEGIN_READ`] for its contract.
    DirectoryBeginRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_DERIVE_OBSERVER`] for its contract.
    ProcessDeriveObserver,
    /// Decoded method; see [`HYPER_NATIVE_OP_TASK_GROUP_REQUEST_STOP`] for its contract.
    TaskGroupRequestStop,
    /// Decoded method; see [`HYPER_NATIVE_OP_CONSOLE_DISCARD_INPUT`] for its contract.
    ConsoleDiscardInput,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONFIGURE`] for its contract.
    GuestNotificationConfigure,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_NOTIFICATION_ACCESS`] for its contract.
    GuestNotificationAccess,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_LOCK_MEMORY`] for its contract.
    VmarLockMemory,
    /// Decoded method; see [`HYPER_NATIVE_OP_VMAR_UNLOCK_MEMORY`] for its contract.
    VmarUnlockMemory,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_SET_MEMORY_LOCK`] for its contract.
    ProcessBuilderSetMemoryLock,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILESYSTEM_BEGIN_ROOT_SWITCH`] for its contract.
    FilesystemBeginRootSwitch,
    /// Decoded method; see [`HYPER_NATIVE_OP_FILESYSTEM_FINISH_ROOT_SWITCH`] for its contract.
    FilesystemFinishRootSwitch,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MEMORY_TRANSFER`] for its contract.
    GuestMemoryTransfer,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_MEMORY_SCALAR`] for its contract.
    GuestMemoryScalar,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_INTERRUPT_CREATE`] for its contract.
    GuestInterruptCreate,
    /// Decoded method; see [`HYPER_NATIVE_OP_GUEST_INTERRUPT_SET_LEVEL`] for its contract.
    GuestInterruptSetLevel,
    /// Decoded method; see [`HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ_STATISTICS`] for its contract.
    MemoryInspectorReadStatistics,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_RETAIN`] for its contract.
    ProcessBuilderRetain,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_RETIREMENT_GET_INFO`] for its contract.
    ProcessRetirementGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_RETIREMENT_OPEN_VIEW`] for its contract.
    ProcessRetirementOpenView,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_RETIREMENT_RELEASE`] for its contract.
    ProcessRetirementRelease,
    /// Decoded method; see [`HYPER_NATIVE_OP_CRASH_VIEW_GET_INFO`] for its contract.
    CrashViewGetInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_CRASH_VIEW_READ`] for its contract.
    CrashViewRead,
    /// Decoded method; see [`HYPER_NATIVE_OP_CRASH_VIEW_THREAD`] for its contract.
    CrashViewThread,
    /// Decoded method; see [`HYPER_NATIVE_OP_CRASH_VIEW_MAPPING`] for its contract.
    CrashViewMapping,
    /// Decoded method; see [`HYPER_NATIVE_OP_CRASH_VIEW_HANDLE`] for its contract.
    CrashViewHandle,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_SET_CURRENT_NAME`] for its contract.
    ThreadSetCurrentName,
    /// Decoded method; see [`HYPER_NATIVE_OP_DEVICE_DMA_FAULT_INFO`] for its contract.
    DeviceDmaFaultInfo,
    /// Decoded method; see [`HYPER_NATIVE_OP_NATIVE_BLOCK_TRANSFER`] for its contract.
    NativeBlockTransfer,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_GET_CURRENT_SCHEDULING`] for its contract.
    ThreadGetCurrentScheduling,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_SET_CURRENT_SCHEDULING`] for its contract.
    ThreadSetCurrentScheduling,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_GET_SCHEDULING`] for its contract.
    ThreadGetScheduling,
    /// Decoded method; see [`HYPER_NATIVE_OP_THREAD_SET_SCHEDULING`] for its contract.
    ThreadSetScheduling,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_GET_SCHEDULING`] for its contract.
    VirtualCpuGetScheduling,
    /// Decoded method; see [`HYPER_NATIVE_OP_VIRTUAL_CPU_SET_SCHEDULING`] for its contract.
    VirtualCpuSetScheduling,
    /// Decoded method; see [`HYPER_NATIVE_OP_PROCESS_BUILDER_SET_SCHEDULING`] for its contract.
    ProcessBuilderSetScheduling,
}

#[rustfmt::skip]
impl NativeOperation {
    /// Decode a declared object kind and object-local operation. Unknown pairs return `None`.
    /// This checks vocabulary only, not handle validity or authority.
    pub const fn from_raw(object_kind: u32, operation: u64) -> Option<Self> {
        match object_kind {
            HYPER_NATIVE_OBJECT_EVENT => {},
            HYPER_NATIVE_OBJECT_BYTE_CHANNEL => {},
            HYPER_NATIVE_OBJECT_THREAD => {},
            HYPER_NATIVE_OBJECT_PROCESS => {},
            HYPER_NATIVE_OBJECT_TASK_GROUP => {},
            HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN => {},
            HYPER_NATIVE_OBJECT_TASK_FACTORY => {},
            HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY => {},
            HYPER_NATIVE_OBJECT_VMO => {},
            HYPER_NATIVE_OBJECT_VMAR => {},
            HYPER_NATIVE_OBJECT_CONSOLE => {},
            HYPER_NATIVE_OBJECT_DIRECTORY => {},
            HYPER_NATIVE_OBJECT_FILE => {},
            HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL => {},
            HYPER_NATIVE_OBJECT_PROCESS_BUILDER => {},
            HYPER_NATIVE_OBJECT_TASK_INSPECTOR => {},
            HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR => {},
            HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR => {},
            HYPER_NATIVE_OBJECT_CPU_INSPECTOR => {},
            HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY => {},
            HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE => {},
            HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE => {},
            HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE => {},
            HYPER_NATIVE_OBJECT_VIRTUAL_CPU => {},
            HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL => {},
            HYPER_NATIVE_OBJECT_WAIT_SET => {},
            HYPER_NATIVE_OBJECT_GUEST_MEMORY => {},
            HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY => {},
            HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE => {},
            HYPER_NATIVE_OBJECT_GUEST_MAILBOX => {},
            HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION => {},
            HYPER_NATIVE_OBJECT_NATIVE_BLOCK => {},
            HYPER_NATIVE_OBJECT_GUEST_MAPPING => {},
            HYPER_NATIVE_OBJECT_FILE_PROTECTION => {},
            HYPER_NATIVE_OBJECT_DIRECTORY_STREAM => {},
            HYPER_NATIVE_OBJECT_ROOT_TRANSITION => {},
            HYPER_NATIVE_OBJECT_GUEST_INTERRUPT => {},
            HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT => {},
            HYPER_NATIVE_OBJECT_CRASH_VIEW => {},
            HYPER_NATIVE_OBJECT_BACKEND_MEMORY_LEASE => {},
            HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT => {},
            HYPER_NATIVE_OBJECT_OBJECT_FACTORY => {},
            HYPER_NATIVE_OBJECT_SYSTEM_INFO => {},
            _ => return None,
        }
        match (object_kind, operation) {
            (HYPER_NATIVE_OBJECT_SYSTEM_INFO, HYPER_NATIVE_OP_ABI_QUERY) => Some(Self::AbiQuery),
            (_, HYPER_NATIVE_OP_HANDLE_CLOSE) => Some(Self::HandleClose),
            (_, HYPER_NATIVE_OP_HANDLE_DUPLICATE) => Some(Self::HandleDuplicate),
            (_, HYPER_NATIVE_OP_HANDLE_REPLACE) => Some(Self::HandleReplace),
            (_, HYPER_NATIVE_OP_HANDLE_GET_INFO) => Some(Self::HandleGetInfo),
            (_, HYPER_NATIVE_OP_OBJECT_GET_BASIC_INFO) => Some(Self::ObjectGetBasicInfo),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_YIELD) => Some(Self::ThreadYield),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_EXIT) => Some(Self::ThreadExit),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_PROCESS_EXIT) => Some(Self::ProcessExit),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_EVENT_CREATE) => Some(Self::EventCreate),
            (HYPER_NATIVE_OBJECT_EVENT, HYPER_NATIVE_OP_EVENT_SIGNAL) => Some(Self::EventSignal),
            (_, HYPER_NATIVE_OP_OBJECT_WAIT_ONE) => Some(Self::ObjectWaitOne),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_BYTE_CHANNEL_CREATE) => Some(Self::ByteChannelCreate),
            (HYPER_NATIVE_OBJECT_BYTE_CHANNEL, HYPER_NATIVE_OP_BYTE_CHANNEL_WRITE) => Some(Self::ByteChannelWrite),
            (HYPER_NATIVE_OBJECT_BYTE_CHANNEL, HYPER_NATIVE_OP_BYTE_CHANNEL_READ) => Some(Self::ByteChannelRead),
            (HYPER_NATIVE_OBJECT_CONSOLE, HYPER_NATIVE_OP_CONSOLE_READ) => Some(Self::ConsoleRead),
            (HYPER_NATIVE_OBJECT_CONSOLE, HYPER_NATIVE_OP_CONSOLE_WRITE) => Some(Self::ConsoleWrite),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE) => Some(Self::DirectoryOpenFile),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_READ_AT) => Some(Self::FileReadAt),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_CAPABILITY_CHANNEL_CREATE) => Some(Self::CapabilityChannelCreate),
            (HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL, HYPER_NATIVE_OP_CAPABILITY_CHANNEL_TRY_SEND) => Some(Self::CapabilityChannelTrySend),
            (HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL, HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE) => Some(Self::CapabilityChannelReceive),
            (HYPER_NATIVE_OBJECT_TASK_FACTORY, HYPER_NATIVE_OP_PROCESS_BUILDER_CREATE) => Some(Self::ProcessBuilderCreate),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_SET_NAME) => Some(Self::ProcessBuilderSetName),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_SET_DATA) => Some(Self::ProcessBuilderSetData),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_SET_AFFINITY) => Some(Self::ProcessBuilderSetAffinity),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_ADD_HANDLE) => Some(Self::ProcessBuilderAddHandle),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_SEAL) => Some(Self::ProcessBuilderSeal),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_START) => Some(Self::ProcessBuilderStart),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_ABORT) => Some(Self::ProcessBuilderAbort),
            (HYPER_NATIVE_OBJECT_PROCESS, HYPER_NATIVE_OP_PROCESS_REQUEST_STOP) => Some(Self::ProcessRequestStop),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_OBJECT_WAIT_MANY) => Some(Self::ObjectWaitMany),
            (HYPER_NATIVE_OBJECT_PROCESS, HYPER_NATIVE_OP_PROCESS_GET_INFO) => Some(Self::ProcessGetInfo),
            (HYPER_NATIVE_OBJECT_TASK_INSPECTOR, HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_PROCESSES) => Some(Self::TaskInspectorScanProcesses),
            (HYPER_NATIVE_OBJECT_TASK_INSPECTOR, HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_THREADS) => Some(Self::TaskInspectorScanThreads),
            (HYPER_NATIVE_OBJECT_TASK_INSPECTOR, HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_PROCESS) => Some(Self::TaskInspectorDeriveProcess),
            (HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_OBJECTS) => Some(Self::ObjectInspectorScanObjects),
            (HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_HANDLES) => Some(Self::ObjectInspectorScanHandles),
            (HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_PROCESS) => Some(Self::ObjectInspectorDeriveProcess),
            (HYPER_NATIVE_OBJECT_TASK_INSPECTOR, HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_TASK_GROUP) => Some(Self::TaskInspectorDeriveTaskGroup),
            (HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_TASK_GROUP) => Some(Self::ObjectInspectorDeriveTaskGroup),
            (HYPER_NATIVE_OBJECT_TASK_INSPECTOR, HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN) => Some(Self::TaskInspectorDeriveResourceDomain),
            (HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN) => Some(Self::ObjectInspectorDeriveResourceDomain),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY) => Some(Self::DirectoryOpenDirectory),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_VMO_CREATE) => Some(Self::VmoCreate),
            (HYPER_NATIVE_OBJECT_VMO, HYPER_NATIVE_OP_VMO_READ) => Some(Self::VmoRead),
            (HYPER_NATIVE_OBJECT_VMO, HYPER_NATIVE_OP_VMO_WRITE) => Some(Self::VmoWrite),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_ALLOCATE) => Some(Self::VmarAllocate),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_MAP) => Some(Self::VmarMap),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_PROTECT) => Some(Self::VmarProtect),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_UNMAP) => Some(Self::VmarUnmap),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_DESTROY) => Some(Self::VmarDestroy),
            (HYPER_NATIVE_OBJECT_DIRECTORY_STREAM, HYPER_NATIVE_OP_DIRECTORY_READ) => Some(Self::DirectoryRead),
            (HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR, HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ) => Some(Self::MemoryInspectorRead),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_START) => Some(Self::CpuInspectorProfileStart),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_STOP) => Some(Self::CpuInspectorProfileStop),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_READ) => Some(Self::CpuInspectorProfileRead),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_START) => Some(Self::CpuInspectorTraceStart),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_STOP) => Some(Self::CpuInspectorTraceStop),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_READ) => Some(Self::CpuInspectorTraceRead),
            (HYPER_NATIVE_OBJECT_CPU_INSPECTOR, HYPER_NATIVE_OP_CPU_INSPECTOR_READ) => Some(Self::CpuInspectorRead),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_GET_INFO) => Some(Self::FileGetInfo),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_GET_INFO) => Some(Self::DirectoryGetInfo),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY, HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_CREATE) => Some(Self::VirtualMachineCreationLeaseCreate),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATE) => Some(Self::VirtualMachineCreate),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_MEMORY) => Some(Self::PendingVirtualMachineSetMemory),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP) => Some(Self::PendingVirtualMachineSetBootstrap),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SEAL) => Some(Self::PendingVirtualMachineSeal),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_INSTALL) => Some(Self::PendingVirtualMachineInstall),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ABORT) => Some(Self::PendingVirtualMachineAbort),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_REQUEST_STOP) => Some(Self::VirtualMachineRequestStop),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_INFO) => Some(Self::VirtualMachineGetInfo),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_GET_INFO) => Some(Self::VirtualCpuGetInfo),
            (HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN, HYPER_NATIVE_OP_RESOURCE_DOMAIN_CREATE) => Some(Self::ResourceDomainCreate),
            (HYPER_NATIVE_OBJECT_TASK_FACTORY, HYPER_NATIVE_OP_TASK_GROUP_CREATE) => Some(Self::TaskGroupCreate),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_START) => Some(Self::VirtualCpuStart),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL) => Some(Self::PendingVirtualMachineSetVirtualSerial),
            (HYPER_NATIVE_OBJECT_SYSTEM_INFO, HYPER_NATIVE_OP_CLOCK_GET_MONOTONIC) => Some(Self::ClockGetMonotonic),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_VIRTUAL_SERIAL_CREATE) => Some(Self::VirtualSerialCreate),
            (HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL, HYPER_NATIVE_OP_VIRTUAL_SERIAL_REGISTER_OUTPUT) => Some(Self::VirtualSerialRegisterOutput),
            (HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL, HYPER_NATIVE_OP_VIRTUAL_SERIAL_WRITE) => Some(Self::VirtualSerialWrite),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_CREATE) => Some(Self::ThreadCreate),
            (HYPER_NATIVE_OBJECT_THREAD, HYPER_NATIVE_OP_THREAD_START) => Some(Self::ThreadStart),
            (HYPER_NATIVE_OBJECT_THREAD, HYPER_NATIVE_OP_THREAD_REQUEST_STOP) => Some(Self::ThreadRequestStop),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_ATOMIC_WAIT) => Some(Self::AtomicWait),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_ATOMIC_WAKE) => Some(Self::AtomicWake),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_SLEEP) => Some(Self::ThreadSleep),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_WRITE_AT) => Some(Self::FileWriteAt),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_RESIZE) => Some(Self::FileResize),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_CREATE_FILE) => Some(Self::DirectoryCreateFile),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_CREATE_DIRECTORY) => Some(Self::DirectoryCreateDirectory),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_REMOVE) => Some(Self::DirectoryRemove),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_WAIT_SET_CREATE) => Some(Self::WaitSetCreate),
            (HYPER_NATIVE_OBJECT_WAIT_SET, HYPER_NATIVE_OP_WAIT_SET_ADD) => Some(Self::WaitSetAdd),
            (HYPER_NATIVE_OBJECT_WAIT_SET, HYPER_NATIVE_OP_WAIT_SET_REARM) => Some(Self::WaitSetRearm),
            (HYPER_NATIVE_OBJECT_WAIT_SET, HYPER_NATIVE_OP_WAIT_SET_REMOVE) => Some(Self::WaitSetRemove),
            (HYPER_NATIVE_OBJECT_WAIT_SET, HYPER_NATIVE_OP_WAIT_SET_WAIT) => Some(Self::WaitSetWait),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_PROCESS_GET_CURRENT_ID) => Some(Self::ProcessGetCurrentId),
            (HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL, HYPER_NATIVE_OP_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT) => Some(Self::VirtualSerialAcknowledgeOutput),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_SCOPE_CREATE) => Some(Self::DirectoryScopeCreate),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_GET_METADATA) => Some(Self::DirectoryGetMetadata),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_GET_METADATA) => Some(Self::FileGetMetadata),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_GET_SELF_METADATA) => Some(Self::DirectoryGetSelfMetadata),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_SET_METADATA) => Some(Self::DirectorySetMetadata),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_SET_METADATA) => Some(Self::FileSetMetadata),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_RENAME) => Some(Self::DirectoryRename),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_LINK) => Some(Self::DirectoryLink),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_SYMLINK) => Some(Self::DirectorySymlink),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_READ_LINK) => Some(Self::DirectoryReadLink),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_CANONICALIZE) => Some(Self::DirectoryCanonicalize),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_REMOVE_IF) => Some(Self::DirectoryRemoveIf),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW) => Some(Self::DirectoryOpenDirectoryNofollow),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_SYNC) => Some(Self::FileSync),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_LOCK) => Some(Self::FileLock),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_UNLOCK) => Some(Self::FileUnlock),
            (HYPER_NATIVE_OBJECT_SYSTEM_INFO, HYPER_NATIVE_OP_CLOCK_GET_REALTIME) => Some(Self::ClockGetRealtime),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE_WITH_OPTIONS) => Some(Self::DirectoryOpenFileWithOptions),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO) => Some(Self::VirtualMachineCreationLeaseGetPlatformInfo),
            (HYPER_NATIVE_OBJECT_VMO, HYPER_NATIVE_OP_VMO_CREATE_SNAPSHOT) => Some(Self::VmoCreateSnapshot),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_MAP_PRIVATE) => Some(Self::VmarMapPrivate),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_POWER_REQUEST) => Some(Self::VirtualMachineGetPowerRequest),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST) => Some(Self::VirtualMachineCompletePowerRequest),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_OPEN_VCPU) => Some(Self::VirtualMachineOpenVcpu),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_VIRTUAL_MACHINE_REGISTER_MMIO) => Some(Self::VirtualMachineRegisterMmio),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_GET_MMIO_REQUEST) => Some(Self::VirtualCpuGetMmioRequest),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_COMPLETE_MMIO) => Some(Self::VirtualCpuCompleteMmio),
            (HYPER_NATIVE_OBJECT_VMO, HYPER_NATIVE_OP_GUEST_MEMORY_CREATE) => Some(Self::GuestMemoryCreate),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_MAP_MEMORY) => Some(Self::PendingVirtualMachineMapMemory),
            (HYPER_NATIVE_OBJECT_OBJECT_FACTORY, HYPER_NATIVE_OP_VMO_CREATE_CONTIGUOUS) => Some(Self::VmoCreateContiguous),
            (HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY, HYPER_NATIVE_OP_DEVICE_CLAIM) => Some(Self::DeviceClaim),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_PHYSICAL_DEVICE_INFO) => Some(Self::PhysicalDeviceInfo),
            (HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY, HYPER_NATIVE_OP_VMO_GET_DMA_EXTENT) => Some(Self::VmoGetDmaExtent),
            (HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE, HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE) => Some(Self::PendingVirtualMachineAssignDevice),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_GUEST_MAILBOX_CREATE) => Some(Self::GuestMailboxCreate),
            (HYPER_NATIVE_OBJECT_GUEST_MAILBOX, HYPER_NATIVE_OP_GUEST_MAILBOX_SEND) => Some(Self::GuestMailboxSend),
            (HYPER_NATIVE_OBJECT_GUEST_MAILBOX, HYPER_NATIVE_OP_GUEST_MAILBOX_RECEIVE) => Some(Self::GuestMailboxReceive),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_GUEST_NOTIFICATION_CREATE) => Some(Self::GuestNotificationCreate),
            (HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION, HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONTROL) => Some(Self::GuestNotificationControl),
            (HYPER_NATIVE_OBJECT_GUEST_MEMORY, HYPER_NATIVE_OP_NATIVE_BLOCK_CREATE) => Some(Self::NativeBlockCreate),
            (HYPER_NATIVE_OBJECT_NATIVE_BLOCK, HYPER_NATIVE_OP_NATIVE_BLOCK_ACTIVATE) => Some(Self::NativeBlockActivate),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_FILESYSTEM_MOUNT) => Some(Self::FilesystemMount),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_GUEST_MAPPING_CREATE) => Some(Self::GuestMappingCreate),
            (HYPER_NATIVE_OBJECT_GUEST_MAPPING, HYPER_NATIVE_OP_GUEST_MAPPING_RELEASE) => Some(Self::GuestMappingRelease),
            (HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY, HYPER_NATIVE_OP_DEVICE_CLAIM_MATCHING) => Some(Self::DeviceClaimMatching),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_DEVICE_PROFILE_INFO) => Some(Self::DeviceProfileInfo),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_DEVICE_RESOURCE_INFO) => Some(Self::DeviceResourceInfo),
            (HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY, HYPER_NATIVE_OP_DEVICE_FIRMWARE_READ) => Some(Self::DeviceFirmwareRead),
            (HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY, HYPER_NATIVE_OP_DEVICE_CLAIM_BUNDLE) => Some(Self::DeviceClaimBundle),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_DEVICE_MMIO) => Some(Self::DeviceMmio),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_DEVICE_IRQ_PENDING) => Some(Self::DeviceIrqPending),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_DEVICE_IRQ_COMPLETE) => Some(Self::DeviceIrqComplete),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_SET_AFFINITY) => Some(Self::VirtualCpuSetAffinity),
            (HYPER_NATIVE_OBJECT_SYSTEM_INFO, HYPER_NATIVE_OP_SYSTEM_CONFIG) => Some(Self::SystemConfig),
            (HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, HYPER_NATIVE_OP_OBJECT_INSPECTOR_READ_DETAILS) => Some(Self::ObjectInspectorReadDetails),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_FILESYSTEM_UNMOUNT) => Some(Self::FilesystemUnmount),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_FILESYSTEM_MOUNT_RAMFS) => Some(Self::FilesystemMountRamfs),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_FILESYSTEM_SWITCH_ROOT) => Some(Self::FilesystemSwitchRoot),
            (HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_FILE_PROTECT) => Some(Self::FileProtect),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_MAP_FILE) => Some(Self::VmarMapFile),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_DIRECTORY_BEGIN_READ) => Some(Self::DirectoryBeginRead),
            (HYPER_NATIVE_OBJECT_PROCESS, HYPER_NATIVE_OP_PROCESS_DERIVE_OBSERVER) => Some(Self::ProcessDeriveObserver),
            (HYPER_NATIVE_OBJECT_TASK_GROUP, HYPER_NATIVE_OP_TASK_GROUP_REQUEST_STOP) => Some(Self::TaskGroupRequestStop),
            (HYPER_NATIVE_OBJECT_CONSOLE, HYPER_NATIVE_OP_CONSOLE_DISCARD_INPUT) => Some(Self::ConsoleDiscardInput),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONFIGURE) => Some(Self::GuestNotificationConfigure),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_GUEST_NOTIFICATION_ACCESS) => Some(Self::GuestNotificationAccess),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_LOCK_MEMORY) => Some(Self::VmarLockMemory),
            (HYPER_NATIVE_OBJECT_VMAR, HYPER_NATIVE_OP_VMAR_UNLOCK_MEMORY) => Some(Self::VmarUnlockMemory),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_SET_MEMORY_LOCK) => Some(Self::ProcessBuilderSetMemoryLock),
            (HYPER_NATIVE_OBJECT_DIRECTORY, HYPER_NATIVE_OP_FILESYSTEM_BEGIN_ROOT_SWITCH) => Some(Self::FilesystemBeginRootSwitch),
            (HYPER_NATIVE_OBJECT_ROOT_TRANSITION, HYPER_NATIVE_OP_FILESYSTEM_FINISH_ROOT_SWITCH) => Some(Self::FilesystemFinishRootSwitch),
            (HYPER_NATIVE_OBJECT_GUEST_MEMORY, HYPER_NATIVE_OP_GUEST_MEMORY_TRANSFER) => Some(Self::GuestMemoryTransfer),
            (HYPER_NATIVE_OBJECT_GUEST_MEMORY, HYPER_NATIVE_OP_GUEST_MEMORY_SCALAR) => Some(Self::GuestMemoryScalar),
            (HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE, HYPER_NATIVE_OP_GUEST_INTERRUPT_CREATE) => Some(Self::GuestInterruptCreate),
            (HYPER_NATIVE_OBJECT_GUEST_INTERRUPT, HYPER_NATIVE_OP_GUEST_INTERRUPT_SET_LEVEL) => Some(Self::GuestInterruptSetLevel),
            (HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR, HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ_STATISTICS) => Some(Self::MemoryInspectorReadStatistics),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_RETAIN) => Some(Self::ProcessBuilderRetain),
            (HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT, HYPER_NATIVE_OP_PROCESS_RETIREMENT_GET_INFO) => Some(Self::ProcessRetirementGetInfo),
            (HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT, HYPER_NATIVE_OP_PROCESS_RETIREMENT_OPEN_VIEW) => Some(Self::ProcessRetirementOpenView),
            (HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT, HYPER_NATIVE_OP_PROCESS_RETIREMENT_RELEASE) => Some(Self::ProcessRetirementRelease),
            (HYPER_NATIVE_OBJECT_CRASH_VIEW, HYPER_NATIVE_OP_CRASH_VIEW_GET_INFO) => Some(Self::CrashViewGetInfo),
            (HYPER_NATIVE_OBJECT_CRASH_VIEW, HYPER_NATIVE_OP_CRASH_VIEW_READ) => Some(Self::CrashViewRead),
            (HYPER_NATIVE_OBJECT_CRASH_VIEW, HYPER_NATIVE_OP_CRASH_VIEW_THREAD) => Some(Self::CrashViewThread),
            (HYPER_NATIVE_OBJECT_CRASH_VIEW, HYPER_NATIVE_OP_CRASH_VIEW_MAPPING) => Some(Self::CrashViewMapping),
            (HYPER_NATIVE_OBJECT_CRASH_VIEW, HYPER_NATIVE_OP_CRASH_VIEW_HANDLE) => Some(Self::CrashViewHandle),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_SET_CURRENT_NAME) => Some(Self::ThreadSetCurrentName),
            (HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE, HYPER_NATIVE_OP_DEVICE_DMA_FAULT_INFO) => Some(Self::DeviceDmaFaultInfo),
            (HYPER_NATIVE_OBJECT_NATIVE_BLOCK, HYPER_NATIVE_OP_NATIVE_BLOCK_TRANSFER) => Some(Self::NativeBlockTransfer),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_GET_CURRENT_SCHEDULING) => Some(Self::ThreadGetCurrentScheduling),
            (HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT, HYPER_NATIVE_OP_THREAD_SET_CURRENT_SCHEDULING) => Some(Self::ThreadSetCurrentScheduling),
            (HYPER_NATIVE_OBJECT_THREAD, HYPER_NATIVE_OP_THREAD_GET_SCHEDULING) => Some(Self::ThreadGetScheduling),
            (HYPER_NATIVE_OBJECT_THREAD, HYPER_NATIVE_OP_THREAD_SET_SCHEDULING) => Some(Self::ThreadSetScheduling),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_GET_SCHEDULING) => Some(Self::VirtualCpuGetScheduling),
            (HYPER_NATIVE_OBJECT_VIRTUAL_CPU, HYPER_NATIVE_OP_VIRTUAL_CPU_SET_SCHEDULING) => Some(Self::VirtualCpuSetScheduling),
            (HYPER_NATIVE_OBJECT_PROCESS_BUILDER, HYPER_NATIVE_OP_PROCESS_BUILDER_SET_SCHEDULING) => Some(Self::ProcessBuilderSetScheduling),
            _ => None,
        }
    }

    /// Object-local wire operation number; combine with the receiver kind for dispatch.
    pub const fn operation(self) -> u64 {
        match self {
            Self::AbiQuery => HYPER_NATIVE_OP_ABI_QUERY,
            Self::HandleClose => HYPER_NATIVE_OP_HANDLE_CLOSE,
            Self::HandleDuplicate => HYPER_NATIVE_OP_HANDLE_DUPLICATE,
            Self::HandleReplace => HYPER_NATIVE_OP_HANDLE_REPLACE,
            Self::HandleGetInfo => HYPER_NATIVE_OP_HANDLE_GET_INFO,
            Self::ObjectGetBasicInfo => HYPER_NATIVE_OP_OBJECT_GET_BASIC_INFO,
            Self::ThreadYield => HYPER_NATIVE_OP_THREAD_YIELD,
            Self::ThreadExit => HYPER_NATIVE_OP_THREAD_EXIT,
            Self::ProcessExit => HYPER_NATIVE_OP_PROCESS_EXIT,
            Self::EventCreate => HYPER_NATIVE_OP_EVENT_CREATE,
            Self::EventSignal => HYPER_NATIVE_OP_EVENT_SIGNAL,
            Self::ObjectWaitOne => HYPER_NATIVE_OP_OBJECT_WAIT_ONE,
            Self::ByteChannelCreate => HYPER_NATIVE_OP_BYTE_CHANNEL_CREATE,
            Self::ByteChannelWrite => HYPER_NATIVE_OP_BYTE_CHANNEL_WRITE,
            Self::ByteChannelRead => HYPER_NATIVE_OP_BYTE_CHANNEL_READ,
            Self::ConsoleRead => HYPER_NATIVE_OP_CONSOLE_READ,
            Self::ConsoleWrite => HYPER_NATIVE_OP_CONSOLE_WRITE,
            Self::DirectoryOpenFile => HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE,
            Self::FileReadAt => HYPER_NATIVE_OP_FILE_READ_AT,
            Self::CapabilityChannelCreate => HYPER_NATIVE_OP_CAPABILITY_CHANNEL_CREATE,
            Self::CapabilityChannelTrySend => HYPER_NATIVE_OP_CAPABILITY_CHANNEL_TRY_SEND,
            Self::CapabilityChannelReceive => HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE,
            Self::ProcessBuilderCreate => HYPER_NATIVE_OP_PROCESS_BUILDER_CREATE,
            Self::ProcessBuilderSetName => HYPER_NATIVE_OP_PROCESS_BUILDER_SET_NAME,
            Self::ProcessBuilderSetData => HYPER_NATIVE_OP_PROCESS_BUILDER_SET_DATA,
            Self::ProcessBuilderSetAffinity => HYPER_NATIVE_OP_PROCESS_BUILDER_SET_AFFINITY,
            Self::ProcessBuilderAddHandle => HYPER_NATIVE_OP_PROCESS_BUILDER_ADD_HANDLE,
            Self::ProcessBuilderSeal => HYPER_NATIVE_OP_PROCESS_BUILDER_SEAL,
            Self::ProcessBuilderStart => HYPER_NATIVE_OP_PROCESS_BUILDER_START,
            Self::ProcessBuilderAbort => HYPER_NATIVE_OP_PROCESS_BUILDER_ABORT,
            Self::ProcessRequestStop => HYPER_NATIVE_OP_PROCESS_REQUEST_STOP,
            Self::ObjectWaitMany => HYPER_NATIVE_OP_OBJECT_WAIT_MANY,
            Self::ProcessGetInfo => HYPER_NATIVE_OP_PROCESS_GET_INFO,
            Self::TaskInspectorScanProcesses => HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_PROCESSES,
            Self::TaskInspectorScanThreads => HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_THREADS,
            Self::TaskInspectorDeriveProcess => HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_PROCESS,
            Self::ObjectInspectorScanObjects => HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_OBJECTS,
            Self::ObjectInspectorScanHandles => HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_HANDLES,
            Self::ObjectInspectorDeriveProcess => HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_PROCESS,
            Self::TaskInspectorDeriveTaskGroup => HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_TASK_GROUP,
            Self::ObjectInspectorDeriveTaskGroup => HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_TASK_GROUP,
            Self::TaskInspectorDeriveResourceDomain => HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
            Self::ObjectInspectorDeriveResourceDomain => HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
            Self::DirectoryOpenDirectory => HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY,
            Self::VmoCreate => HYPER_NATIVE_OP_VMO_CREATE,
            Self::VmoRead => HYPER_NATIVE_OP_VMO_READ,
            Self::VmoWrite => HYPER_NATIVE_OP_VMO_WRITE,
            Self::VmarAllocate => HYPER_NATIVE_OP_VMAR_ALLOCATE,
            Self::VmarMap => HYPER_NATIVE_OP_VMAR_MAP,
            Self::VmarProtect => HYPER_NATIVE_OP_VMAR_PROTECT,
            Self::VmarUnmap => HYPER_NATIVE_OP_VMAR_UNMAP,
            Self::VmarDestroy => HYPER_NATIVE_OP_VMAR_DESTROY,
            Self::DirectoryRead => HYPER_NATIVE_OP_DIRECTORY_READ,
            Self::MemoryInspectorRead => HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ,
            Self::CpuInspectorProfileStart => HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_START,
            Self::CpuInspectorProfileStop => HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_STOP,
            Self::CpuInspectorProfileRead => HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_READ,
            Self::CpuInspectorTraceStart => HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_START,
            Self::CpuInspectorTraceStop => HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_STOP,
            Self::CpuInspectorTraceRead => HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_READ,
            Self::CpuInspectorRead => HYPER_NATIVE_OP_CPU_INSPECTOR_READ,
            Self::FileGetInfo => HYPER_NATIVE_OP_FILE_GET_INFO,
            Self::DirectoryGetInfo => HYPER_NATIVE_OP_DIRECTORY_GET_INFO,
            Self::VirtualMachineCreationLeaseCreate => HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_CREATE,
            Self::VirtualMachineCreate => HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATE,
            Self::PendingVirtualMachineSetMemory => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_MEMORY,
            Self::PendingVirtualMachineSetBootstrap => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP,
            Self::PendingVirtualMachineSeal => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SEAL,
            Self::PendingVirtualMachineInstall => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_INSTALL,
            Self::PendingVirtualMachineAbort => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ABORT,
            Self::VirtualMachineRequestStop => HYPER_NATIVE_OP_VIRTUAL_MACHINE_REQUEST_STOP,
            Self::VirtualMachineGetInfo => HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_INFO,
            Self::VirtualCpuGetInfo => HYPER_NATIVE_OP_VIRTUAL_CPU_GET_INFO,
            Self::ResourceDomainCreate => HYPER_NATIVE_OP_RESOURCE_DOMAIN_CREATE,
            Self::TaskGroupCreate => HYPER_NATIVE_OP_TASK_GROUP_CREATE,
            Self::VirtualCpuStart => HYPER_NATIVE_OP_VIRTUAL_CPU_START,
            Self::PendingVirtualMachineSetVirtualSerial => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL,
            Self::ClockGetMonotonic => HYPER_NATIVE_OP_CLOCK_GET_MONOTONIC,
            Self::VirtualSerialCreate => HYPER_NATIVE_OP_VIRTUAL_SERIAL_CREATE,
            Self::VirtualSerialRegisterOutput => HYPER_NATIVE_OP_VIRTUAL_SERIAL_REGISTER_OUTPUT,
            Self::VirtualSerialWrite => HYPER_NATIVE_OP_VIRTUAL_SERIAL_WRITE,
            Self::ThreadCreate => HYPER_NATIVE_OP_THREAD_CREATE,
            Self::ThreadStart => HYPER_NATIVE_OP_THREAD_START,
            Self::ThreadRequestStop => HYPER_NATIVE_OP_THREAD_REQUEST_STOP,
            Self::AtomicWait => HYPER_NATIVE_OP_ATOMIC_WAIT,
            Self::AtomicWake => HYPER_NATIVE_OP_ATOMIC_WAKE,
            Self::ThreadSleep => HYPER_NATIVE_OP_THREAD_SLEEP,
            Self::FileWriteAt => HYPER_NATIVE_OP_FILE_WRITE_AT,
            Self::FileResize => HYPER_NATIVE_OP_FILE_RESIZE,
            Self::DirectoryCreateFile => HYPER_NATIVE_OP_DIRECTORY_CREATE_FILE,
            Self::DirectoryCreateDirectory => HYPER_NATIVE_OP_DIRECTORY_CREATE_DIRECTORY,
            Self::DirectoryRemove => HYPER_NATIVE_OP_DIRECTORY_REMOVE,
            Self::WaitSetCreate => HYPER_NATIVE_OP_WAIT_SET_CREATE,
            Self::WaitSetAdd => HYPER_NATIVE_OP_WAIT_SET_ADD,
            Self::WaitSetRearm => HYPER_NATIVE_OP_WAIT_SET_REARM,
            Self::WaitSetRemove => HYPER_NATIVE_OP_WAIT_SET_REMOVE,
            Self::WaitSetWait => HYPER_NATIVE_OP_WAIT_SET_WAIT,
            Self::ProcessGetCurrentId => HYPER_NATIVE_OP_PROCESS_GET_CURRENT_ID,
            Self::VirtualSerialAcknowledgeOutput => HYPER_NATIVE_OP_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT,
            Self::DirectoryScopeCreate => HYPER_NATIVE_OP_DIRECTORY_SCOPE_CREATE,
            Self::DirectoryGetMetadata => HYPER_NATIVE_OP_DIRECTORY_GET_METADATA,
            Self::FileGetMetadata => HYPER_NATIVE_OP_FILE_GET_METADATA,
            Self::DirectoryGetSelfMetadata => HYPER_NATIVE_OP_DIRECTORY_GET_SELF_METADATA,
            Self::DirectorySetMetadata => HYPER_NATIVE_OP_DIRECTORY_SET_METADATA,
            Self::FileSetMetadata => HYPER_NATIVE_OP_FILE_SET_METADATA,
            Self::DirectoryRename => HYPER_NATIVE_OP_DIRECTORY_RENAME,
            Self::DirectoryLink => HYPER_NATIVE_OP_DIRECTORY_LINK,
            Self::DirectorySymlink => HYPER_NATIVE_OP_DIRECTORY_SYMLINK,
            Self::DirectoryReadLink => HYPER_NATIVE_OP_DIRECTORY_READ_LINK,
            Self::DirectoryCanonicalize => HYPER_NATIVE_OP_DIRECTORY_CANONICALIZE,
            Self::DirectoryRemoveIf => HYPER_NATIVE_OP_DIRECTORY_REMOVE_IF,
            Self::DirectoryOpenDirectoryNofollow => HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW,
            Self::FileSync => HYPER_NATIVE_OP_FILE_SYNC,
            Self::FileLock => HYPER_NATIVE_OP_FILE_LOCK,
            Self::FileUnlock => HYPER_NATIVE_OP_FILE_UNLOCK,
            Self::ClockGetRealtime => HYPER_NATIVE_OP_CLOCK_GET_REALTIME,
            Self::DirectoryOpenFileWithOptions => HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE_WITH_OPTIONS,
            Self::VirtualMachineCreationLeaseGetPlatformInfo => HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO,
            Self::VmoCreateSnapshot => HYPER_NATIVE_OP_VMO_CREATE_SNAPSHOT,
            Self::VmarMapPrivate => HYPER_NATIVE_OP_VMAR_MAP_PRIVATE,
            Self::VirtualMachineGetPowerRequest => HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_POWER_REQUEST,
            Self::VirtualMachineCompletePowerRequest => HYPER_NATIVE_OP_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST,
            Self::VirtualMachineOpenVcpu => HYPER_NATIVE_OP_VIRTUAL_MACHINE_OPEN_VCPU,
            Self::VirtualMachineRegisterMmio => HYPER_NATIVE_OP_VIRTUAL_MACHINE_REGISTER_MMIO,
            Self::VirtualCpuGetMmioRequest => HYPER_NATIVE_OP_VIRTUAL_CPU_GET_MMIO_REQUEST,
            Self::VirtualCpuCompleteMmio => HYPER_NATIVE_OP_VIRTUAL_CPU_COMPLETE_MMIO,
            Self::GuestMemoryCreate => HYPER_NATIVE_OP_GUEST_MEMORY_CREATE,
            Self::PendingVirtualMachineMapMemory => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_MAP_MEMORY,
            Self::VmoCreateContiguous => HYPER_NATIVE_OP_VMO_CREATE_CONTIGUOUS,
            Self::DeviceClaim => HYPER_NATIVE_OP_DEVICE_CLAIM,
            Self::PhysicalDeviceInfo => HYPER_NATIVE_OP_PHYSICAL_DEVICE_INFO,
            Self::VmoGetDmaExtent => HYPER_NATIVE_OP_VMO_GET_DMA_EXTENT,
            Self::PendingVirtualMachineAssignDevice => HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE,
            Self::GuestMailboxCreate => HYPER_NATIVE_OP_GUEST_MAILBOX_CREATE,
            Self::GuestMailboxSend => HYPER_NATIVE_OP_GUEST_MAILBOX_SEND,
            Self::GuestMailboxReceive => HYPER_NATIVE_OP_GUEST_MAILBOX_RECEIVE,
            Self::GuestNotificationCreate => HYPER_NATIVE_OP_GUEST_NOTIFICATION_CREATE,
            Self::GuestNotificationControl => HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONTROL,
            Self::NativeBlockCreate => HYPER_NATIVE_OP_NATIVE_BLOCK_CREATE,
            Self::NativeBlockActivate => HYPER_NATIVE_OP_NATIVE_BLOCK_ACTIVATE,
            Self::FilesystemMount => HYPER_NATIVE_OP_FILESYSTEM_MOUNT,
            Self::GuestMappingCreate => HYPER_NATIVE_OP_GUEST_MAPPING_CREATE,
            Self::GuestMappingRelease => HYPER_NATIVE_OP_GUEST_MAPPING_RELEASE,
            Self::DeviceClaimMatching => HYPER_NATIVE_OP_DEVICE_CLAIM_MATCHING,
            Self::DeviceProfileInfo => HYPER_NATIVE_OP_DEVICE_PROFILE_INFO,
            Self::DeviceResourceInfo => HYPER_NATIVE_OP_DEVICE_RESOURCE_INFO,
            Self::DeviceFirmwareRead => HYPER_NATIVE_OP_DEVICE_FIRMWARE_READ,
            Self::DeviceClaimBundle => HYPER_NATIVE_OP_DEVICE_CLAIM_BUNDLE,
            Self::DeviceMmio => HYPER_NATIVE_OP_DEVICE_MMIO,
            Self::DeviceIrqPending => HYPER_NATIVE_OP_DEVICE_IRQ_PENDING,
            Self::DeviceIrqComplete => HYPER_NATIVE_OP_DEVICE_IRQ_COMPLETE,
            Self::VirtualCpuSetAffinity => HYPER_NATIVE_OP_VIRTUAL_CPU_SET_AFFINITY,
            Self::SystemConfig => HYPER_NATIVE_OP_SYSTEM_CONFIG,
            Self::ObjectInspectorReadDetails => HYPER_NATIVE_OP_OBJECT_INSPECTOR_READ_DETAILS,
            Self::FilesystemUnmount => HYPER_NATIVE_OP_FILESYSTEM_UNMOUNT,
            Self::FilesystemMountRamfs => HYPER_NATIVE_OP_FILESYSTEM_MOUNT_RAMFS,
            Self::FilesystemSwitchRoot => HYPER_NATIVE_OP_FILESYSTEM_SWITCH_ROOT,
            Self::FileProtect => HYPER_NATIVE_OP_FILE_PROTECT,
            Self::VmarMapFile => HYPER_NATIVE_OP_VMAR_MAP_FILE,
            Self::DirectoryBeginRead => HYPER_NATIVE_OP_DIRECTORY_BEGIN_READ,
            Self::ProcessDeriveObserver => HYPER_NATIVE_OP_PROCESS_DERIVE_OBSERVER,
            Self::TaskGroupRequestStop => HYPER_NATIVE_OP_TASK_GROUP_REQUEST_STOP,
            Self::ConsoleDiscardInput => HYPER_NATIVE_OP_CONSOLE_DISCARD_INPUT,
            Self::GuestNotificationConfigure => HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONFIGURE,
            Self::GuestNotificationAccess => HYPER_NATIVE_OP_GUEST_NOTIFICATION_ACCESS,
            Self::VmarLockMemory => HYPER_NATIVE_OP_VMAR_LOCK_MEMORY,
            Self::VmarUnlockMemory => HYPER_NATIVE_OP_VMAR_UNLOCK_MEMORY,
            Self::ProcessBuilderSetMemoryLock => HYPER_NATIVE_OP_PROCESS_BUILDER_SET_MEMORY_LOCK,
            Self::FilesystemBeginRootSwitch => HYPER_NATIVE_OP_FILESYSTEM_BEGIN_ROOT_SWITCH,
            Self::FilesystemFinishRootSwitch => HYPER_NATIVE_OP_FILESYSTEM_FINISH_ROOT_SWITCH,
            Self::GuestMemoryTransfer => HYPER_NATIVE_OP_GUEST_MEMORY_TRANSFER,
            Self::GuestMemoryScalar => HYPER_NATIVE_OP_GUEST_MEMORY_SCALAR,
            Self::GuestInterruptCreate => HYPER_NATIVE_OP_GUEST_INTERRUPT_CREATE,
            Self::GuestInterruptSetLevel => HYPER_NATIVE_OP_GUEST_INTERRUPT_SET_LEVEL,
            Self::MemoryInspectorReadStatistics => HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ_STATISTICS,
            Self::ProcessBuilderRetain => HYPER_NATIVE_OP_PROCESS_BUILDER_RETAIN,
            Self::ProcessRetirementGetInfo => HYPER_NATIVE_OP_PROCESS_RETIREMENT_GET_INFO,
            Self::ProcessRetirementOpenView => HYPER_NATIVE_OP_PROCESS_RETIREMENT_OPEN_VIEW,
            Self::ProcessRetirementRelease => HYPER_NATIVE_OP_PROCESS_RETIREMENT_RELEASE,
            Self::CrashViewGetInfo => HYPER_NATIVE_OP_CRASH_VIEW_GET_INFO,
            Self::CrashViewRead => HYPER_NATIVE_OP_CRASH_VIEW_READ,
            Self::CrashViewThread => HYPER_NATIVE_OP_CRASH_VIEW_THREAD,
            Self::CrashViewMapping => HYPER_NATIVE_OP_CRASH_VIEW_MAPPING,
            Self::CrashViewHandle => HYPER_NATIVE_OP_CRASH_VIEW_HANDLE,
            Self::ThreadSetCurrentName => HYPER_NATIVE_OP_THREAD_SET_CURRENT_NAME,
            Self::DeviceDmaFaultInfo => HYPER_NATIVE_OP_DEVICE_DMA_FAULT_INFO,
            Self::NativeBlockTransfer => HYPER_NATIVE_OP_NATIVE_BLOCK_TRANSFER,
            Self::ThreadGetCurrentScheduling => HYPER_NATIVE_OP_THREAD_GET_CURRENT_SCHEDULING,
            Self::ThreadSetCurrentScheduling => HYPER_NATIVE_OP_THREAD_SET_CURRENT_SCHEDULING,
            Self::ThreadGetScheduling => HYPER_NATIVE_OP_THREAD_GET_SCHEDULING,
            Self::ThreadSetScheduling => HYPER_NATIVE_OP_THREAD_SET_SCHEDULING,
            Self::VirtualCpuGetScheduling => HYPER_NATIVE_OP_VIRTUAL_CPU_GET_SCHEDULING,
            Self::VirtualCpuSetScheduling => HYPER_NATIVE_OP_VIRTUAL_CPU_SET_SCHEDULING,
            Self::ProcessBuilderSetScheduling => HYPER_NATIVE_OP_PROCESS_BUILDER_SET_SCHEDULING,
        }
    }

    /// Required receiver kind, or `None` for a common handle method.
    pub const fn receiver_kind(self) -> Option<u32> {
        match self {
            Self::AbiQuery => Some(HYPER_NATIVE_OBJECT_SYSTEM_INFO),
            Self::HandleClose => None,
            Self::HandleDuplicate => None,
            Self::HandleReplace => None,
            Self::HandleGetInfo => None,
            Self::ObjectGetBasicInfo => None,
            Self::ThreadYield => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ThreadExit => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ProcessExit => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::EventCreate => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::EventSignal => Some(HYPER_NATIVE_OBJECT_EVENT),
            Self::ObjectWaitOne => None,
            Self::ByteChannelCreate => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::ByteChannelWrite => Some(HYPER_NATIVE_OBJECT_BYTE_CHANNEL),
            Self::ByteChannelRead => Some(HYPER_NATIVE_OBJECT_BYTE_CHANNEL),
            Self::ConsoleRead => Some(HYPER_NATIVE_OBJECT_CONSOLE),
            Self::ConsoleWrite => Some(HYPER_NATIVE_OBJECT_CONSOLE),
            Self::DirectoryOpenFile => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FileReadAt => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::CapabilityChannelCreate => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::CapabilityChannelTrySend => Some(HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL),
            Self::CapabilityChannelReceive => Some(HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL),
            Self::ProcessBuilderCreate => Some(HYPER_NATIVE_OBJECT_TASK_FACTORY),
            Self::ProcessBuilderSetName => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessBuilderSetData => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessBuilderSetAffinity => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessBuilderAddHandle => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessBuilderSeal => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessBuilderStart => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessBuilderAbort => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessRequestStop => Some(HYPER_NATIVE_OBJECT_PROCESS),
            Self::ObjectWaitMany => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ProcessGetInfo => Some(HYPER_NATIVE_OBJECT_PROCESS),
            Self::TaskInspectorScanProcesses => Some(HYPER_NATIVE_OBJECT_TASK_INSPECTOR),
            Self::TaskInspectorScanThreads => Some(HYPER_NATIVE_OBJECT_TASK_INSPECTOR),
            Self::TaskInspectorDeriveProcess => Some(HYPER_NATIVE_OBJECT_TASK_INSPECTOR),
            Self::ObjectInspectorScanObjects => Some(HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR),
            Self::ObjectInspectorScanHandles => Some(HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR),
            Self::ObjectInspectorDeriveProcess => Some(HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR),
            Self::TaskInspectorDeriveTaskGroup => Some(HYPER_NATIVE_OBJECT_TASK_INSPECTOR),
            Self::ObjectInspectorDeriveTaskGroup => Some(HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR),
            Self::TaskInspectorDeriveResourceDomain => Some(HYPER_NATIVE_OBJECT_TASK_INSPECTOR),
            Self::ObjectInspectorDeriveResourceDomain => Some(HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR),
            Self::DirectoryOpenDirectory => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::VmoCreate => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::VmoRead => Some(HYPER_NATIVE_OBJECT_VMO),
            Self::VmoWrite => Some(HYPER_NATIVE_OBJECT_VMO),
            Self::VmarAllocate => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::VmarMap => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::VmarProtect => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::VmarUnmap => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::VmarDestroy => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::DirectoryRead => Some(HYPER_NATIVE_OBJECT_DIRECTORY_STREAM),
            Self::MemoryInspectorRead => Some(HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR),
            Self::CpuInspectorProfileStart => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::CpuInspectorProfileStop => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::CpuInspectorProfileRead => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::CpuInspectorTraceStart => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::CpuInspectorTraceStop => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::CpuInspectorTraceRead => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::CpuInspectorRead => Some(HYPER_NATIVE_OBJECT_CPU_INSPECTOR),
            Self::FileGetInfo => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::DirectoryGetInfo => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::VirtualMachineCreationLeaseCreate => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY),
            Self::VirtualMachineCreate => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE),
            Self::PendingVirtualMachineSetMemory => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::PendingVirtualMachineSetBootstrap => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::PendingVirtualMachineSeal => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::PendingVirtualMachineInstall => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::PendingVirtualMachineAbort => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::VirtualMachineRequestStop => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VirtualMachineGetInfo => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VirtualCpuGetInfo => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::ResourceDomainCreate => Some(HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN),
            Self::TaskGroupCreate => Some(HYPER_NATIVE_OBJECT_TASK_FACTORY),
            Self::VirtualCpuStart => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::PendingVirtualMachineSetVirtualSerial => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::ClockGetMonotonic => Some(HYPER_NATIVE_OBJECT_SYSTEM_INFO),
            Self::VirtualSerialCreate => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::VirtualSerialRegisterOutput => Some(HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL),
            Self::VirtualSerialWrite => Some(HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL),
            Self::ThreadCreate => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ThreadStart => Some(HYPER_NATIVE_OBJECT_THREAD),
            Self::ThreadRequestStop => Some(HYPER_NATIVE_OBJECT_THREAD),
            Self::AtomicWait => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::AtomicWake => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ThreadSleep => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::FileWriteAt => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::FileResize => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::DirectoryCreateFile => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryCreateDirectory => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryRemove => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::WaitSetCreate => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::WaitSetAdd => Some(HYPER_NATIVE_OBJECT_WAIT_SET),
            Self::WaitSetRearm => Some(HYPER_NATIVE_OBJECT_WAIT_SET),
            Self::WaitSetRemove => Some(HYPER_NATIVE_OBJECT_WAIT_SET),
            Self::WaitSetWait => Some(HYPER_NATIVE_OBJECT_WAIT_SET),
            Self::ProcessGetCurrentId => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::VirtualSerialAcknowledgeOutput => Some(HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL),
            Self::DirectoryScopeCreate => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryGetMetadata => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FileGetMetadata => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::DirectoryGetSelfMetadata => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectorySetMetadata => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FileSetMetadata => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::DirectoryRename => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryLink => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectorySymlink => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryReadLink => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryCanonicalize => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryRemoveIf => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::DirectoryOpenDirectoryNofollow => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FileSync => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::FileLock => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::FileUnlock => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::ClockGetRealtime => Some(HYPER_NATIVE_OBJECT_SYSTEM_INFO),
            Self::DirectoryOpenFileWithOptions => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::VirtualMachineCreationLeaseGetPlatformInfo => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE),
            Self::VmoCreateSnapshot => Some(HYPER_NATIVE_OBJECT_VMO),
            Self::VmarMapPrivate => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::VirtualMachineGetPowerRequest => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VirtualMachineCompletePowerRequest => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VirtualMachineOpenVcpu => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VirtualMachineRegisterMmio => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VirtualCpuGetMmioRequest => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::VirtualCpuCompleteMmio => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::GuestMemoryCreate => Some(HYPER_NATIVE_OBJECT_VMO),
            Self::PendingVirtualMachineMapMemory => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::VmoCreateContiguous => Some(HYPER_NATIVE_OBJECT_OBJECT_FACTORY),
            Self::DeviceClaim => Some(HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY),
            Self::PhysicalDeviceInfo => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::VmoGetDmaExtent => Some(HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY),
            Self::PendingVirtualMachineAssignDevice => Some(HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE),
            Self::GuestMailboxCreate => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::GuestMailboxSend => Some(HYPER_NATIVE_OBJECT_GUEST_MAILBOX),
            Self::GuestMailboxReceive => Some(HYPER_NATIVE_OBJECT_GUEST_MAILBOX),
            Self::GuestNotificationCreate => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::GuestNotificationControl => Some(HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION),
            Self::NativeBlockCreate => Some(HYPER_NATIVE_OBJECT_GUEST_MEMORY),
            Self::NativeBlockActivate => Some(HYPER_NATIVE_OBJECT_NATIVE_BLOCK),
            Self::FilesystemMount => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::GuestMappingCreate => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::GuestMappingRelease => Some(HYPER_NATIVE_OBJECT_GUEST_MAPPING),
            Self::DeviceClaimMatching => Some(HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY),
            Self::DeviceProfileInfo => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::DeviceResourceInfo => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::DeviceFirmwareRead => Some(HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY),
            Self::DeviceClaimBundle => Some(HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY),
            Self::DeviceMmio => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::DeviceIrqPending => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::DeviceIrqComplete => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::VirtualCpuSetAffinity => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::SystemConfig => Some(HYPER_NATIVE_OBJECT_SYSTEM_INFO),
            Self::ObjectInspectorReadDetails => Some(HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR),
            Self::FilesystemUnmount => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FilesystemMountRamfs => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FilesystemSwitchRoot => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FileProtect => Some(HYPER_NATIVE_OBJECT_FILE),
            Self::VmarMapFile => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::DirectoryBeginRead => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::ProcessDeriveObserver => Some(HYPER_NATIVE_OBJECT_PROCESS),
            Self::TaskGroupRequestStop => Some(HYPER_NATIVE_OBJECT_TASK_GROUP),
            Self::ConsoleDiscardInput => Some(HYPER_NATIVE_OBJECT_CONSOLE),
            Self::GuestNotificationConfigure => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::GuestNotificationAccess => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::VmarLockMemory => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::VmarUnlockMemory => Some(HYPER_NATIVE_OBJECT_VMAR),
            Self::ProcessBuilderSetMemoryLock => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::FilesystemBeginRootSwitch => Some(HYPER_NATIVE_OBJECT_DIRECTORY),
            Self::FilesystemFinishRootSwitch => Some(HYPER_NATIVE_OBJECT_ROOT_TRANSITION),
            Self::GuestMemoryTransfer => Some(HYPER_NATIVE_OBJECT_GUEST_MEMORY),
            Self::GuestMemoryScalar => Some(HYPER_NATIVE_OBJECT_GUEST_MEMORY),
            Self::GuestInterruptCreate => Some(HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE),
            Self::GuestInterruptSetLevel => Some(HYPER_NATIVE_OBJECT_GUEST_INTERRUPT),
            Self::MemoryInspectorReadStatistics => Some(HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR),
            Self::ProcessBuilderRetain => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
            Self::ProcessRetirementGetInfo => Some(HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT),
            Self::ProcessRetirementOpenView => Some(HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT),
            Self::ProcessRetirementRelease => Some(HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT),
            Self::CrashViewGetInfo => Some(HYPER_NATIVE_OBJECT_CRASH_VIEW),
            Self::CrashViewRead => Some(HYPER_NATIVE_OBJECT_CRASH_VIEW),
            Self::CrashViewThread => Some(HYPER_NATIVE_OBJECT_CRASH_VIEW),
            Self::CrashViewMapping => Some(HYPER_NATIVE_OBJECT_CRASH_VIEW),
            Self::CrashViewHandle => Some(HYPER_NATIVE_OBJECT_CRASH_VIEW),
            Self::ThreadSetCurrentName => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::DeviceDmaFaultInfo => Some(HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE),
            Self::NativeBlockTransfer => Some(HYPER_NATIVE_OBJECT_NATIVE_BLOCK),
            Self::ThreadGetCurrentScheduling => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ThreadSetCurrentScheduling => Some(HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT),
            Self::ThreadGetScheduling => Some(HYPER_NATIVE_OBJECT_THREAD),
            Self::ThreadSetScheduling => Some(HYPER_NATIVE_OBJECT_THREAD),
            Self::VirtualCpuGetScheduling => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::VirtualCpuSetScheduling => Some(HYPER_NATIVE_OBJECT_VIRTUAL_CPU),
            Self::ProcessBuilderSetScheduling => Some(HYPER_NATIVE_OBJECT_PROCESS_BUILDER),
        }
    }

    /// Business argument count, excluding the receiver and operation selectors.
    pub const fn argument_count(self) -> usize {
        match self {
            Self::AbiQuery => 0,
            Self::HandleClose => 0,
            Self::HandleDuplicate => 1,
            Self::HandleReplace => 1,
            Self::HandleGetInfo => 2,
            Self::ObjectGetBasicInfo => 2,
            Self::ThreadYield => 0,
            Self::ThreadExit => 1,
            Self::ProcessExit => 1,
            Self::EventCreate => 1,
            Self::EventSignal => 2,
            Self::ObjectWaitOne => 2,
            Self::ByteChannelCreate => 1,
            Self::ByteChannelWrite => 3,
            Self::ByteChannelRead => 3,
            Self::ConsoleRead => 3,
            Self::ConsoleWrite => 3,
            Self::DirectoryOpenFile => 4,
            Self::FileReadAt => 4,
            Self::CapabilityChannelCreate => 1,
            Self::CapabilityChannelTrySend => 5,
            Self::CapabilityChannelReceive => 5,
            Self::ProcessBuilderCreate => 4,
            Self::ProcessBuilderSetName => 2,
            Self::ProcessBuilderSetData => 2,
            Self::ProcessBuilderSetAffinity => 2,
            Self::ProcessBuilderAddHandle => 5,
            Self::ProcessBuilderSeal => 0,
            Self::ProcessBuilderStart => 0,
            Self::ProcessBuilderAbort => 0,
            Self::ProcessRequestStop => 0,
            Self::ObjectWaitMany => 3,
            Self::ProcessGetInfo => 2,
            Self::TaskInspectorScanProcesses => 3,
            Self::TaskInspectorScanThreads => 3,
            Self::TaskInspectorDeriveProcess => 1,
            Self::ObjectInspectorScanObjects => 3,
            Self::ObjectInspectorScanHandles => 4,
            Self::ObjectInspectorDeriveProcess => 1,
            Self::TaskInspectorDeriveTaskGroup => 1,
            Self::ObjectInspectorDeriveTaskGroup => 1,
            Self::TaskInspectorDeriveResourceDomain => 1,
            Self::ObjectInspectorDeriveResourceDomain => 1,
            Self::DirectoryOpenDirectory => 4,
            Self::VmoCreate => 1,
            Self::VmoRead => 3,
            Self::VmoWrite => 3,
            Self::VmarAllocate => 3,
            Self::VmarMap => 5,
            Self::VmarProtect => 3,
            Self::VmarUnmap => 2,
            Self::VmarDestroy => 0,
            Self::DirectoryRead => 4,
            Self::MemoryInspectorRead => 2,
            Self::CpuInspectorProfileStart => 2,
            Self::CpuInspectorProfileStop => 1,
            Self::CpuInspectorProfileRead => 5,
            Self::CpuInspectorTraceStart => 1,
            Self::CpuInspectorTraceStop => 1,
            Self::CpuInspectorTraceRead => 5,
            Self::CpuInspectorRead => 2,
            Self::FileGetInfo => 2,
            Self::DirectoryGetInfo => 2,
            Self::VirtualMachineCreationLeaseCreate => 1,
            Self::VirtualMachineCreate => 2,
            Self::PendingVirtualMachineSetMemory => 1,
            Self::PendingVirtualMachineSetBootstrap => 2,
            Self::PendingVirtualMachineSeal => 0,
            Self::PendingVirtualMachineInstall => 0,
            Self::PendingVirtualMachineAbort => 0,
            Self::VirtualMachineRequestStop => 0,
            Self::VirtualMachineGetInfo => 2,
            Self::VirtualCpuGetInfo => 2,
            Self::ResourceDomainCreate => 2,
            Self::TaskGroupCreate => 1,
            Self::VirtualCpuStart => 0,
            Self::PendingVirtualMachineSetVirtualSerial => 1,
            Self::ClockGetMonotonic => 0,
            Self::VirtualSerialCreate => 0,
            Self::VirtualSerialRegisterOutput => 1,
            Self::VirtualSerialWrite => 2,
            Self::ThreadCreate => 6,
            Self::ThreadStart => 0,
            Self::ThreadRequestStop => 0,
            Self::AtomicWait => 3,
            Self::AtomicWake => 2,
            Self::ThreadSleep => 1,
            Self::FileWriteAt => 4,
            Self::FileResize => 1,
            Self::DirectoryCreateFile => 4,
            Self::DirectoryCreateDirectory => 3,
            Self::DirectoryRemove => 3,
            Self::WaitSetCreate => 1,
            Self::WaitSetAdd => 2,
            Self::WaitSetRearm => 1,
            Self::WaitSetRemove => 1,
            Self::WaitSetWait => 3,
            Self::ProcessGetCurrentId => 0,
            Self::VirtualSerialAcknowledgeOutput => 1,
            Self::DirectoryScopeCreate => 2,
            Self::DirectoryGetMetadata => 5,
            Self::FileGetMetadata => 2,
            Self::DirectoryGetSelfMetadata => 2,
            Self::DirectorySetMetadata => 5,
            Self::FileSetMetadata => 2,
            Self::DirectoryRename => 5,
            Self::DirectoryLink => 5,
            Self::DirectorySymlink => 4,
            Self::DirectoryReadLink => 4,
            Self::DirectoryCanonicalize => 4,
            Self::DirectoryRemoveIf => 4,
            Self::DirectoryOpenDirectoryNofollow => 3,
            Self::FileSync => 1,
            Self::FileLock => 2,
            Self::FileUnlock => 0,
            Self::ClockGetRealtime => 0,
            Self::DirectoryOpenFileWithOptions => 5,
            Self::VirtualMachineCreationLeaseGetPlatformInfo => 3,
            Self::VmoCreateSnapshot => 0,
            Self::VmarMapPrivate => 3,
            Self::VirtualMachineGetPowerRequest => 2,
            Self::VirtualMachineCompletePowerRequest => 2,
            Self::VirtualMachineOpenVcpu => 1,
            Self::VirtualMachineRegisterMmio => 3,
            Self::VirtualCpuGetMmioRequest => 2,
            Self::VirtualCpuCompleteMmio => 3,
            Self::GuestMemoryCreate => 0,
            Self::PendingVirtualMachineMapMemory => 4,
            Self::VmoCreateContiguous => 1,
            Self::DeviceClaim => 1,
            Self::PhysicalDeviceInfo => 2,
            Self::VmoGetDmaExtent => 5,
            Self::PendingVirtualMachineAssignDevice => 3,
            Self::GuestMailboxCreate => 2,
            Self::GuestMailboxSend => 2,
            Self::GuestMailboxReceive => 2,
            Self::GuestNotificationCreate => 5,
            Self::GuestNotificationControl => 1,
            Self::NativeBlockCreate => 4,
            Self::NativeBlockActivate => 1,
            Self::FilesystemMount => 5,
            Self::GuestMappingCreate => 2,
            Self::GuestMappingRelease => 0,
            Self::DeviceClaimMatching => 4,
            Self::DeviceProfileInfo => 2,
            Self::DeviceResourceInfo => 3,
            Self::DeviceFirmwareRead => 3,
            Self::DeviceClaimBundle => 3,
            Self::DeviceMmio => 4,
            Self::DeviceIrqPending => 0,
            Self::DeviceIrqComplete => 2,
            Self::VirtualCpuSetAffinity => 2,
            Self::SystemConfig => 1,
            Self::ObjectInspectorReadDetails => 5,
            Self::FilesystemUnmount => 2,
            Self::FilesystemMountRamfs => 2,
            Self::FilesystemSwitchRoot => 1,
            Self::FileProtect => 0,
            Self::VmarMapFile => 4,
            Self::DirectoryBeginRead => 0,
            Self::ProcessDeriveObserver => 0,
            Self::TaskGroupRequestStop => 0,
            Self::ConsoleDiscardInput => 0,
            Self::GuestNotificationConfigure => 3,
            Self::GuestNotificationAccess => 3,
            Self::VmarLockMemory => 3,
            Self::VmarUnlockMemory => 3,
            Self::ProcessBuilderSetMemoryLock => 1,
            Self::FilesystemBeginRootSwitch => 1,
            Self::FilesystemFinishRootSwitch => 1,
            Self::GuestMemoryTransfer => 4,
            Self::GuestMemoryScalar => 4,
            Self::GuestInterruptCreate => 1,
            Self::GuestInterruptSetLevel => 1,
            Self::MemoryInspectorReadStatistics => 4,
            Self::ProcessBuilderRetain => 2,
            Self::ProcessRetirementGetInfo => 2,
            Self::ProcessRetirementOpenView => 0,
            Self::ProcessRetirementRelease => 0,
            Self::CrashViewGetInfo => 2,
            Self::CrashViewRead => 3,
            Self::CrashViewThread => 3,
            Self::CrashViewMapping => 3,
            Self::CrashViewHandle => 3,
            Self::ThreadSetCurrentName => 2,
            Self::DeviceDmaFaultInfo => 2,
            Self::NativeBlockTransfer => 4,
            Self::ThreadGetCurrentScheduling => 1,
            Self::ThreadSetCurrentScheduling => 1,
            Self::ThreadGetScheduling => 1,
            Self::ThreadSetScheduling => 1,
            Self::VirtualCpuGetScheduling => 1,
            Self::VirtualCpuSetScheduling => 1,
            Self::ProcessBuilderSetScheduling => 2,
        }
    }

    /// Unconditional receiver rights. Selector-dependent and indirect-handle checks still apply.
    pub const fn receiver_rights(self) -> u64 {
        match self {
            Self::AbiQuery => 16,
            Self::HandleClose => 0,
            Self::HandleDuplicate => 1,
            Self::HandleReplace => 0,
            Self::HandleGetInfo => 0,
            Self::ObjectGetBasicInfo => 8,
            Self::ThreadYield => 32,
            Self::ThreadExit => 2048,
            Self::ProcessExit => 2048,
            Self::EventCreate => 549755813888,
            Self::EventSignal => 524288,
            Self::ObjectWaitOne => 4,
            Self::ByteChannelCreate => 549755813888,
            Self::ByteChannelWrite => 32,
            Self::ByteChannelRead => 16,
            Self::ConsoleRead => 16,
            Self::ConsoleWrite => 32,
            Self::DirectoryOpenFile => 16,
            Self::FileReadAt => 16,
            Self::CapabilityChannelCreate => 549755813888,
            Self::CapabilityChannelTrySend => 32,
            Self::CapabilityChannelReceive => 16,
            Self::ProcessBuilderCreate => 1048576,
            Self::ProcessBuilderSetName => 32,
            Self::ProcessBuilderSetData => 32,
            Self::ProcessBuilderSetAffinity => 32,
            Self::ProcessBuilderAddHandle => 32,
            Self::ProcessBuilderSeal => 32,
            Self::ProcessBuilderStart => 1024,
            Self::ProcessBuilderAbort => 2048,
            Self::ProcessRequestStop => 2048,
            Self::ObjectWaitMany => 32,
            Self::ProcessGetInfo => 8,
            Self::TaskInspectorScanProcesses => 8,
            Self::TaskInspectorScanThreads => 8,
            Self::TaskInspectorDeriveProcess => 268435467,
            Self::ObjectInspectorScanObjects => 8,
            Self::ObjectInspectorScanHandles => 8,
            Self::ObjectInspectorDeriveProcess => 268435467,
            Self::TaskInspectorDeriveTaskGroup => 268435467,
            Self::ObjectInspectorDeriveTaskGroup => 268435467,
            Self::TaskInspectorDeriveResourceDomain => 268435467,
            Self::ObjectInspectorDeriveResourceDomain => 268435467,
            Self::DirectoryOpenDirectory => 16,
            Self::VmoCreate => 549755813888,
            Self::VmoRead => 16,
            Self::VmoWrite => 32,
            Self::VmarAllocate => 64,
            Self::VmarMap => 64,
            Self::VmarProtect => 64,
            Self::VmarUnmap => 64,
            Self::VmarDestroy => 64,
            Self::DirectoryRead => 16,
            Self::MemoryInspectorRead => 8,
            Self::CpuInspectorProfileStart => 2147483648,
            Self::CpuInspectorProfileStop => 2147483648,
            Self::CpuInspectorProfileRead => 8589934592,
            Self::CpuInspectorTraceStart => 2147483648,
            Self::CpuInspectorTraceStop => 2147483648,
            Self::CpuInspectorTraceRead => 8589934592,
            Self::CpuInspectorRead => 8,
            Self::FileGetInfo => 8,
            Self::DirectoryGetInfo => 8,
            Self::VirtualMachineCreationLeaseCreate => 805306368,
            Self::VirtualMachineCreate => 536870912,
            Self::PendingVirtualMachineSetMemory => 32,
            Self::PendingVirtualMachineSetBootstrap => 32,
            Self::PendingVirtualMachineSeal => 32,
            Self::PendingVirtualMachineInstall => 1024,
            Self::PendingVirtualMachineAbort => 2048,
            Self::VirtualMachineRequestStop => 2048,
            Self::VirtualMachineGetInfo => 8,
            Self::VirtualCpuGetInfo => 8,
            Self::ResourceDomainCreate => 8388608,
            Self::TaskGroupCreate => 4194304,
            Self::VirtualCpuStart => 1024,
            Self::PendingVirtualMachineSetVirtualSerial => 32,
            Self::ClockGetMonotonic => 16,
            Self::VirtualSerialCreate => 549755813888,
            Self::VirtualSerialRegisterOutput => 32,
            Self::VirtualSerialWrite => 32,
            Self::ThreadCreate => 2097152,
            Self::ThreadStart => 1024,
            Self::ThreadRequestStop => 2048,
            Self::AtomicWait => 32,
            Self::AtomicWake => 32,
            Self::ThreadSleep => 32,
            Self::FileWriteAt => 32,
            Self::FileResize => 32,
            Self::DirectoryCreateFile => 48,
            Self::DirectoryCreateDirectory => 48,
            Self::DirectoryRemove => 48,
            Self::WaitSetCreate => 549755813888,
            Self::WaitSetAdd => 1073741824,
            Self::WaitSetRearm => 1073741824,
            Self::WaitSetRemove => 1073741824,
            Self::WaitSetWait => 4,
            Self::ProcessGetCurrentId => 16,
            Self::VirtualSerialAcknowledgeOutput => 16,
            Self::DirectoryScopeCreate => 16,
            Self::DirectoryGetMetadata => 24,
            Self::FileGetMetadata => 8,
            Self::DirectoryGetSelfMetadata => 8,
            Self::DirectorySetMetadata => 2147483664,
            Self::FileSetMetadata => 2147483648,
            Self::DirectoryRename => 48,
            Self::DirectoryLink => 48,
            Self::DirectorySymlink => 48,
            Self::DirectoryReadLink => 16,
            Self::DirectoryCanonicalize => 16,
            Self::DirectoryRemoveIf => 48,
            Self::DirectoryOpenDirectoryNofollow => 16,
            Self::FileSync => 32,
            Self::FileLock => 4294967296,
            Self::FileUnlock => 4294967296,
            Self::ClockGetRealtime => 16,
            Self::DirectoryOpenFileWithOptions => 16,
            Self::VirtualMachineCreationLeaseGetPlatformInfo => 8,
            Self::VmoCreateSnapshot => 16,
            Self::VmarMapPrivate => 64,
            Self::VirtualMachineGetPowerRequest => 32,
            Self::VirtualMachineCompletePowerRequest => 32,
            Self::VirtualMachineOpenVcpu => 32,
            Self::VirtualMachineRegisterMmio => 32,
            Self::VirtualCpuGetMmioRequest => 32,
            Self::VirtualCpuCompleteMmio => 32,
            Self::GuestMemoryCreate => 112,
            Self::PendingVirtualMachineMapMemory => 32,
            Self::VmoCreateContiguous => 549755813888,
            Self::DeviceClaim => 32768,
            Self::PhysicalDeviceInfo => 8,
            Self::VmoGetDmaExtent => 65536,
            Self::PendingVirtualMachineAssignDevice => 32,
            Self::GuestMailboxCreate => 32,
            Self::GuestMailboxSend => 32,
            Self::GuestMailboxReceive => 16,
            Self::GuestNotificationCreate => 32,
            Self::GuestNotificationControl => 32,
            Self::NativeBlockCreate => 64,
            Self::NativeBlockActivate => 32,
            Self::FilesystemMount => 160,
            Self::GuestMappingCreate => 32,
            Self::GuestMappingRelease => 32,
            Self::DeviceClaimMatching => 32768,
            Self::DeviceProfileInfo => 8,
            Self::DeviceResourceInfo => 8,
            Self::DeviceFirmwareRead => 8,
            Self::DeviceClaimBundle => 32768,
            Self::DeviceMmio => 0,
            Self::DeviceIrqPending => 4,
            Self::DeviceIrqComplete => 131072,
            Self::VirtualCpuSetAffinity => 32,
            Self::SystemConfig => 16,
            Self::ObjectInspectorReadDetails => 8589934600,
            Self::FilesystemUnmount => 160,
            Self::FilesystemMountRamfs => 17179869344,
            Self::FilesystemSwitchRoot => 17179869344,
            Self::FileProtect => 34359738368,
            Self::VmarMapFile => 64,
            Self::DirectoryBeginRead => 16,
            Self::ProcessDeriveObserver => 2054,
            Self::TaskGroupRequestStop => 2048,
            Self::ConsoleDiscardInput => 16,
            Self::GuestNotificationConfigure => 32,
            Self::GuestNotificationAccess => 32,
            Self::VmarLockMemory => 68719476736,
            Self::VmarUnlockMemory => 68719476736,
            Self::ProcessBuilderSetMemoryLock => 68719476768,
            Self::FilesystemBeginRootSwitch => 17179869344,
            Self::FilesystemFinishRootSwitch => 32,
            Self::GuestMemoryTransfer => 0,
            Self::GuestMemoryScalar => 0,
            Self::GuestInterruptCreate => 32,
            Self::GuestInterruptSetLevel => 32,
            Self::MemoryInspectorReadStatistics => 8,
            Self::ProcessBuilderRetain => 137438953504,
            Self::ProcessRetirementGetInfo => 8,
            Self::ProcessRetirementOpenView => 8589934592,
            Self::ProcessRetirementRelease => 274877906944,
            Self::CrashViewGetInfo => 8,
            Self::CrashViewRead => 16,
            Self::CrashViewThread => 16,
            Self::CrashViewMapping => 16,
            Self::CrashViewHandle => 16,
            Self::ThreadSetCurrentName => 32,
            Self::DeviceDmaFaultInfo => 32776,
            Self::NativeBlockTransfer => 0,
            Self::ThreadGetCurrentScheduling => 8,
            Self::ThreadSetCurrentScheduling => 0,
            Self::ThreadGetScheduling => 8,
            Self::ThreadSetScheduling => 0,
            Self::VirtualCpuGetScheduling => 8,
            Self::VirtualCpuSetScheduling => 0,
            Self::ProcessBuilderSetScheduling => 0,
        }
    }

    /// Return the auxiliary-result registers defined on a failure status.
    /// Bit N denotes auxiliary result N, excluding the separate signed status register.
    /// Zero means no defined failure results (also returned for unknown methods and OK).
    /// This is not the result-validity mask for a successful call.
    pub const fn failure_result_mask(self, status: HyperNativeStatus) -> u64 {
        match (self, status) {
            (Self::ByteChannelRead, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) => 1,
            (Self::ConsoleRead, HYPER_NATIVE_STATUS_WOULD_BLOCK) => 1,
            (Self::ConsoleWrite, HYPER_NATIVE_STATUS_WOULD_BLOCK) => 1,
            (Self::CapabilityChannelReceive, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) => 3,
            _ => 0,
        }
    }
}

/// Return the auxiliary-result registers defined on a failure status.
/// Bit N denotes auxiliary result N, excluding the separate signed status register.
/// Zero means no defined failure results (also returned for unknown methods and OK).
/// This is not the result-validity mask for a successful call.
pub const fn hyper_native_failure_result_mask(
    object_kind: u32,
    operation: u64,
    status: HyperNativeStatus,
) -> u64 {
    match NativeOperation::from_raw(object_kind, operation) {
        Some(method) => method.failure_result_mask(status),
        None => 0,
    }
}

/// Minimum supported prefix of `trace_config`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TRACE_CONFIG_MIN_SIZE: usize = 56;
/// Wire layout for `trace_config`.
///
/// Size: 56 bytes; alignment: 8 bytes; supported minimum prefix: 56 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `cpu_inspector_trace_start`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTraceConfig {
    /// Wire offset 0 bytes; width 8 bytes.
    pub categories: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub duration_ms: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub capacity: u64,
    /// Wire offset 24 bytes; width 32 bytes.
    pub cpu_mask: [u8; 32],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTraceConfig>() == 56);
const _: () = assert!(core::mem::align_of::<HyperNativeTraceConfig>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceConfig, categories) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceConfig, duration_ms) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceConfig, capacity) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceConfig, cpu_mask) == 24);

/// Minimum supported prefix of `trace_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TRACE_INFO_MIN_SIZE: usize = 160;
/// Wire layout for `trace_info`.
///
/// Size: 160 bytes; alignment: 8 bytes; supported minimum prefix: 160 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTraceInfo {
    /// Wire offset 0 bytes; width 8 bytes.
    pub generation: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub started_ns: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub deadline_ns: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub categories: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub cpu_count: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub capacity: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub maximum_capacity: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub running: u64,
    /// Wire offset 64 bytes; width 32 bytes.
    pub cpu_mask: [u8; 32],
    /// Wire offset 96 bytes; width 64 bytes.
    pub revision: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTraceInfo>() == 160);
const _: () = assert!(core::mem::align_of::<HyperNativeTraceInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, generation) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, started_ns) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, deadline_ns) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, categories) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, cpu_count) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, capacity) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, maximum_capacity) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, running) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, cpu_mask) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceInfo, revision) == 96);

/// Minimum supported prefix of `trace_record`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TRACE_RECORD_MIN_SIZE: usize = 328;
/// Wire layout for `trace_record`.
///
/// Size: 328 bytes; alignment: 8 bytes; supported minimum prefix: 328 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTraceRecord {
    /// Wire offset 0 bytes; width 8 bytes.
    pub sequence: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub timestamp_ns: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub category: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub field_count: u64,
    /// Wire offset 32 bytes; width 32 bytes.
    pub values: [u8; 32],
    /// Wire offset 64 bytes; width 64 bytes.
    pub name: [u8; 64],
    /// Wire offset 128 bytes; width 96 bytes.
    pub fields: [u8; 96],
    /// Wire offset 224 bytes; width 96 bytes.
    pub source: [u8; 96],
    /// Wire offset 320 bytes; width 8 bytes.
    pub line: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeTraceRecord>() == 328);
const _: () = assert!(core::mem::align_of::<HyperNativeTraceRecord>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, sequence) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, timestamp_ns) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, category) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, field_count) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, values) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, name) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, fields) == 128);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, source) == 224);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceRecord, line) == 320);

/// Minimum supported prefix of `trace_batch`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TRACE_BATCH_MIN_SIZE: usize = 2656;
/// Wire layout for `trace_batch`.
///
/// Size: 2656 bytes; alignment: 8 bytes; supported minimum prefix: 2656 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTraceBatch {
    /// Wire offset 0 bytes; width 8 bytes.
    pub count: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub next: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub overwritten: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub incomplete: u64,
    /// Wire offset 32 bytes; width 2624 bytes.
    pub records: [u8; 2624],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTraceBatch>() == 2656);
const _: () = assert!(core::mem::align_of::<HyperNativeTraceBatch>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceBatch, count) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceBatch, next) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceBatch, overwritten) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceBatch, incomplete) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceBatch, records) == 32);

/// Minimum supported prefix of `trace_thread`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TRACE_THREAD_MIN_SIZE: usize = 72;
/// Wire layout for `trace_thread`.
///
/// Size: 72 bytes; alignment: 8 bytes; supported minimum prefix: 72 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTraceThread {
    /// Wire offset 0 bytes; width 8 bytes.
    pub thread: u64,
    /// Wire offset 8 bytes; width 64 bytes.
    pub name: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTraceThread>() == 72);
const _: () = assert!(core::mem::align_of::<HyperNativeTraceThread>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceThread, thread) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTraceThread, name) == 8);

/// Minimum supported prefix of `scheduling_parameters`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_SCHEDULING_PARAMETERS_MIN_SIZE: usize = 64;
/// Wire layout for `scheduling_parameters`.
///
/// Size: 64 bytes; alignment: 8 bytes; supported minimum prefix: 64 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `thread_get_current_scheduling`, `thread_set_current_scheduling`, `thread_get_scheduling`, `thread_set_scheduling`, `virtual_cpu_get_scheduling`, `virtual_cpu_set_scheduling`, `process_builder_set_scheduling`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeSchedulingParameters {
    /// Wire offset 0 bytes; width 4 bytes.
    pub policy: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub band: u32,
    /// Wire offset 8 bytes; width 4 bytes.
    pub role: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub weight: u32,
    /// Wire offset 16 bytes; width 4 bytes.
    pub fifo_priority: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub fair_backend: u32,
    /// Wire offset 24 bytes; width 8 bytes.
    pub quantum_ns: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub budget_ns: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub period_ns: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub burst_ns: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeSchedulingParameters>() == 64);
const _: () = assert!(core::mem::align_of::<HyperNativeSchedulingParameters>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, policy) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, band) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, role) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, weight) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, fifo_priority) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, fair_backend) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, quantum_ns) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, budget_ns) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, period_ns) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, burst_ns) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeSchedulingParameters, reserved) == 56);

/// Minimum supported prefix of `cpu_profile_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CPU_PROFILE_INFO_MIN_SIZE: usize = 144;
/// Wire layout for `cpu_profile_info`.
///
/// Size: 144 bytes; alignment: 8 bytes; supported minimum prefix: 144 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCpuProfileInfo {
    /// Wire offset 0 bytes; width 8 bytes.
    pub generation: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub period: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub deadline_ns: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub cpu_count: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub running_generation: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub armed_mask: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub capacity: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub depth: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub active_cpus: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub dropped: u64,
    /// Wire offset 80 bytes; width 64 bytes.
    pub revision: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeCpuProfileInfo>() == 144);
const _: () = assert!(core::mem::align_of::<HyperNativeCpuProfileInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, generation) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, period) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, deadline_ns) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, cpu_count) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, running_generation) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, armed_mask) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, capacity) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, depth) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, active_cpus) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, dropped) == 72);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileInfo, revision) == 80);

/// Minimum supported prefix of `cpu_profile_sample`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CPU_PROFILE_SAMPLE_MIN_SIZE: usize = 192;
/// Wire layout for `cpu_profile_sample`.
///
/// Size: 192 bytes; alignment: 8 bytes; supported minimum prefix: 192 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCpuProfileSample {
    /// Wire offset 0 bytes; width 8 bytes.
    pub sequence: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub timestamp_ns: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub cpu: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub world: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub depth: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub flags: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub dropped: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub period: u64,
    /// Wire offset 64 bytes; width 128 bytes.
    pub frames: [u8; 128],
}
const _: () = assert!(core::mem::size_of::<HyperNativeCpuProfileSample>() == 192);
const _: () = assert!(core::mem::align_of::<HyperNativeCpuProfileSample>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, sequence) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, timestamp_ns) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, cpu) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, world) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, depth) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, flags) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, dropped) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, period) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuProfileSample, frames) == 64);

/// Minimum supported prefix of `process_retirement_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_PROCESS_RETIREMENT_INFO_MIN_SIZE: usize = 104;
/// Wire layout for `process_retirement_info`.
///
/// Size: 104 bytes; alignment: 8 bytes; supported minimum prefix: 104 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `process_retirement_get_info`, `crash_view_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeProcessRetirementInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub size: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub phase: u32,
    /// Wire offset 8 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub release_reason: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub process_koid: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub deadline_ns: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub retained_bytes: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub memory_limit: u64,
    /// Wire offset 48 bytes; width 4 bytes.
    pub terminal_reason: u32,
    /// Wire offset 52 bytes; width 4 bytes.
    pub fault_class: u32,
    /// Wire offset 56 bytes; width 8 bytes.
    pub exit_status: i64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub fault_pc: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub fault_address: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub fault_syndrome: u64,
    /// Wire offset 88 bytes; width 4 bytes.
    pub thread_count: u32,
    /// Wire offset 92 bytes; width 4 bytes.
    pub mapping_count: u32,
    /// Wire offset 96 bytes; width 4 bytes.
    pub handle_count: u32,
    /// Wire offset 100 bytes; width 4 bytes.
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeProcessRetirementInfo>() == 104);
const _: () = assert!(core::mem::align_of::<HyperNativeProcessRetirementInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, size) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, phase) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, flags) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, release_reason) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, process_koid) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, deadline_ns) == 24);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, retained_bytes) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, memory_limit) == 40);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, terminal_reason) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, fault_class) == 52);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, exit_status) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, fault_pc) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, fault_address) == 72);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, fault_syndrome) == 80);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, thread_count) == 88);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, mapping_count) == 92);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, handle_count) == 96);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessRetirementInfo, reserved) == 100);

/// Minimum supported prefix of `crash_thread`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CRASH_THREAD_MIN_SIZE: usize = 840;
/// Wire layout for `crash_thread`.
///
/// Size: 840 bytes; alignment: 8 bytes; supported minimum prefix: 840 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `crash_view_thread`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCrashThread {
    /// Wire offset 0 bytes; width 4 bytes.
    pub size: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub machine: u32,
    /// Wire offset 8 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub thread_koid: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub pc: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub sp: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub tls: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub processor_state: u64,
    /// Wire offset 56 bytes; width 256 bytes.
    pub general: [u8; 256],
    /// Wire offset 312 bytes; width 512 bytes.
    pub vector: [u8; 512],
    /// Wire offset 824 bytes; width 16 bytes.
    pub fp_control: [u8; 16],
}
const _: () = assert!(core::mem::size_of::<HyperNativeCrashThread>() == 840);
const _: () = assert!(core::mem::align_of::<HyperNativeCrashThread>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, size) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, machine) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, flags) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, reserved) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, thread_koid) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, pc) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, sp) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, tls) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, processor_state) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, general) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, vector) == 312);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashThread, fp_control) == 824);

/// Minimum supported prefix of `crash_mapping`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CRASH_MAPPING_MIN_SIZE: usize = 48;
/// Wire layout for `crash_mapping`.
///
/// Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `crash_view_mapping`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCrashMapping {
    /// Wire offset 0 bytes; width 4 bytes.
    pub size: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub base: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub length: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub object_offset: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub object_koid: u64,
    /// Wire offset 40 bytes; width 4 bytes.
    pub permissions: u32,
    /// Wire offset 44 bytes; width 4 bytes.
    pub kind: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCrashMapping>() == 48);
const _: () = assert!(core::mem::align_of::<HyperNativeCrashMapping>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, size) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, base) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, length) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, object_offset) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, object_koid) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, permissions) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashMapping, kind) == 44);

/// Minimum supported prefix of `crash_handle`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CRASH_HANDLE_MIN_SIZE: usize = 40;
/// Wire layout for `crash_handle`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `crash_view_handle`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCrashHandle {
    /// Wire offset 0 bytes; width 4 bytes.
    pub size: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub kind: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub value: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub koid: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub rights: u64,
    /// Wire offset 32 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCrashHandle>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeCrashHandle>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, size) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, kind) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, value) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, koid) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, rights) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, flags) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeCrashHandle, reserved) == 36);

/// Minimum supported prefix of `memory_heap_statistics`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_MEMORY_HEAP_STATISTICS_MIN_SIZE: usize = 176;
/// Wire layout for `memory_heap_statistics`.
///
/// Size: 176 bytes; alignment: 8 bytes; supported minimum prefix: 176 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeMemoryHeapStatistics {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub live_objects: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub live_slab_objects: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub live_large_objects: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub requested_bytes: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub peak_objects: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub peak_requested_bytes: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub allocations: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub failures: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub cached_objects: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub cache_hits: u64,
    /// Wire offset 88 bytes; width 8 bytes.
    pub cache_misses: u64,
    /// Wire offset 96 bytes; width 8 bytes.
    pub cache_refills: u64,
    /// Wire offset 104 bytes; width 8 bytes.
    pub cache_drains: u64,
    /// Wire offset 112 bytes; width 8 bytes.
    pub pressure_reclaims: u64,
    /// Wire offset 120 bytes; width 8 bytes.
    pub reclaimed_objects: u64,
    /// Wire offset 128 bytes; width 8 bytes.
    pub cache_cpus: u64,
    /// Wire offset 136 bytes; width 8 bytes.
    pub profiling_enabled: u64,
    /// Wire offset 144 bytes; width 8 bytes.
    pub page_allocations: u64,
    /// Wire offset 152 bytes; width 8 bytes.
    pub page_failures: u64,
    /// Wire offset 160 bytes; width 8 bytes.
    pub page_deallocations: u64,
    /// Wire offset 168 bytes; width 8 bytes.
    pub peak_allocated_pages: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeMemoryHeapStatistics>() == 176);
const _: () = assert!(core::mem::align_of::<HyperNativeMemoryHeapStatistics>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, captured_at_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, live_objects) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, live_slab_objects) == 16);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, live_large_objects) == 24);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, requested_bytes) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, peak_objects) == 40);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, peak_requested_bytes) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, allocations) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, failures) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, cached_objects) == 72);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, cache_hits) == 80);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, cache_misses) == 88);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, cache_refills) == 96);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, cache_drains) == 104);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, pressure_reclaims) == 112);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, reclaimed_objects) == 120);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, cache_cpus) == 128);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, profiling_enabled) == 136);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, page_allocations) == 144);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, page_failures) == 152);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, page_deallocations) == 160);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapStatistics, peak_allocated_pages) == 168);

/// Minimum supported prefix of `memory_buddy_statistics`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_MEMORY_BUDDY_STATISTICS_MIN_SIZE: usize = 24;
/// Wire layout for `memory_buddy_statistics`.
///
/// Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeMemoryBuddyStatistics {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub order: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub free_blocks: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeMemoryBuddyStatistics>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeMemoryBuddyStatistics>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryBuddyStatistics, captured_at_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryBuddyStatistics, order) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryBuddyStatistics, free_blocks) == 16);

/// Minimum supported prefix of `memory_owner_statistics`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_MEMORY_OWNER_STATISTICS_MIN_SIZE: usize = 48;
/// Wire layout for `memory_owner_statistics`.
///
/// Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeMemoryOwnerStatistics {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub resident_pages: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub peak_pages: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub allocations: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub failures: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub deallocations: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeMemoryOwnerStatistics>() == 48);
const _: () = assert!(core::mem::align_of::<HyperNativeMemoryOwnerStatistics>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryOwnerStatistics, captured_at_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryOwnerStatistics, resident_pages) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryOwnerStatistics, peak_pages) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryOwnerStatistics, allocations) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryOwnerStatistics, failures) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryOwnerStatistics, deallocations) == 40);

/// Minimum supported prefix of `memory_file_cache_statistics`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_MEMORY_FILE_CACHE_STATISTICS_MIN_SIZE: usize = 120;
/// Wire layout for `memory_file_cache_statistics`.
///
/// Size: 120 bytes; alignment: 8 bytes; supported minimum prefix: 120 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeMemoryFileCacheStatistics {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub payload_pages: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub metadata_slab_pages: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub metadata_large_pages: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub index_capacity: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub clean_entries: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub loading_entries: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub live_payloads: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub metadata_budget_pages: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub table_present: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub admission_enabled: u64,
    /// Wire offset 88 bytes; width 8 bytes.
    pub admission_paused: u64,
    /// Wire offset 96 bytes; width 8 bytes.
    pub maintenance_pending: u64,
    /// Wire offset 104 bytes; width 8 bytes.
    pub maintenance_running: u64,
    /// Wire offset 112 bytes; width 8 bytes.
    pub growth_requested: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeMemoryFileCacheStatistics>() == 120);
const _: () = assert!(core::mem::align_of::<HyperNativeMemoryFileCacheStatistics>() == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, captured_at_ns) == 0);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, payload_pages) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, metadata_slab_pages) == 16);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, metadata_large_pages) == 24
);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, index_capacity) == 32);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, clean_entries) == 40);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, loading_entries) == 48);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, live_payloads) == 56);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, metadata_budget_pages) == 64
);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, table_present) == 72);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, admission_enabled) == 80);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, admission_paused) == 88);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, maintenance_pending) == 96);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, maintenance_running) == 104
);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryFileCacheStatistics, growth_requested) == 112);

/// Minimum supported prefix of `memory_heap_class_statistics`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_MEMORY_HEAP_CLASS_STATISTICS_MIN_SIZE: usize = 88;
/// Wire layout for `memory_heap_class_statistics`.
///
/// Size: 88 bytes; alignment: 8 bytes; supported minimum prefix: 88 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeMemoryHeapClassStatistics {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub pool: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub object_size: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub pages: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub capacity: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub reserved: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub cached: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub live: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub requested_bytes: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub allocations: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub deallocations: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeMemoryHeapClassStatistics>() == 88);
const _: () = assert!(core::mem::align_of::<HyperNativeMemoryHeapClassStatistics>() == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, captured_at_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, pool) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, object_size) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, pages) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, capacity) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, reserved) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, cached) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, live) == 56);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, requested_bytes) == 64);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, allocations) == 72);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryHeapClassStatistics, deallocations) == 80);

/// Minimum supported prefix of `filesystem_root_switch`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_FILESYSTEM_ROOT_SWITCH_MIN_SIZE: usize = 48;
/// Wire layout for `filesystem_root_switch`.
///
/// Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `filesystem_switch_root`, `filesystem_begin_root_switch`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeFilesystemRootSwitch {
    /// Wire offset 0 bytes; width 8 bytes.
    pub new_root: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub new_root_length: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub put_old: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub put_old_length: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub moves: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub move_count: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeFilesystemRootSwitch>() == 48);
const _: () = assert!(core::mem::align_of::<HyperNativeFilesystemRootSwitch>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemRootSwitch, new_root) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemRootSwitch, new_root_length) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemRootSwitch, put_old) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemRootSwitch, put_old_length) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemRootSwitch, moves) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemRootSwitch, move_count) == 40);

/// Minimum supported prefix of `filesystem_mount_move`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_FILESYSTEM_MOUNT_MOVE_MIN_SIZE: usize = 32;
/// Wire layout for `filesystem_mount_move`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeFilesystemMountMove {
    /// Wire offset 0 bytes; width 8 bytes.
    pub source: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub source_length: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub target: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub target_length: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeFilesystemMountMove>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeFilesystemMountMove>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemMountMove, source) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemMountMove, source_length) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemMountMove, target) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeFilesystemMountMove, target_length) == 24);

/// Minimum supported prefix of `object_details`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_OBJECT_DETAILS_MIN_SIZE: usize = 88;
/// Wire layout for `object_details`.
///
/// Size: 88 bytes; alignment: 8 bytes; supported minimum prefix: 88 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `object_inspector_read_details`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectDetails {
    /// Wire offset 0 bytes; width 8 bytes.
    pub koid: u64,
    /// Wire offset 8 bytes; width 4 bytes.
    pub object_kind: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub record_kind: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub next_cursor: u64,
    /// Wire offset 24 bytes; width 64 bytes.
    pub payload: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectDetails>() == 88);
const _: () = assert!(core::mem::align_of::<HyperNativeObjectDetails>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectDetails, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectDetails, object_kind) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectDetails, record_kind) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectDetails, next_cursor) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectDetails, payload) == 24);

/// Minimum supported prefix of `device_dma_fault_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DEVICE_DMA_FAULT_INFO_MIN_SIZE: usize = 40;
/// Wire layout for `device_dma_fault_info`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `device_dma_fault_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDeviceDmaFaultInfo {
    /// Wire offset 0 bytes; width 8 bytes.
    pub timestamp_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub address: u64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub stream_id: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub source: u32,
    /// Wire offset 24 bytes; width 4 bytes.
    pub scope: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub code: u32,
    /// Wire offset 32 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDeviceDmaFaultInfo>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeDeviceDmaFaultInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, timestamp_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, address) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, stream_id) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, source) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, scope) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, code) == 28);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, flags) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceDmaFaultInfo, reserved) == 36);

/// Minimum supported prefix of `device_profile_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DEVICE_PROFILE_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `device_profile_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `device_profile_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDeviceProfileInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub profile: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub interrupt_count: u32,
    /// Wire offset 8 bytes; width 4 bytes.
    pub resource_count: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub pci_identity: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub dma_bus_offset: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub aperture_size: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDeviceProfileInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeDeviceProfileInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceProfileInfo, profile) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceProfileInfo, interrupt_count) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceProfileInfo, resource_count) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceProfileInfo, pci_identity) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceProfileInfo, dma_bus_offset) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceProfileInfo, aperture_size) == 24);

/// Minimum supported prefix of `device_firmware_query`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_QUERY_MIN_SIZE: usize = 24;
/// Wire layout for `device_firmware_query`.
///
/// Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `device_firmware_read`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDeviceFirmwareQuery {
    /// Wire offset 0 bytes; width 4 bytes.
    pub node: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub field: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub name_address: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub name_length: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDeviceFirmwareQuery>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeDeviceFirmwareQuery>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareQuery, node) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareQuery, field) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareQuery, name_address) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareQuery, name_length) == 16);

/// Minimum supported prefix of `device_firmware_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DEVICE_FIRMWARE_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `device_firmware_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDeviceFirmwareInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub register_count: u32,
    /// Wire offset 8 bytes; width 4 bytes.
    pub irq_number: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub irq_trigger: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub reserved0: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub reserved1: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDeviceFirmwareInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeDeviceFirmwareInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareInfo, flags) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareInfo, register_count) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareInfo, irq_number) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareInfo, irq_trigger) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareInfo, reserved0) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceFirmwareInfo, reserved1) == 24);

/// Minimum supported prefix of `device_bundle_entry`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DEVICE_BUNDLE_ENTRY_MIN_SIZE: usize = 16;
/// Wire layout for `device_bundle_entry`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `device_claim_bundle`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDeviceBundleEntry {
    /// Wire offset 0 bytes; width 4 bytes.
    pub node: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub resource: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub offset: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDeviceBundleEntry>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeDeviceBundleEntry>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceBundleEntry, node) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceBundleEntry, resource) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceBundleEntry, offset) == 8);

/// Minimum supported prefix of `device_resource_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DEVICE_RESOURCE_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `device_resource_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `device_resource_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDeviceResourceInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub kind: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub offset: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub length: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub bus_address: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDeviceResourceInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeDeviceResourceInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceResourceInfo, kind) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceResourceInfo, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceResourceInfo, offset) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceResourceInfo, length) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeDeviceResourceInfo, bus_address) == 24);

/// Minimum supported prefix of `physical_device_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_PHYSICAL_DEVICE_INFO_MIN_SIZE: usize = 16;
/// Wire layout for `physical_device_info`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `physical_device_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativePhysicalDeviceInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub device_id: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub transport_version: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub mmio_size: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativePhysicalDeviceInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativePhysicalDeviceInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativePhysicalDeviceInfo, device_id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativePhysicalDeviceInfo, transport_version) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativePhysicalDeviceInfo, mmio_size) == 8);

/// Minimum supported prefix of `dma_extent`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DMA_EXTENT_MIN_SIZE: usize = 16;
/// Wire layout for `dma_extent`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `vmo_get_dma_extent`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDmaExtent {
    /// Wire offset 0 bytes; width 8 bytes.
    pub physical_base: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub length: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDmaExtent>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeDmaExtent>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDmaExtent, physical_base) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDmaExtent, length) == 8);

/// Minimum supported prefix of `virtual_cpu_mmio_request`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_CPU_MMIO_REQUEST_MIN_SIZE: usize = 48;
/// Wire layout for `virtual_cpu_mmio_request`.
///
/// Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `virtual_cpu_get_mmio_request`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualCpuMmioRequest {
    /// Wire offset 0 bytes; width 8 bytes.
    pub id: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub device: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub address: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub value: u64,
    /// Wire offset 32 bytes; width 4 bytes.
    pub operation: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub width: u32,
    /// Wire offset 40 bytes; width 8 bytes.
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualCpuMmioRequest>() == 48);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualCpuMmioRequest>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, device) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, address) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, value) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, operation) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, width) == 36);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuMmioRequest, reserved) == 40);

/// Minimum supported prefix of `private_mapping`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_PRIVATE_MAPPING_MIN_SIZE: usize = 48;
/// Wire layout for `private_mapping`.
///
/// Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `vmar_map_private`, `vmar_map_file`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativePrivateMapping {
    /// Wire offset 0 bytes; width 8 bytes.
    pub source_offset: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub source_length: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub address: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub size: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub data_offset: u64,
    /// Wire offset 40 bytes; width 4 bytes.
    pub permissions: u32,
    /// Wire offset 44 bytes; width 4 bytes.
    pub mode: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativePrivateMapping>() == 48);
const _: () = assert!(core::mem::align_of::<HyperNativePrivateMapping>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, source_offset) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, source_length) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, address) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, size) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, data_offset) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, permissions) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativePrivateMapping, mode) == 44);

/// Minimum supported prefix of `virtual_machine_platform_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_PLATFORM_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `virtual_machine_platform_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `virtual_machine_creation_lease_get_platform_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualMachinePlatformInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub architecture: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub platform_profile: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub counter_frequency_hz: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub riscv_isa: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub aarch64_gic_version: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualMachinePlatformInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualMachinePlatformInfo>() == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachinePlatformInfo, architecture) == 0);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachinePlatformInfo, platform_profile) == 4);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeVirtualMachinePlatformInfo, counter_frequency_hz) == 8
);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachinePlatformInfo, riscv_isa) == 16);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeVirtualMachinePlatformInfo, aarch64_gic_version) == 24
);

/// Minimum supported prefix of `file_metadata`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_FILE_METADATA_MIN_SIZE: usize = 112;
/// Wire layout for `file_metadata`.
///
/// Size: 112 bytes; alignment: 8 bytes; supported minimum prefix: 112 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `directory_get_metadata`, `file_get_metadata`, `directory_get_self_metadata`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeFileMetadata {
    /// Wire offset 0 bytes; width 8 bytes.
    pub filesystem_id: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub mount_id: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub node_id: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub size: u64,
    /// Wire offset 32 bytes; width 4 bytes.
    pub mode: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub kind: u32,
    /// Wire offset 40 bytes; width 4 bytes.
    pub valid_times: u32,
    /// Wire offset 44 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 48 bytes; width 8 bytes.
    pub accessed_seconds: i64,
    /// Wire offset 56 bytes; width 4 bytes.
    pub accessed_nanoseconds: u32,
    /// Wire offset 60 bytes; width 4 bytes.
    pub accessed_reserved: u32,
    /// Wire offset 64 bytes; width 8 bytes.
    pub modified_seconds: i64,
    /// Wire offset 72 bytes; width 4 bytes.
    pub modified_nanoseconds: u32,
    /// Wire offset 76 bytes; width 4 bytes.
    pub modified_reserved: u32,
    /// Wire offset 80 bytes; width 8 bytes.
    pub created_seconds: i64,
    /// Wire offset 88 bytes; width 4 bytes.
    pub created_nanoseconds: u32,
    /// Wire offset 92 bytes; width 4 bytes.
    pub created_reserved: u32,
    /// Wire offset 96 bytes; width 8 bytes.
    pub changed_seconds: i64,
    /// Wire offset 104 bytes; width 4 bytes.
    pub changed_nanoseconds: u32,
    /// Wire offset 108 bytes; width 4 bytes.
    pub changed_reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeFileMetadata>() == 112);
const _: () = assert!(core::mem::align_of::<HyperNativeFileMetadata>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, filesystem_id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, mount_id) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, node_id) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, size) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, mode) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, kind) == 36);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, valid_times) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, reserved) == 44);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, accessed_seconds) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, accessed_nanoseconds) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, accessed_reserved) == 60);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, modified_seconds) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, modified_nanoseconds) == 72);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, modified_reserved) == 76);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, created_seconds) == 80);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, created_nanoseconds) == 88);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, created_reserved) == 92);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, changed_seconds) == 96);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, changed_nanoseconds) == 104);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadata, changed_reserved) == 108);

/// Minimum supported prefix of `file_metadata_update`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_FILE_METADATA_UPDATE_MIN_SIZE: usize = 40;
/// Wire layout for `file_metadata_update`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `directory_set_metadata`, `file_set_metadata`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeFileMetadataUpdate {
    /// Wire offset 0 bytes; width 4 bytes.
    pub mask: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub mode: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub accessed_seconds: i64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub accessed_nanoseconds: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub accessed_reserved: u32,
    /// Wire offset 24 bytes; width 8 bytes.
    pub modified_seconds: i64,
    /// Wire offset 32 bytes; width 4 bytes.
    pub modified_nanoseconds: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub modified_reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeFileMetadataUpdate>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeFileMetadataUpdate>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, mask) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, mode) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, accessed_seconds) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, accessed_nanoseconds) == 16);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, accessed_reserved) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, modified_seconds) == 24);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, modified_nanoseconds) == 32);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeFileMetadataUpdate, modified_reserved) == 36);

/// Minimum supported prefix of `wait_set_event`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_WAIT_SET_EVENT_MIN_SIZE: usize = 24;
/// Wire layout for `wait_set_event`.
///
/// Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `wait_set_wait`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeWaitSetEvent {
    /// Wire offset 0 bytes; width 8 bytes.
    pub registration: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub signals: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub sequence: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeWaitSetEvent>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeWaitSetEvent>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeWaitSetEvent, registration) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeWaitSetEvent, signals) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeWaitSetEvent, sequence) == 16);

/// Minimum supported prefix of `handle_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_HANDLE_INFO_MIN_SIZE: usize = 16;
/// Wire layout for `handle_info`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `handle_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeHandleInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub object_kind: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub rights: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeHandleInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeHandleInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInfo, object_kind) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInfo, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInfo, rights) == 8);

/// Minimum supported prefix of `object_basic_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE: usize = 16;
/// Wire layout for `object_basic_info`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `object_get_basic_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectBasicInfo {
    /// Wire offset 0 bytes; width 8 bytes.
    pub koid: u64,
    /// Wire offset 8 bytes; width 4 bytes.
    pub object_kind: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectBasicInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeObjectBasicInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectBasicInfo, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectBasicInfo, object_kind) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectBasicInfo, reserved) == 12);

/// Minimum supported prefix of `object_wait_item`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_OBJECT_WAIT_ITEM_MIN_SIZE: usize = 16;
/// Wire layout for `object_wait_item`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `object_wait_many`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectWaitItem {
    /// Wire offset 0 bytes; width 8 bytes.
    pub handle: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub signals: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectWaitItem>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeObjectWaitItem>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectWaitItem, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectWaitItem, signals) == 8);

/// Minimum supported prefix of `process_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_PROCESS_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `process_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `process_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeProcessInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub phase: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub terminal_reason: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub detail0: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub detail1: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeProcessInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeProcessInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, phase) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, terminal_reason) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, detail0) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, detail1) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, reserved) == 24);

/// Minimum supported prefix of `capability_disposition`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_MIN_SIZE: usize = 24;
/// Wire layout for `capability_disposition`.
///
/// Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `capability_channel_try_send`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCapabilityDisposition {
    /// Wire offset 0 bytes; width 8 bytes.
    pub handle: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub rights: u64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub expected_kind: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub operation: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCapabilityDisposition>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeCapabilityDisposition>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, rights) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, expected_kind) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, operation) == 20);

/// Minimum supported prefix of `capability_receive_slot`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CAPABILITY_RECEIVE_SLOT_MIN_SIZE: usize = 24;
/// Wire layout for `capability_receive_slot`.
///
/// Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `capability_channel_receive`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCapabilityReceiveSlot {
    /// Wire offset 0 bytes; width 8 bytes.
    pub handle: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub rights: u64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub expected_kind: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub flags: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCapabilityReceiveSlot>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeCapabilityReceiveSlot>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, rights) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, expected_kind) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, flags) == 20);

/// Minimum supported prefix of `loader_startup`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_LOADER_STARTUP_MIN_SIZE: usize = 96;
/// Wire layout for `loader_startup`.
///
/// Size: 96 bytes; alignment: 8 bytes; supported minimum prefix: 96 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeLoaderStartup {
    /// Wire offset 0 bytes; width 4 bytes.
    pub size: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub handle_count: u32,
    /// Wire offset 8 bytes; width 4 bytes.
    pub data_size: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub executable: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub root_vmar: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub stack_vmar: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub stack_base: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub stack_size: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub loader_base: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub loader_size: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub execution_context: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub object_factory: u64,
    /// Wire offset 88 bytes; width 8 bytes.
    pub system_info: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeLoaderStartup>() == 96);
const _: () = assert!(core::mem::align_of::<HyperNativeLoaderStartup>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, size) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, handle_count) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, data_size) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, flags) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, executable) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, root_vmar) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, stack_vmar) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, stack_base) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, stack_size) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, loader_base) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, loader_size) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, execution_context) == 72);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, object_factory) == 80);
const _: () = assert!(core::mem::offset_of!(HyperNativeLoaderStartup, system_info) == 88);

/// Minimum supported prefix of `startup_handle`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_STARTUP_HANDLE_MIN_SIZE: usize = 16;
/// Wire layout for `startup_handle`.
///
/// Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeStartupHandle {
    /// Wire offset 0 bytes; width 4 bytes.
    pub purpose: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub handle: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeStartupHandle>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeStartupHandle>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeStartupHandle, purpose) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeStartupHandle, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeStartupHandle, handle) == 8);

/// Minimum supported prefix of `task_process`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TASK_PROCESS_MIN_SIZE: usize = 96;
/// Wire layout for `task_process`.
///
/// Size: 96 bytes; alignment: 8 bytes; supported minimum prefix: 96 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `task_inspector_scan_processes`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTaskProcess {
    /// Wire offset 0 bytes; width 8 bytes.
    pub koid: u64,
    /// Wire offset 8 bytes; width 4 bytes.
    pub phase: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub terminal_reason: u32,
    /// Wire offset 16 bytes; width 4 bytes.
    pub pending_threads: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub active_threads: u32,
    /// Wire offset 24 bytes; width 4 bytes.
    pub name_length: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 32 bytes; width 64 bytes.
    pub name: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTaskProcess>() == 96);
const _: () = assert!(core::mem::align_of::<HyperNativeTaskProcess>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, phase) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, terminal_reason) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, pending_threads) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, active_threads) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, name_length) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, reserved) == 28);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskProcess, name) == 32);

/// Minimum supported prefix of `task_thread`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_TASK_THREAD_MIN_SIZE: usize = 104;
/// Wire layout for `task_thread`.
///
/// Size: 104 bytes; alignment: 8 bytes; supported minimum prefix: 104 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `task_inspector_scan_threads`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTaskThread {
    /// Wire offset 0 bytes; width 8 bytes.
    pub koid: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub process_koid: u64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub role: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub registry_phase: u32,
    /// Wire offset 24 bytes; width 4 bytes.
    pub name_length: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 32 bytes; width 8 bytes.
    pub runtime_ns: u64,
    /// Wire offset 40 bytes; width 64 bytes.
    pub name: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTaskThread>() == 104);
const _: () = assert!(core::mem::align_of::<HyperNativeTaskThread>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, process_koid) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, role) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, registry_phase) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, name_length) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, reserved) == 28);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, runtime_ns) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, name) == 40);

/// Minimum supported prefix of `memory_observation`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE: usize = 144;
/// Wire layout for `memory_observation`.
///
/// Size: 144 bytes; alignment: 8 bytes; supported minimum prefix: 144 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `memory_inspector_read`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeMemoryObservation {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub page_size: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub total_bytes: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub managed_bytes: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub free_bytes: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub kernel_bytes: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub slab_bytes: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub large_heap_bytes: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub page_table_bytes: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub vmo_bytes: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub guest_loaned_bytes: u64,
    /// Wire offset 88 bytes; width 8 bytes.
    pub guest_private_bytes: u64,
    /// Wire offset 96 bytes; width 8 bytes.
    pub file_cache_bytes: u64,
    /// Wire offset 104 bytes; width 8 bytes.
    pub slab_reclaimable_bytes: u64,
    /// Wire offset 112 bytes; width 8 bytes.
    pub slab_sample_complete: u64,
    /// Wire offset 120 bytes; width 8 bytes.
    pub file_cache_reclaimable_bytes: u64,
    /// Wire offset 128 bytes; width 8 bytes.
    pub file_cache_sample_complete: u64,
    /// Wire offset 136 bytes; width 8 bytes.
    pub ramfs_capacity_bytes: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeMemoryObservation>() == 144);
const _: () = assert!(core::mem::align_of::<HyperNativeMemoryObservation>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, captured_at_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, page_size) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, total_bytes) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, managed_bytes) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, free_bytes) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, kernel_bytes) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, slab_bytes) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, large_heap_bytes) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, page_table_bytes) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, vmo_bytes) == 72);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryObservation, guest_loaned_bytes) == 80);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryObservation, guest_private_bytes) == 88);
const _: () = assert!(core::mem::offset_of!(HyperNativeMemoryObservation, file_cache_bytes) == 96);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryObservation, slab_reclaimable_bytes) == 104);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryObservation, slab_sample_complete) == 112);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeMemoryObservation, file_cache_reclaimable_bytes) == 120
);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryObservation, file_cache_sample_complete) == 128);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeMemoryObservation, ramfs_capacity_bytes) == 136);

/// Minimum supported prefix of `cpu_observation`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE: usize = 64;
/// Wire layout for `cpu_observation`.
///
/// Size: 64 bytes; alignment: 8 bytes; supported minimum prefix: 64 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `cpu_inspector_read`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCpuObservation {
    /// Wire offset 0 bytes; width 8 bytes.
    pub captured_at_ns: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub reserved_clock: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub online_cpus: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub idle_ns: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub kernel_thread_ns: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub user_thread_ns: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub vcpu_ns: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCpuObservation>() == 64);
const _: () = assert!(core::mem::align_of::<HyperNativeCpuObservation>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, captured_at_ns) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, reserved_clock) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, online_cpus) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, idle_ns) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, kernel_thread_ns) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, user_thread_ns) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, vcpu_ns) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeCpuObservation, reserved) == 56);

/// Minimum supported prefix of `object_inspection`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_OBJECT_INSPECTION_MIN_SIZE: usize = 104;
/// Wire layout for `object_inspection`.
///
/// Size: 104 bytes; alignment: 8 bytes; supported minimum prefix: 104 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `object_inspector_scan_objects`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectInspection {
    /// Wire offset 0 bytes; width 8 bytes.
    pub koid: u64,
    /// Wire offset 8 bytes; width 4 bytes.
    pub object_kind: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub handle_state: u32,
    /// Wire offset 16 bytes; width 8 bytes.
    pub active_handles: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub supported_rights: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub strong_references: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub kernel_service_references: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub scheduler_references: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub operation_references: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub user_authority_references: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub publication_references: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub diagnostic_references: u64,
    /// Wire offset 88 bytes; width 8 bytes.
    pub retirement_references: u64,
    /// Wire offset 96 bytes; width 8 bytes.
    pub vm_device_binding_references: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectInspection>() == 104);
const _: () = assert!(core::mem::align_of::<HyperNativeObjectInspection>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectInspection, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectInspection, object_kind) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectInspection, handle_state) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectInspection, active_handles) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectInspection, supported_rights) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectInspection, strong_references) == 32);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, kernel_service_references) == 40);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, scheduler_references) == 48);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, operation_references) == 56);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, user_authority_references) == 64);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, publication_references) == 72);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, diagnostic_references) == 80);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, retirement_references) == 88);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeObjectInspection, vm_device_binding_references) == 96);

/// Minimum supported prefix of `handle_inspection`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_HANDLE_INSPECTION_MIN_SIZE: usize = 40;
/// Wire layout for `handle_inspection`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `object_inspector_scan_handles`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeHandleInspection {
    /// Wire offset 0 bytes; width 8 bytes.
    pub process_koid: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub handle: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub object_koid: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub rights: u64,
    /// Wire offset 32 bytes; width 4 bytes.
    pub object_kind: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub flags: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeHandleInspection>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeHandleInspection>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInspection, process_koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInspection, handle) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInspection, object_koid) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInspection, rights) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInspection, object_kind) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInspection, flags) == 36);

/// Minimum supported prefix of `directory_entry`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DIRECTORY_ENTRY_MIN_SIZE: usize = 280;
/// Wire layout for `directory_entry`.
///
/// Size: 280 bytes; alignment: 8 bytes; supported minimum prefix: 280 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `directory_read`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDirectoryEntry {
    /// Wire offset 0 bytes; width 8 bytes.
    pub size: u64,
    /// Wire offset 8 bytes; width 4 bytes.
    pub mode: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub kind: u32,
    /// Wire offset 16 bytes; width 4 bytes.
    pub name_length: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 24 bytes; width 256 bytes.
    pub name: [u8; 256],
}
const _: () = assert!(core::mem::size_of::<HyperNativeDirectoryEntry>() == 280);
const _: () = assert!(core::mem::align_of::<HyperNativeDirectoryEntry>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryEntry, size) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryEntry, mode) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryEntry, kind) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryEntry, name_length) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryEntry, reserved) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryEntry, name) == 24);

/// Minimum supported prefix of `file_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_FILE_INFO_MIN_SIZE: usize = 40;
/// Wire layout for `file_info`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `file_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeFileInfo {
    /// Wire offset 0 bytes; width 8 bytes.
    pub filesystem_id: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub mount_id: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub node_id: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub size: u64,
    /// Wire offset 32 bytes; width 4 bytes.
    pub mode: u32,
    /// Wire offset 36 bytes; width 4 bytes.
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeFileInfo>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeFileInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileInfo, filesystem_id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileInfo, mount_id) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileInfo, node_id) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileInfo, size) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileInfo, mode) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeFileInfo, reserved) == 36);

/// Minimum supported prefix of `directory_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `directory_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `directory_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeDirectoryInfo {
    /// Wire offset 0 bytes; width 8 bytes.
    pub filesystem_id: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub mount_id: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub node_id: u64,
    /// Wire offset 24 bytes; width 4 bytes.
    pub mode: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeDirectoryInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeDirectoryInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryInfo, filesystem_id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryInfo, mount_id) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryInfo, node_id) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryInfo, mode) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeDirectoryInfo, reserved) == 28);

/// Minimum supported prefix of `virtual_machine_configuration`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_CONFIGURATION_MIN_SIZE: usize = 32;
/// Wire layout for `virtual_machine_configuration`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `virtual_machine_create`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualMachineConfiguration {
    /// Wire offset 0 bytes; width 8 bytes.
    pub guest_physical_base: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub memory_size: u64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub vcpu_count: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub architecture: u32,
    /// Wire offset 24 bytes; width 4 bytes.
    pub platform_profile: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub flags: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualMachineConfiguration>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualMachineConfiguration>() == 8);
const _: () = assert!(
    core::mem::offset_of!(HyperNativeVirtualMachineConfiguration, guest_physical_base) == 0
);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachineConfiguration, memory_size) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachineConfiguration, vcpu_count) == 16);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachineConfiguration, architecture) == 20);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachineConfiguration, platform_profile) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachineConfiguration, flags) == 28);

/// Minimum supported prefix of `virtual_cpu_bootstrap`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_CPU_BOOTSTRAP_MIN_SIZE: usize = 64;
/// Wire layout for `virtual_cpu_bootstrap`.
///
/// Size: 64 bytes; alignment: 8 bytes; supported minimum prefix: 64 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `pending_virtual_machine_set_bootstrap`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualCpuBootstrap {
    /// Wire offset 0 bytes; width 8 bytes.
    pub entry: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub stack: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub argument0: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub argument1: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub argument2: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub argument3: u64,
    /// Wire offset 48 bytes; width 4 bytes.
    pub vcpu_id: u32,
    /// Wire offset 52 bytes; width 4 bytes.
    pub flags: u32,
    /// Wire offset 56 bytes; width 8 bytes.
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualCpuBootstrap>() == 64);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualCpuBootstrap>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, entry) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, stack) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, argument0) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, argument1) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, argument2) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, argument3) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, vcpu_id) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, flags) == 52);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuBootstrap, reserved) == 56);

/// Minimum supported prefix of `virtual_machine_power_request`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_POWER_REQUEST_MIN_SIZE: usize = 40;
/// Wire layout for `virtual_machine_power_request`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `virtual_machine_get_power_request`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualMachinePowerRequest {
    /// Wire offset 0 bytes; width 8 bytes.
    pub id: u64,
    /// Wire offset 8 bytes; width 4 bytes.
    pub vcpu: u32,
    /// Wire offset 12 bytes; width 4 bytes.
    pub operation: u32,
    /// Wire offset 16 bytes; width 4 bytes.
    pub target: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 24 bytes; width 8 bytes.
    pub entry: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub context: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualMachinePowerRequest>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualMachinePowerRequest>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, vcpu) == 8);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, operation) == 12);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, target) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, reserved) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, entry) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachinePowerRequest, context) == 32);

/// Minimum supported prefix of `virtual_machine_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE: usize = 32;
/// Wire layout for `virtual_machine_info`.
///
/// Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `virtual_machine_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualMachineInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub phase: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub vcpu_count: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub guest_physical_base: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub memory_size: u64,
    /// Wire offset 24 bytes; width 4 bytes.
    pub architecture: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub platform_profile: u32,
    /// Wire offset 32 bytes; width 8 bytes.
    pub resident_memory_bytes: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualMachineInfo>() == 40);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualMachineInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, phase) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, vcpu_count) == 4);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, guest_physical_base) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, memory_size) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, architecture) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, platform_profile) == 28);
const _: () =
    assert!(core::mem::offset_of!(HyperNativeVirtualMachineInfo, resident_memory_bytes) == 32);

/// Minimum supported prefix of `virtual_cpu_info`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE: usize = 24;
/// Wire layout for `virtual_cpu_info`.
///
/// Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `virtual_cpu_get_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeVirtualCpuInfo {
    /// Wire offset 0 bytes; width 4 bytes.
    pub vcpu_id: u32,
    /// Wire offset 4 bytes; width 4 bytes.
    pub phase: u32,
    /// Wire offset 8 bytes; width 8 bytes.
    pub scheduler_thread_id: u64,
    /// Wire offset 16 bytes; width 4 bytes.
    pub terminal_reason: u32,
    /// Wire offset 20 bytes; width 4 bytes.
    pub reserved: u32,
    /// Wire offset 24 bytes; width 4 bytes.
    pub host_cpu: u32,
    /// Wire offset 28 bytes; width 4 bytes.
    pub migration_target: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeVirtualCpuInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeVirtualCpuInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, vcpu_id) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, phase) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, scheduler_thread_id) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, terminal_reason) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, reserved) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, host_cpu) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeVirtualCpuInfo, migration_target) == 28);

/// Minimum supported prefix of `resource_limits`, in bytes; individual calls define their buffer lengths.
pub const HYPER_NATIVE_RESOURCE_LIMITS_MIN_SIZE: usize = 160;
/// Wire layout for `resource_limits`.
///
/// Size: 160 bytes; alignment: 8 bytes; supported minimum prefix: 160 bytes.
/// Scalar widths and member offsets are fixed across supported architectures.
/// The minimum prefix describes extensibility, not permission to truncate every call's buffer;
/// the consuming operation defines its accepted length.
///
/// Used by: `resource_domain_create`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeResourceLimits {
    /// Wire offset 0 bytes; width 8 bytes.
    pub kernel_memory_bytes: u64,
    /// Wire offset 8 bytes; width 8 bytes.
    pub processes: u64,
    /// Wire offset 16 bytes; width 8 bytes.
    pub threads: u64,
    /// Wire offset 24 bytes; width 8 bytes.
    pub handles: u64,
    /// Wire offset 32 bytes; width 8 bytes.
    pub kernel_objects: u64,
    /// Wire offset 40 bytes; width 8 bytes.
    pub committed_pages: u64,
    /// Wire offset 48 bytes; width 8 bytes.
    pub pinned_pages: u64,
    /// Wire offset 56 bytes; width 8 bytes.
    pub guest_pages: u64,
    /// Wire offset 64 bytes; width 8 bytes.
    pub ipc_messages: u64,
    /// Wire offset 72 bytes; width 8 bytes.
    pub ipc_bytes: u64,
    /// Wire offset 80 bytes; width 8 bytes.
    pub ipc_handles: u64,
    /// Wire offset 88 bytes; width 8 bytes.
    pub subscriptions: u64,
    /// Wire offset 96 bytes; width 8 bytes.
    pub timers: u64,
    /// Wire offset 104 bytes; width 8 bytes.
    pub virtual_machines: u64,
    /// Wire offset 112 bytes; width 8 bytes.
    pub virtual_cpus: u64,
    /// Wire offset 120 bytes; width 8 bytes.
    pub device_leases: u64,
    /// Wire offset 128 bytes; width 8 bytes.
    pub dma_mappings: u64,
    /// Wire offset 136 bytes; width 8 bytes.
    pub user_address_spaces: u64,
    /// Wire offset 144 bytes; width 8 bytes.
    pub user_mappings: u64,
    /// Wire offset 152 bytes; width 8 bytes.
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeResourceLimits>() == 160);
const _: () = assert!(core::mem::align_of::<HyperNativeResourceLimits>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, kernel_memory_bytes) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, processes) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, threads) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, handles) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, kernel_objects) == 32);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, committed_pages) == 40);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, pinned_pages) == 48);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, guest_pages) == 56);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, ipc_messages) == 64);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, ipc_bytes) == 72);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, ipc_handles) == 80);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, subscriptions) == 88);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, timers) == 96);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, virtual_machines) == 104);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, virtual_cpus) == 112);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, device_leases) == 120);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, dma_mappings) == 128);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, user_address_spaces) == 136);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, user_mappings) == 144);
const _: () = assert!(core::mem::offset_of!(HyperNativeResourceLimits, reserved) == 152);
