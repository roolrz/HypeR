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
}

impl FieldKind {
    pub const fn size(self) -> u16 {
        match self {
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    pub const fn alignment(self) -> u8 {
        match self {
            Self::U32 => 4,
            Self::U64 => 8,
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
    /// Handle values in input records are moved only when the syscall commits.
    ConsumeRecords {
        handle_field: &'static str,
        rights_field: &'static str,
        expected_kind_field: &'static str,
        required_rights: u64,
    },
    /// The kernel publishes transferred handles into an output element array.
    ProduceTransferred,
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
];

const RIGHT_DUPLICATE_BIT: u8 = 0;
const RIGHT_INSPECT_BIT: u8 = 3;
const RIGHT_SIGNAL_BIT: u8 = 19;

pub const OBJECT_KINDS: &[ObjectKind] = &[
    ObjectKind {
        value: 0,
        name: "none",
    },
    ObjectKind {
        value: 1,
        name: "event",
    },
    ObjectKind {
        value: 2,
        name: "channel",
    },
    ObjectKind {
        value: 3,
        name: "thread",
    },
];

pub const RIGHTS: &[Right] = &[
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
];

pub const RIGHT_DUPLICATE: u64 = 1 << RIGHT_DUPLICATE_BIT;
pub const RIGHT_INSPECT: u64 = 1 << RIGHT_INSPECT_BIT;
pub const RIGHT_TRANSFER: u64 = 1 << 1;
pub const RIGHT_WAIT: u64 = 1 << 2;
pub const RIGHT_SIGNAL: u64 = 1 << RIGHT_SIGNAL_BIT;
pub const RIGHT_READ: u64 = 1 << 4;
pub const RIGHT_WRITE: u64 = 1 << 5;

pub const EVENT_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_SIGNAL;
pub const CHANNEL_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE;

pub const SIGNALS: &[Signal] = &[
    Signal {
        object: "event",
        bit: 0,
        name: "signaled",
    },
    Signal {
        object: "channel",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "channel",
        bit: 1,
        name: "writable",
    },
    Signal {
        object: "channel",
        bit: 2,
        name: "peer_closed",
    },
    Signal {
        object: "thread",
        bit: 0,
        name: "terminated",
    },
];

pub const CONSTANTS: &[AbiConstant] = &[
    AbiConstant {
        name: "deadline_infinite",
        value: u64::MAX,
    },
    AbiConstant {
        name: "channel_disposition_same_rights",
        value: u64::MAX,
    },
    AbiConstant {
        name: "channel_max_message_bytes",
        value: 64 * 1024,
    },
    AbiConstant {
        name: "channel_max_message_handles",
        value: 64,
    },
    AbiConstant {
        name: "channel_max_queued_messages",
        value: 16,
    },
    AbiConstant {
        name: "channel_max_queued_bytes",
        value: 16 * 64 * 1024,
    },
    AbiConstant {
        name: "channel_max_queued_handles",
        value: 16 * 64,
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

const CHANNEL_DISPOSITION_FIELDS: &[Field] = &[
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
        name: "reserved",
        kind: FieldKind::U32,
        offset: 20,
    },
];

pub const RECORDS: &[Record] = &[
    Record {
        name: "handle_info",
        fields: HANDLE_INFO_FIELDS,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "object_basic_info",
        fields: OBJECT_BASIC_INFO_FIELDS,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "channel_disposition",
        fields: CHANNEL_DISPOSITION_FIELDS,
        size: 24,
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
                maximum_bytes: 16,
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
                maximum_bytes: 16,
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
            object: ProducedObject::Kind("channel"),
            rights: ProducedRights::Fixed(CHANNEL_RIGHTS),
        }),
    },
    ResultValue {
        name: "endpoint1",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("channel"),
            rights: ProducedRights::Fixed(CHANNEL_RIGHTS),
        }),
    },
];
const CHANNEL_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "endpoint",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("channel"),
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
    Argument {
        name: "dispositions",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "disposition_count",
                maximum_elements: 64,
                element_size: 24,
            },
            record: Some("channel_disposition"),
            // The initial implementation accepts Event handles here. Moving a
            // Channel endpoint returns NOT_SUPPORTED until revocation and
            // iterative teardown make queued endpoint cycles reclaimable.
            handles: Some(IndirectHandles::ConsumeRecords {
                handle_field: "handle",
                rights_field: "rights",
                expected_kind_field: "expected_kind",
                required_rights: RIGHT_TRANSFER,
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
const CHANNEL_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "endpoint",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("channel"),
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
    Argument {
        name: "handles",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "handle_capacity",
                maximum_elements: 64,
                element_size: 8,
            },
            record: None,
            handles: Some(IndirectHandles::ProduceTransferred),
            validation_order: 1,
        }),
    },
    Argument {
        name: "handle_capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];
const CHANNEL_READ_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "actual_bytes",
        kind: ValueKind::ByteCount,
        handle: None,
    },
    ResultValue {
        name: "actual_handles",
        kind: ValueKind::ElementCount,
        handle: None,
    },
];
const CHANNEL_READ_BUFFER_TOO_SMALL_RESULTS: &[&str] = &["actual_bytes", "actual_handles"];
const CHANNEL_READ_FAILURE_RESULTS: &[FailureResults] = &[FailureResults {
    status: "buffer_too_small",
    results: CHANNEL_READ_BUFFER_TOO_SMALL_RESULTS,
}];

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
        number: 5,
        name: "object_get_basic_info",
        feature: FeatureGate::Core,
        arguments: OBJECT_GET_BASIC_INFO_ARGUMENTS,
        results: &[],
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
        name: "channel_create",
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
        name: "channel_write",
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
        name: "channel_read",
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
};

const _: () = assert!(SYSCALL_ARGUMENT_REGISTERS == 6);
const _: () = assert!(SYSCALL_RESULT_REGISTERS == 2);
