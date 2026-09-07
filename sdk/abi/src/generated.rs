// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Generated from schema/native.rs. Do not edit.

pub const HYPER_NATIVE_ABI_REVISION: u64 = 0;
pub const HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS: usize = 6;
pub const HYPER_NATIVE_SYSCALL_RESULT_REGISTERS: usize = 2;
pub type HyperNativeHandle = u64;
pub type HyperNativeStatus = i64;

pub const HYPER_NATIVE_FEATURE_CORE: u64 = 1_u64 << 0;

pub const HYPER_NATIVE_STATUS_OK: HyperNativeStatus = 0;
pub const HYPER_NATIVE_STATUS_INVALID_ARGUMENT: HyperNativeStatus = -1;
pub const HYPER_NATIVE_STATUS_BAD_HANDLE: HyperNativeStatus = -2;
pub const HYPER_NATIVE_STATUS_ACCESS_DENIED: HyperNativeStatus = -3;
pub const HYPER_NATIVE_STATUS_NOT_SUPPORTED: HyperNativeStatus = -4;
pub const HYPER_NATIVE_STATUS_NO_MEMORY: HyperNativeStatus = -5;
pub const HYPER_NATIVE_STATUS_BAD_STATE: HyperNativeStatus = -6;
pub const HYPER_NATIVE_STATUS_FAULT: HyperNativeStatus = -7;
pub const HYPER_NATIVE_STATUS_RESOURCE_LIMIT: HyperNativeStatus = -8;
pub const HYPER_NATIVE_STATUS_BUSY: HyperNativeStatus = -9;
pub const HYPER_NATIVE_STATUS_INTERNAL: HyperNativeStatus = -10;
pub const HYPER_NATIVE_STATUS_TIMED_OUT: HyperNativeStatus = -11;
pub const HYPER_NATIVE_STATUS_CANCELLED: HyperNativeStatus = -12;
pub const HYPER_NATIVE_STATUS_WOULD_BLOCK: HyperNativeStatus = -13;
pub const HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL: HyperNativeStatus = -14;
pub const HYPER_NATIVE_STATUS_PEER_CLOSED: HyperNativeStatus = -15;
pub const HYPER_NATIVE_STATUS_NOT_FOUND: HyperNativeStatus = -16;

pub const HYPER_NATIVE_OBJECT_NONE: u32 = 0;
pub const HYPER_NATIVE_OBJECT_EVENT: u32 = 1;
pub const HYPER_NATIVE_OBJECT_BYTE_CHANNEL: u32 = 2;
pub const HYPER_NATIVE_OBJECT_THREAD: u32 = 3;
pub const HYPER_NATIVE_OBJECT_PROCESS: u32 = 4;
pub const HYPER_NATIVE_OBJECT_TASK_GROUP: u32 = 5;
pub const HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN: u32 = 6;
pub const HYPER_NATIVE_OBJECT_TASK_FACTORY: u32 = 7;
pub const HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY: u32 = 8;
pub const HYPER_NATIVE_OBJECT_VMO: u32 = 9;
pub const HYPER_NATIVE_OBJECT_VMAR: u32 = 10;
pub const HYPER_NATIVE_OBJECT_CONSOLE: u32 = 11;
pub const HYPER_NATIVE_OBJECT_DIRECTORY: u32 = 12;
pub const HYPER_NATIVE_OBJECT_FILE: u32 = 13;
pub const HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL: u32 = 14;
pub const HYPER_NATIVE_OBJECT_PROCESS_BUILDER: u32 = 15;
pub const HYPER_NATIVE_OBJECT_TASK_INSPECTOR: u32 = 16;
pub const HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR: u32 = 17;

pub const HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN: u32 = 0;
pub const HYPER_NATIVE_TRANSFER_CLASS_GENERAL: u32 = 1;
pub const HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY: u32 = 2;

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
        _ => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,
    }
}

pub const HYPER_NATIVE_RIGHT_DUPLICATE: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_RIGHT_TRANSFER: u64 = 1_u64 << 1;
pub const HYPER_NATIVE_RIGHT_WAIT: u64 = 1_u64 << 2;
pub const HYPER_NATIVE_RIGHT_INSPECT: u64 = 1_u64 << 3;
pub const HYPER_NATIVE_RIGHT_READ: u64 = 1_u64 << 4;
pub const HYPER_NATIVE_RIGHT_WRITE: u64 = 1_u64 << 5;
pub const HYPER_NATIVE_RIGHT_MAP: u64 = 1_u64 << 6;
pub const HYPER_NATIVE_RIGHT_EXECUTE: u64 = 1_u64 << 7;
pub const HYPER_NATIVE_RIGHT_RESIZE: u64 = 1_u64 << 8;
pub const HYPER_NATIVE_RIGHT_PIN: u64 = 1_u64 << 9;
pub const HYPER_NATIVE_RIGHT_START: u64 = 1_u64 << 10;
pub const HYPER_NATIVE_RIGHT_REQUEST_STOP: u64 = 1_u64 << 11;
pub const HYPER_NATIVE_RIGHT_RUN_VCPU: u64 = 1_u64 << 12;
pub const HYPER_NATIVE_RIGHT_INJECT_INTERRUPT: u64 = 1_u64 << 13;
pub const HYPER_NATIVE_RIGHT_GRANT_MEMORY: u64 = 1_u64 << 14;
pub const HYPER_NATIVE_RIGHT_ASSIGN_DEVICE: u64 = 1_u64 << 15;
pub const HYPER_NATIVE_RIGHT_MAP_DMA: u64 = 1_u64 << 16;
pub const HYPER_NATIVE_RIGHT_ACK_INTERRUPT: u64 = 1_u64 << 17;
pub const HYPER_NATIVE_RIGHT_REVOKE: u64 = 1_u64 << 18;
pub const HYPER_NATIVE_RIGHT_SIGNAL: u64 = 1_u64 << 19;
pub const HYPER_NATIVE_RIGHT_CREATE_PROCESS: u64 = 1_u64 << 20;
pub const HYPER_NATIVE_RIGHT_CREATE_THREAD: u64 = 1_u64 << 21;
pub const HYPER_NATIVE_RIGHT_CREATE_TASK_GROUP: u64 = 1_u64 << 22;
pub const HYPER_NATIVE_RIGHT_CREATE_RESOURCE_DOMAIN: u64 = 1_u64 << 23;
pub const HYPER_NATIVE_RIGHT_SET_LIMITS: u64 = 1_u64 << 24;
pub const HYPER_NATIVE_RIGHT_CREATE_EXECUTABLE: u64 = 1_u64 << 25;
pub const HYPER_NATIVE_RIGHT_TASK_GROUP_ATTACH_PROCESS: u64 = 1_u64 << 26;
pub const HYPER_NATIVE_RIGHT_RESOURCE_DOMAIN_SPONSOR: u64 = 1_u64 << 27;
pub const HYPER_NATIVE_RIGHT_DERIVE: u64 = 1_u64 << 28;

pub const HYPER_NATIVE_RIGHTS_MASK: u64 = 0x1fffffff;

pub const HYPER_NATIVE_SIGNAL_EVENT_SIGNALED: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE: u64 = 1_u64 << 1;
pub const HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED: u64 = 1_u64 << 2;
pub const HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_RECEIVING: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_CLOSED: u64 = 1_u64 << 1;
pub const HYPER_NATIVE_SIGNAL_THREAD_TERMINATED: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_SIGNAL_CONSOLE_READABLE: u64 = 1_u64 << 0;
pub const HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE: u64 = 1_u64 << 1;

pub const HYPER_NATIVE_ELF_OSABI: u64 = 63;
pub const HYPER_NATIVE_ELF_ABI_VERSION: u64 = 0;
pub const HYPER_NATIVE_AUXV_STARTUP_HANDLES: u64 = 1213792257;
pub const HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT: u64 = 1213792258;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN: u64 = 1;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP: u64 = 2;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY: u64 = 3;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY: u64 = 4;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR: u64 = 5;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE: u64 = 6;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY: u64 = 7;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_INSPECTOR: u64 = 8;
pub const HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_INSPECTOR: u64 = 9;
pub const HYPER_NATIVE_STARTUP_MAX_HANDLES: u64 = 256;
pub const HYPER_NATIVE_DEADLINE_INFINITE: u64 = 18446744073709551615;
pub const HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS: u64 = 64;
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS: u64 = 18446744073709551615;
pub const HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES: u64 = 65536;
pub const HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_MESSAGES: u64 = 16;
pub const HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_BYTES: u64 = 1048576;
pub const HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES: u64 = 4096;
pub const HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES: u64 = 16;
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE: u64 = 0;
pub const HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE: u64 = 1;
pub const HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES: u64 = 4096;
pub const HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES: u64 = 4096;
pub const HYPER_NATIVE_FILE_MAX_READ_BYTES: u64 = 65536;
pub const HYPER_NATIVE_PROCESS_NAME_MAX_BYTES: u64 = 64;
pub const HYPER_NATIVE_PROCESS_ARGUMENT_MAX_BYTES: u64 = 4096;
pub const HYPER_NATIVE_PROCESS_ENVIRONMENT_MAX_BYTES: u64 = 4096;
pub const HYPER_NATIVE_PROCESS_MAX_ARGUMENTS: u64 = 64;
pub const HYPER_NATIVE_PROCESS_MAX_ENVIRONMENT: u64 = 64;
pub const HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS: u64 = 4;
pub const HYPER_NATIVE_PROCESS_AFFINITY_MAX_CPUS: u64 = 256;
pub const HYPER_NATIVE_PROCESS_PHASE_PREPARED: u64 = 0;
pub const HYPER_NATIVE_PROCESS_PHASE_CREATED: u64 = 1;
pub const HYPER_NATIVE_PROCESS_PHASE_RUNNING: u64 = 2;
pub const HYPER_NATIVE_PROCESS_PHASE_STOPPING: u64 = 3;
pub const HYPER_NATIVE_PROCESS_PHASE_STOPPED: u64 = 4;
pub const HYPER_NATIVE_PROCESS_PHASE_RETIRING: u64 = 5;
pub const HYPER_NATIVE_PROCESS_PHASE_RETIRED: u64 = 6;
pub const HYPER_NATIVE_PROCESS_TERMINAL_NONE: u64 = 0;
pub const HYPER_NATIVE_PROCESS_TERMINAL_REQUESTED: u64 = 1;
pub const HYPER_NATIVE_PROCESS_TERMINAL_THREAD_EXITED: u64 = 2;
pub const HYPER_NATIVE_PROCESS_TERMINAL_PROCESS_EXITED: u64 = 3;
pub const HYPER_NATIVE_PROCESS_TERMINAL_LAST_THREAD_EXITED: u64 = 4;
pub const HYPER_NATIVE_PROCESS_TERMINAL_FAULT: u64 = 5;
pub const HYPER_NATIVE_PROCESS_TERMINAL_TASK_GROUP_STOP: u64 = 6;
pub const HYPER_NATIVE_TASK_INSPECTOR_PROCESS_PAGE_CAPACITY: u64 = 8;
pub const HYPER_NATIVE_TASK_INSPECTOR_THREAD_PAGE_CAPACITY: u64 = 8;
pub const HYPER_NATIVE_OBJECT_INSPECTOR_OBJECT_PAGE_CAPACITY: u64 = 8;
pub const HYPER_NATIVE_OBJECT_INSPECTOR_HANDLE_PAGE_CAPACITY: u64 = 8;
pub const HYPER_NATIVE_THREAD_ROLE_BOOTSTRAP: u64 = 1;
pub const HYPER_NATIVE_THREAD_ROLE_IDLE: u64 = 2;
pub const HYPER_NATIVE_THREAD_ROLE_KERNEL: u64 = 3;
pub const HYPER_NATIVE_THREAD_ROLE_USER: u64 = 4;
pub const HYPER_NATIVE_THREAD_ROLE_VCPU: u64 = 5;
pub const HYPER_NATIVE_THREAD_REGISTRY_RESIDENT: u64 = 1;
pub const HYPER_NATIVE_THREAD_REGISTRY_RETIRING: u64 = 2;
pub const HYPER_NATIVE_OBJECT_HANDLE_STATE_UNPUBLISHED: u64 = 1;
pub const HYPER_NATIVE_OBJECT_HANDLE_STATE_ACTIVE: u64 = 2;
pub const HYPER_NATIVE_OBJECT_HANDLE_STATE_RETIRED: u64 = 3;

pub const HYPER_NATIVE_SYS_ABI_QUERY: u64 = 0;
pub const HYPER_NATIVE_SYS_HANDLE_CLOSE: u64 = 1;
pub const HYPER_NATIVE_SYS_HANDLE_DUPLICATE: u64 = 2;
pub const HYPER_NATIVE_SYS_HANDLE_REPLACE: u64 = 3;
pub const HYPER_NATIVE_SYS_HANDLE_GET_INFO: u64 = 4;
pub const HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO: u64 = 5;
pub const HYPER_NATIVE_SYS_THREAD_YIELD: u64 = 6;
pub const HYPER_NATIVE_SYS_THREAD_EXIT: u64 = 7;
pub const HYPER_NATIVE_SYS_PROCESS_EXIT: u64 = 8;
pub const HYPER_NATIVE_SYS_EVENT_CREATE: u64 = 9;
pub const HYPER_NATIVE_SYS_EVENT_SIGNAL: u64 = 10;
pub const HYPER_NATIVE_SYS_OBJECT_WAIT_ONE: u64 = 11;
pub const HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE: u64 = 12;
pub const HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE: u64 = 13;
pub const HYPER_NATIVE_SYS_BYTE_CHANNEL_READ: u64 = 14;
pub const HYPER_NATIVE_SYS_CONSOLE_READ: u64 = 15;
pub const HYPER_NATIVE_SYS_CONSOLE_WRITE: u64 = 16;
pub const HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE: u64 = 17;
pub const HYPER_NATIVE_SYS_FILE_READ_AT: u64 = 18;
pub const HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE: u64 = 19;
pub const HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND: u64 = 20;
pub const HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE: u64 = 21;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE: u64 = 22;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME: u64 = 23;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT: u64 = 24;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT: u64 = 25;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY: u64 = 26;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE: u64 = 27;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL: u64 = 28;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_START: u64 = 29;
pub const HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT: u64 = 30;
pub const HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP: u64 = 31;
pub const HYPER_NATIVE_SYS_OBJECT_WAIT_MANY: u64 = 32;
pub const HYPER_NATIVE_SYS_PROCESS_GET_INFO: u64 = 33;
pub const HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES: u64 = 34;
pub const HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS: u64 = 35;
pub const HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS: u64 = 36;
pub const HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS: u64 = 37;
pub const HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES: u64 = 38;
pub const HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS: u64 = 39;
pub const HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP: u64 = 40;
pub const HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP: u64 = 41;
pub const HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN: u64 = 42;
pub const HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN: u64 = 43;

pub const fn hyper_native_failure_result_mask(
    syscall_number: u64,
    status: HyperNativeStatus,
) -> u64 {
    match (syscall_number, status) {
        (HYPER_NATIVE_SYS_BYTE_CHANNEL_READ, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) => 1,
        (HYPER_NATIVE_SYS_CONSOLE_READ, HYPER_NATIVE_STATUS_WOULD_BLOCK) => 1,
        (HYPER_NATIVE_SYS_CONSOLE_WRITE, HYPER_NATIVE_STATUS_WOULD_BLOCK) => 1,
        (HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) => 3,
        _ => 0,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeHandleInfo {
    pub object_kind: u32,
    pub flags: u32,
    pub rights: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeHandleInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeHandleInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInfo, object_kind) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInfo, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeHandleInfo, rights) == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectBasicInfo {
    pub koid: u64,
    pub object_kind: u32,
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectBasicInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeObjectBasicInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectBasicInfo, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectBasicInfo, object_kind) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectBasicInfo, reserved) == 12);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectWaitItem {
    pub handle: u64,
    pub signals: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectWaitItem>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeObjectWaitItem>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectWaitItem, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeObjectWaitItem, signals) == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeProcessInfo {
    pub phase: u32,
    pub terminal_reason: u32,
    pub detail0: u64,
    pub detail1: u64,
    pub reserved: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeProcessInfo>() == 32);
const _: () = assert!(core::mem::align_of::<HyperNativeProcessInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, phase) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, terminal_reason) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, detail0) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, detail1) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeProcessInfo, reserved) == 24);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCapabilityDisposition {
    pub handle: u64,
    pub rights: u64,
    pub expected_kind: u32,
    pub operation: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCapabilityDisposition>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeCapabilityDisposition>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, rights) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, expected_kind) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityDisposition, operation) == 20);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeCapabilityReceiveSlot {
    pub handle: u64,
    pub rights: u64,
    pub expected_kind: u32,
    pub flags: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeCapabilityReceiveSlot>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeCapabilityReceiveSlot>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, rights) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, expected_kind) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeCapabilityReceiveSlot, flags) == 20);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeStartupHandle {
    pub purpose: u32,
    pub flags: u32,
    pub handle: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeStartupHandle>() == 16);
const _: () = assert!(core::mem::align_of::<HyperNativeStartupHandle>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeStartupHandle, purpose) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeStartupHandle, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperNativeStartupHandle, handle) == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTaskProcess {
    pub koid: u64,
    pub phase: u32,
    pub terminal_reason: u32,
    pub pending_threads: u32,
    pub active_threads: u32,
    pub name_length: u32,
    pub reserved: u32,
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeTaskThread {
    pub koid: u64,
    pub process_koid: u64,
    pub role: u32,
    pub registry_phase: u32,
    pub name_length: u32,
    pub reserved: u32,
    pub name: [u8; 64],
}
const _: () = assert!(core::mem::size_of::<HyperNativeTaskThread>() == 96);
const _: () = assert!(core::mem::align_of::<HyperNativeTaskThread>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, process_koid) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, role) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, registry_phase) == 20);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, name_length) == 24);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, reserved) == 28);
const _: () = assert!(core::mem::offset_of!(HyperNativeTaskThread, name) == 32);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeObjectInspection {
    pub koid: u64,
    pub object_kind: u32,
    pub handle_state: u32,
    pub active_handles: u64,
    pub supported_rights: u64,
    pub strong_references: u64,
    pub kernel_service_references: u64,
    pub scheduler_references: u64,
    pub operation_references: u64,
    pub user_authority_references: u64,
    pub publication_references: u64,
    pub diagnostic_references: u64,
    pub retirement_references: u64,
}
const _: () = assert!(core::mem::size_of::<HyperNativeObjectInspection>() == 96);
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperNativeHandleInspection {
    pub process_koid: u64,
    pub handle: u64,
    pub object_koid: u64,
    pub rights: u64,
    pub object_kind: u32,
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
