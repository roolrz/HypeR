// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Status, object kinds, rights, transfer rules and signals.
//!
//! Object tags select an operation namespace; rights authorize operations on an
//! already held handle. Neither a KOID nor an inspection result grants authority.
//! Transfer classes additionally restrict which transport can carry an object.
//! Keep these three decisions separate when adding an object kind.

use super::model::*;

/// Bits returned by ABI discovery; these are unrelated to Cargo feature flags.
pub const FEATURES: &[Feature] = &[Feature {
    bit: 0,
    name: "core",
}];

/// Signed method status values. Failure does not define auxiliary registers
/// unless the invocation explicitly lists them in `failure_results`.
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
    Status {
        value: -19,
        name: "not_directory",
    },
    Status {
        value: -20,
        name: "is_directory",
    },
    Status {
        value: -21,
        name: "symlink_loop",
    },
    Status {
        value: -22,
        name: "cross_device",
    },
    Status {
        value: -23,
        name: "io_error",
    },
    Status {
        value: -24,
        name: "no_space",
    },
    Status {
        value: -25,
        name: "read_only",
    },
    Status {
        value: -26,
        name: "revoked",
    },
];

// Shared selector values for direct and record-based capability dispositions.
// Their public scalar-domain exports live in values.rs.
pub(super) const CAPABILITY_OPERATION_MOVE: u32 = 0;
pub(super) const CAPABILITY_OPERATION_DUPLICATE: u32 = 1;

/// Wire object tags and permitted transfer mechanisms.
/// Entries may include kernel-only kinds for inspection without making them
/// user-exportable; a tag is not a handle-construction interface.
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
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 26,
        name: "wait_set",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 27,
        name: "guest_memory",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 28,
        name: "device_assignment_authority",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 29,
        name: "physical_device",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 30,
        name: "guest_mailbox",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 31,
        name: "guest_notification",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 32,
        name: "native_block",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 33,
        name: "guest_mapping",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 34,
        name: "file_protection",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 35,
        name: "directory_stream",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 36,
        name: "root_transition",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 37,
        name: "guest_interrupt",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 38,
        name: "process_retirement",
        transfer: TransferClass::RendezvousOnly,
    },
    ObjectKind {
        value: 39,
        name: "crash_view",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 40,
        name: "backend_memory_lease",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 41,
        name: "execution_context",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 42,
        name: "object_factory",
        transfer: TransferClass::Forbidden,
    },
    ObjectKind {
        value: 43,
        name: "system_info",
        transfer: TransferClass::General,
    },
    ObjectKind {
        value: 44,
        name: "block_queue",
        transfer: TransferClass::RendezvousOnly,
    },
];

/// Read-only authority returned for a stateful directory enumeration stream.
pub const DIRECTORY_STREAM_RIGHTS: u64 =
    RIGHT_READ | RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT;

/// A protection token permits mapping/inspection, not writing the protected file.
pub const FILE_PROTECTION_RIGHTS: u64 = RIGHT_MAP | RIGHT_INSPECT;

/// Guest-memory grant ceiling; hardware ownership and dedication add independent
/// restrictions even when the handle carries READ or WRITE.
pub const GUEST_MEMORY_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_DUPLICATE | RIGHT_MAP | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE;

// Declare each right once: operation contracts and generated bindings consume
// the same bit assignment. The explicit Rust identifier avoids token-pasting
// dependencies in this core-only schema.
macro_rules! rights {
    ($($name:ident => $mask:ident = $bit:literal,)*) => {
        $(pub const $mask: u64 = 1_u64 << $bit;)*
        pub const RIGHTS: &[Right] = &[
            $(Right { name: stringify!($name), bit: $bit },)*
        ];
    };
}

rights! {
    // DUPLICATE retains the source; TRANSFER authorizes moving across a boundary.
    duplicate => RIGHT_DUPLICATE = 0,
    transfer => RIGHT_TRANSFER = 1,
    wait => RIGHT_WAIT = 2,
    inspect => RIGHT_INSPECT = 3,
    read => RIGHT_READ = 4,
    write => RIGHT_WRITE = 5,
    map => RIGHT_MAP = 6,
    // Mapping, execution and residency are separate authorities.
    execute => RIGHT_EXECUTE = 7,
    resize => RIGHT_RESIZE = 8,
    pin => RIGHT_PIN = 9,
    start => RIGHT_START = 10,
    request_stop => RIGHT_REQUEST_STOP = 11,
    run_vcpu => RIGHT_RUN_VCPU = 12,
    inject_interrupt => RIGHT_INJECT_INTERRUPT = 13,
    grant_memory => RIGHT_GRANT_MEMORY = 14,
    assign_device => RIGHT_ASSIGN_DEVICE = 15,
    // DMA address disclosure is not implied by device inspection or register access.
    map_dma => RIGHT_MAP_DMA = 16,
    ack_interrupt => RIGHT_ACK_INTERRUPT = 17,
    revoke => RIGHT_REVOKE = 18,
    signal => RIGHT_SIGNAL = 19,
    create_process => RIGHT_CREATE_PROCESS = 20,
    create_thread => RIGHT_CREATE_THREAD = 21,
    create_task_group => RIGHT_CREATE_TASK_GROUP = 22,
    create_resource_domain => RIGHT_CREATE_RESOURCE_DOMAIN = 23,
    set_limits => RIGHT_SET_LIMITS = 24,
    create_executable => RIGHT_CREATE_EXECUTABLE = 25,
    task_group_attach_process => RIGHT_TASK_GROUP_ATTACH_PROCESS = 26,
    resource_domain_sponsor => RIGHT_RESOURCE_DOMAIN_SPONSOR = 27,
    derive => RIGHT_DERIVE = 28,
    create_virtual_machine => RIGHT_CREATE_VIRTUAL_MACHINE = 29,
    bind_wait => RIGHT_BIND_WAIT = 30,
    set_attributes => RIGHT_SET_ATTRIBUTES = 31,
    lock_file => RIGHT_LOCK_FILE = 32,
    // Basic INSPECT does not disclose detailed layouts or device resource addresses.
    inspect_details => RIGHT_INSPECT_DETAILS = 33,
    manage_namespace => RIGHT_MANAGE_NAMESPACE = 34,
    // Mandatory write exclusion is separate from the cooperative LOCK_FILE protocol.
    protect => RIGHT_PROTECT = 35,
    // LOCK_MEMORY authorizes residency; it does not forbid later unmapping.
    lock_memory => RIGHT_LOCK_MEMORY = 36,
    // Registering retention and releasing retained state are distinct capabilities.
    retain_process => RIGHT_RETAIN_PROCESS = 37,
    retire => RIGHT_RETIRE = 38,
    create_object => RIGHT_CREATE_OBJECT = 39,
    // Scheduling elevation is never implied by WRITE or thread ownership.
    sched_lower => RIGHT_SCHED_LOWER = 40,
    sched_priority => RIGHT_SCHED_PRIORITY = 41,
    sched_class => RIGHT_SCHED_CLASS = 42,
    sched_budget => RIGHT_SCHED_BUDGET = 43,
}

pub const SCHEDULING_ELEVATION_RIGHTS: u64 =
    RIGHT_SCHED_PRIORITY | RIGHT_SCHED_CLASS | RIGHT_SCHED_BUDGET;
pub const SCHEDULING_RIGHTS: u64 = RIGHT_SCHED_LOWER | SCHEDULING_ELEVATION_RIGHTS;

/// Supervisor authority over a retained process; RETIRE releases retained state.
pub const PROCESS_RETIREMENT_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_INSPECT_DETAILS | RIGHT_RETIRE;
/// Read-only analysis authority; the view cannot resume the process or mutate it.
pub const CRASH_VIEW_RIGHTS: u64 = RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_READ;

// These masks describe a particular constructor's grant or a derivation ceiling,
// not rights automatically granted to every handle of that object kind.
// ProducedRights in the invocation determines how each mask is applied.
pub const EVENT_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_SIGNAL;
pub const BYTE_CHANNEL_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE;
pub const CAPABILITY_CHANNEL_RIGHTS: u64 = BYTE_CHANNEL_RIGHTS | RIGHT_DUPLICATE;
pub const DIRECTORY_RIGHTS: u64 = RIGHT_DUPLICATE
    | RIGHT_TRANSFER
    | RIGHT_INSPECT
    | RIGHT_READ
    | RIGHT_WRITE
    | RIGHT_EXECUTE
    | RIGHT_SET_ATTRIBUTES
    | RIGHT_LOCK_FILE
    | RIGHT_PROTECT;
pub const FILE_RIGHTS: u64 = DIRECTORY_RIGHTS;
pub const VMO_WRITABLE_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_READ | RIGHT_WRITE | RIGHT_MAP;
pub const VMO_EXECUTABLE_RIGHTS: u64 =
    RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_READ | RIGHT_EXECUTE | RIGHT_MAP;
pub const VMAR_RIGHTS: u64 = RIGHT_DUPLICATE | RIGHT_TRANSFER | RIGHT_INSPECT | RIGHT_MAP;
pub const PROCESS_BUILDER_RIGHTS: u64 = RIGHT_TRANSFER
    | RIGHT_INSPECT
    | RIGHT_WRITE
    | RIGHT_START
    | RIGHT_REQUEST_STOP
    | RIGHT_SCHED_LOWER;
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
    RIGHT_TRANSFER | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_WRITE | RIGHT_REQUEST_STOP;
pub const VIRTUAL_CPU_RIGHTS: u64 =
    RIGHT_TRANSFER | RIGHT_WRITE | RIGHT_WAIT | RIGHT_INSPECT | RIGHT_START | RIGHT_SCHED_LOWER;
pub const VIRTUAL_SERIAL_RIGHTS: u64 = RIGHT_DUPLICATE
    | RIGHT_TRANSFER
    | RIGHT_INSPECT
    | RIGHT_READ
    | RIGHT_WRITE
    | RIGHT_WAIT
    | RIGHT_ASSIGN_DEVICE;

/// Ownership selected by a transfer disposition. Both operations still require
/// the enclosing contract's common rights; duplication additionally needs DUPLICATE.
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

/// Object-local wait bits. Matching bit numbers across kinds are not interchangeable;
/// observation requires WAIT and does not itself consume or acknowledge an event.
pub const SIGNALS: &[Signal] = &[
    Signal {
        object: "process_retirement",
        bit: 0,
        name: "ready",
    },
    Signal {
        object: "process_retirement",
        bit: 1,
        name: "released",
    },
    Signal {
        object: "physical_device",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "physical_device",
        bit: 1,
        name: "dma_fault",
    },
    Signal {
        object: "native_block",
        bit: 0,
        name: "peer_closed",
    },
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
        object: "task_group",
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
        object: "virtual_machine",
        bit: 1,
        name: "power_request",
    },
    Signal {
        object: "virtual_machine",
        bit: 2,
        name: "vcpu_terminated",
    },
    Signal {
        object: "virtual_cpu",
        bit: 0,
        name: "terminated",
    },
    Signal {
        object: "virtual_cpu",
        bit: 1,
        name: "mmio_request",
    },
    Signal {
        object: "guest_notification",
        bit: 0,
        name: "peer_closed",
    },
    Signal {
        object: "guest_mailbox",
        bit: 0,
        name: "readable",
    },
    Signal {
        object: "guest_mailbox",
        bit: 1,
        name: "writable",
    },
    Signal {
        object: "guest_mailbox",
        bit: 2,
        name: "peer_closed",
    },
    Signal {
        object: "block_queue",
        bit: 0,
        name: "completed",
    },
    Signal {
        object: "block_queue",
        bit: 1,
        name: "writable",
    },
    Signal {
        object: "block_queue",
        bit: 2,
        name: "peer_closed",
    },
];
