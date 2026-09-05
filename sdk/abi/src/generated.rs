// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Generated from schema/native.rs. Do not edit.

pub const HYPER_NATIVE_ABI_REVISION: u64 = 0;
pub const HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS: usize = 6;
pub const HYPER_NATIVE_SYSCALL_RESULT_REGISTERS: usize = 2;
pub type HyperNativeHandle = u64;
pub type HyperNativeStatus = i64;

pub const HYPER_NATIVE_FEATURE_CORE: u64 = 1;

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

pub const HYPER_NATIVE_OBJECT_NONE: u32 = 0;
pub const HYPER_NATIVE_OBJECT_EVENT: u32 = 1;
pub const HYPER_NATIVE_OBJECT_CHANNEL: u32 = 2;
pub const HYPER_NATIVE_OBJECT_THREAD: u32 = 3;
pub const HYPER_NATIVE_OBJECT_PROCESS: u32 = 4;
pub const HYPER_NATIVE_OBJECT_TASK_GROUP: u32 = 5;
pub const HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN: u32 = 6;
pub const HYPER_NATIVE_OBJECT_TASK_FACTORY: u32 = 7;
pub const HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY: u32 = 8;
pub const HYPER_NATIVE_OBJECT_VMO: u32 = 9;
pub const HYPER_NATIVE_OBJECT_VMAR: u32 = 10;
pub const HYPER_NATIVE_OBJECT_CONSOLE: u32 = 11;

pub const HYPER_NATIVE_RIGHT_DUPLICATE: u64 = 1;
pub const HYPER_NATIVE_RIGHT_TRANSFER: u64 = 2;
pub const HYPER_NATIVE_RIGHT_WAIT: u64 = 4;
pub const HYPER_NATIVE_RIGHT_INSPECT: u64 = 8;
pub const HYPER_NATIVE_RIGHT_READ: u64 = 16;
pub const HYPER_NATIVE_RIGHT_WRITE: u64 = 32;
pub const HYPER_NATIVE_RIGHT_MAP: u64 = 64;
pub const HYPER_NATIVE_RIGHT_EXECUTE: u64 = 128;
pub const HYPER_NATIVE_RIGHT_RESIZE: u64 = 256;
pub const HYPER_NATIVE_RIGHT_PIN: u64 = 512;
pub const HYPER_NATIVE_RIGHT_START: u64 = 1024;
pub const HYPER_NATIVE_RIGHT_REQUEST_STOP: u64 = 2048;
pub const HYPER_NATIVE_RIGHT_RUN_VCPU: u64 = 4096;
pub const HYPER_NATIVE_RIGHT_INJECT_INTERRUPT: u64 = 8192;
pub const HYPER_NATIVE_RIGHT_GRANT_MEMORY: u64 = 16384;
pub const HYPER_NATIVE_RIGHT_ASSIGN_DEVICE: u64 = 32768;
pub const HYPER_NATIVE_RIGHT_MAP_DMA: u64 = 65536;
pub const HYPER_NATIVE_RIGHT_ACK_INTERRUPT: u64 = 131072;
pub const HYPER_NATIVE_RIGHT_REVOKE: u64 = 262144;
pub const HYPER_NATIVE_RIGHT_SIGNAL: u64 = 524288;
pub const HYPER_NATIVE_RIGHT_CREATE_PROCESS: u64 = 1048576;
pub const HYPER_NATIVE_RIGHT_CREATE_THREAD: u64 = 2097152;
pub const HYPER_NATIVE_RIGHT_CREATE_TASK_GROUP: u64 = 4194304;
pub const HYPER_NATIVE_RIGHT_CREATE_RESOURCE_DOMAIN: u64 = 8388608;
pub const HYPER_NATIVE_RIGHT_SET_LIMITS: u64 = 16777216;
pub const HYPER_NATIVE_RIGHT_CREATE_EXECUTABLE: u64 = 33554432;

pub const HYPER_NATIVE_RIGHTS_MASK: u64 = 67108863;

pub const HYPER_NATIVE_SIGNAL_EVENT_SIGNALED: u64 = 1;
pub const HYPER_NATIVE_SIGNAL_CHANNEL_READABLE: u64 = 1;
pub const HYPER_NATIVE_SIGNAL_CHANNEL_WRITABLE: u64 = 2;
pub const HYPER_NATIVE_SIGNAL_CHANNEL_PEER_CLOSED: u64 = 4;
pub const HYPER_NATIVE_SIGNAL_THREAD_TERMINATED: u64 = 1;
pub const HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED: u64 = 1;
pub const HYPER_NATIVE_SIGNAL_CONSOLE_READABLE: u64 = 1;
pub const HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE: u64 = 2;

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
pub const HYPER_NATIVE_DEADLINE_INFINITE: u64 = 18446744073709551615;
pub const HYPER_NATIVE_CHANNEL_DISPOSITION_SAME_RIGHTS: u64 = 18446744073709551615;
pub const HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES: u64 = 65536;
pub const HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES: u64 = 64;
pub const HYPER_NATIVE_CHANNEL_MAX_QUEUED_MESSAGES: u64 = 16;
pub const HYPER_NATIVE_CHANNEL_MAX_QUEUED_BYTES: u64 = 1048576;
pub const HYPER_NATIVE_CHANNEL_MAX_QUEUED_HANDLES: u64 = 1024;
pub const HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES: u64 = 4096;

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
pub const HYPER_NATIVE_SYS_CHANNEL_CREATE: u64 = 12;
pub const HYPER_NATIVE_SYS_CHANNEL_WRITE: u64 = 13;
pub const HYPER_NATIVE_SYS_CHANNEL_READ: u64 = 14;
pub const HYPER_NATIVE_SYS_CONSOLE_READ: u64 = 15;
pub const HYPER_NATIVE_SYS_CONSOLE_WRITE: u64 = 16;

pub const fn hyper_native_failure_result_mask(
    syscall_number: u64,
    status: HyperNativeStatus,
) -> u64 {
    match (syscall_number, status) {
        (HYPER_NATIVE_SYS_CHANNEL_READ, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) => 3,
        (HYPER_NATIVE_SYS_CONSOLE_READ, HYPER_NATIVE_STATUS_WOULD_BLOCK) => 1,
        (HYPER_NATIVE_SYS_CONSOLE_WRITE, HYPER_NATIVE_STATUS_WOULD_BLOCK) => 1,
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
pub struct HyperNativeChannelDisposition {
    pub handle: u64,
    pub rights: u64,
    pub expected_kind: u32,
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperNativeChannelDisposition>() == 24);
const _: () = assert!(core::mem::align_of::<HyperNativeChannelDisposition>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeChannelDisposition, handle) == 0);
const _: () = assert!(core::mem::offset_of!(HyperNativeChannelDisposition, rights) == 8);
const _: () = assert!(core::mem::offset_of!(HyperNativeChannelDisposition, expected_kind) == 16);
const _: () = assert!(core::mem::offset_of!(HyperNativeChannelDisposition, reserved) == 20);

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
