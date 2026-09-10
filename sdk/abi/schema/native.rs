// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Compiler-checked source of truth for the `HypeR` Native ABI.
//!
//! This module deliberately depends only on `core`. Host tools, build scripts,
//! and the kernel may include it directly without acquiring a parser or schema
//! dependency. The schema describes machine-visible values; it does not contain
//! kernel dispatch policy or handler implementations.

#![allow(dead_code)]

/// Pre-release ABI revision.
///
/// `HypeR` does not start ABI versioning until the project explicitly publishes
/// its first supported userspace ABI. Keep this value at zero during
/// pre-release development, regardless of schema changes.
pub const ABI_REVISION: u64 = 0;
pub const SYSCALL_ARGUMENT_REGISTERS: usize = 6;
pub const SYSCALL_RESULT_REGISTERS: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbiSchema {
    pub revision: u64,
    pub features: &'static [Feature],
    pub statuses: &'static [Status],
    pub object_kinds: &'static [ObjectKind],
    pub rights: &'static [Right],
    pub signals: &'static [Signal],
    pub constants: &'static [AbiConstant],
    pub records: &'static [Record],
    pub syscalls: &'static [Syscall],
    pub semantic_rules: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    pub value: i64,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Feature {
    pub bit: u8,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectKind {
    pub value: u32,
    pub name: &'static str,
    pub transfer: TransferClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferClass {
    Forbidden,
    General,
    RendezvousOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Right {
    pub bit: u8,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signal {
    pub object: &'static str,
    pub bit: u8,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbiConstant {
    pub name: &'static str,
    pub value: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    pub name: &'static str,
    pub fields: &'static [Field],
    /// Smallest prefix which preserves this record's original ABI contract.
    ///
    /// Future extensions may increase `size`, but must not increase this value
    /// while the original prefix remains supported.
    pub minimum_size: u16,
    pub size: u16,
    pub alignment: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: &'static str,
    pub kind: FieldKind,
    pub offset: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    U32,
    U64,
    Bytes(u16),
}

impl FieldKind {
    pub const fn size(self) -> u16 {
        match self {
            Self::U32 => 4,
            Self::U64 => 8,
            Self::Bytes(size) => size,
        }
    }

    pub const fn alignment(self) -> u8 {
        match self {
            Self::U32 => 4,
            Self::U64 => 8,
            Self::Bytes(_) => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Syscall {
    pub number: u32,
    pub name: &'static str,
    pub feature: FeatureGate,
    pub arguments: &'static [Argument],
    pub results: &'static [ResultValue],
    pub blocking: BlockingClass,
    pub cancellation: CancellationClass,
    pub restart: RestartClass,
    pub completion: CompletionClass,
    pub audit: AuditClass,
    pub flags: FlagPolicy,
    /// Auxiliary results which remain defined for specific failure statuses.
    pub failure_results: &'static [FailureResults],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureResults {
    pub status: &'static str,
    pub results: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureGate {
    Core,
    Named(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argument {
    pub name: &'static str,
    pub kind: ValueKind,
    pub handle: Option<HandleArgument>,
    pub memory: Option<UserMemory>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultValue {
    pub name: &'static str,
    pub kind: ValueKind,
    pub handle: Option<ProducedHandle>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    U32,
    U64,
    I64,
    Handle,
    UserAddress,
    ByteCount,
    ElementCount,
    Rights,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleArgument {
    pub object: ObjectConstraint,
    pub required_rights: u64,
    pub disposition: HandleDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectConstraint {
    Any,
    Kind(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleDisposition {
    Borrow,
    ConsumeOnCommit,
    ByOperation {
        argument: &'static str,
        operations: &'static [HandleOperation],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleOperation {
    pub name: &'static str,
    pub value: u32,
    pub disposition: OperationDisposition,
    /// Rights required in addition to the handle argument's common rights.
    pub additional_rights: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationDisposition {
    Borrow,
    ConsumeOnCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProducedHandle {
    pub object: ProducedObject,
    pub rights: ProducedRights,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducedObject {
    SameAsArgument(&'static str),
    Kind(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducedRights {
    RequestedSubsetOf(&'static str),
    ExactRequested {
        argument: &'static str,
        allowed_rights: u64,
        /// Optional source handle which must already carry every requested
        /// right before the produced handle can be published.
        authority_source: Option<&'static str>,
    },
    Fixed(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserMemory {
    pub direction: MemoryDirection,
    pub length: MemoryLength,
    pub record: Option<&'static str>,
    pub handles: Option<IndirectHandles>,
    pub validation_order: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLength {
    Bytes {
        argument: &'static str,
        maximum_bytes: u32,
    },
    Elements {
        argument: &'static str,
        maximum_elements: u32,
        element_size: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndirectHandles {
    /// Input records borrow handles for the duration of one syscall.
    BorrowRecords {
        handle_field: &'static str,
        required_rights: u64,
    },
    /// Input records describe conditional borrow-or-move operations which all
    /// commit together only when the syscall returns `OK`.
    ConsumeRecords {
        handle_field: &'static str,
        rights_field: &'static str,
        expected_kind_field: &'static str,
        operation_field: &'static str,
        common_rights: u64,
        operations: &'static [HandleOperation],
        commit: CapabilityCommit,
    },
    /// In/out records declare exact receive authority before the kernel
    /// publishes transferred handles into them.
    ProduceTransferred {
        handle_field: &'static str,
        rights_field: &'static str,
        expected_kind_field: &'static str,
        flags_field: &'static str,
        commit: CapabilityCommit,
    },
}

const OBJECT_WAIT_MANY_MAX_ITEMS: u32 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityCommit {
    /// Capability ownership and live-handle publication commit only on `OK`.
    ///
    /// User copies may have partially modified output storage when the call
    /// reports `FAULT`; those bytes never constitute a published capability.
    AtomicOnOk,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryDirection {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockingClass {
    Never,
    MayBlock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationClass {
    None,
    Explicit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartClass {
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionClass {
    Returns,
    NoReturn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditClass {
    Abi,
    Capability,
    Object,
    Task,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagPolicy {
    None,
    Strict,
    Flexible,
}

pub const FEATURES: &[Feature] = &[Feature {
    bit: 0,
    name: "core",
}];

pub const STATUSES: &[Status] = &[
    Status {
        value: 0,
        name: "ok",
    },
    Status {
        value: -1,
        name: "invalid_argument",
    },
    Status {
        value: -2,
        name: "bad_handle",
    },
    Status {
        value: -3,
        name: "access_denied",
    },
    Status {
        value: -4,
        name: "not_supported",
    },
    Status {
        value: -5,
        name: "no_memory",
    },
    Status {
        value: -6,
        name: "bad_state",
    },
    Status {
        value: -7,
        name: "fault",
    },
    Status {
        value: -8,
        name: "resource_limit",
    },
    Status {
        value: -9,
        name: "busy",
    },
    Status {
        value: -10,
        name: "internal",
    },
    Status {
        value: -11,
        name: "timed_out",
    },
    Status {
        value: -12,
        name: "cancelled",
    },
    Status {
        value: -13,
        name: "would_block",
    },
    Status {
        value: -14,
        name: "buffer_too_small",
    },
    Status {
        value: -15,
        name: "peer_closed",
    },
    Status {
        value: -16,
        name: "not_found",
    },
    Status {
        value: -17,
        name: "already_exists",
    },
    Status {
        value: -18,
        name: "not_empty",
    },
];

const RIGHT_DUPLICATE_BIT: u8 = 0;
const RIGHT_INSPECT_BIT: u8 = 3;
const RIGHT_SIGNAL_BIT: u8 = 19;
const RIGHT_CREATE_PROCESS_BIT: u8 = 20;
const RIGHT_CREATE_THREAD_BIT: u8 = 21;
const RIGHT_CREATE_TASK_GROUP_BIT: u8 = 22;
const RIGHT_CREATE_RESOURCE_DOMAIN_BIT: u8 = 23;
const RIGHT_SET_LIMITS_BIT: u8 = 24;
const RIGHT_CREATE_EXECUTABLE_BIT: u8 = 25;
const RIGHT_TASK_GROUP_ATTACH_PROCESS_BIT: u8 = 26;
const RIGHT_RESOURCE_DOMAIN_SPONSOR_BIT: u8 = 27;
const RIGHT_DERIVE_BIT: u8 = 28;
const RIGHT_CREATE_VIRTUAL_MACHINE_BIT: u8 = 29;
const CAPABILITY_OPERATION_MOVE: u32 = 0;
const CAPABILITY_OPERATION_DUPLICATE: u32 = 1;

pub const OBJECT_KINDS: &[ObjectKind] = &[
    ObjectKind {
        value: 0,
        name: "none",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 1,
        name: "event",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 2,
        name: "byte_channel",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 3,
        name: "thread",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 4,
        name: "process",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 5,
        name: "task_group",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 6,
        name: "resource_domain",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 7,
        name: "task_factory",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 8,
        name: "executable_authority",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 9,
        name: "vmo",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 10,
        name: "vmar",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 11,
        name: "console",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 12,
        name: "directory",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 13,
        name: "file",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 14,
        name: "capability_channel",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 15,
        name: "process_builder",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 16,
        name: "task_inspector",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 17,
        name: "object_inspector",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 18,
        name: "memory_inspector",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 19,
        name: "cpu_inspector",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 20,
        name: "virtual_machine_creation_authority",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 21,
        name: "virtual_machine_creation_lease",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 22,
        name: "pending_virtual_machine",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 23,
        name: "virtual_machine",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 24,
        name: "virtual_cpu",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 25,
        name: "virtual_serial",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 26,
        name: "wait_set",
        transfer: TransferClass::Forbidden,
    },
];

pub const RIGHTS: &[Right] = &[
    Right {
        bit: 30,
        name: "bind_wait",
    },
    Right {
        bit: RIGHT_DUPLICATE_BIT,
        name: "duplicate",
    },
    Right {
        bit: 1,
        name: "transfer",
    },
    Right {
        bit: 2,
        name: "wait",
    },
    Right {
        bit: RIGHT_INSPECT_BIT,
        name: "inspect",
    },
    Right {
        bit: 4,
        name: "read",
    },
    Right {
        bit: 5,
        name: "write",
    },
    Right {
        bit: 6,
        name: "map",
    },
    Right {
        bit: 7,
        name: "execute",
    },
    Right {
        bit: 8,
        name: "resize",
    },
    Right {
        bit: 9,
        name: "pin",
    },
    Right {
        bit: 10,
        name: "start",
    },
    Right {
        bit: 11,
        name: "request_stop",
    },
    Right {
        bit: 12,
        name: "run_vcpu",
    },
    Right {
        bit: 13,
        name: "inject_interrupt",
    },
    Right {
        bit: 14,
        name: "grant_memory",
    },
    Right {
        bit: 15,
        name: "assign_device",
    },
    Right {
        bit: 16,
        name: "map_dma",
    },
    Right {
        bit: 17,
        name: "ack_interrupt",
    },
    Right {
        bit: 18,
        name: "revoke",
    },
    Right {
        bit: RIGHT_SIGNAL_BIT,
        name: "signal",
    },
    Right {
        bit: RIGHT_CREATE_PROCESS_BIT,
        name: "create_process",
    },
    Right {
        bit: RIGHT_CREATE_THREAD_BIT,
        name: "create_thread",
    },
    Right {
        bit: RIGHT_CREATE_TASK_GROUP_BIT,
        name: "create_task_group",
    },
    Right {
        bit: RIGHT_CREATE_RESOURCE_DOMAIN_BIT,
        name: "create_resource_domain",
    },
    Right {
        bit: RIGHT_SET_LIMITS_BIT,
        name: "set_limits",
    },
    Right {
        bit: RIGHT_CREATE_EXECUTABLE_BIT,
        name: "create_executable",
    },
    Right {
        bit: RIGHT_TASK_GROUP_ATTACH_PROCESS_BIT,
        name: "task_group_attach_process",
    },
    Right {
        bit: RIGHT_RESOURCE_DOMAIN_SPONSOR_BIT,
        name: "resource_domain_sponsor",
    },
    Right {
        bit: RIGHT_DERIVE_BIT,
        name: "derive",
    },
    Right {
        bit: RIGHT_CREATE_VIRTUAL_MACHINE_BIT,
        name: "create_virtual_machine",
    },
];

pub const RIGHT_DUPLICATE: u64 = 1 << RIGHT_DUPLICATE_BIT;
pub const RIGHT_INSPECT: u64 = 1 << RIGHT_INSPECT_BIT;
pub const RIGHT_TRANSFER: u64 = 1 << 1;
pub const RIGHT_WAIT: u64 = 1 << 2;
pub const RIGHT_SIGNAL: u64 = 1 << RIGHT_SIGNAL_BIT;
pub const RIGHT_READ: u64 = 1 << 4;
pub const RIGHT_WRITE: u64 = 1 << 5;
pub const RIGHT_MAP: u64 = 1 << 6;
pub const RIGHT_EXECUTE: u64 = 1 << 7;
pub const RIGHT_START: u64 = 1 << 10;
pub const RIGHT_REQUEST_STOP: u64 = 1 << 11;
pub const RIGHT_RUN_VCPU: u64 = 1 << 12;
pub const RIGHT_INJECT_INTERRUPT: u64 = 1 << 13;
pub const RIGHT_ASSIGN_DEVICE: u64 = 1 << 15;
pub const RIGHT_REVOKE: u64 = 1 << 18;
pub const RIGHT_CREATE_PROCESS: u64 = 1 << RIGHT_CREATE_PROCESS_BIT;
pub const RIGHT_CREATE_THREAD: u64 = 1 << RIGHT_CREATE_THREAD_BIT;
pub const RIGHT_CREATE_TASK_GROUP: u64 = 1 << RIGHT_CREATE_TASK_GROUP_BIT;
pub const RIGHT_CREATE_RESOURCE_DOMAIN: u64 = 1 << RIGHT_CREATE_RESOURCE_DOMAIN_BIT;
pub const RIGHT_SET_LIMITS: u64 = 1 << RIGHT_SET_LIMITS_BIT;
pub const RIGHT_CREATE_EXECUTABLE: u64 = 1 << RIGHT_CREATE_EXECUTABLE_BIT;
pub const RIGHT_TASK_GROUP_ATTACH_PROCESS: u64 = 1 << RIGHT_TASK_GROUP_ATTACH_PROCESS_BIT;
pub const RIGHT_RESOURCE_DOMAIN_SPONSOR: u64 = 1 << RIGHT_RESOURCE_DOMAIN_SPONSOR_BIT;
pub const RIGHT_DERIVE: u64 = 1 << RIGHT_DERIVE_BIT;
pub const RIGHT_CREATE_VIRTUAL_MACHINE: u64 = 1 << RIGHT_CREATE_VIRTUAL_MACHINE_BIT;

pub const RIGHT_BIND_WAIT: u64 = 1 << 30;

pub const EVENT_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_SIGNAL;
pub const BYTE_CHANNEL_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE;
pub const CAPABILITY_CHANNEL_RIGHTS: u64 = BYTE_CHANNEL_RIGHTS | RIGHT_DUPLICATE;
pub const DIRECTORY_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE | RIGHT_EXECUTE;
pub const FILE_RIGHTS: u64 = DIRECTORY_RIGHTS;
pub const VMO_WRITABLE_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE | RIGHT_MAP;
pub const VMO_EXECUTABLE_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_READ | RIGHT_EXECUTE | RIGHT_MAP;
pub const VMAR_RIGHTS: u64 = RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_MAP;
pub const PROCESS_BUILDER_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_WRITE | RIGHT_START | RIGHT_REQUEST_STOP;
pub const PROCESS_SUPERVISOR_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_REQUEST_STOP;
pub const TASK_INSPECTOR_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_DERIVE;
pub const OBJECT_INSPECTOR_RIGHTS: u64 = TASK_INSPECTOR_RIGHTS;
pub const VIRTUAL_MACHINE_CREATION_AUTHORITY_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_DERIVE | RIGHT_CREATE_VIRTUAL_MACHINE;
pub const VIRTUAL_MACHINE_CREATION_LEASE_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_CREATE_VIRTUAL_MACHINE;
pub const PENDING_VIRTUAL_MACHINE_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_WRITE | RIGHT_START | RIGHT_REQUEST_STOP;
pub const VIRTUAL_MACHINE_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_REQUEST_STOP;
pub const VIRTUAL_CPU_RIGHTS: u64 = RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_START;
pub const VIRTUAL_SERIAL_RIGHTS: u64 = RIGHT_DUPLICATE
    | RIGHT_TRANSFER
    | RIGHT_INSPECT
    | RIGHT_READ
    | RIGHT_WRITE
    | RIGHT_WAIT
    | RIGHT_ASSIGN_DEVICE;

pub const CAPABILITY_OPERATIONS: &[HandleOperation] = &[
    HandleOperation {
        name: "move",
        value: CAPABILITY_OPERATION_MOVE,
        disposition: OperationDisposition::ConsumeOnCommit,
        additional_rights: 0,
    },
    HandleOperation {
        name: "duplicate",
        value: CAPABILITY_OPERATION_DUPLICATE,
        disposition: OperationDisposition::Borrow,
        additional_rights: RIGHT_DUPLICATE,
    },
];

pub const SIGNALS: &[Signal] = &[
    Signal {
        object: "virtual_serial",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "virtual_serial",
        bit: 1,
        name: "writable",
    },
    Signal {
        object: "virtual_serial",
        bit: 2,
        name: "peer_closed",
    },
    Signal {
        object: "wait_set",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "event",
        bit: 0,
        name: "signaled",
    },
    Signal {
        object: "byte_channel",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "byte_channel",
        bit: 1,
        name: "writable",
    },
    Signal {
        object: "byte_channel",
        bit: 2,
        name: "peer_closed",
    },
    Signal {
        object: "capability_channel",
        bit: 0,
        name: "peer_receiving",
    },
    Signal {
        object: "capability_channel",
        bit: 1,
        name: "peer_closed",
    },
    Signal {
        object: "thread",
        bit: 0,
        name: "terminated",
    },
    Signal {
        object: "process",
        bit: 0,
        name: "terminated",
    },
    Signal {
        object: "console",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "console",
        bit: 1,
        name: "writable",
    },
    Signal {
        object: "virtual_machine",
        bit: 0,
        name: "terminated",
    },
    Signal {
        object: "virtual_cpu",
        bit: 0,
        name: "terminated",
    },
];

const CONSOLE_MAX_TRANSFER_BYTES: u32 = 4 * 1024;
const VIRTUAL_SERIAL_MAX_TRANSFER_BYTES: u32 = 4 * 1024;
const EXTENSIBLE_RECORD_MAX_BYTES: u32 = 4 * 1024;
const DIRECTORY_ENTRY_PAGE_CAPACITY: u32 = 4;
const DIRECTORY_ENTRY_NAME_CAPACITY: u32 = 256;
const DIRECTORY_ENTRY_RECORD_SIZE: u16 = 24 + DIRECTORY_ENTRY_NAME_CAPACITY as u16;

pub const CONSTANTS: &[AbiConstant] = &[
    AbiConstant {
        name: "page_size",
        value: 4096,
    },
    AbiConstant {
        name: "extensible_record_max_bytes",
        value: EXTENSIBLE_RECORD_MAX_BYTES as u64,
    },
    AbiConstant {
        name: "virtual_serial_max_transfer_bytes",
        value: VIRTUAL_SERIAL_MAX_TRANSFER_BYTES as u64,
    },
    AbiConstant {
        name: "virtual_serial_output_capacity",
        value: 65536,
    },
    AbiConstant {
        name: "virtual_serial_output_header_bytes",
        value: 4096,
    },
    AbiConstant {
        name: "virtual_serial_output_bytes",
        value: 4096 + 65536,
    },
    AbiConstant {
        name: "elf_osabi",
        value: 63,
    },
    AbiConstant {
        name: "elf_abi_version",
        value: 0,
    },
    // HypeR-private auxiliary-vector tags. The pointed-to startup-handle
    // array is immutable process-startup data and contains exactly the number
    // of records carried by `auxv_startup_handle_count`.
    AbiConstant {
        name: "auxv_startup_handles",
        value: 0x4859_0001,
    },
    AbiConstant {
        name: "auxv_startup_handle_count",
        value: 0x4859_0002,
    },
    AbiConstant {
        name: "startup_handle_purpose_resource_domain",
        value: 1,
    },
    AbiConstant {
        name: "startup_handle_purpose_task_group",
        value: 2,
    },
    AbiConstant {
        name: "startup_handle_purpose_task_factory",
        value: 3,
    },
    AbiConstant {
        name: "startup_handle_purpose_executable_authority",
        value: 4,
    },
    AbiConstant {
        name: "startup_handle_purpose_root_vmar",
        value: 5,
    },
    AbiConstant {
        name: "startup_handle_purpose_console",
        value: 6,
    },
    AbiConstant {
        name: "startup_handle_purpose_root_directory",
        value: 7,
    },
    AbiConstant {
        name: "startup_handle_purpose_task_inspector",
        value: 8,
    },
    AbiConstant {
        name: "startup_handle_purpose_object_inspector",
        value: 9,
    },
    AbiConstant {
        name: "startup_handle_purpose_dynamic_library_directory",
        value: 10,
    },
    AbiConstant {
        name: "startup_handle_purpose_memory_inspector",
        value: 11,
    },
    AbiConstant {
        name: "startup_handle_purpose_cpu_inspector",
        value: 12,
    },
    AbiConstant {
        name: "startup_handle_purpose_virtual_machine_creation_authority",
        value: 13,
    },
    AbiConstant {
        name: "virtual_machine_architecture_aarch64",
        value: 1,
    },
    AbiConstant {
        name: "virtual_machine_architecture_riscv64",
        value: 2,
    },
    AbiConstant {
        name: "virtual_machine_architecture_x86_64",
        value: 3,
    },
    // Immutable guest-visible layout of the first HypeR AArch64 virtual
    // platform. Kernel device construction and userspace boot metadata must
    // consume these values rather than maintain parallel board descriptions.
    AbiConstant {
        name: "virtual_platform_aarch64_reference",
        value: 1,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_guest_ram_base",
        value: 0x4000_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_dtb_offset",
        value: 0x0001_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_gic_distributor_base",
        value: 0x0800_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_gic_distributor_size",
        value: 0x0001_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_gic_redistributor_base",
        value: 0x080a_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_gic_redistributor_size",
        value: 0x0002_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_uart_base",
        value: 0x0900_0000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_uart_size",
        value: 0x1000,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_uart_interrupt",
        value: 33,
    },
    AbiConstant {
        name: "virtual_platform_aarch64_reference_timer_interrupt",
        value: 27,
    },
    AbiConstant {
        name: "virtual_machine_phase_installed",
        value: 1,
    },
    AbiConstant {
        name: "virtual_machine_phase_running",
        value: 2,
    },
    AbiConstant {
        name: "virtual_machine_phase_stopping",
        value: 3,
    },
    AbiConstant {
        name: "virtual_machine_phase_stopped",
        value: 4,
    },
    AbiConstant {
        name: "virtual_cpu_phase_dormant",
        value: 1,
    },
    AbiConstant {
        name: "virtual_cpu_phase_started",
        value: 2,
    },
    AbiConstant {
        name: "virtual_cpu_phase_stopped",
        value: 3,
    },
    AbiConstant {
        name: "virtual_cpu_terminal_none",
        value: 0,
    },
    AbiConstant {
        name: "virtual_cpu_terminal_memory_fault",
        value: 1,
    },
    AbiConstant {
        name: "virtual_cpu_terminal_mmio",
        value: 2,
    },
    AbiConstant {
        name: "virtual_cpu_terminal_synchronous",
        value: 3,
    },
    AbiConstant {
        name: "virtual_cpu_terminal_administrative",
        value: 4,
    },
    AbiConstant {
        name: "directory_entry_page_capacity",
        value: DIRECTORY_ENTRY_PAGE_CAPACITY as u64,
    },
    AbiConstant {
        name: "directory_entry_name_max_bytes",
        value: (DIRECTORY_ENTRY_NAME_CAPACITY - 1) as u64,
    },
    AbiConstant {
        name: "directory_entry_kind_file",
        value: 1,
    },
    AbiConstant {
        name: "directory_entry_kind_directory",
        value: 2,
    },
    AbiConstant {
        name: "directory_entry_kind_symlink",
        value: 3,
    },
    AbiConstant {
        name: "directory_entry_kind_other",
        value: 4,
    },
    AbiConstant {
        name: "startup_max_handles",
        value: 256,
    },
    AbiConstant {
        name: "deadline_infinite",
        value: u64::MAX,
    },
    AbiConstant {
        name: "object_wait_many_max_items",
        value: OBJECT_WAIT_MANY_MAX_ITEMS as u64,
    },
    AbiConstant {
        name: "capability_disposition_same_rights",
        value: u64::MAX,
    },
    AbiConstant {
        name: "byte_channel_max_message_bytes",
        value: 64 * 1024,
    },
    AbiConstant {
        name: "byte_channel_max_queued_messages",
        value: 16,
    },
    AbiConstant {
        name: "byte_channel_max_queued_bytes",
        value: 16 * 64 * 1024,
    },
    AbiConstant {
        name: "capability_channel_max_message_bytes",
        value: 4 * 1024,
    },
    AbiConstant {
        name: "capability_channel_max_handles",
        value: 16,
    },
    AbiConstant {
        name: "capability_disposition_move",
        value: CAPABILITY_OPERATION_MOVE as u64,
    },
    AbiConstant {
        name: "capability_disposition_duplicate",
        value: CAPABILITY_OPERATION_DUPLICATE as u64,
    },
    AbiConstant {
        name: "console_max_transfer_bytes",
        value: CONSOLE_MAX_TRANSFER_BYTES as u64,
    },
    AbiConstant {
        name: "directory_max_path_bytes",
        value: 4096,
    },
    AbiConstant {
        name: "file_max_read_bytes",
        value: 64 * 1024,
    },
    AbiConstant {
        name: "vmo_max_size_bytes",
        value: 4 * 1024 * 1024 * 1024,
    },
    AbiConstant {
        name: "vmo_max_transfer_bytes",
        value: 64 * 1024,
    },
    AbiConstant {
        name: "vmar_permission_read",
        value: 1 << 0,
    },
    AbiConstant {
        name: "vmar_permission_write",
        value: 1 << 1,
    },
    AbiConstant {
        name: "vmar_permission_execute",
        value: 1 << 2,
    },
    AbiConstant {
        name: "process_name_max_bytes",
        value: 64,
    },
    AbiConstant {
        name: "process_argument_max_bytes",
        value: 4 * 1024,
    },
    AbiConstant {
        name: "process_environment_max_bytes",
        value: 4 * 1024,
    },
    AbiConstant {
        name: "process_max_arguments",
        value: 64,
    },
    AbiConstant {
        name: "process_max_environment",
        value: 64,
    },
    AbiConstant {
        name: "process_affinity_max_words",
        value: 4,
    },
    AbiConstant {
        name: "process_affinity_max_cpus",
        value: 256,
    },
    AbiConstant {
        name: "process_phase_prepared",
        value: 0,
    },
    AbiConstant {
        name: "process_phase_created",
        value: 1,
    },
    AbiConstant {
        name: "process_phase_running",
        value: 2,
    },
    AbiConstant {
        name: "process_phase_stopping",
        value: 3,
    },
    AbiConstant {
        name: "process_phase_stopped",
        value: 4,
    },
    AbiConstant {
        name: "process_phase_retiring",
        value: 5,
    },
    AbiConstant {
        name: "process_phase_retired",
        value: 6,
    },
    AbiConstant {
        name: "process_terminal_none",
        value: 0,
    },
    AbiConstant {
        name: "process_terminal_requested",
        value: 1,
    },
    AbiConstant {
        name: "process_terminal_thread_exited",
        value: 2,
    },
    AbiConstant {
        name: "process_terminal_process_exited",
        value: 3,
    },
    AbiConstant {
        name: "process_terminal_last_thread_exited",
        value: 4,
    },
    AbiConstant {
        name: "process_terminal_fault",
        value: 5,
    },
    AbiConstant {
        name: "process_terminal_task_group_stop",
        value: 6,
    },
    AbiConstant {
        name: "task_inspector_process_page_capacity",
        value: 8,
    },
    AbiConstant {
        name: "task_inspector_thread_page_capacity",
        value: 8,
    },
    AbiConstant {
        name: "object_inspector_object_page_capacity",
        value: 8,
    },
    AbiConstant {
        name: "object_inspector_handle_page_capacity",
        value: 8,
    },
    AbiConstant {
        name: "thread_role_bootstrap",
        value: 1,
    },
    AbiConstant {
        name: "thread_role_idle",
        value: 2,
    },
    AbiConstant {
        name: "thread_role_kernel",
        value: 3,
    },
    AbiConstant {
        name: "thread_role_user",
        value: 4,
    },
    AbiConstant {
        name: "thread_role_vcpu",
        value: 5,
    },
    AbiConstant {
        name: "thread_registry_resident",
        value: 1,
    },
    AbiConstant {
        name: "thread_registry_retiring",
        value: 2,
    },
    AbiConstant {
        name: "object_handle_state_unpublished",
        value: 1,
    },
    AbiConstant {
        name: "object_handle_state_active",
        value: 2,
    },
    AbiConstant {
        name: "object_handle_state_retired",
        value: 3,
    },
];

const HANDLE_INFO_FIELDS: &[Field] = &[
    Field {
        name: "object_kind",
        kind: FieldKind::U32,
        offset: 0,
    },
    Field {
        name: "flags",
        kind: FieldKind::U32,
        offset: 4,
    },
    Field {
        name: "rights",
        kind: FieldKind::U64,
        offset: 8,
    },
];

const OBJECT_BASIC_INFO_FIELDS: &[Field] = &[
    Field {
        name: "koid",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "object_kind",
        kind: FieldKind::U32,
        offset: 8,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 12,
    },
];

const OBJECT_WAIT_ITEM_FIELDS: &[Field] = &[
    Field {
        name: "handle",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "signals",
        kind: FieldKind::U64,
        offset: 8,
    },
];

const PROCESS_INFO_FIELDS: &[Field] = &[
    Field {
        name: "phase",
        kind: FieldKind::U32,
        offset: 0,
    },
    Field {
        name: "terminal_reason",
        kind: FieldKind::U32,
        offset: 4,
    },
    Field {
        name: "detail0",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "detail1",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 24,
    },
];

const CAPABILITY_DISPOSITION_FIELDS: &[Field] = &[
    Field {
        name: "handle",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "rights",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "expected_kind",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "operation",
        kind: FieldKind::U32,
        offset: 20,
    },
];

const CAPABILITY_RECEIVE_SLOT_FIELDS: &[Field] = &[
    Field {
        name: "handle",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "rights",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "expected_kind",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "flags",
        kind: FieldKind::U32,
        offset: 20,
    },
];

const STARTUP_HANDLE_FIELDS: &[Field] = &[
    Field {
        name: "purpose",
        kind: FieldKind::U32,
        offset: 0,
    },
    Field {
        name: "flags",
        kind: FieldKind::U32,
        offset: 4,
    },
    Field {
        name: "handle",
        kind: FieldKind::U64,
        offset: 8,
    },
];

const TASK_PROCESS_FIELDS: &[Field] = &[
    Field {
        name: "koid",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "phase",
        kind: FieldKind::U32,
        offset: 8,
    },
    Field {
        name: "terminal_reason",
        kind: FieldKind::U32,
        offset: 12,
    },
    Field {
        name: "pending_threads",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "active_threads",
        kind: FieldKind::U32,
        offset: 20,
    },
    Field {
        name: "name_length",
        kind: FieldKind::U32,
        offset: 24,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 28,
    },
    Field {
        name: "name",
        kind: FieldKind::Bytes(64),
        offset: 32,
    },
];

const TASK_THREAD_FIELDS: &[Field] = &[
    Field {
        name: "koid",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "process_koid",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "role",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "registry_phase",
        kind: FieldKind::U32,
        offset: 20,
    },
    Field {
        name: "name_length",
        kind: FieldKind::U32,
        offset: 24,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 28,
    },
    Field {
        name: "runtime_ticks",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "name",
        kind: FieldKind::Bytes(64),
        offset: 40,
    },
];

const MEMORY_OBSERVATION_FIELDS: &[Field] = &[
    Field {
        name: "captured_at_ns",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "page_size",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "total_bytes",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "reserved_bytes",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "managed_bytes",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "free_bytes",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "used_bytes",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "kernel_bytes",
        kind: FieldKind::U64,
        offset: 56,
    },
    Field {
        name: "heap_bytes",
        kind: FieldKind::U64,
        offset: 64,
    },
    Field {
        name: "page_table_bytes",
        kind: FieldKind::U64,
        offset: 72,
    },
    Field {
        name: "user_bytes",
        kind: FieldKind::U64,
        offset: 80,
    },
    Field {
        name: "guest_bytes",
        kind: FieldKind::U64,
        offset: 88,
    },
    Field {
        name: "unattributed_bytes",
        kind: FieldKind::U64,
        offset: 96,
    },
    Field {
        name: "reclaimable_bytes",
        kind: FieldKind::U64,
        offset: 104,
    },
];

const CPU_OBSERVATION_FIELDS: &[Field] = &[
    Field {
        name: "captured_at_ns",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "ticks_per_second",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "online_cpus",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "idle_ticks",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "kernel_thread_ticks",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "user_thread_ticks",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "vcpu_ticks",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 56,
    },
];

const OBJECT_INSPECTION_FIELDS: &[Field] = &[
    Field {
        name: "koid",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "object_kind",
        kind: FieldKind::U32,
        offset: 8,
    },
    Field {
        name: "handle_state",
        kind: FieldKind::U32,
        offset: 12,
    },
    Field {
        name: "active_handles",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "supported_rights",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "strong_references",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "kernel_service_references",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "scheduler_references",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "operation_references",
        kind: FieldKind::U64,
        offset: 56,
    },
    Field {
        name: "user_authority_references",
        kind: FieldKind::U64,
        offset: 64,
    },
    Field {
        name: "publication_references",
        kind: FieldKind::U64,
        offset: 72,
    },
    Field {
        name: "diagnostic_references",
        kind: FieldKind::U64,
        offset: 80,
    },
    Field {
        name: "retirement_references",
        kind: FieldKind::U64,
        offset: 88,
    },
    Field {
        name: "vm_device_binding_references",
        kind: FieldKind::U64,
        offset: 96,
    },
];

const HANDLE_INSPECTION_FIELDS: &[Field] = &[
    Field {
        name: "process_koid",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "handle",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "object_koid",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "rights",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "object_kind",
        kind: FieldKind::U32,
        offset: 32,
    },
    Field {
        name: "flags",
        kind: FieldKind::U32,
        offset: 36,
    },
];

const DIRECTORY_ENTRY_FIELDS: &[Field] = &[
    Field {
        name: "size",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "mode",
        kind: FieldKind::U32,
        offset: 8,
    },
    Field {
        name: "kind",
        kind: FieldKind::U32,
        offset: 12,
    },
    Field {
        name: "name_length",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 20,
    },
    Field {
        name: "name",
        kind: FieldKind::Bytes(DIRECTORY_ENTRY_NAME_CAPACITY as u16),
        offset: 24,
    },
];

const FILE_INFO_FIELDS: &[Field] = &[
    Field {
        name: "filesystem_id",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "mount_id",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "node_id",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "size",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "mode",
        kind: FieldKind::U32,
        offset: 32,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 36,
    },
];

const DIRECTORY_INFO_FIELDS: &[Field] = &[
    Field {
        name: "filesystem_id",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "mount_id",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "node_id",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "mode",
        kind: FieldKind::U32,
        offset: 24,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 28,
    },
];

const VIRTUAL_MACHINE_CONFIGURATION_FIELDS: &[Field] = &[
    Field {
        name: "guest_physical_base",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "memory_size",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "vcpu_count",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "architecture",
        kind: FieldKind::U32,
        offset: 20,
    },
    Field {
        name: "platform_profile",
        kind: FieldKind::U32,
        offset: 24,
    },
    Field {
        name: "flags",
        kind: FieldKind::U32,
        offset: 28,
    },
];

const VIRTUAL_CPU_BOOTSTRAP_FIELDS: &[Field] = &[
    Field {
        name: "entry",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "stack",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "argument0",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "argument1",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "argument2",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "argument3",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "vcpu_id",
        kind: FieldKind::U32,
        offset: 48,
    },
    Field {
        name: "flags",
        kind: FieldKind::U32,
        offset: 52,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 56,
    },
];

const VIRTUAL_MACHINE_INFO_FIELDS: &[Field] = &[
    Field {
        name: "phase",
        kind: FieldKind::U32,
        offset: 0,
    },
    Field {
        name: "vcpu_count",
        kind: FieldKind::U32,
        offset: 4,
    },
    Field {
        name: "guest_physical_base",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "memory_size",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "architecture",
        kind: FieldKind::U32,
        offset: 24,
    },
    Field {
        name: "platform_profile",
        kind: FieldKind::U32,
        offset: 28,
    },
];

const VIRTUAL_CPU_INFO_FIELDS: &[Field] = &[
    Field {
        name: "vcpu_id",
        kind: FieldKind::U32,
        offset: 0,
    },
    Field {
        name: "phase",
        kind: FieldKind::U32,
        offset: 4,
    },
    Field {
        name: "scheduler_thread_id",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "terminal_reason",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 20,
    },
];

const RESOURCE_LIMITS_FIELDS: &[Field] = &[
    Field {
        name: "kernel_memory_bytes",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "processes",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "threads",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "handles",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "kernel_objects",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "committed_pages",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "pinned_pages",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "guest_pages",
        kind: FieldKind::U64,
        offset: 56,
    },
    Field {
        name: "ipc_messages",
        kind: FieldKind::U64,
        offset: 64,
    },
    Field {
        name: "ipc_bytes",
        kind: FieldKind::U64,
        offset: 72,
    },
    Field {
        name: "ipc_handles",
        kind: FieldKind::U64,
        offset: 80,
    },
    Field {
        name: "subscriptions",
        kind: FieldKind::U64,
        offset: 88,
    },
    Field {
        name: "timers",
        kind: FieldKind::U64,
        offset: 96,
    },
    Field {
        name: "virtual_machines",
        kind: FieldKind::U64,
        offset: 104,
    },
    Field {
        name: "virtual_cpus",
        kind: FieldKind::U64,
        offset: 112,
    },
    Field {
        name: "device_leases",
        kind: FieldKind::U64,
        offset: 120,
    },
    Field {
        name: "dma_mappings",
        kind: FieldKind::U64,
        offset: 128,
    },
    Field {
        name: "user_address_spaces",
        kind: FieldKind::U64,
        offset: 136,
    },
    Field {
        name: "user_mappings",
        kind: FieldKind::U64,
        offset: 144,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 152,
    },
];

pub const RECORDS: &[Record] = &[
    Record {
        name: "wait_set_event",
        minimum_size: 24,
        size: 24,
        alignment: 8,
        fields: &[
            Field {
                name: "registration",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "signals",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "sequence",
                kind: FieldKind::U64,
                offset: 16,
            },
        ],
    },
    Record {
        name: "handle_info",
        fields: HANDLE_INFO_FIELDS,
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "object_basic_info",
        fields: OBJECT_BASIC_INFO_FIELDS,
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "object_wait_item",
        fields: OBJECT_WAIT_ITEM_FIELDS,
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "process_info",
        fields: PROCESS_INFO_FIELDS,
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "capability_disposition",
        fields: CAPABILITY_DISPOSITION_FIELDS,
        minimum_size: 24,
        size: 24,
        alignment: 8,
    },
    Record {
        name: "capability_receive_slot",
        fields: CAPABILITY_RECEIVE_SLOT_FIELDS,
        minimum_size: 24,
        size: 24,
        alignment: 8,
    },
    Record {
        name: "startup_handle",
        fields: STARTUP_HANDLE_FIELDS,
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "task_process",
        fields: TASK_PROCESS_FIELDS,
        minimum_size: 96,
        size: 96,
        alignment: 8,
    },
    Record {
        name: "task_thread",
        fields: TASK_THREAD_FIELDS,
        minimum_size: 104,
        size: 104,
        alignment: 8,
    },
    Record {
        name: "memory_observation",
        fields: MEMORY_OBSERVATION_FIELDS,
        minimum_size: 112,
        size: 112,
        alignment: 8,
    },
    Record {
        name: "cpu_observation",
        fields: CPU_OBSERVATION_FIELDS,
        minimum_size: 64,
        size: 64,
        alignment: 8,
    },
    Record {
        name: "object_inspection",
        fields: OBJECT_INSPECTION_FIELDS,
        minimum_size: 104,
        size: 104,
        alignment: 8,
    },
    Record {
        name: "handle_inspection",
        fields: HANDLE_INSPECTION_FIELDS,
        minimum_size: 40,
        size: 40,
        alignment: 8,
    },
    Record {
        name: "directory_entry",
        fields: DIRECTORY_ENTRY_FIELDS,
        minimum_size: DIRECTORY_ENTRY_RECORD_SIZE,
        size: DIRECTORY_ENTRY_RECORD_SIZE,
        alignment: 8,
    },
    Record {
        name: "file_info",
        fields: FILE_INFO_FIELDS,
        minimum_size: 40,
        size: 40,
        alignment: 8,
    },
    Record {
        name: "directory_info",
        fields: DIRECTORY_INFO_FIELDS,
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "virtual_machine_configuration",
        fields: VIRTUAL_MACHINE_CONFIGURATION_FIELDS,
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "virtual_cpu_bootstrap",
        fields: VIRTUAL_CPU_BOOTSTRAP_FIELDS,
        minimum_size: 64,
        size: 64,
        alignment: 8,
    },
    Record {
        name: "virtual_machine_info",
        fields: VIRTUAL_MACHINE_INFO_FIELDS,
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "virtual_cpu_info",
        fields: VIRTUAL_CPU_INFO_FIELDS,
        minimum_size: 24,
        size: 24,
        alignment: 8,
    },
    Record {
        name: "resource_limits",
        fields: RESOURCE_LIMITS_FIELDS,
        minimum_size: 160,
        size: 160,
        alignment: 8,
    },
];

const NO_ARGUMENTS: &[Argument] = &[];
const ABI_QUERY_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "revision",
        kind: ValueKind::U64,
        handle: None,
    },
    ResultValue {
        name: "features",
        kind: ValueKind::U64,
        handle: None,
    },
];
const CLOCK_GET_MONOTONIC_RESULTS: &[ResultValue] = &[ResultValue {
    name: "nanoseconds",
    kind: ValueKind::U64,
    handle: None,
}];
const INFO_RECORD_RESULTS: &[ResultValue] = &[ResultValue {
    name: "supported_size",
    kind: ValueKind::ByteCount,
    handle: None,
}];
const HANDLE_CLOSE_ARGUMENTS: &[Argument] = &[Argument {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: 0,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
}];
const HANDLE_DUPLICATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "source",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: RIGHT_DUPLICATE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "requested_rights",
        kind: ValueKind::Rights,
        handle: None,
        memory: None,
    },
];
const HANDLE_DUPLICATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::SameAsArgument("source"),
        rights: ProducedRights::RequestedSubsetOf("requested_rights"),
    }),
}];
const HANDLE_REPLACE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "source",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: 0,
            disposition: HandleDisposition::ConsumeOnCommit,
        }),
        memory: None,
    },
    Argument {
        name: "requested_rights",
        kind: ValueKind::Rights,
        handle: None,
        memory: None,
    },
];
const HANDLE_REPLACE_RESULTS: &[ResultValue] = HANDLE_DUPLICATE_RESULTS;
const HANDLE_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "handle",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: 0,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("handle_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const OBJECT_GET_BASIC_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "handle",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("object_basic_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const EXIT_ARGUMENTS: &[Argument] = &[Argument {
    name: "status",
    kind: ValueKind::I64,
    handle: None,
    memory: None,
}];
const EVENT_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "options",
    kind: ValueKind::U32,
    handle: None,
    memory: None,
}];
const EVENT_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("event"),
        rights: ProducedRights::Fixed(EVENT_RIGHTS),
    }),
}];
const EVENT_SIGNAL_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "event",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("event"),
            required_rights: RIGHT_SIGNAL,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "clear_mask",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "set_mask",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
];
const OBJECT_WAIT_ONE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "object",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: RIGHT_WAIT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "signals",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "deadline",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
];
const OBJECT_WAIT_ONE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "observed",
    kind: ValueKind::U64,
    handle: None,
}];

const OBJECT_WAIT_MANY_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "items",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "item_count",
                maximum_elements: OBJECT_WAIT_MANY_MAX_ITEMS,
                element_size: 16,
            },
            record: Some("object_wait_item"),
            handles: Some(IndirectHandles::BorrowRecords {
                handle_field: "handle",
                required_rights: RIGHT_WAIT,
            }),
            validation_order: 0,
        }),
    },
    Argument {
        name: "item_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "deadline",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
];
const OBJECT_WAIT_MANY_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "index",
        kind: ValueKind::ElementCount,
        handle: None,
    },
    ResultValue {
        name: "observed",
        kind: ValueKind::U64,
        handle: None,
    },
];

const CHANNEL_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "options",
    kind: ValueKind::U32,
    handle: None,
    memory: None,
}];
const CHANNEL_CREATE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "endpoint0",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("byte_channel"),
            rights: ProducedRights::Fixed(BYTE_CHANNEL_RIGHTS),
        }),
    },
    ResultValue {
        name: "endpoint1",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("byte_channel"),
            rights: ProducedRights::Fixed(BYTE_CHANNEL_RIGHTS),
        }),
    },
];
const CHANNEL_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "endpoint",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("byte_channel"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const CHANNEL_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "endpoint",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("byte_channel"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_capacity",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const CHANNEL_READ_RESULTS: &[ResultValue] = &[ResultValue {
    name: "actual_bytes",
    kind: ValueKind::ByteCount,
    handle: None,
}];
const CHANNEL_READ_BUFFER_TOO_SMALL_RESULTS: &[&str] = &["actual_bytes"];
const CHANNEL_READ_FAILURE_RESULTS: &[FailureResults] = &[FailureResults {
    status: "buffer_too_small",
    results: CHANNEL_READ_BUFFER_TOO_SMALL_RESULTS,
}];

const CAPABILITY_CHANNEL_CREATE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "endpoint0",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("capability_channel"),
            rights: ProducedRights::Fixed(CAPABILITY_CHANNEL_RIGHTS),
        }),
    },
    ResultValue {
        name: "endpoint1",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("capability_channel"),
            rights: ProducedRights::Fixed(CAPABILITY_CHANNEL_RIGHTS),
        }),
    },
];

const CAPABILITY_CHANNEL_SEND_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "endpoint",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("capability_channel"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 4 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "dispositions",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "disposition_count",
                maximum_elements: 16,
                element_size: 24,
            },
            record: Some("capability_disposition"),
            handles: Some(IndirectHandles::ConsumeRecords {
                handle_field: "handle",
                rights_field: "rights",
                expected_kind_field: "expected_kind",
                operation_field: "operation",
                common_rights: RIGHT_TRANSFER,
                operations: CAPABILITY_OPERATIONS,
                commit: CapabilityCommit::AtomicOnOk,
            }),
            validation_order: 1,
        }),
    },
    Argument {
        name: "disposition_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

const CAPABILITY_CHANNEL_RECEIVE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "endpoint",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("capability_channel"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "deadline",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_capacity",
                maximum_bytes: 4 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "capability_slots",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::ReadWrite,
            length: MemoryLength::Elements {
                argument: "slot_count",
                maximum_elements: 16,
                element_size: 24,
            },
            record: Some("capability_receive_slot"),
            handles: Some(IndirectHandles::ProduceTransferred {
                handle_field: "handle",
                rights_field: "rights",
                expected_kind_field: "expected_kind",
                flags_field: "flags",
                commit: CapabilityCommit::AtomicOnOk,
            }),
            validation_order: 1,
        }),
    },
    Argument {
        name: "slot_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];
const CAPABILITY_CHANNEL_RECEIVE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "actual_bytes",
        kind: ValueKind::ByteCount,
        handle: None,
    },
    ResultValue {
        name: "actual_capabilities",
        kind: ValueKind::ElementCount,
        handle: None,
    },
];
const CAPABILITY_CHANNEL_RECEIVE_BUFFER_TOO_SMALL_RESULTS: &[&str] =
    &["actual_bytes", "actual_capabilities"];
const CAPABILITY_CHANNEL_RECEIVE_FAILURE_RESULTS: &[FailureResults] = &[FailureResults {
    status: "buffer_too_small",
    results: CAPABILITY_CHANNEL_RECEIVE_BUFFER_TOO_SMALL_RESULTS,
}];

const CONSOLE_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "console",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("console"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_capacity",
                maximum_bytes: CONSOLE_MAX_TRANSFER_BYTES,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const CONSOLE_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "console",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("console"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: CONSOLE_MAX_TRANSFER_BYTES,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const CONSOLE_IO_RESULTS: &[ResultValue] = &[ResultValue {
    name: "actual_bytes",
    kind: ValueKind::ByteCount,
    handle: None,
}];
const CONSOLE_IO_WOULD_BLOCK_RESULTS: &[&str] = &["actual_bytes"];
const CONSOLE_IO_FAILURE_RESULTS: &[FailureResults] = &[FailureResults {
    status: "would_block",
    results: CONSOLE_IO_WOULD_BLOCK_RESULTS,
}];

const DIRECTORY_OPEN_FILE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "directory",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("directory"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "path",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "path_size",
                maximum_bytes: 4096,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "path_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "requested_rights",
        kind: ValueKind::Rights,
        handle: None,
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
];
const DIRECTORY_OPEN_FILE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "file",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("file"),
        rights: ProducedRights::ExactRequested {
            argument: "requested_rights",
            allowed_rights: FILE_RIGHTS,
            authority_source: Some("directory"),
        },
    }),
}];

const DIRECTORY_OPEN_DIRECTORY_RESULTS: &[ResultValue] = &[ResultValue {
    name: "child",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("directory"),
        rights: ProducedRights::ExactRequested {
            argument: "requested_rights",
            allowed_rights: DIRECTORY_RIGHTS,
            authority_source: Some("directory"),
        },
    }),
}];

const DIRECTORY_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "directory",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("directory"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "cookie",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: DIRECTORY_ENTRY_PAGE_CAPACITY,
                element_size: DIRECTORY_ENTRY_RECORD_SIZE,
            },
            record: Some("directory_entry"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
];

const DIRECTORY_READ_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "count",
        kind: ValueKind::ElementCount,
        handle: None,
    },
    ResultValue {
        name: "next_cookie",
        kind: ValueKind::U64,
        handle: None,
    },
];

const FILE_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "file",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("file_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const DIRECTORY_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "directory",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("directory"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("directory_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const VMO_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "size",
    kind: ValueKind::ByteCount,
    handle: None,
    memory: None,
}];
const VMO_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "vmo",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("vmo"),
        rights: ProducedRights::Fixed(VMO_WRITABLE_RIGHTS),
    }),
}];

const FILE_CREATE_EXECUTABLE_VMO_ARGUMENTS: &[Argument] = &[Argument {
    name: "file",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("file"),
        required_rights: RIGHT_READ | RIGHT_EXECUTE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];
const FILE_CREATE_EXECUTABLE_VMO_RESULTS: &[ResultValue] = &[ResultValue {
    name: "vmo",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("vmo"),
        rights: ProducedRights::Fixed(VMO_EXECUTABLE_RIGHTS),
    }),
}];

const VMO_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "vmo",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmo"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "offset",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const VMO_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "vmo",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmo"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "offset",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const VMAR_ALLOCATE_ARGUMENTS: &[Argument] = &[
    vmar_argument(RIGHT_MAP),
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
];
const VMAR_ALLOCATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "child",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("vmar"),
        rights: ProducedRights::Fixed(VMAR_RIGHTS),
    }),
}];

const VMAR_MAP_ARGUMENTS: &[Argument] = &[
    vmar_argument(RIGHT_MAP),
    vmo_argument(RIGHT_MAP),
    scalar_argument("vmo_offset", ValueKind::U64),
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
    scalar_argument("permissions", ValueKind::U32),
];

const VMAR_RANGE_ARGUMENTS: &[Argument] = &[
    vmar_argument(RIGHT_MAP),
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
    scalar_argument("permissions", ValueKind::U32),
];

const VMAR_UNMAP_ARGUMENTS: &[Argument] = &[
    vmar_argument(RIGHT_MAP),
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
];

const VMAR_DESTROY_ARGUMENTS: &[Argument] = &[Argument {
    name: "vmar",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("vmar"),
        required_rights: RIGHT_MAP,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
}];

const FILE_READ_AT_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "file",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: RIGHT_READ,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "offset",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_capacity",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const FILE_WRITE_AT_ARGUMENTS: &[Argument] = &[
    file_argument(RIGHT_WRITE),
    scalar_argument("options", ValueKind::U32),
    scalar_argument("offset", ValueKind::U64),
    Argument {
        name: "input",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "input_size",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("input_size", ValueKind::ByteCount),
];
const fn wait_set_argument(rights: u64) -> Argument {
    Argument {
        name: "wait_set",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("wait_set"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

const fn file_argument(rights: u64) -> Argument {
    Argument {
        name: "file",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

const FILE_READ_AT_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "actual_bytes",
        kind: ValueKind::ByteCount,
        handle: None,
    },
    ResultValue {
        name: "file_size",
        kind: ValueKind::ByteCount,
        handle: None,
    },
];

const PROCESS_BUILDER_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "factory",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_factory"),
            required_rights: RIGHT_CREATE_PROCESS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "group",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_group"),
            required_rights: RIGHT_TASK_GROUP_ATTACH_PROCESS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "domain",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_RESOURCE_DOMAIN_SPONSOR,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "executable",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: RIGHT_EXECUTE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];
const PROCESS_BUILDER_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "builder",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("process_builder"),
        rights: ProducedRights::Fixed(PROCESS_BUILDER_RIGHTS),
    }),
}];

const PROCESS_BUILDER_SET_NAME_ARGUMENTS: &[Argument] = &[
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow),
    Argument {
        name: "name",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "name_size",
                maximum_bytes: 64,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "name_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const PROCESS_BUILDER_ADD_ARGUMENT_ARGUMENTS: &[Argument] = &[
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow),
    Argument {
        name: "argument",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "argument_size",
                maximum_bytes: 4 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "argument_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const PROCESS_BUILDER_ADD_ENVIRONMENT_ARGUMENTS: &[Argument] = &[
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow),
    Argument {
        name: "environment",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "environment_size",
                maximum_bytes: 4 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "environment_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const PROCESS_BUILDER_SET_AFFINITY_ARGUMENTS: &[Argument] = &[
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow),
    Argument {
        name: "affinity_words",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "word_count",
                maximum_elements: 4,
                element_size: 8,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "word_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

const PROCESS_BUILDER_ADD_HANDLE_ARGUMENTS: &[Argument] = &[
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow),
    Argument {
        name: "source",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: RIGHT_TRANSFER,
            disposition: HandleDisposition::ByOperation {
                argument: "operation",
                operations: CAPABILITY_OPERATIONS,
            },
        }),
        memory: None,
    },
    Argument {
        name: "purpose",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "expected_kind",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "rights",
        kind: ValueKind::Rights,
        handle: None,
        memory: None,
    },
    Argument {
        name: "operation",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
];

const PROCESS_BUILDER_SEAL_ARGUMENTS: &[Argument] = &[process_builder_argument(
    RIGHT_WRITE,
    HandleDisposition::Borrow,
)];
const PROCESS_BUILDER_START_ARGUMENTS: &[Argument] = &[process_builder_argument(
    RIGHT_START,
    HandleDisposition::ConsumeOnCommit,
)];
const PROCESS_BUILDER_START_RESULTS: &[ResultValue] = &[ResultValue {
    name: "process",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("process"),
        rights: ProducedRights::Fixed(PROCESS_SUPERVISOR_RIGHTS),
    }),
}];
const PROCESS_BUILDER_ABORT_ARGUMENTS: &[Argument] = &[process_builder_argument(
    RIGHT_REQUEST_STOP,
    HandleDisposition::ConsumeOnCommit,
)];

const PROCESS_REQUEST_STOP_ARGUMENTS: &[Argument] = &[Argument {
    name: "process",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("process"),
        required_rights: RIGHT_REQUEST_STOP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

const PROCESS_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "process",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("process"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "info",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "info_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("process_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "info_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const TASK_INSPECTOR_SCAN_PROCESSES_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_inspector"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 96,
            },
            record: Some("task_process"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

const TASK_INSPECTOR_SCAN_THREADS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_inspector"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 104,
            },
            record: Some("task_thread"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

const OBJECT_INSPECTOR_SCAN_OBJECTS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("object_inspector"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 104,
            },
            record: Some("object_inspection"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

const OBJECT_INSPECTOR_SCAN_HANDLES_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("object_inspector"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "process_koid",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 40,
            },
            record: Some("handle_inspection"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

const INSPECTOR_SCAN_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "count",
        kind: ValueKind::ElementCount,
        handle: None,
    },
    ResultValue {
        name: "next_cursor",
        kind: ValueKind::U64,
        handle: None,
    },
];

const TASK_INSPECTOR_DERIVE_PROCESS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_inspector"),
            required_rights: TASK_INSPECTOR_RIGHTS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "process",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("process"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const TASK_INSPECTOR_DERIVE_PROCESS_RESULTS: &[ResultValue] = &[ResultValue {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("task_inspector"),
        rights: ProducedRights::Fixed(TASK_INSPECTOR_RIGHTS),
    }),
}];

const OBJECT_INSPECTOR_DERIVE_PROCESS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("object_inspector"),
            required_rights: OBJECT_INSPECTOR_RIGHTS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "process",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("process"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const OBJECT_INSPECTOR_DERIVE_PROCESS_RESULTS: &[ResultValue] = &[ResultValue {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("object_inspector"),
        rights: ProducedRights::Fixed(OBJECT_INSPECTOR_RIGHTS),
    }),
}];

const MEMORY_INSPECTOR_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("memory_inspector"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "observation",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "observation_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("memory_observation"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "observation_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const CPU_INSPECTOR_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("cpu_inspector"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "observation",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "observation_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("cpu_observation"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "observation_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const TASK_INSPECTOR_DERIVE_TASK_GROUP_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_inspector"),
            required_rights: TASK_INSPECTOR_RIGHTS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "task_group",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_group"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const OBJECT_INSPECTOR_DERIVE_TASK_GROUP_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("object_inspector"),
            required_rights: OBJECT_INSPECTOR_RIGHTS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "task_group",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_group"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_inspector"),
            required_rights: TASK_INSPECTOR_RIGHTS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "resource_domain",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "inspector",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("object_inspector"),
            required_rights: OBJECT_INSPECTOR_RIGHTS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "resource_domain",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const fn process_builder_argument(
    required_rights: u64,
    disposition: HandleDisposition,
) -> Argument {
    Argument {
        name: "builder",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("process_builder"),
            required_rights,
            disposition,
        }),
        memory: None,
    }
}

const fn scalar_result(name: &'static str, kind: ValueKind) -> ResultValue {
    ResultValue {
        name,
        kind,
        handle: None,
    }
}

const fn scalar_argument(name: &'static str, kind: ValueKind) -> Argument {
    Argument {
        name,
        kind,
        handle: None,
        memory: None,
    }
}

const fn vmar_argument(required_rights: u64) -> Argument {
    Argument {
        name: "vmar",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmar"),
            required_rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

const fn vmo_argument(required_rights: u64) -> Argument {
    Argument {
        name: "vmo",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmo"),
            required_rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

const VIRTUAL_MACHINE_CREATION_LEASE_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "authority",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_machine_creation_authority"),
            required_rights: RIGHT_DERIVE | RIGHT_CREATE_VIRTUAL_MACHINE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "resource_domain",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_RESOURCE_DOMAIN_SPONSOR,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];
const VIRTUAL_MACHINE_CREATION_LEASE_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "lease",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("virtual_machine_creation_lease"),
        rights: ProducedRights::Fixed(VIRTUAL_MACHINE_CREATION_LEASE_RIGHTS),
    }),
}];

const RESOURCE_DOMAIN_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "parent",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_CREATE_RESOURCE_DOMAIN,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "limits",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "limits_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("resource_limits"),
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("limits_size", ValueKind::ByteCount),
];
const RESOURCE_DOMAIN_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "child",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("resource_domain"),
        rights: ProducedRights::Fixed(
            RIGHT_DUPLICATE
                | RIGHT_TRANSFER
                | RIGHT_INSPECT
                | RIGHT_CREATE_RESOURCE_DOMAIN
                | RIGHT_SET_LIMITS
                | RIGHT_REVOKE
                | RIGHT_RESOURCE_DOMAIN_SPONSOR,
        ),
    }),
}];

const TASK_GROUP_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "factory",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_factory"),
            required_rights: RIGHT_CREATE_TASK_GROUP,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "resource_domain",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_RESOURCE_DOMAIN_SPONSOR,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];
const TASK_GROUP_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "group",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("task_group"),
        rights: ProducedRights::Fixed(
            RIGHT_DUPLICATE
                | RIGHT_TRANSFER
                | RIGHT_INSPECT
                | RIGHT_REQUEST_STOP
                | RIGHT_TASK_GROUP_ATTACH_PROCESS,
        ),
    }),
}];

const VIRTUAL_MACHINE_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "lease",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_machine_creation_lease"),
            required_rights: RIGHT_CREATE_VIRTUAL_MACHINE,
            disposition: HandleDisposition::ConsumeOnCommit,
        }),
        memory: None,
    },
    Argument {
        name: "configuration",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "configuration_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_machine_configuration"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "configuration_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
const VIRTUAL_MACHINE_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "pending_virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("pending_virtual_machine"),
        rights: ProducedRights::Fixed(PENDING_VIRTUAL_MACHINE_RIGHTS),
    }),
}];

const PENDING_VIRTUAL_MACHINE_SET_MEMORY_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "pending_virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("pending_virtual_machine"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "vmo",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmo"),
            required_rights: RIGHT_READ | RIGHT_WRITE | RIGHT_MAP,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "pending_virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("pending_virtual_machine"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "virtual_serial",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_serial"),
            required_rights: RIGHT_TRANSFER | RIGHT_ASSIGN_DEVICE,
            disposition: HandleDisposition::ConsumeOnCommit,
        }),
        memory: None,
    },
];

const VIRTUAL_SERIAL_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "virtual_serial",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("virtual_serial"),
        rights: ProducedRights::Fixed(VIRTUAL_SERIAL_RIGHTS),
    }),
}];

const VIRTUAL_SERIAL_REGISTER_OUTPUT_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "virtual_serial",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_serial"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "buffer",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmo"),
            required_rights: RIGHT_READ | RIGHT_WRITE | RIGHT_MAP,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];

const VIRTUAL_SERIAL_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "virtual_serial",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_serial"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: VIRTUAL_SERIAL_MAX_TRANSFER_BYTES,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const VIRTUAL_SERIAL_IO_RESULTS: &[ResultValue] = &[ResultValue {
    name: "actual_bytes",
    kind: ValueKind::ByteCount,
    handle: None,
}];

const PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "pending_virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("pending_virtual_machine"),
            required_rights: RIGHT_WRITE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "bootstrap",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "bootstrap_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_cpu_bootstrap"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "bootstrap_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

const fn pending_virtual_machine_argument(
    required_rights: u64,
    disposition: HandleDisposition,
) -> Argument {
    Argument {
        name: "pending_virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("pending_virtual_machine"),
            required_rights,
            disposition,
        }),
        memory: None,
    }
}

const PENDING_VIRTUAL_MACHINE_SEAL_ARGUMENTS: &[Argument] = &[pending_virtual_machine_argument(
    RIGHT_WRITE,
    HandleDisposition::Borrow,
)];
const PENDING_VIRTUAL_MACHINE_INSTALL_ARGUMENTS: &[Argument] = &[pending_virtual_machine_argument(
    RIGHT_START,
    HandleDisposition::ConsumeOnCommit,
)];
const PENDING_VIRTUAL_MACHINE_INSTALL_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("virtual_machine"),
            rights: ProducedRights::Fixed(VIRTUAL_MACHINE_RIGHTS),
        }),
    },
    ResultValue {
        name: "boot_virtual_cpu",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("virtual_cpu"),
            rights: ProducedRights::Fixed(VIRTUAL_CPU_RIGHTS),
        }),
    },
];
const PENDING_VIRTUAL_MACHINE_ABORT_ARGUMENTS: &[Argument] = &[pending_virtual_machine_argument(
    RIGHT_REQUEST_STOP,
    HandleDisposition::ConsumeOnCommit,
)];

const VIRTUAL_MACHINE_REQUEST_STOP_ARGUMENTS: &[Argument] = &[Argument {
    name: "virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_machine"),
        required_rights: RIGHT_REQUEST_STOP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

const VIRTUAL_MACHINE_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_machine"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "info",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "info_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_machine_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("info_size", ValueKind::ByteCount),
];

const VIRTUAL_CPU_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "virtual_cpu",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("virtual_cpu"),
            required_rights: RIGHT_INSPECT,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "info",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "info_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_cpu_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("info_size", ValueKind::ByteCount),
];

const VIRTUAL_CPU_START_ARGUMENTS: &[Argument] = &[Argument {
    name: "virtual_cpu",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_cpu"),
        required_rights: RIGHT_START,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

const THREAD_CREATE_ARGUMENTS: &[Argument] = &[
    scalar_argument("entry", ValueKind::U64),
    scalar_argument("stack", ValueKind::U64),
    scalar_argument("tls", ValueKind::U64),
    scalar_argument("argument", ValueKind::U64),
];
const THREAD_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "thread",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("thread"),
        rights: ProducedRights::Fixed(
            RIGHT_DUPLICATE | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_START | RIGHT_REQUEST_STOP,
        ),
    }),
}];
const fn thread_argument(rights: u64) -> Argument {
    Argument {
        name: "thread",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("thread"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}
const ATOMIC_WAIT_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("expected", ValueKind::U32),
    scalar_argument("deadline", ValueKind::U64),
];
const ATOMIC_WAKE_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("count", ValueKind::U32),
];

pub const SYSCALLS: &[Syscall] = &[
    Syscall {
        number: 0,
        name: "abi_query",
        feature: FeatureGate::Core,
        arguments: NO_ARGUMENTS,
        results: ABI_QUERY_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Abi,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 1,
        name: "handle_close",
        feature: FeatureGate::Core,
        arguments: HANDLE_CLOSE_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 2,
        name: "handle_duplicate",
        feature: FeatureGate::Core,
        arguments: HANDLE_DUPLICATE_ARGUMENTS,
        results: HANDLE_DUPLICATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 3,
        name: "handle_replace",
        feature: FeatureGate::Core,
        arguments: HANDLE_REPLACE_ARGUMENTS,
        results: HANDLE_REPLACE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 4,
        name: "handle_get_info",
        feature: FeatureGate::Core,
        arguments: HANDLE_GET_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 5,
        name: "object_get_basic_info",
        feature: FeatureGate::Core,
        arguments: OBJECT_GET_BASIC_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 6,
        name: "thread_yield",
        feature: FeatureGate::Core,
        arguments: NO_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 7,
        name: "thread_exit",
        feature: FeatureGate::Core,
        arguments: EXIT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::NoReturn,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 8,
        name: "process_exit",
        feature: FeatureGate::Core,
        arguments: EXIT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::NoReturn,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 9,
        name: "event_create",
        feature: FeatureGate::Core,
        arguments: EVENT_CREATE_ARGUMENTS,
        results: EVENT_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 10,
        name: "event_signal",
        feature: FeatureGate::Core,
        arguments: EVENT_SIGNAL_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 11,
        name: "object_wait_one",
        feature: FeatureGate::Core,
        arguments: OBJECT_WAIT_ONE_ARGUMENTS,
        results: OBJECT_WAIT_ONE_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::Explicit,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 12,
        name: "byte_channel_create",
        feature: FeatureGate::Core,
        arguments: CHANNEL_CREATE_ARGUMENTS,
        results: CHANNEL_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 13,
        name: "byte_channel_write",
        feature: FeatureGate::Core,
        arguments: CHANNEL_WRITE_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 14,
        name: "byte_channel_read",
        feature: FeatureGate::Core,
        arguments: CHANNEL_READ_ARGUMENTS,
        results: CHANNEL_READ_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: CHANNEL_READ_FAILURE_RESULTS,
    },
    Syscall {
        number: 15,
        name: "console_read",
        feature: FeatureGate::Core,
        arguments: CONSOLE_READ_ARGUMENTS,
        results: CONSOLE_IO_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: CONSOLE_IO_FAILURE_RESULTS,
    },
    Syscall {
        number: 16,
        name: "console_write",
        feature: FeatureGate::Core,
        arguments: CONSOLE_WRITE_ARGUMENTS,
        results: CONSOLE_IO_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: CONSOLE_IO_FAILURE_RESULTS,
    },
    Syscall {
        number: 17,
        name: "directory_open_file",
        feature: FeatureGate::Core,
        arguments: DIRECTORY_OPEN_FILE_ARGUMENTS,
        results: DIRECTORY_OPEN_FILE_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 18,
        name: "file_read_at",
        feature: FeatureGate::Core,
        arguments: FILE_READ_AT_ARGUMENTS,
        results: FILE_READ_AT_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 19,
        name: "capability_channel_create",
        feature: FeatureGate::Core,
        arguments: CHANNEL_CREATE_ARGUMENTS,
        results: CAPABILITY_CHANNEL_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 20,
        name: "capability_channel_try_send",
        feature: FeatureGate::Core,
        arguments: CAPABILITY_CHANNEL_SEND_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 21,
        name: "capability_channel_receive",
        feature: FeatureGate::Core,
        arguments: CAPABILITY_CHANNEL_RECEIVE_ARGUMENTS,
        results: CAPABILITY_CHANNEL_RECEIVE_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::Explicit,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: CAPABILITY_CHANNEL_RECEIVE_FAILURE_RESULTS,
    },
    Syscall {
        number: 22,
        name: "process_builder_create",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_CREATE_ARGUMENTS,
        results: PROCESS_BUILDER_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 23,
        name: "process_builder_set_name",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_SET_NAME_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 24,
        name: "process_builder_add_argument",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_ADD_ARGUMENT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 25,
        name: "process_builder_add_environment",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_ADD_ENVIRONMENT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 26,
        name: "process_builder_set_affinity",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_SET_AFFINITY_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 27,
        name: "process_builder_add_handle",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_ADD_HANDLE_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 28,
        name: "process_builder_seal",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_SEAL_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 29,
        name: "process_builder_start",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_START_ARGUMENTS,
        results: PROCESS_BUILDER_START_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 30,
        name: "process_builder_abort",
        feature: FeatureGate::Core,
        arguments: PROCESS_BUILDER_ABORT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 31,
        name: "process_request_stop",
        feature: FeatureGate::Core,
        arguments: PROCESS_REQUEST_STOP_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 32,
        name: "object_wait_many",
        feature: FeatureGate::Core,
        arguments: OBJECT_WAIT_MANY_ARGUMENTS,
        results: OBJECT_WAIT_MANY_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::Explicit,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 33,
        name: "process_get_info",
        feature: FeatureGate::Core,
        arguments: PROCESS_GET_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 34,
        name: "task_inspector_scan_processes",
        feature: FeatureGate::Core,
        arguments: TASK_INSPECTOR_SCAN_PROCESSES_ARGUMENTS,
        results: INSPECTOR_SCAN_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 35,
        name: "task_inspector_scan_threads",
        feature: FeatureGate::Core,
        arguments: TASK_INSPECTOR_SCAN_THREADS_ARGUMENTS,
        results: INSPECTOR_SCAN_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 36,
        name: "task_inspector_derive_process",
        feature: FeatureGate::Core,
        arguments: TASK_INSPECTOR_DERIVE_PROCESS_ARGUMENTS,
        results: TASK_INSPECTOR_DERIVE_PROCESS_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 37,
        name: "object_inspector_scan_objects",
        feature: FeatureGate::Core,
        arguments: OBJECT_INSPECTOR_SCAN_OBJECTS_ARGUMENTS,
        results: INSPECTOR_SCAN_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 38,
        name: "object_inspector_scan_handles",
        feature: FeatureGate::Core,
        arguments: OBJECT_INSPECTOR_SCAN_HANDLES_ARGUMENTS,
        results: INSPECTOR_SCAN_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 39,
        name: "object_inspector_derive_process",
        feature: FeatureGate::Core,
        arguments: OBJECT_INSPECTOR_DERIVE_PROCESS_ARGUMENTS,
        results: OBJECT_INSPECTOR_DERIVE_PROCESS_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 40,
        name: "task_inspector_derive_task_group",
        feature: FeatureGate::Core,
        arguments: TASK_INSPECTOR_DERIVE_TASK_GROUP_ARGUMENTS,
        results: TASK_INSPECTOR_DERIVE_PROCESS_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 41,
        name: "object_inspector_derive_task_group",
        feature: FeatureGate::Core,
        arguments: OBJECT_INSPECTOR_DERIVE_TASK_GROUP_ARGUMENTS,
        results: OBJECT_INSPECTOR_DERIVE_PROCESS_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 42,
        name: "task_inspector_derive_resource_domain",
        feature: FeatureGate::Core,
        arguments: TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN_ARGUMENTS,
        results: TASK_INSPECTOR_DERIVE_PROCESS_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 43,
        name: "object_inspector_derive_resource_domain",
        feature: FeatureGate::Core,
        arguments: OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN_ARGUMENTS,
        results: OBJECT_INSPECTOR_DERIVE_PROCESS_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 44,
        name: "directory_open_directory",
        feature: FeatureGate::Core,
        arguments: DIRECTORY_OPEN_FILE_ARGUMENTS,
        results: DIRECTORY_OPEN_DIRECTORY_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 45,
        name: "vmo_create",
        feature: FeatureGate::Core,
        arguments: VMO_CREATE_ARGUMENTS,
        results: VMO_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 46,
        name: "file_create_executable_vmo",
        feature: FeatureGate::Core,
        arguments: FILE_CREATE_EXECUTABLE_VMO_ARGUMENTS,
        results: FILE_CREATE_EXECUTABLE_VMO_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 47,
        name: "vmo_read",
        feature: FeatureGate::Core,
        arguments: VMO_READ_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 48,
        name: "vmo_write",
        feature: FeatureGate::Core,
        arguments: VMO_WRITE_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 49,
        name: "vmar_allocate",
        feature: FeatureGate::Core,
        arguments: VMAR_ALLOCATE_ARGUMENTS,
        results: VMAR_ALLOCATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 50,
        name: "vmar_map",
        feature: FeatureGate::Core,
        arguments: VMAR_MAP_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 51,
        name: "vmar_protect",
        feature: FeatureGate::Core,
        arguments: VMAR_RANGE_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 52,
        name: "vmar_unmap",
        feature: FeatureGate::Core,
        arguments: VMAR_UNMAP_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 53,
        name: "vmar_destroy",
        feature: FeatureGate::Core,
        arguments: VMAR_DESTROY_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 54,
        name: "directory_read",
        feature: FeatureGate::Core,
        arguments: DIRECTORY_READ_ARGUMENTS,
        results: DIRECTORY_READ_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 55,
        name: "memory_inspector_read",
        feature: FeatureGate::Core,
        arguments: MEMORY_INSPECTOR_READ_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 56,
        name: "cpu_inspector_read",
        feature: FeatureGate::Core,
        arguments: CPU_INSPECTOR_READ_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 57,
        name: "file_get_info",
        feature: FeatureGate::Core,
        arguments: FILE_GET_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 58,
        name: "directory_get_info",
        feature: FeatureGate::Core,
        arguments: DIRECTORY_GET_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 59,
        name: "virtual_machine_creation_lease_create",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_MACHINE_CREATION_LEASE_CREATE_ARGUMENTS,
        results: VIRTUAL_MACHINE_CREATION_LEASE_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 60,
        name: "virtual_machine_create",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_MACHINE_CREATE_ARGUMENTS,
        results: VIRTUAL_MACHINE_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 61,
        name: "pending_virtual_machine_set_memory",
        feature: FeatureGate::Core,
        arguments: PENDING_VIRTUAL_MACHINE_SET_MEMORY_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 62,
        name: "pending_virtual_machine_set_bootstrap",
        feature: FeatureGate::Core,
        arguments: PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 63,
        name: "pending_virtual_machine_seal",
        feature: FeatureGate::Core,
        arguments: PENDING_VIRTUAL_MACHINE_SEAL_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 64,
        name: "pending_virtual_machine_install",
        feature: FeatureGate::Core,
        arguments: PENDING_VIRTUAL_MACHINE_INSTALL_ARGUMENTS,
        results: PENDING_VIRTUAL_MACHINE_INSTALL_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 65,
        name: "pending_virtual_machine_abort",
        feature: FeatureGate::Core,
        arguments: PENDING_VIRTUAL_MACHINE_ABORT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 66,
        name: "virtual_machine_request_stop",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_MACHINE_REQUEST_STOP_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 67,
        name: "virtual_machine_get_info",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_MACHINE_GET_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 68,
        name: "virtual_cpu_get_info",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_CPU_GET_INFO_ARGUMENTS,
        results: INFO_RECORD_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Object,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 69,
        name: "resource_domain_create",
        feature: FeatureGate::Core,
        arguments: RESOURCE_DOMAIN_CREATE_ARGUMENTS,
        results: RESOURCE_DOMAIN_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 70,
        name: "task_group_create",
        feature: FeatureGate::Core,
        arguments: TASK_GROUP_CREATE_ARGUMENTS,
        results: TASK_GROUP_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 71,
        name: "virtual_cpu_start",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_CPU_START_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 72,
        name: "pending_virtual_machine_set_virtual_serial",
        feature: FeatureGate::Core,
        arguments: PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 73,
        name: "clock_get_monotonic",
        feature: FeatureGate::Core,
        arguments: NO_ARGUMENTS,
        results: CLOCK_GET_MONOTONIC_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Abi,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 74,
        name: "virtual_serial_create",
        feature: FeatureGate::Core,
        arguments: NO_ARGUMENTS,
        results: VIRTUAL_SERIAL_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 75,
        name: "virtual_serial_register_output",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_SERIAL_REGISTER_OUTPUT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 76,
        name: "virtual_serial_write",
        feature: FeatureGate::Core,
        arguments: VIRTUAL_SERIAL_WRITE_ARGUMENTS,
        results: VIRTUAL_SERIAL_IO_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 77,
        name: "thread_create",
        feature: FeatureGate::Core,
        arguments: THREAD_CREATE_ARGUMENTS,
        results: THREAD_CREATE_RESULTS,
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 78,
        name: "thread_start",
        feature: FeatureGate::Core,
        arguments: &[thread_argument(RIGHT_START)],
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 79,
        name: "thread_request_stop",
        feature: FeatureGate::Core,
        arguments: &[thread_argument(RIGHT_REQUEST_STOP)],
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 80,
        name: "atomic_wait",
        feature: FeatureGate::Core,
        arguments: ATOMIC_WAIT_ARGUMENTS,
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::Explicit,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 81,
        name: "atomic_wake",
        feature: FeatureGate::Core,
        arguments: ATOMIC_WAKE_ARGUMENTS,
        results: &[ResultValue {
            name: "woken",
            kind: ValueKind::U64,
            handle: None,
        }],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 82,
        name: "thread_sleep",
        feature: FeatureGate::Core,
        arguments: &[scalar_argument("deadline", ValueKind::U64)],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::Explicit,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::None,
        failure_results: &[],
    },
    Syscall {
        number: 83,
        name: "file_write_at",
        feature: FeatureGate::Core,
        arguments: FILE_WRITE_AT_ARGUMENTS,
        results: &[
            scalar_result("actual_bytes", ValueKind::ByteCount),
            scalar_result("end_offset", ValueKind::U64),
        ],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 84,
        name: "file_resize",
        feature: FeatureGate::Core,
        arguments: &[
            file_argument(RIGHT_WRITE),
            scalar_argument("size", ValueKind::ByteCount),
        ],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 85,
        name: "directory_create_file",
        feature: FeatureGate::Core,
        arguments: &[
            Argument {
                name: "directory",
                kind: ValueKind::Handle,
                handle: Some(HandleArgument {
                    object: ObjectConstraint::Kind("directory"),
                    required_rights: RIGHT_READ | RIGHT_WRITE,
                    disposition: HandleDisposition::Borrow,
                }),
                memory: None,
            },
            DIRECTORY_OPEN_FILE_ARGUMENTS[1],
            DIRECTORY_OPEN_FILE_ARGUMENTS[2],
            DIRECTORY_OPEN_FILE_ARGUMENTS[3],
            scalar_argument("mode", ValueKind::U32),
        ],
        results: DIRECTORY_OPEN_FILE_RESULTS,
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 86,
        name: "directory_create_directory",
        feature: FeatureGate::Core,
        arguments: &[
            Argument {
                name: "directory",
                kind: ValueKind::Handle,
                handle: Some(HandleArgument {
                    object: ObjectConstraint::Kind("directory"),
                    required_rights: RIGHT_READ | RIGHT_WRITE,
                    disposition: HandleDisposition::Borrow,
                }),
                memory: None,
            },
            DIRECTORY_OPEN_FILE_ARGUMENTS[1],
            DIRECTORY_OPEN_FILE_ARGUMENTS[2],
            scalar_argument("mode", ValueKind::U32),
        ],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 87,
        name: "directory_remove",
        feature: FeatureGate::Core,
        arguments: &[
            Argument {
                name: "directory",
                kind: ValueKind::Handle,
                handle: Some(HandleArgument {
                    object: ObjectConstraint::Kind("directory"),
                    required_rights: RIGHT_READ | RIGHT_WRITE,
                    disposition: HandleDisposition::Borrow,
                }),
                memory: None,
            },
            DIRECTORY_OPEN_FILE_ARGUMENTS[1],
            DIRECTORY_OPEN_FILE_ARGUMENTS[2],
            scalar_argument("options", ValueKind::U32),
        ],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 88,
        name: "wait_set_create",
        feature: FeatureGate::Core,
        arguments: &[scalar_argument("capacity", ValueKind::ElementCount)],
        results: &[ResultValue {
            name: "wait_set",
            kind: ValueKind::Handle,
            handle: Some(ProducedHandle {
                object: ProducedObject::Kind("wait_set"),
                rights: ProducedRights::Fixed(
                    RIGHT_DUPLICATE | RIGHT_WAIT | RIGHT_BIND_WAIT | RIGHT_INSPECT,
                ),
            }),
        }],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 89,
        name: "wait_set_add",
        feature: FeatureGate::Core,
        arguments: &[
            wait_set_argument(RIGHT_BIND_WAIT),
            OBJECT_WAIT_ONE_ARGUMENTS[0],
            scalar_argument("signals", ValueKind::U64),
        ],
        results: &[scalar_result("registration", ValueKind::U64)],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 90,
        name: "wait_set_rearm",
        feature: FeatureGate::Core,
        arguments: &[
            wait_set_argument(RIGHT_BIND_WAIT),
            scalar_argument("registration", ValueKind::U64),
        ],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 91,
        name: "wait_set_remove",
        feature: FeatureGate::Core,
        arguments: &[
            wait_set_argument(RIGHT_BIND_WAIT),
            scalar_argument("registration", ValueKind::U64),
        ],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 92,
        name: "wait_set_wait",
        feature: FeatureGate::Core,
        arguments: &[
            wait_set_argument(RIGHT_WAIT),
            scalar_argument("deadline", ValueKind::U64),
            Argument {
                name: "output",
                kind: ValueKind::UserAddress,
                handle: None,
                memory: Some(UserMemory {
                    direction: MemoryDirection::Write,
                    length: MemoryLength::Bytes {
                        argument: "output_size",
                        maximum_bytes: 24,
                    },
                    record: Some("wait_set_event"),
                    handles: None,
                    validation_order: 0,
                }),
            },
            scalar_argument("output_size", ValueKind::ByteCount),
        ],
        results: &[],
        blocking: BlockingClass::MayBlock,
        cancellation: CancellationClass::Explicit,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 93,
        name: "process_get_current_id",
        feature: FeatureGate::Core,
        arguments: &[],
        results: &[scalar_result("koid", ValueKind::U64)],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Task,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
    Syscall {
        number: 94,
        name: "virtual_serial_acknowledge_output",
        feature: FeatureGate::Core,
        arguments: &[
            Argument {
                name: "virtual_serial",
                kind: ValueKind::Handle,
                handle: Some(HandleArgument {
                    object: ObjectConstraint::Kind("virtual_serial"),
                    required_rights: RIGHT_READ,
                    disposition: HandleDisposition::Borrow,
                }),
                memory: None,
            },
            scalar_argument("consumed", ValueKind::U64),
        ],
        results: &[],
        blocking: BlockingClass::Never,
        cancellation: CancellationClass::None,
        restart: RestartClass::Never,
        completion: CompletionClass::Returns,
        audit: AuditClass::Capability,
        flags: FlagPolicy::Strict,
        failure_results: &[],
    },
];

pub const NATIVE_ABI: AbiSchema = AbiSchema {
    revision: ABI_REVISION,
    features: FEATURES,
    statuses: STATUSES,
    object_kinds: OBJECT_KINDS,
    rights: RIGHTS,
    signals: SIGNALS,
    constants: CONSTANTS,
    records: RECORDS,
    syscalls: SYSCALLS,
    semantic_rules: SEMANTIC_RULES,
};

pub const SEMANTIC_RULES: &[&str] = &[
    "WaitSets are process-local, non-transferable objects with capacity 1..1024. BIND_WAIT authorizes add/rearm/remove, WAIT authorizes consumption. Add requires source WAIT and reserves one event slot; WaitSet and CapabilityChannel sources are unsupported. Registration IDs are globally non-reused. Bind/rearm observe signal levels and sequence under the source lock; one-shot publication does not allocate. Rearm is busy until successful event consumption. Wait returns one exact 24-byte record (registration ID, signal bits, sequence), using an absolute monotonic deadline. Copyout failure restores the event unless removal or closure cancelled it. Source handle close does not cancel object-lifetime subscriptions; final set handle close detaches registrations and wakes consumers. Future CapabilityChannel subscriptions require ownership-epoch invalidation.",
    "ByteChannel duplicate authority permits shared endpoint ownership; peer_closed is published only when the last active endpoint handle closes. Internal operation pins, including WaitSet subscriptions, do not retain active endpoint authority.",
    "process_get_current_id returns the calling Process KOID for observation only. It creates no handle or operational authority and is not a PID-to-handle lookup.",
    "Ramfs storage and all current VFS operations remain kernel-owned. Open File and Directory objects pin their nodes after unlink. Directory WRITE permits namespace mutation; File WRITE permits content mutation. directory_create_file exclusively creates a regular file and publishes its requested handle atomically with the new name. Mode accepts permission bits 0777. directory_remove options 0 removes a non-directory entry without following the final symlink; 1 removes an empty directory. file_write_at options 0 uses offset, 1 atomically appends (offset must be zero); returns actual bytes and end offset. Transfers may complete short. file_resize zero-fills extension. Directory cookies are monotonic, never reused, and enumeration is weakly consistent across mutations. Executable snapshots remain immutable across writes.",
    "Thread creation prepares a dormant thread in the calling Process and returns duplicate, wait, inspect, start and request_stop authority. Entry, 16-byte-aligned stack, TLS and an opaque first argument define its initial context. The caller retains stack/TLS ownership through the thread terminated signal. Start publishes runnable execution; request_stop abandons execution without language destructors. Closing the last handle to a dormant thread cancels it; running threads continue independently of handle ownership. Existing thread_exit terminates only the caller, while process_exit stops every thread.",
    "Atomic wait/wake currently operate on aligned writable u32 words in the calling Process. Wait checks the value and publishes its generation atomically with respect to wake; mismatch returns ok, and spurious wakes are allowed. Wait uses an absolute monotonic nanosecond deadline, with UINT64_MAX meaning infinite; timeout returns timed_out and cancellation returns cancelled. Wake returns the number of registrations actually notified, bounded by count. Wake does not replace the caller's release operation or the waiter's acquire predicate check. Keys include the non-reused mapping identity and virtual address; aliases and other Processes have separate wait domains. Keep the mapping live while waiting: unmap/protect does not move an existing wait to a replacement mapping. Pinned backing survives concurrent unmap until admitted accesses finish; process stop cancels outstanding waits.",
    "Thread sleep accepts an absolute monotonic nanosecond deadline and returns ok when it expires, including an already elapsed deadline. UINT64_MAX sleeps until cancellation. It parks the current thread through scheduler timeout arbitration rather than polling or yielding.",
    "The monotonic clock syscall returns absolute nanoseconds from the kernel's monotonic clock domain. Ambient monotonic observation is intentionally not a capability because reading it conveys no mutable authority; future virtual or adjustable clocks may be represented by handle objects without changing this clock domain.",
    "Extensible input records carry their caller size in the syscall's explicit byte-count argument. The size must be at least the record's published minimum prefix and no greater than extensible_record_max_bytes; missing bytes through the kernel's current record size default to zero, while bytes beyond that size must be zero. Extensible information outputs accept any capacity from the record's minimum prefix through extensible_record_max_bytes, write only the intersection of caller capacity and kernel-supported size, and return the kernel-supported size in value0 on ok. Bytes beyond that intersection remain untouched.",
    "Directory lookup is capability-relative. A leading slash restarts at that Directory's traversal root, and parent components cannot escape it. Every open requires read traversal authority, and every requested File right must already be present on the source Directory before the result is further bounded by the File object's node-specific ceiling.",
    "Directory reads require the exact published page capacity. Cookie zero starts a scan and a returned next_cookie of zero ends it. Every name is one valid path component encoded as name_length UTF-8 bytes followed by zero-filled capacity. Pages are weakly consistent with concurrent filesystem mutation; callers must neither interpret nor synthesize cookies.",
    "Native task and object inspectors are immutable capability-scoped views. Process, thread, and object KOIDs plus scan cursors are observation-only values and can never be exchanged for operational authority. Out-of-scope targeted lookup returns not_found.",
    "Task inspector records carry a bounded UTF-8 name as name_length bytes followed by zero-filled capacity. Process names are the immutable labels committed by ProcessBuilder publication; Thread names are immutable scheduler identity labels retained through the retiring registry phase.",
    "Inspector derivation is monotonic: a derived Process, TaskGroup, or ResourceDomain view cannot widen its parent's task scope, object scope, visibility, or rights. Derivation requires the inspector's complete supported rights because the returned handle carries that fixed rights set; callers attenuate it before delegation. Native task operations remain handle-based; numeric PID and TID namespaces belong exclusively to compatibility personalities.",
    "Every live TaskGroup handle participates in shared group-lifetime ownership regardless of its attenuated rights. Closing the last TaskGroup handle asynchronously requests stop for every member. Rights control operations available through a handle; they do not change this ownership effect.",
    "Inspector scans require the exact published page capacity for their record type. Cursor zero starts a scan and a returned next_cursor of zero ends it. Pages and complete scans are weakly consistent with concurrent task, object, and handle-table mutation; generation-qualified handle values prevent slot reuse from aliasing an earlier observation.",
    "Memory and CPU inspectors publish immutable point-in-time copies. Their handles grant observation only; they never expose writable accounting storage or allocator and scheduler synchronization to userspace. CPU categories are scheduler-tick observations and a multi-CPU snapshot is weakly consistent across CPUs.",
    "File and Directory information reports attribute snapshots plus filesystem, mount, and node identities for diagnostics and correlation. These identities do not grant authority, cannot be resolved back into handles, and do not define a pathname; directory entries, hard links, renames, mount namespaces, and unlinks make pathnames namespace-dependent observations rather than object identity.",
    "Object wait-many borrows every input handle for the complete wait, canonicalizes duplicate object identities, and selects the lowest input index whose requested mask intersects the winning object's committed level snapshot. Source-handle close after resolution does not cancel the wait.",
    "Process terminal detail fields are reason-specific: exit reasons encode the signed status as two's-complement in detail0; fault encodes class in detail0 and code in detail1; task-group stop encodes generation in detail0; unused details are zero.",
    "Object transfer classes constrain generic capability transports. General objects may be retained by buffered or rendezvous transports. Rendezvous-only objects may move or duplicate only by a direct source-to-destination commit which never creates an in-transit owner. Forbidden objects cannot cross a userspace handle table boundary.",
    "AtomicOnOk capability transactions commit handle-table ownership, the rendezvous message, and live output-handle installation together only when both participants return ok. Every non-ok status preserves all input owners and the message. Non-fault failures leave output memory unchanged; fault may partially modify output byte or slot memory, but no handle value written by a failed call is live or installed. Bindings must ignore every output-memory byte after any non-ok status.",
    "A capability-channel receiver advertises peer_receiving only after its byte range, typed capability slots, and destination handle-table capacity are validated, reserved, and fully published on the endpoint's FIFO receiver queue. The signal is level-triggered but may race another sender.",
    "A capability disposition's rights are the sender's offered ceiling; capability_disposition_same_rights offers all source rights. A receive slot's rights are the exact installed grant, so receiver rights must be a subset of the sender offer and the offer a subset of source rights. Sender and receiver expected_kind values are nonzero and must both equal the actual object kind.",
    "A capability receive slot enters with handle and flags zero. On ok, only handle changes. Implementations reserve destination entries, prewrite their future handle values, then perform an infallible handle-table commit. Buffer-too-small changes no slot or byte memory and reports both required counts.",
    "Capability disposition move consumes its source only on ok and requires transfer. Capability disposition duplicate retains its source and requires transfer plus duplicate. Unknown operations, repeated source handles, kind mismatches, and rights violations reject the complete transaction.",
    "Capability-channel try_send returns peer_closed when no peer remains; otherwise it returns would_block only when no receiver is queued. It FIFO-matches the oldest fully published receiver. A size, kind, rights, operation, or copy mismatch rejects that transaction for both participants without scanning later receivers.",
    "Capability-channel timeout, cancellation, and peer close can win only before a receiver is matched. Once matched, sender completion or rejection owns the transaction through commit, including races with the deadline, cancellation, or close; both participants observe the same committed outcome.",
    "Process-builder create borrows its authority handles and retains kernel object references independently of the caller handles. Builders are mutable only before seal. Every successful mutator applies completely; every failure leaves the builder unchanged. Seal is irreversible. Start requires a sealed builder and returns only a supervisor process handle. Start and abort consume the builder handle only on ok; every failure preserves it.",
    "A process-builder name is nonempty UTF-8 without embedded NUL bytes. The argv vector contains at least one entry; individual argument strings are UTF-8 and may be empty but contain no NUL byte. Every UTF-8 environment entry contains a nonempty name with no '=' followed by '=' and a NUL-free value. Counts and individual byte lengths remain within the published constants.",
    "Process-builder set_name and set_affinity replace their prior values; add_argument and add_environment append in order. Process-builder affinity is a nonempty little-endian array of u64 CPU-mask words. Bits above process_affinity_max_cpus and bits which cannot designate an allowed CPU are rejected.",
    "Process-builder add_handle requires a nonzero purpose unique within the builder, an expected nonzero exact object kind, and either exact granted rights or capability_disposition_same_rights. Move consumes the source only when the mutator returns ok; duplicate retains it and additionally requires duplicate. Failure preserves both builder and source.",
    "A VirtualMachineCreationAuthority may derive one resource-domain-bound VirtualMachineCreationLease. The lease is single-use and is consumed only when VirtualMachine creation publishes a PendingVirtualMachine handle successfully.",
    "A PendingVirtualMachine is mutable until seal. It must own exactly one writable VMO whose size equals the configured guest RAM and one bootstrap record for boot vCPU 0. The configured vcpu_count fixes immutable topology; architecture power-on protocols supply secondary-vCPU runtime entry state, and future additive VirtualMachine operations may expose their control handles. A guest serial route is optional and exists only when a caller transfers a VirtualSerial handle with assign-device authority before seal. Successful binding consumes the supplied handle and commits a VM-owned reference until VM retirement; a rejected binding leaves the handle unchanged. Guest output is published to a read-only shared VMO consumed by the owning runtime, and VM retirement disconnects the input route without invalidating existing output mappings. Seal is irreversible; install consumes the pending handle only on ok and publishes the installed VirtualMachine and dormant boot VirtualCpu handles together. VirtualCpu start is a separate operation after handle publication. The started VirtualCpu phase means that start committed successfully; it is not an observation that the scheduler currently considers the vCPU runnable or executing. The current implementation accepts one vCPU.",
    "VirtualSerial handles are process-local; device assignment consumes a same-process handle. register_output borrows a caller-allocated writable VMO of exactly 69632 bytes and registers it once before assignment. An exclusive write lease rejects existing writable mappings, direct accesses, snapshots, and further writers until port retirement; read-only mappings may coexist. Offset 0 is an atomic u64 produced count, offset 8 a saturating dropped-byte count, and offset 4096 begins 65536 atomic byte slots. Registration initializes counters; callers must not access contents during registration. The producer release-publishes bytes; the runtime acquire-loads production and reads a batch. acknowledge_output requires READ and submits the absolute consumed position after reading. Regressing or future positions return invalid_argument without mutation. Publication, acknowledgement, and closure serialize on the port: READABLE means unacknowledged output, WRITABLE means a connected input route has queue space, and PEER_CLOSED means no future output or input. WAIT authorizes object waits and WaitSet subscriptions. Acknowledgement clears READABLE only when caught up; new output reasserts it, without lost wakeups or periodic polling. Full output discards new bytes without blocking or overwriting unconsumed slots. Counters never wrap. Last active handle closure synchronizes with writers and closes publication; registered pages remain pinned through final VM/object retirement. The SDK maps output read-only and acknowledges batches by syscall without copying payload through the syscall. Write remains nonblocking input injection: busy means the queue is full, bad_state means disconnected. Runtime policy owns retention and client transport.",
    "The creating process retains its guest VMO handle, but attaching it to a PendingVirtualMachine acquires exclusive hardware-write ownership and rejects any active Native writable mapping or direct VMO operation. Direct VMO access, snapshots, and writable Native mappings remain closed until VM retirement removes and invalidates every stage-2 mapping and releases the independent backing reference; read-only Native mappings may coexist.",
];

const _: () = assert!(SYSCALL_ARGUMENT_REGISTERS == 6);
const _: () = assert!(SYSCALL_RESULT_REGISTERS == 2);
