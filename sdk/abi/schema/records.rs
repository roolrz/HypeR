// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Wire records with explicit field offsets and minimum prefixes.
//!
//! These are layout declarations, not native Rust structs. Offsets, extents and
//! alignments are bytes; gaps become explicit generated padding. Extensible
//! records retain their original `minimum_size` when fields are appended.
//! A minimum prefix does not override a method's buffer-length requirements.
//!
//! Handle-looking fields need context: capability dispositions contain live
//! process-local handles, while inspection and crash records contain identities
//! that grant no authority. Opaque byte arrays need record-specific encoding;
//! do not reinterpret them as host-pointer arrays.

use super::model::*;
use super::values::*;

/// Kind and rights of one process-local handle, without a global object lookup.
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

/// Global generation-qualified object identity. A KOID is not an invocable handle.
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

/// One borrowed handle and its requested object-local signal mask for wait-many.
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

/// Process lifecycle observation; `terminal_reason` selects the meaning of `detail0/1`.
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

/// Outgoing transfer instruction: source handle, requested rights, exact expected
/// object kind and move/duplicate selector. Ownership changes only on commit.
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

/// Typed receive slot. Input handle/flags are zero; rights/kind constrain admission.
/// On OK only handle changes. Failed copyout never publishes a usable capability.
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

/// Purpose-tagged capability installed into the child's handle table at startup.
/// Purpose is a routing label, not an object kind or an authority bit.
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

/// Bounded process-list observation; `name_length` bounds the fixed UTF-8 name array.
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

/// Thread identity, owner and scheduler role. Name and sampled runtime are diagnostic.
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
        name: "runtime_ns",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "name",
        kind: FieldKind::Bytes(64),
        offset: 40,
    },
];

/// Independently sampled accounting categories. Reclaimability and page ownership
/// overlap; clients must not add every field as if they were disjoint allocations.
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
        name: "managed_bytes",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "free_bytes",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "kernel_bytes",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "slab_bytes",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "large_heap_bytes",
        kind: FieldKind::U64,
        offset: 56,
    },
    Field {
        name: "page_table_bytes",
        kind: FieldKind::U64,
        offset: 64,
    },
    Field {
        name: "vmo_bytes",
        kind: FieldKind::U64,
        offset: 72,
    },
    Field {
        name: "guest_loaned_bytes",
        kind: FieldKind::U64,
        offset: 80,
    },
    Field {
        name: "guest_private_bytes",
        kind: FieldKind::U64,
        offset: 88,
    },
    Field {
        name: "file_cache_bytes",
        kind: FieldKind::U64,
        offset: 96,
    },
    Field {
        name: "slab_reclaimable_bytes",
        kind: FieldKind::U64,
        offset: 104,
    },
    Field {
        name: "slab_sample_complete",
        kind: FieldKind::U64,
        offset: 112,
    },
    Field {
        name: "file_cache_reclaimable_bytes",
        kind: FieldKind::U64,
        offset: 120,
    },
    Field {
        name: "file_cache_sample_complete",
        kind: FieldKind::U64,
        offset: 128,
    },
    Field {
        name: "ramfs_capacity_bytes",
        kind: FieldKind::U64,
        offset: 136,
    },
];

/// Aggregate CPU runtime counters; rate calculations require two timestamped
/// observations and the online CPU count. All runtime counters are nanoseconds.
const CPU_OBSERVATION_FIELDS: &[Field] = &[
    Field {
        name: "captured_at_ns",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "reserved_clock",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "online_cpus",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "idle_ns",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "kernel_thread_ns",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "user_thread_ns",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "vcpu_ns",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 56,
    },
];

/// Kind-tagged, eight-word little-endian payload. See object-details semantic rules
/// for each tag; unused words are zero and payloads never expose kernel pointers.
const OBJECT_DETAILS_FIELDS: &[Field] = &[
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
        name: "record_kind",
        kind: FieldKind::U32,
        offset: 12,
    },
    Field {
        name: "next_cursor",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "payload",
        kind: FieldKind::Bytes(64),
        offset: 24,
    },
];

/// Registry observation of one object. Counts describe ownership, not new references
/// or capabilities held on behalf of the inspecting process.
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

/// Handle-table observation in the named process; the reported handle is not
/// installed in the inspector's table and cannot be used there as authority.
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

/// One bounded directory entry: `name_length` UTF-8 bytes followed by zero-filled
/// name capacity. Entry names are single components, not full paths.
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

/// Guest IPA envelope and immutable topology, not a request to back every IPA byte.
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

/// Guest entry context. Entry/stack/argument addresses are guest addresses;
/// architecture-specific startup register meanings belong to the guest profile.
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

/// One trapped guest access. Completion must echo its request id so stale replies
/// cannot complete a later access; width is bytes and address is guest physical.
const VIRTUAL_CPU_MMIO_REQUEST_FIELDS: &[Field] = &[
    Field {
        name: "id",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "device",
        kind: FieldKind::U64,
        offset: 8,
    },
    Field {
        name: "address",
        kind: FieldKind::U64,
        offset: 16,
    },
    Field {
        name: "value",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "operation",
        kind: FieldKind::U32,
        offset: 32,
    },
    Field {
        name: "width",
        kind: FieldKind::U32,
        offset: 36,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 40,
    },
];

/// Guest power request correlated by id; the operation selects whether target,
/// entry and context describe a secondary-vCPU startup or a VM-wide transition.
const VIRTUAL_MACHINE_POWER_REQUEST_FIELDS: &[Field] = &[
    Field {
        name: "id",
        kind: FieldKind::U64,
        offset: 0,
    },
    Field {
        name: "vcpu",
        kind: FieldKind::U32,
        offset: 8,
    },
    Field {
        name: "operation",
        kind: FieldKind::U32,
        offset: 12,
    },
    Field {
        name: "target",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U32,
        offset: 20,
    },
    Field {
        name: "entry",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "context",
        kind: FieldKind::U64,
        offset: 32,
    },
];

/// Resident backing is appended after the original 32-byte prefix. It excludes
/// stage-2 tables, aliases and Native runtime memory; `u64::MAX` means unavailable.
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
    Field {
        name: "resident_memory_bytes",
        kind: FieldKind::U64,
        offset: 32,
    },
];

/// `vcpu_id` is a guest topology index; `scheduler_thread_id` and `host_cpu` describe
/// host scheduling. A started phase does not prove the vCPU is currently running.
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
    Field {
        name: "host_cpu",
        kind: FieldKind::U32,
        offset: 24,
    },
    Field {
        name: "migration_target",
        kind: FieldKind::U32,
        offset: 28,
    },
];

/// Resource-domain ceilings in the named units: byte fields are bytes, page
/// fields count pages, and the remaining dimensions count objects/resources.
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

/// Complete public record catalog. Named field arrays above factor reusable
/// layouts; inline fields obey the same offset/alignment validation.
/// Atomic scheduling configuration. Algorithm-inapplicable fields must be zero.
const SCHEDULING_PARAMETERS_FIELDS: &[Field] = &[
    Field {
        name: "policy",
        kind: FieldKind::U32,
        offset: 0,
    },
    Field {
        name: "band",
        kind: FieldKind::U32,
        offset: 4,
    },
    Field {
        name: "role",
        kind: FieldKind::U32,
        offset: 8,
    },
    Field {
        name: "weight",
        kind: FieldKind::U32,
        offset: 12,
    },
    Field {
        name: "fifo_priority",
        kind: FieldKind::U32,
        offset: 16,
    },
    Field {
        name: "fair_backend",
        kind: FieldKind::U32,
        offset: 20,
    },
    Field {
        name: "quantum_ns",
        kind: FieldKind::U64,
        offset: 24,
    },
    Field {
        name: "budget_ns",
        kind: FieldKind::U64,
        offset: 32,
    },
    Field {
        name: "period_ns",
        kind: FieldKind::U64,
        offset: 40,
    },
    Field {
        name: "burst_ns",
        kind: FieldKind::U64,
        offset: 48,
    },
    Field {
        name: "reserved",
        kind: FieldKind::U64,
        offset: 56,
    },
];

pub const RECORDS: &[Record] = &[
    // Static tracepoints expose fixed-size scalar records, never kernel pointers.
    Record {
        name: "trace_config",
        minimum_size: 56,
        size: 56,
        alignment: 8,
        fields: &[
            Field {
                name: "categories",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "duration_ms",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "capacity",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "cpu_mask",
                kind: FieldKind::Bytes(32),
                offset: 24,
            },
        ],
    },
    Record {
        name: "trace_info",
        minimum_size: 160,
        size: 160,
        alignment: 8,
        fields: &[
            Field {
                name: "generation",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "started_ns",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "deadline_ns",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "categories",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "cpu_count",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "capacity",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "maximum_capacity",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "running",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "cpu_mask",
                kind: FieldKind::Bytes(32),
                offset: 64,
            },
            Field {
                name: "revision",
                kind: FieldKind::Bytes(64),
                offset: 96,
            },
        ],
    },
    Record {
        name: "trace_record",
        minimum_size: 328,
        size: 328,
        alignment: 8,
        fields: &[
            Field {
                name: "sequence",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "timestamp_ns",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "category",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "field_count",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "values",
                kind: FieldKind::Bytes(32),
                offset: 32,
            },
            Field {
                name: "name",
                kind: FieldKind::Bytes(64),
                offset: 64,
            },
            Field {
                name: "fields",
                kind: FieldKind::Bytes(96),
                offset: 128,
            },
            Field {
                name: "source",
                kind: FieldKind::Bytes(96),
                offset: 224,
            },
            Field {
                name: "line",
                kind: FieldKind::U64,
                offset: 320,
            },
        ],
    },
    Record {
        name: "trace_batch",
        minimum_size: 2656,
        size: 2656,
        alignment: 8,
        fields: &[
            Field {
                name: "count",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "next",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "overwritten",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "incomplete",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "records",
                kind: FieldKind::Bytes(2624),
                offset: 32,
            },
        ],
    },
    Record {
        name: "trace_thread",
        minimum_size: 72,
        size: 72,
        alignment: 8,
        fields: &[
            Field {
                name: "thread",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "name",
                kind: FieldKind::Bytes(64),
                offset: 8,
            },
        ],
    },
    Record {
        name: "scheduling_parameters",
        fields: SCHEDULING_PARAMETERS_FIELDS,
        minimum_size: 64,
        size: 64,
        alignment: 8,
    },
    // Capture identity and buffer geometry; period is a PMU event-count period,
    // while deadline_ns is absolute monotonic time. Generations detect reuse.
    Record {
        name: "cpu_profile_info",
        fields: &[
            Field {
                name: "generation",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "period",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "deadline_ns",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "cpu_count",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "running_generation",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "armed_mask",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "capacity",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "depth",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "active_cpus",
                kind: FieldKind::U64,
                offset: 64,
            },
            Field {
                name: "dropped",
                kind: FieldKind::U64,
                offset: 72,
            },
            Field {
                name: "revision",
                kind: FieldKind::Bytes(64),
                offset: 80,
            },
        ],
        minimum_size: 144,
        size: 144,
        alignment: 8,
    },
    // One bounded sample, not a live stack reference. depth bounds the PC array;
    // dropped reports missing samples rather than implying a complete trace.
    Record {
        name: "cpu_profile_sample",
        fields: &[
            Field {
                name: "sequence",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "timestamp_ns",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "cpu",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "world",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "depth",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "flags",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "dropped",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "period",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "frames",
                kind: FieldKind::Bytes(128),
                offset: 64,
            },
        ],
        minimum_size: 192,
        size: 192,
        alignment: 8,
    },
    // Retention lifecycle and release reason are independent of live process state.
    Record {
        name: "process_retirement_info",
        minimum_size: 104,
        size: 104,
        alignment: 8,
        fields: &[
            Field {
                name: "size",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "phase",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 8,
            },
            Field {
                name: "release_reason",
                kind: FieldKind::U32,
                offset: 12,
            },
            Field {
                name: "process_koid",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "deadline_ns",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "retained_bytes",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "memory_limit",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "terminal_reason",
                kind: FieldKind::U32,
                offset: 48,
            },
            Field {
                name: "fault_class",
                kind: FieldKind::U32,
                offset: 52,
            },
            Field {
                name: "exit_status",
                kind: FieldKind::I64,
                offset: 56,
            },
            Field {
                name: "fault_pc",
                kind: FieldKind::U64,
                offset: 64,
            },
            Field {
                name: "fault_address",
                kind: FieldKind::U64,
                offset: 72,
            },
            Field {
                name: "fault_syndrome",
                kind: FieldKind::U64,
                offset: 80,
            },
            Field {
                name: "thread_count",
                kind: FieldKind::U32,
                offset: 88,
            },
            Field {
                name: "mapping_count",
                kind: FieldKind::U32,
                offset: 92,
            },
            Field {
                name: "handle_count",
                kind: FieldKind::U32,
                offset: 96,
            },
            Field {
                name: "reserved",
                kind: FieldKind::U32,
                offset: 100,
            },
        ],
    },
    // Architecture-tagged saved context. Byte arrays carry register encodings,
    // never pointers to a live kernel exception frame.
    Record {
        name: "crash_thread",
        minimum_size: 840,
        size: 840,
        alignment: 8,
        fields: &[
            Field {
                name: "size",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "machine",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 8,
            },
            Field {
                name: "reserved",
                kind: FieldKind::U32,
                offset: 12,
            },
            Field {
                name: "thread_koid",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "pc",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "sp",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "tls",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "processor_state",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "general",
                kind: FieldKind::Bytes(256),
                offset: 56,
            },
            Field {
                name: "vector",
                kind: FieldKind::Bytes(512),
                offset: 312,
            },
            Field {
                name: "fp_control",
                kind: FieldKind::Bytes(16),
                offset: 824,
            },
        ],
    },
    // Retained/excluded/partial flags distinguish unavailable memory from a
    // readable retained range; an observed mapping is not a writable grant.
    Record {
        name: "crash_mapping",
        minimum_size: 48,
        size: 48,
        alignment: 8,
        fields: &[
            Field {
                name: "size",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "base",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "length",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "object_offset",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "object_koid",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "permissions",
                kind: FieldKind::U32,
                offset: 40,
            },
            Field {
                name: "kind",
                kind: FieldKind::U32,
                offset: 44,
            },
        ],
    },
    // Diagnostic identity only: handles were released when the process stopped.
    Record {
        name: "crash_handle",
        minimum_size: 40,
        size: 40,
        alignment: 8,
        fields: &[
            Field {
                name: "size",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "kind",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "value",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "koid",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "rights",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 32,
            },
            Field {
                name: "reserved",
                kind: FieldKind::U32,
                offset: 36,
            },
        ],
    },
    // Statistics topics are independent samples, not an atomic cross-topic snapshot.
    // Physical metadata and reservations may overlap other accounting categories.
    Record {
        name: "memory_heap_statistics",
        fields: &[
            Field {
                name: "captured_at_ns",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "live_objects",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "live_slab_objects",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "live_large_objects",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "requested_bytes",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "peak_objects",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "peak_requested_bytes",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "allocations",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "failures",
                kind: FieldKind::U64,
                offset: 64,
            },
            Field {
                name: "cached_objects",
                kind: FieldKind::U64,
                offset: 72,
            },
            Field {
                name: "cache_hits",
                kind: FieldKind::U64,
                offset: 80,
            },
            Field {
                name: "cache_misses",
                kind: FieldKind::U64,
                offset: 88,
            },
            Field {
                name: "cache_refills",
                kind: FieldKind::U64,
                offset: 96,
            },
            Field {
                name: "cache_drains",
                kind: FieldKind::U64,
                offset: 104,
            },
            Field {
                name: "pressure_reclaims",
                kind: FieldKind::U64,
                offset: 112,
            },
            Field {
                name: "reclaimed_objects",
                kind: FieldKind::U64,
                offset: 120,
            },
            Field {
                name: "cache_cpus",
                kind: FieldKind::U64,
                offset: 128,
            },
            Field {
                name: "profiling_enabled",
                kind: FieldKind::U64,
                offset: 136,
            },
            Field {
                name: "page_allocations",
                kind: FieldKind::U64,
                offset: 144,
            },
            Field {
                name: "page_failures",
                kind: FieldKind::U64,
                offset: 152,
            },
            Field {
                name: "page_deallocations",
                kind: FieldKind::U64,
                offset: 160,
            },
            Field {
                name: "peak_allocated_pages",
                kind: FieldKind::U64,
                offset: 168,
            },
        ],
        minimum_size: 176,
        size: 176,
        alignment: 8,
    },
    Record {
        name: "memory_buddy_statistics",
        fields: &[
            Field {
                name: "captured_at_ns",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "order",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "free_blocks",
                kind: FieldKind::U64,
                offset: 16,
            },
        ],
        minimum_size: 24,
        size: 24,
        alignment: 8,
    },
    Record {
        name: "memory_owner_statistics",
        fields: &[
            Field {
                name: "captured_at_ns",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "resident_pages",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "peak_pages",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "allocations",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "failures",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "deallocations",
                kind: FieldKind::U64,
                offset: 40,
            },
        ],
        minimum_size: 48,
        size: 48,
        alignment: 8,
    },
    Record {
        name: "memory_file_cache_statistics",
        fields: &[
            Field {
                name: "captured_at_ns",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "payload_pages",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "metadata_slab_pages",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "metadata_large_pages",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "index_capacity",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "clean_entries",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "loading_entries",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "live_payloads",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "metadata_budget_pages",
                kind: FieldKind::U64,
                offset: 64,
            },
            Field {
                name: "table_present",
                kind: FieldKind::U64,
                offset: 72,
            },
            Field {
                name: "admission_enabled",
                kind: FieldKind::U64,
                offset: 80,
            },
            Field {
                name: "admission_paused",
                kind: FieldKind::U64,
                offset: 88,
            },
            Field {
                name: "maintenance_pending",
                kind: FieldKind::U64,
                offset: 96,
            },
            Field {
                name: "maintenance_running",
                kind: FieldKind::U64,
                offset: 104,
            },
            Field {
                name: "growth_requested",
                kind: FieldKind::U64,
                offset: 112,
            },
        ],
        minimum_size: 120,
        size: 120,
        alignment: 8,
    },
    Record {
        name: "memory_heap_class_statistics",
        fields: &[
            Field {
                name: "captured_at_ns",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "pool",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "object_size",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "pages",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "capacity",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "reserved",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "cached",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "live",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "requested_bytes",
                kind: FieldKind::U64,
                offset: 64,
            },
            Field {
                name: "allocations",
                kind: FieldKind::U64,
                offset: 72,
            },
            Field {
                name: "deallocations",
                kind: FieldKind::U64,
                offset: 80,
            },
        ],
        minimum_size: 88,
        size: 88,
        alignment: 8,
    },
    // Root replacement paths and relocation array. The invocation receiver supplies
    // namespace authority; embedded addresses and path spellings do not grant it.
    Record {
        name: "filesystem_root_switch",
        fields: &[
            Field {
                name: "new_root",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "new_root_length",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "put_old",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "put_old_length",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "moves",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "move_count",
                kind: FieldKind::U64,
                offset: 40,
            },
        ],
        minimum_size: 48,
        size: 48,
        alignment: 8,
    },
    Record {
        name: "filesystem_mount_move",
        fields: &[
            Field {
                name: "source",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "source_length",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "target",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "target_length",
                kind: FieldKind::U64,
                offset: 24,
            },
        ],
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "object_details",
        fields: OBJECT_DETAILS_FIELDS,
        minimum_size: 88,
        size: 88,
        alignment: 8,
    },
    // First retained containment report; querying neither consumes it nor resumes
    // DMA. Interpret address/direction only when the corresponding flags are set.
    Record {
        name: "device_dma_fault_info",
        fields: &[
            Field {
                name: "timestamp_ns",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "address",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "stream_id",
                kind: FieldKind::U32,
                offset: 16,
            },
            Field {
                name: "source",
                kind: FieldKind::U32,
                offset: 20,
            },
            Field {
                name: "scope",
                kind: FieldKind::U32,
                offset: 24,
            },
            Field {
                name: "code",
                kind: FieldKind::U32,
                offset: 28,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 32,
            },
            Field {
                name: "reserved",
                kind: FieldKind::U32,
                offset: 36,
            },
        ],
        minimum_size: 40,
        size: 40,
        alignment: 8,
    },
    // Admitted guest aperture and DMA bus translation, not arbitrary host mappings.
    Record {
        name: "device_profile_info",
        fields: &[
            Field {
                name: "profile",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "interrupt_count",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "resource_count",
                kind: FieldKind::U32,
                offset: 8,
            },
            Field {
                name: "pci_identity",
                kind: FieldKind::U32,
                offset: 12,
            },
            Field {
                name: "dma_bus_offset",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "aperture_size",
                kind: FieldKind::U64,
                offset: 24,
            },
        ],
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "device_firmware_query",
        fields: &[
            Field {
                name: "node",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "field",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "name_address",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "name_length",
                kind: FieldKind::U64,
                offset: 16,
            },
        ],
        minimum_size: 24,
        size: 24,
        alignment: 8,
    },
    Record {
        name: "device_firmware_info",
        fields: &[
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "register_count",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "irq_number",
                kind: FieldKind::U32,
                offset: 8,
            },
            Field {
                name: "irq_trigger",
                kind: FieldKind::U32,
                offset: 12,
            },
            Field {
                name: "reserved0",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "reserved1",
                kind: FieldKind::U64,
                offset: 24,
            },
        ],
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "device_bundle_entry",
        fields: &[
            Field {
                name: "node",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "resource",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "offset",
                kind: FieldKind::U64,
                offset: 8,
            },
        ],
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "device_resource_info",
        fields: &[
            Field {
                name: "kind",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "offset",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "length",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "bus_address",
                kind: FieldKind::U64,
                offset: 24,
            },
        ],
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "physical_device_info",
        fields: &[
            Field {
                name: "device_id",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "transport_version",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "mmio_size",
                kind: FieldKind::U64,
                offset: 8,
            },
        ],
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "dma_extent",
        fields: &[
            Field {
                name: "physical_base",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "length",
                kind: FieldKind::U64,
                offset: 8,
            },
        ],
        minimum_size: 16,
        size: 16,
        alignment: 8,
    },
    Record {
        name: "virtual_cpu_mmio_request",
        fields: VIRTUAL_CPU_MMIO_REQUEST_FIELDS,
        minimum_size: 48,
        size: 48,
        alignment: 8,
    },
    // Source range and destination placement are independent: data_offset places
    // source bytes inside the mapping; mode selects eager or copy-on-write backing.
    Record {
        name: "private_mapping",
        fields: &[
            Field {
                name: "source_offset",
                kind: FieldKind::U64,
                offset: 0,
            },
            Field {
                name: "source_length",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "address",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "size",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "data_offset",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "permissions",
                kind: FieldKind::U32,
                offset: 40,
            },
            Field {
                name: "mode",
                kind: FieldKind::U32,
                offset: 44,
            },
        ],
        minimum_size: 48,
        size: 48,
        alignment: 8,
    },
    Record {
        name: "virtual_machine_platform_info",
        fields: &[
            Field {
                name: "architecture",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "platform_profile",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "counter_frequency_hz",
                kind: FieldKind::U64,
                offset: 8,
            },
            Field {
                name: "riscv_isa",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "aarch64_gic_version",
                kind: FieldKind::U64,
                offset: 24,
            },
        ],
        minimum_size: 32,
        size: 32,
        alignment: 8,
    },
    Record {
        name: "file_metadata",
        fields: &[
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
                name: "kind",
                kind: FieldKind::U32,
                offset: 36,
            },
            Field {
                name: "valid_times",
                kind: FieldKind::U32,
                offset: 40,
            },
            Field {
                name: "reserved",
                kind: FieldKind::U32,
                offset: 44,
            },
            Field {
                name: "accessed_seconds",
                kind: FieldKind::I64,
                offset: 48,
            },
            Field {
                name: "accessed_nanoseconds",
                kind: FieldKind::U32,
                offset: 56,
            },
            Field {
                name: "accessed_reserved",
                kind: FieldKind::U32,
                offset: 60,
            },
            Field {
                name: "modified_seconds",
                kind: FieldKind::I64,
                offset: 64,
            },
            Field {
                name: "modified_nanoseconds",
                kind: FieldKind::U32,
                offset: 72,
            },
            Field {
                name: "modified_reserved",
                kind: FieldKind::U32,
                offset: 76,
            },
            Field {
                name: "created_seconds",
                kind: FieldKind::I64,
                offset: 80,
            },
            Field {
                name: "created_nanoseconds",
                kind: FieldKind::U32,
                offset: 88,
            },
            Field {
                name: "created_reserved",
                kind: FieldKind::U32,
                offset: 92,
            },
            Field {
                name: "changed_seconds",
                kind: FieldKind::I64,
                offset: 96,
            },
            Field {
                name: "changed_nanoseconds",
                kind: FieldKind::U32,
                offset: 104,
            },
            Field {
                name: "changed_reserved",
                kind: FieldKind::U32,
                offset: 108,
            },
        ],
        minimum_size: 112,
        size: 112,
        alignment: 8,
    },
    Record {
        name: "file_metadata_update",
        fields: &[
            Field {
                name: "mask",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "mode",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "accessed_seconds",
                kind: FieldKind::I64,
                offset: 8,
            },
            Field {
                name: "accessed_nanoseconds",
                kind: FieldKind::U32,
                offset: 16,
            },
            Field {
                name: "accessed_reserved",
                kind: FieldKind::U32,
                offset: 20,
            },
            Field {
                name: "modified_seconds",
                kind: FieldKind::I64,
                offset: 24,
            },
            Field {
                name: "modified_nanoseconds",
                kind: FieldKind::U32,
                offset: 32,
            },
            Field {
                name: "modified_reserved",
                kind: FieldKind::U32,
                offset: 36,
            },
        ],
        minimum_size: 40,
        size: 40,
        alignment: 8,
    },
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
    // Kernel-to-loader bootstrap envelope. Handles belong to the new process;
    // arguments, environment, auxv and final application stack remain loader policy.
    Record {
        name: "loader_startup",
        fields: &[
            Field {
                name: "size",
                kind: FieldKind::U32,
                offset: 0,
            },
            Field {
                name: "handle_count",
                kind: FieldKind::U32,
                offset: 4,
            },
            Field {
                name: "data_size",
                kind: FieldKind::U32,
                offset: 8,
            },
            Field {
                name: "flags",
                kind: FieldKind::U32,
                offset: 12,
            },
            Field {
                name: "executable",
                kind: FieldKind::U64,
                offset: 16,
            },
            Field {
                name: "root_vmar",
                kind: FieldKind::U64,
                offset: 24,
            },
            Field {
                name: "stack_vmar",
                kind: FieldKind::U64,
                offset: 32,
            },
            Field {
                name: "stack_base",
                kind: FieldKind::U64,
                offset: 40,
            },
            Field {
                name: "stack_size",
                kind: FieldKind::U64,
                offset: 48,
            },
            Field {
                name: "loader_base",
                kind: FieldKind::U64,
                offset: 56,
            },
            Field {
                name: "loader_size",
                kind: FieldKind::U64,
                offset: 64,
            },
            Field {
                name: "execution_context",
                kind: FieldKind::U64,
                offset: 72,
            },
            Field {
                name: "object_factory",
                kind: FieldKind::U64,
                offset: 80,
            },
            Field {
                name: "system_info",
                kind: FieldKind::U64,
                offset: 88,
            },
        ],
        minimum_size: 96,
        size: 96,
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
        minimum_size: 144,
        size: 144,
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
        name: "virtual_machine_power_request",
        fields: VIRTUAL_MACHINE_POWER_REQUEST_FIELDS,
        minimum_size: 40,
        size: 40,
        alignment: 8,
    },
    Record {
        name: "virtual_machine_info",
        fields: VIRTUAL_MACHINE_INFO_FIELDS,
        minimum_size: 32,
        size: 40,
        alignment: 8,
    },
    Record {
        name: "virtual_cpu_info",
        fields: VIRTUAL_CPU_INFO_FIELDS,
        minimum_size: 24,
        size: 32,
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
