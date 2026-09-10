<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Native ABI reference

This file is generated from `schema/native.rs`. Do not edit it directly.

ABI revision: `0`.

## Status values

| Value | Name |
| ---: | --- |
| 0 | `ok` |
| -1 | `invalid_argument` |
| -2 | `bad_handle` |
| -3 | `access_denied` |
| -4 | `not_supported` |
| -5 | `no_memory` |
| -6 | `bad_state` |
| -7 | `fault` |
| -8 | `resource_limit` |
| -9 | `busy` |
| -10 | `internal` |
| -11 | `timed_out` |
| -12 | `cancelled` |
| -13 | `would_block` |
| -14 | `buffer_too_small` |
| -15 | `peer_closed` |
| -16 | `not_found` |
| -17 | `already_exists` |
| -18 | `not_empty` |

## Object kinds

| Value | Name | Transfer |
| ---: | --- | --- |
| 0 | `none` | `forbidden` |
| 1 | `event` | `general` |
| 2 | `byte_channel` | `general` |
| 3 | `thread` | `rendezvous_only` |
| 4 | `process` | `rendezvous_only` |
| 5 | `task_group` | `rendezvous_only` |
| 6 | `resource_domain` | `general` |
| 7 | `task_factory` | `general` |
| 8 | `executable_authority` | `general` |
| 9 | `vmo` | `general` |
| 10 | `vmar` | `rendezvous_only` |
| 11 | `console` | `general` |
| 12 | `directory` | `general` |
| 13 | `file` | `general` |
| 14 | `capability_channel` | `rendezvous_only` |
| 15 | `process_builder` | `rendezvous_only` |
| 16 | `task_inspector` | `general` |
| 17 | `object_inspector` | `general` |
| 18 | `memory_inspector` | `general` |
| 19 | `cpu_inspector` | `general` |
| 20 | `virtual_machine_creation_authority` | `general` |
| 21 | `virtual_machine_creation_lease` | `rendezvous_only` |
| 22 | `pending_virtual_machine` | `rendezvous_only` |
| 23 | `virtual_machine` | `rendezvous_only` |
| 24 | `virtual_cpu` | `rendezvous_only` |
| 25 | `virtual_serial` | `forbidden` |
| 26 | `wait_set` | `forbidden` |

## Object signals

| Object | Bit | Name |
| --- | ---: | --- |
| `virtual_serial` | 0 | `readable` |
| `virtual_serial` | 1 | `writable` |
| `virtual_serial` | 2 | `peer_closed` |
| `wait_set` | 0 | `readable` |
| `event` | 0 | `signaled` |
| `byte_channel` | 0 | `readable` |
| `byte_channel` | 1 | `writable` |
| `byte_channel` | 2 | `peer_closed` |
| `capability_channel` | 0 | `peer_receiving` |
| `capability_channel` | 1 | `peer_closed` |
| `thread` | 0 | `terminated` |
| `process` | 0 | `terminated` |
| `console` | 0 | `readable` |
| `console` | 1 | `writable` |
| `virtual_machine` | 0 | `terminated` |
| `virtual_cpu` | 0 | `terminated` |

## Constants

| Name | Value |
| --- | ---: |
| `page_size` | `4096` |
| `extensible_record_max_bytes` | `4096` |
| `virtual_serial_max_transfer_bytes` | `4096` |
| `virtual_serial_output_capacity` | `65536` |
| `virtual_serial_output_header_bytes` | `4096` |
| `virtual_serial_output_bytes` | `69632` |
| `elf_osabi` | `63` |
| `elf_abi_version` | `0` |
| `auxv_startup_handles` | `1213792257` |
| `auxv_startup_handle_count` | `1213792258` |
| `startup_handle_purpose_resource_domain` | `1` |
| `startup_handle_purpose_task_group` | `2` |
| `startup_handle_purpose_task_factory` | `3` |
| `startup_handle_purpose_executable_authority` | `4` |
| `startup_handle_purpose_root_vmar` | `5` |
| `startup_handle_purpose_console` | `6` |
| `startup_handle_purpose_root_directory` | `7` |
| `startup_handle_purpose_task_inspector` | `8` |
| `startup_handle_purpose_object_inspector` | `9` |
| `startup_handle_purpose_dynamic_library_directory` | `10` |
| `startup_handle_purpose_memory_inspector` | `11` |
| `startup_handle_purpose_cpu_inspector` | `12` |
| `startup_handle_purpose_virtual_machine_creation_authority` | `13` |
| `virtual_machine_architecture_aarch64` | `1` |
| `virtual_machine_architecture_riscv64` | `2` |
| `virtual_machine_architecture_x86_64` | `3` |
| `virtual_platform_aarch64_reference` | `1` |
| `virtual_platform_aarch64_reference_guest_ram_base` | `1073741824` |
| `virtual_platform_aarch64_reference_dtb_offset` | `65536` |
| `virtual_platform_aarch64_reference_gic_distributor_base` | `134217728` |
| `virtual_platform_aarch64_reference_gic_distributor_size` | `65536` |
| `virtual_platform_aarch64_reference_gic_redistributor_base` | `134873088` |
| `virtual_platform_aarch64_reference_gic_redistributor_size` | `131072` |
| `virtual_platform_aarch64_reference_uart_base` | `150994944` |
| `virtual_platform_aarch64_reference_uart_size` | `4096` |
| `virtual_platform_aarch64_reference_uart_interrupt` | `33` |
| `virtual_platform_aarch64_reference_timer_interrupt` | `27` |
| `virtual_machine_phase_installed` | `1` |
| `virtual_machine_phase_running` | `2` |
| `virtual_machine_phase_stopping` | `3` |
| `virtual_machine_phase_stopped` | `4` |
| `virtual_cpu_phase_dormant` | `1` |
| `virtual_cpu_phase_started` | `2` |
| `virtual_cpu_phase_stopped` | `3` |
| `virtual_cpu_terminal_none` | `0` |
| `virtual_cpu_terminal_memory_fault` | `1` |
| `virtual_cpu_terminal_mmio` | `2` |
| `virtual_cpu_terminal_synchronous` | `3` |
| `virtual_cpu_terminal_administrative` | `4` |
| `directory_entry_page_capacity` | `4` |
| `directory_entry_name_max_bytes` | `255` |
| `directory_entry_kind_file` | `1` |
| `directory_entry_kind_directory` | `2` |
| `directory_entry_kind_symlink` | `3` |
| `directory_entry_kind_other` | `4` |
| `startup_max_handles` | `256` |
| `deadline_infinite` | `18446744073709551615` |
| `object_wait_many_max_items` | `64` |
| `capability_disposition_same_rights` | `18446744073709551615` |
| `byte_channel_max_message_bytes` | `65536` |
| `byte_channel_max_queued_messages` | `16` |
| `byte_channel_max_queued_bytes` | `1048576` |
| `capability_channel_max_message_bytes` | `4096` |
| `capability_channel_max_handles` | `16` |
| `capability_disposition_move` | `0` |
| `capability_disposition_duplicate` | `1` |
| `console_max_transfer_bytes` | `4096` |
| `directory_max_path_bytes` | `4096` |
| `file_max_read_bytes` | `65536` |
| `vmo_max_size_bytes` | `4294967296` |
| `vmo_max_transfer_bytes` | `65536` |
| `vmar_permission_read` | `1` |
| `vmar_permission_write` | `2` |
| `vmar_permission_execute` | `4` |
| `process_name_max_bytes` | `64` |
| `process_argument_max_bytes` | `4096` |
| `process_environment_max_bytes` | `4096` |
| `process_max_arguments` | `64` |
| `process_max_environment` | `64` |
| `process_affinity_max_words` | `4` |
| `process_affinity_max_cpus` | `256` |
| `process_phase_prepared` | `0` |
| `process_phase_created` | `1` |
| `process_phase_running` | `2` |
| `process_phase_stopping` | `3` |
| `process_phase_stopped` | `4` |
| `process_phase_retiring` | `5` |
| `process_phase_retired` | `6` |
| `process_terminal_none` | `0` |
| `process_terminal_requested` | `1` |
| `process_terminal_thread_exited` | `2` |
| `process_terminal_process_exited` | `3` |
| `process_terminal_last_thread_exited` | `4` |
| `process_terminal_fault` | `5` |
| `process_terminal_task_group_stop` | `6` |
| `task_inspector_process_page_capacity` | `8` |
| `task_inspector_thread_page_capacity` | `8` |
| `object_inspector_object_page_capacity` | `8` |
| `object_inspector_handle_page_capacity` | `8` |
| `thread_role_bootstrap` | `1` |
| `thread_role_idle` | `2` |
| `thread_role_kernel` | `3` |
| `thread_role_user` | `4` |
| `thread_role_vcpu` | `5` |
| `thread_registry_resident` | `1` |
| `thread_registry_retiring` | `2` |
| `object_handle_state_unpublished` | `1` |
| `object_handle_state_active` | `2` |
| `object_handle_state_retired` | `3` |

## Semantic rules

- WaitSets are process-local, non-transferable objects with capacity 1..1024. BIND_WAIT authorizes add/rearm/remove, WAIT authorizes consumption. Add requires source WAIT and reserves one event slot; WaitSet and CapabilityChannel sources are unsupported. Registration IDs are globally non-reused. Bind/rearm observe signal levels and sequence under the source lock; one-shot publication does not allocate. Rearm is busy until successful event consumption. Wait returns one exact 24-byte record (registration ID, signal bits, sequence), using an absolute monotonic deadline. Copyout failure restores the event unless removal or closure cancelled it. Source handle close does not cancel object-lifetime subscriptions; final set handle close detaches registrations and wakes consumers. Future CapabilityChannel subscriptions require ownership-epoch invalidation.
- ByteChannel duplicate authority permits shared endpoint ownership; peer_closed is published only when the last active endpoint handle closes. Internal operation pins, including WaitSet subscriptions, do not retain active endpoint authority.
- process_get_current_id returns the calling Process KOID for observation only. It creates no handle or operational authority and is not a PID-to-handle lookup.
- Ramfs storage and all current VFS operations remain kernel-owned. Open File and Directory objects pin their nodes after unlink. Directory WRITE permits namespace mutation; File WRITE permits content mutation. directory_create_file exclusively creates a regular file and publishes its requested handle atomically with the new name. Mode accepts permission bits 0777. directory_remove options 0 removes a non-directory entry without following the final symlink; 1 removes an empty directory. file_write_at options 0 uses offset, 1 atomically appends (offset must be zero); returns actual bytes and end offset. Transfers may complete short. file_resize zero-fills extension. Directory cookies are monotonic, never reused, and enumeration is weakly consistent across mutations. Executable snapshots remain immutable across writes.
- Thread creation prepares a dormant thread in the calling Process and returns duplicate, wait, inspect, start and request_stop authority. Entry, 16-byte-aligned stack, TLS and an opaque first argument define its initial context. The caller retains stack/TLS ownership through the thread terminated signal. Start publishes runnable execution; request_stop abandons execution without language destructors. Closing the last handle to a dormant thread cancels it; running threads continue independently of handle ownership. Existing thread_exit terminates only the caller, while process_exit stops every thread.
- Atomic wait/wake currently operate on aligned writable u32 words in the calling Process. Wait checks the value and publishes its generation atomically with respect to wake; mismatch returns ok, and spurious wakes are allowed. Wait uses an absolute monotonic nanosecond deadline, with UINT64_MAX meaning infinite; timeout returns timed_out and cancellation returns cancelled. Wake returns the number of registrations actually notified, bounded by count. Wake does not replace the caller's release operation or the waiter's acquire predicate check. Keys include the non-reused mapping identity and virtual address; aliases and other Processes have separate wait domains. Keep the mapping live while waiting: unmap/protect does not move an existing wait to a replacement mapping. Pinned backing survives concurrent unmap until admitted accesses finish; process stop cancels outstanding waits.
- Thread sleep accepts an absolute monotonic nanosecond deadline and returns ok when it expires, including an already elapsed deadline. UINT64_MAX sleeps until cancellation. It parks the current thread through scheduler timeout arbitration rather than polling or yielding.
- The monotonic clock syscall returns absolute nanoseconds from the kernel's monotonic clock domain. Ambient monotonic observation is intentionally not a capability because reading it conveys no mutable authority; future virtual or adjustable clocks may be represented by handle objects without changing this clock domain.
- Extensible input records carry their caller size in the syscall's explicit byte-count argument. The size must be at least the record's published minimum prefix and no greater than extensible_record_max_bytes; missing bytes through the kernel's current record size default to zero, while bytes beyond that size must be zero. Extensible information outputs accept any capacity from the record's minimum prefix through extensible_record_max_bytes, write only the intersection of caller capacity and kernel-supported size, and return the kernel-supported size in value0 on ok. Bytes beyond that intersection remain untouched.
- Directory lookup is capability-relative. A leading slash restarts at that Directory's traversal root, and parent components cannot escape it. Every open requires read traversal authority, and every requested File right must already be present on the source Directory before the result is further bounded by the File object's node-specific ceiling.
- Directory reads require the exact published page capacity. Cookie zero starts a scan and a returned next_cookie of zero ends it. Every name is one valid path component encoded as name_length UTF-8 bytes followed by zero-filled capacity. Pages are weakly consistent with concurrent filesystem mutation; callers must neither interpret nor synthesize cookies.
- Native task and object inspectors are immutable capability-scoped views. Process, thread, and object KOIDs plus scan cursors are observation-only values and can never be exchanged for operational authority. Out-of-scope targeted lookup returns not_found.
- Task inspector records carry a bounded UTF-8 name as name_length bytes followed by zero-filled capacity. Process names are the immutable labels committed by ProcessBuilder publication; Thread names are immutable scheduler identity labels retained through the retiring registry phase.
- Inspector derivation is monotonic: a derived Process, TaskGroup, or ResourceDomain view cannot widen its parent's task scope, object scope, visibility, or rights. Derivation requires the inspector's complete supported rights because the returned handle carries that fixed rights set; callers attenuate it before delegation. Native task operations remain handle-based; numeric PID and TID namespaces belong exclusively to compatibility personalities.
- Every live TaskGroup handle participates in shared group-lifetime ownership regardless of its attenuated rights. Closing the last TaskGroup handle asynchronously requests stop for every member. Rights control operations available through a handle; they do not change this ownership effect.
- Inspector scans require the exact published page capacity for their record type. Cursor zero starts a scan and a returned next_cursor of zero ends it. Pages and complete scans are weakly consistent with concurrent task, object, and handle-table mutation; generation-qualified handle values prevent slot reuse from aliasing an earlier observation.
- Memory and CPU inspectors publish immutable point-in-time copies. Their handles grant observation only; they never expose writable accounting storage or allocator and scheduler synchronization to userspace. CPU categories are scheduler-tick observations and a multi-CPU snapshot is weakly consistent across CPUs.
- File and Directory information reports attribute snapshots plus filesystem, mount, and node identities for diagnostics and correlation. These identities do not grant authority, cannot be resolved back into handles, and do not define a pathname; directory entries, hard links, renames, mount namespaces, and unlinks make pathnames namespace-dependent observations rather than object identity.
- Object wait-many borrows every input handle for the complete wait, canonicalizes duplicate object identities, and selects the lowest input index whose requested mask intersects the winning object's committed level snapshot. Source-handle close after resolution does not cancel the wait.
- Process terminal detail fields are reason-specific: exit reasons encode the signed status as two's-complement in detail0; fault encodes class in detail0 and code in detail1; task-group stop encodes generation in detail0; unused details are zero.
- Object transfer classes constrain generic capability transports. General objects may be retained by buffered or rendezvous transports. Rendezvous-only objects may move or duplicate only by a direct source-to-destination commit which never creates an in-transit owner. Forbidden objects cannot cross a userspace handle table boundary.
- AtomicOnOk capability transactions commit handle-table ownership, the rendezvous message, and live output-handle installation together only when both participants return ok. Every non-ok status preserves all input owners and the message. Non-fault failures leave output memory unchanged; fault may partially modify output byte or slot memory, but no handle value written by a failed call is live or installed. Bindings must ignore every output-memory byte after any non-ok status.
- A capability-channel receiver advertises peer_receiving only after its byte range, typed capability slots, and destination handle-table capacity are validated, reserved, and fully published on the endpoint's FIFO receiver queue. The signal is level-triggered but may race another sender.
- A capability disposition's rights are the sender's offered ceiling; capability_disposition_same_rights offers all source rights. A receive slot's rights are the exact installed grant, so receiver rights must be a subset of the sender offer and the offer a subset of source rights. Sender and receiver expected_kind values are nonzero and must both equal the actual object kind.
- A capability receive slot enters with handle and flags zero. On ok, only handle changes. Implementations reserve destination entries, prewrite their future handle values, then perform an infallible handle-table commit. Buffer-too-small changes no slot or byte memory and reports both required counts.
- Capability disposition move consumes its source only on ok and requires transfer. Capability disposition duplicate retains its source and requires transfer plus duplicate. Unknown operations, repeated source handles, kind mismatches, and rights violations reject the complete transaction.
- Capability-channel try_send returns peer_closed when no peer remains; otherwise it returns would_block only when no receiver is queued. It FIFO-matches the oldest fully published receiver. A size, kind, rights, operation, or copy mismatch rejects that transaction for both participants without scanning later receivers.
- Capability-channel timeout, cancellation, and peer close can win only before a receiver is matched. Once matched, sender completion or rejection owns the transaction through commit, including races with the deadline, cancellation, or close; both participants observe the same committed outcome.
- Process-builder create borrows its authority handles and retains kernel object references independently of the caller handles. Builders are mutable only before seal. Every successful mutator applies completely; every failure leaves the builder unchanged. Seal is irreversible. Start requires a sealed builder and returns only a supervisor process handle. Start and abort consume the builder handle only on ok; every failure preserves it.
- A process-builder name is nonempty UTF-8 without embedded NUL bytes. The argv vector contains at least one entry; individual argument strings are UTF-8 and may be empty but contain no NUL byte. Every UTF-8 environment entry contains a nonempty name with no '=' followed by '=' and a NUL-free value. Counts and individual byte lengths remain within the published constants.
- Process-builder set_name and set_affinity replace their prior values; add_argument and add_environment append in order. Process-builder affinity is a nonempty little-endian array of u64 CPU-mask words. Bits above process_affinity_max_cpus and bits which cannot designate an allowed CPU are rejected.
- Process-builder add_handle requires a nonzero purpose unique within the builder, an expected nonzero exact object kind, and either exact granted rights or capability_disposition_same_rights. Move consumes the source only when the mutator returns ok; duplicate retains it and additionally requires duplicate. Failure preserves both builder and source.
- A VirtualMachineCreationAuthority may derive one resource-domain-bound VirtualMachineCreationLease. The lease is single-use and is consumed only when VirtualMachine creation publishes a PendingVirtualMachine handle successfully.
- A PendingVirtualMachine is mutable until seal. It must own exactly one writable VMO whose size equals the configured guest RAM and one bootstrap record for boot vCPU 0. The configured vcpu_count fixes immutable topology; architecture power-on protocols supply secondary-vCPU runtime entry state, and future additive VirtualMachine operations may expose their control handles. A guest serial route is optional and exists only when a caller transfers a VirtualSerial handle with assign-device authority before seal. Successful binding consumes the supplied handle and commits a VM-owned reference until VM retirement; a rejected binding leaves the handle unchanged. Guest output is published to a read-only shared VMO consumed by the owning runtime, and VM retirement disconnects the input route without invalidating existing output mappings. Seal is irreversible; install consumes the pending handle only on ok and publishes the installed VirtualMachine and dormant boot VirtualCpu handles together. VirtualCpu start is a separate operation after handle publication. The started VirtualCpu phase means that start committed successfully; it is not an observation that the scheduler currently considers the vCPU runnable or executing. The current implementation accepts one vCPU.
- VirtualSerial handles are process-local; device assignment consumes a same-process handle. register_output borrows a caller-allocated writable VMO of exactly 69632 bytes and registers it once before assignment. An exclusive write lease rejects existing writable mappings, direct accesses, snapshots, and further writers until port retirement; read-only mappings may coexist. Offset 0 is an atomic u64 produced count, offset 8 a saturating dropped-byte count, and offset 4096 begins 65536 atomic byte slots. Registration initializes counters; callers must not access contents during registration. The producer release-publishes bytes; the runtime acquire-loads production and reads a batch. acknowledge_output requires READ and submits the absolute consumed position after reading. Regressing or future positions return invalid_argument without mutation. Publication, acknowledgement, and closure serialize on the port: READABLE means unacknowledged output, WRITABLE means a connected input route has queue space, and PEER_CLOSED means no future output or input. WAIT authorizes object waits and WaitSet subscriptions. Acknowledgement clears READABLE only when caught up; new output reasserts it, without lost wakeups or periodic polling. Full output discards new bytes without blocking or overwriting unconsumed slots. Counters never wrap. Last active handle closure synchronizes with writers and closes publication; registered pages remain pinned through final VM/object retirement. The SDK maps output read-only and acknowledges batches by syscall without copying payload through the syscall. Write remains nonblocking input injection: busy means the queue is full, bad_state means disconnected. Runtime policy owns retention and client transport.
- The creating process retains its guest VMO handle, but attaching it to a PendingVirtualMachine acquires exclusive hardware-write ownership and rejects any active Native writable mapping or direct VMO operation. Direct VMO access, snapshots, and writable Native mappings remain closed until VM retirement removes and invalidates every stage-2 mapping and releases the independent backing reference; read-only Native mappings may coexist.

## Syscalls

Auxiliary result registers are defined only for `ok` unless a result is annotated with
`also-on=<status>`. Element-count memory ranges are checked as count times the declared
element size before any user-memory access.

| Number | Name | Arguments | Results | Capability effects | User memory | Execution | Audit |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 0 | `abi_query` | — | `revision: u64`, `features: u64` | — | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Abi` |
| 1 | `handle_close` | `handle: handle` | — | `handle: ConsumeOnCommit, any, rights=0x0` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 2 | `handle_duplicate` | `source: handle`, `requested_rights: rights` | `handle: handle` | `source: Borrow, any, rights=0x1`, `handle: produce, same-as(source), subset-from(requested_rights)` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 3 | `handle_replace` | `source: handle`, `requested_rights: rights` | `handle: handle` | `source: ConsumeOnCommit, any, rights=0x0`, `handle: produce, same-as(source), subset-from(requested_rights)` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 4 | `handle_get_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | `supported_size: byte_count` | `handle: Borrow, any, rights=0x0` | `output: Write, len=output_size bytes, max-bytes=4096, record=handle_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 5 | `object_get_basic_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | `supported_size: byte_count` | `handle: Borrow, any, rights=0x8` | `output: Write, len=output_size bytes, max-bytes=4096, record=object_basic_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 6 | `thread_yield` | — | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 7 | `thread_exit` | `status: i64` | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` | `Task` |
| 8 | `process_exit` | `status: i64` | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` | `Task` |
| 9 | `event_create` | `options: u32` | `handle: handle` | `handle: produce, kind=event, fixed=0x8000f` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 10 | `event_signal` | `event: handle`, `clear_mask: u64`, `set_mask: u64` | — | `event: Borrow, kind=event, rights=0x80000` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 11 | `object_wait_one` | `object: handle`, `signals: u64`, `deadline: u64` | `observed: u64` | `object: Borrow, any, rights=0x4` | — | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Object` |
| 12 | `byte_channel_create` | `options: u32` | `endpoint0: handle`, `endpoint1: handle` | `endpoint0: produce, kind=byte_channel, fixed=0x3f`, `endpoint1: produce, kind=byte_channel, fixed=0x3f` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 13 | `byte_channel_write` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count` | — | `endpoint: Borrow, kind=byte_channel, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 14 | `byte_channel_read` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` | `actual_bytes: byte_count; also-on=buffer_too_small` | `endpoint: Borrow, kind=byte_channel, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 15 | `console_read` | `console: handle`, `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` | `actual_bytes: byte_count; also-on=would_block` | `console: Borrow, kind=console, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 16 | `console_write` | `console: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count` | `actual_bytes: byte_count; also-on=would_block` | `console: Borrow, kind=console, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 17 | `directory_open_file` | `directory: handle`, `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` | `file: handle` | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(requested_rights), allowed=0xbb, required-from=directory` | `path: Read, len=path_size bytes, max-bytes=4096; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 18 | `file_read_at` | `file: handle`, `options: u32`, `offset: u64`, `output: user_address`, `output_capacity: byte_count` | `actual_bytes: byte_count`, `file_size: byte_count` | `file: Borrow, kind=file, rights=0x10` | `output: Write, len=output_capacity bytes, max-bytes=65536; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 19 | `capability_channel_create` | `options: u32` | `endpoint0: handle`, `endpoint1: handle` | `endpoint0: produce, kind=capability_channel, fixed=0x3f`, `endpoint1: produce, kind=capability_channel, fixed=0x3f` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 20 | `capability_channel_try_send` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count`, `dispositions: user_address`, `disposition_count: element_count` | — | `endpoint: Borrow, kind=capability_channel, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0`, `dispositions: Read, len=disposition_count elements, max-elements=16, element-size=24, record=capability_disposition, transactional-handles=(handle, rights, expected_kind, operation), common-rights=0x2, operations=[move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3], commit=AtomicOnOk; order=1` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 21 | `capability_channel_receive` | `endpoint: handle`, `deadline: u64`, `bytes: user_address`, `byte_capacity: byte_count`, `capability_slots: user_address`, `slot_count: element_count` | `actual_bytes: byte_count; also-on=buffer_too_small`, `actual_capabilities: element_count; also-on=buffer_too_small` | `endpoint: Borrow, kind=capability_channel, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0`, `capability_slots: ReadWrite, len=slot_count elements, max-elements=16, element-size=24, record=capability_receive_slot, typed-receive-slots=(handle, rights, expected_kind, flags), produce-transferred-handles, commit=AtomicOnOk; order=1` | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Capability` |
| 22 | `process_builder_create` | `factory: handle`, `group: handle`, `domain: handle`, `executable: handle` | `builder: handle` | `factory: Borrow, kind=task_factory, rights=0x100000`, `group: Borrow, kind=task_group, rights=0x4000000`, `domain: Borrow, kind=resource_domain, rights=0x8000000`, `executable: Borrow, kind=file, rights=0x80`, `builder: produce, kind=process_builder, fixed=0xc2a` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 23 | `process_builder_set_name` | `builder: handle`, `name: user_address`, `name_size: byte_count` | — | `builder: Borrow, kind=process_builder, rights=0x20` | `name: Read, len=name_size bytes, max-bytes=64; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 24 | `process_builder_add_argument` | `builder: handle`, `argument: user_address`, `argument_size: byte_count` | — | `builder: Borrow, kind=process_builder, rights=0x20` | `argument: Read, len=argument_size bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 25 | `process_builder_add_environment` | `builder: handle`, `environment: user_address`, `environment_size: byte_count` | — | `builder: Borrow, kind=process_builder, rights=0x20` | `environment: Read, len=environment_size bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 26 | `process_builder_set_affinity` | `builder: handle`, `affinity_words: user_address`, `word_count: element_count` | — | `builder: Borrow, kind=process_builder, rights=0x20` | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 27 | `process_builder_add_handle` | `builder: handle`, `source: handle`, `purpose: u32`, `expected_kind: u32`, `rights: rights`, `operation: u32` | — | `builder: Borrow, kind=process_builder, rights=0x20`, `source: ByOperation(operation: move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3), any, common-rights=0x2` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 28 | `process_builder_seal` | `builder: handle` | — | `builder: Borrow, kind=process_builder, rights=0x20` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 29 | `process_builder_start` | `builder: handle` | `process: handle` | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400`, `process: produce, kind=process, fixed=0x80e` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 30 | `process_builder_abort` | `builder: handle` | — | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 31 | `process_request_stop` | `process: handle` | — | `process: Borrow, kind=process, rights=0x800` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 32 | `object_wait_many` | `items: user_address`, `item_count: element_count`, `deadline: u64` | `index: element_count`, `observed: u64` | — | `items: Read, len=item_count elements, max-elements=64, element-size=16, record=object_wait_item, borrowed-handles=(handle), required-rights=0x4; order=0` | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Object` |
| 33 | `process_get_info` | `process: handle`, `info: user_address`, `info_size: byte_count` | `supported_size: byte_count` | `process: Borrow, kind=process, rights=0x8` | `info: Write, len=info_size bytes, max-bytes=4096, record=process_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 34 | `task_inspector_scan_processes` | `inspector: handle`, `cursor: u64`, `records: user_address`, `capacity: element_count` | `count: element_count`, `next_cursor: u64` | `inspector: Borrow, kind=task_inspector, rights=0x8` | `records: Write, len=capacity elements, max-elements=8, element-size=96, record=task_process; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 35 | `task_inspector_scan_threads` | `inspector: handle`, `cursor: u64`, `records: user_address`, `capacity: element_count` | `count: element_count`, `next_cursor: u64` | `inspector: Borrow, kind=task_inspector, rights=0x8` | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=task_thread; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 36 | `task_inspector_derive_process` | `inspector: handle`, `process: handle` | `inspector: handle` | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 37 | `object_inspector_scan_objects` | `inspector: handle`, `cursor: u64`, `records: user_address`, `capacity: element_count` | `count: element_count`, `next_cursor: u64` | `inspector: Borrow, kind=object_inspector, rights=0x8` | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=object_inspection; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 38 | `object_inspector_scan_handles` | `inspector: handle`, `process_koid: u64`, `cursor: u64`, `records: user_address`, `capacity: element_count` | `count: element_count`, `next_cursor: u64` | `inspector: Borrow, kind=object_inspector, rights=0x8` | `records: Write, len=capacity elements, max-elements=8, element-size=40, record=handle_inspection; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 39 | `object_inspector_derive_process` | `inspector: handle`, `process: handle` | `inspector: handle` | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 40 | `task_inspector_derive_task_group` | `inspector: handle`, `task_group: handle` | `inspector: handle` | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 41 | `object_inspector_derive_task_group` | `inspector: handle`, `task_group: handle` | `inspector: handle` | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 42 | `task_inspector_derive_resource_domain` | `inspector: handle`, `resource_domain: handle` | `inspector: handle` | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 43 | `object_inspector_derive_resource_domain` | `inspector: handle`, `resource_domain: handle` | `inspector: handle` | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 44 | `directory_open_directory` | `directory: handle`, `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` | `child: handle` | `directory: Borrow, kind=directory, rights=0x10`, `child: produce, kind=directory, exact-from(requested_rights), allowed=0xbb, required-from=directory` | `path: Read, len=path_size bytes, max-bytes=4096; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 45 | `vmo_create` | `size: byte_count` | `vmo: handle` | `vmo: produce, kind=vmo, fixed=0x7b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 46 | `file_create_executable_vmo` | `file: handle` | `vmo: handle` | `file: Borrow, kind=file, rights=0x90`, `vmo: produce, kind=vmo, fixed=0xdb` | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 47 | `vmo_read` | `vmo: handle`, `offset: u64`, `bytes: user_address`, `byte_count: byte_count` | — | `vmo: Borrow, kind=vmo, rights=0x10` | `bytes: Write, len=byte_count bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 48 | `vmo_write` | `vmo: handle`, `offset: u64`, `bytes: user_address`, `byte_count: byte_count` | — | `vmo: Borrow, kind=vmo, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 49 | `vmar_allocate` | `vmar: handle`, `address: u64`, `size: byte_count` | `child: handle` | `vmar: Borrow, kind=vmar, rights=0x40`, `child: produce, kind=vmar, fixed=0x4b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 50 | `vmar_map` | `vmar: handle`, `vmo: handle`, `vmo_offset: u64`, `address: u64`, `size: byte_count`, `permissions: u32` | — | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x40` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 51 | `vmar_protect` | `vmar: handle`, `address: u64`, `size: byte_count`, `permissions: u32` | — | `vmar: Borrow, kind=vmar, rights=0x40` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 52 | `vmar_unmap` | `vmar: handle`, `address: u64`, `size: byte_count` | — | `vmar: Borrow, kind=vmar, rights=0x40` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 53 | `vmar_destroy` | `vmar: handle` | — | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 54 | `directory_read` | `directory: handle`, `cookie: u64`, `records: user_address`, `capacity: element_count`, `options: u32` | `count: element_count`, `next_cookie: u64` | `directory: Borrow, kind=directory, rights=0x10` | `records: Write, len=capacity elements, max-elements=4, element-size=280, record=directory_entry; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 55 | `memory_inspector_read` | `inspector: handle`, `observation: user_address`, `observation_size: byte_count` | `supported_size: byte_count` | `inspector: Borrow, kind=memory_inspector, rights=0x8` | `observation: Write, len=observation_size bytes, max-bytes=4096, record=memory_observation; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 56 | `cpu_inspector_read` | `inspector: handle`, `observation: user_address`, `observation_size: byte_count` | `supported_size: byte_count` | `inspector: Borrow, kind=cpu_inspector, rights=0x8` | `observation: Write, len=observation_size bytes, max-bytes=4096, record=cpu_observation; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 57 | `file_get_info` | `file: handle`, `output: user_address`, `output_size: byte_count` | `supported_size: byte_count` | `file: Borrow, kind=file, rights=0x8` | `output: Write, len=output_size bytes, max-bytes=4096, record=file_info; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 58 | `directory_get_info` | `directory: handle`, `output: user_address`, `output_size: byte_count` | `supported_size: byte_count` | `directory: Borrow, kind=directory, rights=0x8` | `output: Write, len=output_size bytes, max-bytes=4096, record=directory_info; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 59 | `virtual_machine_creation_lease_create` | `authority: handle`, `resource_domain: handle` | `lease: handle` | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `lease: produce, kind=virtual_machine_creation_lease, fixed=0x2000000a` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 60 | `virtual_machine_create` | `lease: handle`, `configuration: user_address`, `configuration_size: byte_count` | `pending_virtual_machine: handle` | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000`, `pending_virtual_machine: produce, kind=pending_virtual_machine, fixed=0xc2a` | `configuration: Read, len=configuration_size bytes, max-bytes=4096, record=virtual_machine_configuration; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 61 | `pending_virtual_machine_set_memory` | `pending_virtual_machine: handle`, `vmo: handle` | — | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `vmo: Borrow, kind=vmo, rights=0x70` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 62 | `pending_virtual_machine_set_bootstrap` | `pending_virtual_machine: handle`, `bootstrap: user_address`, `bootstrap_size: byte_count` | — | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` | `bootstrap: Read, len=bootstrap_size bytes, max-bytes=4096, record=virtual_cpu_bootstrap; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 63 | `pending_virtual_machine_seal` | `pending_virtual_machine: handle` | — | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 64 | `pending_virtual_machine_install` | `pending_virtual_machine: handle` | `virtual_machine: handle`, `boot_virtual_cpu: handle` | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400`, `virtual_machine: produce, kind=virtual_machine, fixed=0x80e`, `boot_virtual_cpu: produce, kind=virtual_cpu, fixed=0x40e` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 65 | `pending_virtual_machine_abort` | `pending_virtual_machine: handle` | — | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 66 | `virtual_machine_request_stop` | `virtual_machine: handle` | — | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 67 | `virtual_machine_get_info` | `virtual_machine: handle`, `info: user_address`, `info_size: byte_count` | `supported_size: byte_count` | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 68 | `virtual_cpu_get_info` | `virtual_cpu: handle`, `info: user_address`, `info_size: byte_count` | `supported_size: byte_count` | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_cpu_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 69 | `resource_domain_create` | `parent: handle`, `limits: user_address`, `limits_size: byte_count` | `child: handle` | `parent: Borrow, kind=resource_domain, rights=0x800000`, `child: produce, kind=resource_domain, fixed=0x984000b` | `limits: Read, len=limits_size bytes, max-bytes=4096, record=resource_limits; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 70 | `task_group_create` | `factory: handle`, `resource_domain: handle` | `group: handle` | `factory: Borrow, kind=task_factory, rights=0x400000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `group: produce, kind=task_group, fixed=0x400080b` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 71 | `virtual_cpu_start` | `virtual_cpu: handle` | — | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 72 | `pending_virtual_machine_set_virtual_serial` | `pending_virtual_machine: handle`, `virtual_serial: handle` | — | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `virtual_serial: ConsumeOnCommit, kind=virtual_serial, rights=0x8002` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 73 | `clock_get_monotonic` | — | `nanoseconds: u64` | — | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Abi` |
| 74 | `virtual_serial_create` | — | `virtual_serial: handle` | `virtual_serial: produce, kind=virtual_serial, fixed=0x803f` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 75 | `virtual_serial_register_output` | `virtual_serial: handle`, `buffer: handle` | — | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20`, `buffer: Borrow, kind=vmo, rights=0x70` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 76 | `virtual_serial_write` | `virtual_serial: handle`, `bytes: user_address`, `byte_count: byte_count` | `actual_bytes: byte_count` | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 77 | `thread_create` | `entry: u64`, `stack: u64`, `tls: u64`, `argument: u64` | `thread: handle` | `thread: produce, kind=thread, fixed=0xc0d` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 78 | `thread_start` | `thread: handle` | — | `thread: Borrow, kind=thread, rights=0x400` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 79 | `thread_request_stop` | `thread: handle` | — | `thread: Borrow, kind=thread, rights=0x800` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 80 | `atomic_wait` | `address: u64`, `expected: u32`, `deadline: u64` | — | — | — | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Task` |
| 81 | `atomic_wake` | `address: u64`, `count: u32` | `woken: u64` | — | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 82 | `thread_sleep` | `deadline: u64` | — | — | — | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Task` |
| 83 | `file_write_at` | `file: handle`, `options: u32`, `offset: u64`, `input: user_address`, `input_size: byte_count` | `actual_bytes: byte_count`, `end_offset: u64` | `file: Borrow, kind=file, rights=0x20` | `input: Read, len=input_size bytes, max-bytes=65536; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 84 | `file_resize` | `file: handle`, `size: byte_count` | — | `file: Borrow, kind=file, rights=0x20` | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 85 | `directory_create_file` | `directory: handle`, `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `mode: u32` | `file: handle` | `directory: Borrow, kind=directory, rights=0x30`, `file: produce, kind=file, exact-from(requested_rights), allowed=0xbb, required-from=directory` | `path: Read, len=path_size bytes, max-bytes=4096; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 86 | `directory_create_directory` | `directory: handle`, `path: user_address`, `path_size: byte_count`, `mode: u32` | — | `directory: Borrow, kind=directory, rights=0x30` | `path: Read, len=path_size bytes, max-bytes=4096; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 87 | `directory_remove` | `directory: handle`, `path: user_address`, `path_size: byte_count`, `options: u32` | — | `directory: Borrow, kind=directory, rights=0x30` | `path: Read, len=path_size bytes, max-bytes=4096; order=0` | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 88 | `wait_set_create` | `capacity: element_count` | `wait_set: handle` | `wait_set: produce, kind=wait_set, fixed=0x4000000d` | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 89 | `wait_set_add` | `wait_set: handle`, `object: handle`, `signals: u64` | `registration: u64` | `wait_set: Borrow, kind=wait_set, rights=0x40000000`, `object: Borrow, any, rights=0x4` | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 90 | `wait_set_rearm` | `wait_set: handle`, `registration: u64` | — | `wait_set: Borrow, kind=wait_set, rights=0x40000000` | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 91 | `wait_set_remove` | `wait_set: handle`, `registration: u64` | — | `wait_set: Borrow, kind=wait_set, rights=0x40000000` | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 92 | `wait_set_wait` | `wait_set: handle`, `deadline: u64`, `output: user_address`, `output_size: byte_count` | — | `wait_set: Borrow, kind=wait_set, rights=0x4` | `output: Write, len=output_size bytes, max-bytes=24, record=wait_set_event; order=0` | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 93 | `process_get_current_id` | — | `koid: u64` | — | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Task` |
| 94 | `virtual_serial_acknowledge_output` | `virtual_serial: handle`, `consumed: u64` | — | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |

## Public records

| Name | Minimum prefix | Size | Alignment | Fields |
| --- | ---: | ---: | ---: | --- |
| `wait_set_event` | 24 | 24 | 8 | `registration: u64 @ 0`, `signals: u64 @ 8`, `sequence: u64 @ 16` |
| `handle_info` | 16 | 16 | 8 | `object_kind: u32 @ 0`, `flags: u32 @ 4`, `rights: u64 @ 8` |
| `object_basic_info` | 16 | 16 | 8 | `koid: u64 @ 0`, `object_kind: u32 @ 8`, `reserved: u32 @ 12` |
| `object_wait_item` | 16 | 16 | 8 | `handle: u64 @ 0`, `signals: u64 @ 8` |
| `process_info` | 32 | 32 | 8 | `phase: u32 @ 0`, `terminal_reason: u32 @ 4`, `detail0: u64 @ 8`, `detail1: u64 @ 16`, `reserved: u64 @ 24` |
| `capability_disposition` | 24 | 24 | 8 | `handle: u64 @ 0`, `rights: u64 @ 8`, `expected_kind: u32 @ 16`, `operation: u32 @ 20` |
| `capability_receive_slot` | 24 | 24 | 8 | `handle: u64 @ 0`, `rights: u64 @ 8`, `expected_kind: u32 @ 16`, `flags: u32 @ 20` |
| `startup_handle` | 16 | 16 | 8 | `purpose: u32 @ 0`, `flags: u32 @ 4`, `handle: u64 @ 8` |
| `task_process` | 96 | 96 | 8 | `koid: u64 @ 0`, `phase: u32 @ 8`, `terminal_reason: u32 @ 12`, `pending_threads: u32 @ 16`, `active_threads: u32 @ 20`, `name_length: u32 @ 24`, `reserved: u32 @ 28`, `name: bytes[64] @ 32` |
| `task_thread` | 104 | 104 | 8 | `koid: u64 @ 0`, `process_koid: u64 @ 8`, `role: u32 @ 16`, `registry_phase: u32 @ 20`, `name_length: u32 @ 24`, `reserved: u32 @ 28`, `runtime_ticks: u64 @ 32`, `name: bytes[64] @ 40` |
| `memory_observation` | 128 | 128 | 8 | `captured_at_ns: u64 @ 0`, `page_size: u64 @ 8`, `total_bytes: u64 @ 16`, `reserved_bytes: u64 @ 24`, `managed_bytes: u64 @ 32`, `free_bytes: u64 @ 40`, `used_bytes: u64 @ 48`, `kernel_bytes: u64 @ 56`, `heap_bytes: u64 @ 64`, `page_table_bytes: u64 @ 72`, `user_bytes: u64 @ 80`, `guest_bytes: u64 @ 88`, `unattributed_bytes: u64 @ 96`, `reclaimable_bytes: u64 @ 104`, `cache_sample_complete: u64 @ 112`, `buffered_bytes: u64 @ 120` |
| `cpu_observation` | 64 | 64 | 8 | `captured_at_ns: u64 @ 0`, `ticks_per_second: u64 @ 8`, `online_cpus: u64 @ 16`, `idle_ticks: u64 @ 24`, `kernel_thread_ticks: u64 @ 32`, `user_thread_ticks: u64 @ 40`, `vcpu_ticks: u64 @ 48`, `reserved: u64 @ 56` |
| `object_inspection` | 104 | 104 | 8 | `koid: u64 @ 0`, `object_kind: u32 @ 8`, `handle_state: u32 @ 12`, `active_handles: u64 @ 16`, `supported_rights: u64 @ 24`, `strong_references: u64 @ 32`, `kernel_service_references: u64 @ 40`, `scheduler_references: u64 @ 48`, `operation_references: u64 @ 56`, `user_authority_references: u64 @ 64`, `publication_references: u64 @ 72`, `diagnostic_references: u64 @ 80`, `retirement_references: u64 @ 88`, `vm_device_binding_references: u64 @ 96` |
| `handle_inspection` | 40 | 40 | 8 | `process_koid: u64 @ 0`, `handle: u64 @ 8`, `object_koid: u64 @ 16`, `rights: u64 @ 24`, `object_kind: u32 @ 32`, `flags: u32 @ 36` |
| `directory_entry` | 280 | 280 | 8 | `size: u64 @ 0`, `mode: u32 @ 8`, `kind: u32 @ 12`, `name_length: u32 @ 16`, `reserved: u32 @ 20`, `name: bytes[256] @ 24` |
| `file_info` | 40 | 40 | 8 | `filesystem_id: u64 @ 0`, `mount_id: u64 @ 8`, `node_id: u64 @ 16`, `size: u64 @ 24`, `mode: u32 @ 32`, `reserved: u32 @ 36` |
| `directory_info` | 32 | 32 | 8 | `filesystem_id: u64 @ 0`, `mount_id: u64 @ 8`, `node_id: u64 @ 16`, `mode: u32 @ 24`, `reserved: u32 @ 28` |
| `virtual_machine_configuration` | 32 | 32 | 8 | `guest_physical_base: u64 @ 0`, `memory_size: u64 @ 8`, `vcpu_count: u32 @ 16`, `architecture: u32 @ 20`, `platform_profile: u32 @ 24`, `flags: u32 @ 28` |
| `virtual_cpu_bootstrap` | 64 | 64 | 8 | `entry: u64 @ 0`, `stack: u64 @ 8`, `argument0: u64 @ 16`, `argument1: u64 @ 24`, `argument2: u64 @ 32`, `argument3: u64 @ 40`, `vcpu_id: u32 @ 48`, `flags: u32 @ 52`, `reserved: u64 @ 56` |
| `virtual_machine_info` | 32 | 32 | 8 | `phase: u32 @ 0`, `vcpu_count: u32 @ 4`, `guest_physical_base: u64 @ 8`, `memory_size: u64 @ 16`, `architecture: u32 @ 24`, `platform_profile: u32 @ 28` |
| `virtual_cpu_info` | 24 | 24 | 8 | `vcpu_id: u32 @ 0`, `phase: u32 @ 4`, `scheduler_thread_id: u64 @ 8`, `terminal_reason: u32 @ 16`, `reserved: u32 @ 20` |
| `resource_limits` | 160 | 160 | 8 | `kernel_memory_bytes: u64 @ 0`, `processes: u64 @ 8`, `threads: u64 @ 16`, `handles: u64 @ 24`, `kernel_objects: u64 @ 32`, `committed_pages: u64 @ 40`, `pinned_pages: u64 @ 48`, `guest_pages: u64 @ 56`, `ipc_messages: u64 @ 64`, `ipc_bytes: u64 @ 72`, `ipc_handles: u64 @ 80`, `subscriptions: u64 @ 88`, `timers: u64 @ 96`, `virtual_machines: u64 @ 104`, `virtual_cpus: u64 @ 112`, `device_leases: u64 @ 120`, `dma_mappings: u64 @ 128`, `user_address_spaces: u64 @ 136`, `user_mappings: u64 @ 144`, `reserved: u64 @ 152` |
