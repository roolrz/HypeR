// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Generated from abi/native/schema.rs. Do not edit.
// This ABI is experimental and unpublished. Names, numbers, and layouts may change.

pub const HYPER_EXPERIMENTAL_ABI_REVISION: u64 = 0;
pub const HYPER_EXPERIMENTAL_SYSCALL_ARGUMENT_REGISTERS: usize = 6;
pub const HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS: usize = 2;
pub type HyperExperimentalHandle = u64;
pub type HyperExperimentalStatus = i64;

pub const HYPER_EXPERIMENTAL_FEATURE_CORE: u64 = 1;

pub const HYPER_EXPERIMENTAL_OBJECT_NONE: u32 = 0;

pub const HYPER_EXPERIMENTAL_RIGHT_DUPLICATE: u64 = 1;
pub const HYPER_EXPERIMENTAL_RIGHT_TRANSFER: u64 = 2;
pub const HYPER_EXPERIMENTAL_RIGHT_WAIT: u64 = 4;
pub const HYPER_EXPERIMENTAL_RIGHT_INSPECT: u64 = 8;
pub const HYPER_EXPERIMENTAL_RIGHT_READ: u64 = 16;
pub const HYPER_EXPERIMENTAL_RIGHT_WRITE: u64 = 32;
pub const HYPER_EXPERIMENTAL_RIGHT_MAP: u64 = 64;
pub const HYPER_EXPERIMENTAL_RIGHT_EXECUTE: u64 = 128;
pub const HYPER_EXPERIMENTAL_RIGHT_RESIZE: u64 = 256;
pub const HYPER_EXPERIMENTAL_RIGHT_PIN: u64 = 512;
pub const HYPER_EXPERIMENTAL_RIGHT_START: u64 = 1024;
pub const HYPER_EXPERIMENTAL_RIGHT_REQUEST_STOP: u64 = 2048;
pub const HYPER_EXPERIMENTAL_RIGHT_RUN_VCPU: u64 = 4096;
pub const HYPER_EXPERIMENTAL_RIGHT_INJECT_INTERRUPT: u64 = 8192;
pub const HYPER_EXPERIMENTAL_RIGHT_GRANT_MEMORY: u64 = 16384;
pub const HYPER_EXPERIMENTAL_RIGHT_ASSIGN_DEVICE: u64 = 32768;
pub const HYPER_EXPERIMENTAL_RIGHT_MAP_DMA: u64 = 65536;
pub const HYPER_EXPERIMENTAL_RIGHT_ACK_INTERRUPT: u64 = 131072;
pub const HYPER_EXPERIMENTAL_RIGHT_REVOKE: u64 = 262144;

pub const HYPER_EXPERIMENTAL_RIGHTS_MASK: u64 = 524287;

pub const HYPER_EXPERIMENTAL_SYS_ABI_QUERY: u64 = 0;
pub const HYPER_EXPERIMENTAL_SYS_HANDLE_CLOSE: u64 = 1;
pub const HYPER_EXPERIMENTAL_SYS_HANDLE_DUPLICATE: u64 = 2;
pub const HYPER_EXPERIMENTAL_SYS_HANDLE_REPLACE: u64 = 3;
pub const HYPER_EXPERIMENTAL_SYS_HANDLE_GET_INFO: u64 = 4;
pub const HYPER_EXPERIMENTAL_SYS_OBJECT_GET_BASIC_INFO: u64 = 5;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperExperimentalHandleInfo {
    pub object_kind: u32,
    pub flags: u32,
    pub rights: u64,
}
const _: () = assert!(core::mem::size_of::<HyperExperimentalHandleInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperExperimentalHandleInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperExperimentalHandleInfo, object_kind) == 0);
const _: () = assert!(core::mem::offset_of!(HyperExperimentalHandleInfo, flags) == 4);
const _: () = assert!(core::mem::offset_of!(HyperExperimentalHandleInfo, rights) == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HyperExperimentalObjectBasicInfo {
    pub koid: u64,
    pub object_kind: u32,
    pub reserved: u32,
}
const _: () = assert!(core::mem::size_of::<HyperExperimentalObjectBasicInfo>() == 16);
const _: () = assert!(core::mem::align_of::<HyperExperimentalObjectBasicInfo>() == 8);
const _: () = assert!(core::mem::offset_of!(HyperExperimentalObjectBasicInfo, koid) == 0);
const _: () = assert!(core::mem::offset_of!(HyperExperimentalObjectBasicInfo, object_kind) == 8);
const _: () = assert!(core::mem::offset_of!(HyperExperimentalObjectBasicInfo, reserved) == 12);
