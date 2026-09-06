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
| 12 | `boot_fs` | `general` |
| 13 | `boot_file` | `general` |
| 14 | `capability_channel` | `rendezvous_only` |
| 15 | `process_builder` | `rendezvous_only` |

## Object signals

| Object | Bit | Name |
| --- | ---: | --- |
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

## Constants

| Name | Value |
| --- | ---: |
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
| `startup_handle_purpose_boot_fs` | `7` |
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
| `bootfs_max_path_bytes` | `4096` |
| `bootfs_max_read_bytes` | `65536` |
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

## Semantic rules

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
| 4 | `handle_get_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | — | `handle: Borrow, any, rights=0x0` | `output: Write, len=output_size bytes, max-bytes=16, record=handle_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 5 | `object_get_basic_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | — | `handle: Borrow, any, rights=0x8` | `output: Write, len=output_size bytes, max-bytes=16, record=object_basic_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 6 | `thread_yield` | — | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 7 | `thread_exit` | `status: i64` | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` | `Task` |
| 8 | `process_exit` | `status: i64` | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` | `Task` |
| 9 | `event_create` | `options: u32` | `handle: handle` | `handle: produce, kind=event, fixed=0x8000f` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 10 | `event_signal` | `event: handle`, `clear_mask: u64`, `set_mask: u64` | — | `event: Borrow, kind=event, rights=0x80000` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 11 | `object_wait_one` | `object: handle`, `signals: u64`, `deadline: u64` | `observed: u64` | `object: Borrow, any, rights=0x4` | — | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Object` |
| 12 | `byte_channel_create` | `options: u32` | `endpoint0: handle`, `endpoint1: handle` | `endpoint0: produce, kind=byte_channel, fixed=0x3e`, `endpoint1: produce, kind=byte_channel, fixed=0x3e` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 13 | `byte_channel_write` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count` | — | `endpoint: Borrow, kind=byte_channel, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 14 | `byte_channel_read` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` | `actual_bytes: byte_count; also-on=buffer_too_small` | `endpoint: Borrow, kind=byte_channel, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 15 | `console_read` | `console: handle`, `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` | `actual_bytes: byte_count; also-on=would_block` | `console: Borrow, kind=console, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 16 | `console_write` | `console: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count` | `actual_bytes: byte_count; also-on=would_block` | `console: Borrow, kind=console, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 17 | `bootfs_open` | `boot_fs: handle`, `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` | `file: handle` | `boot_fs: Borrow, kind=boot_fs, rights=0x10`, `file: produce, kind=boot_file, exact-from(requested_rights), allowed=0x9b` | `path: Read, len=path_size bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 18 | `boot_file_read` | `file: handle`, `options: u32`, `offset: u64`, `output: user_address`, `output_capacity: byte_count` | `actual_bytes: byte_count`, `file_size: byte_count` | `file: Borrow, kind=boot_file, rights=0x10` | `output: Write, len=output_capacity bytes, max-bytes=65536; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 19 | `capability_channel_create` | `options: u32` | `endpoint0: handle`, `endpoint1: handle` | `endpoint0: produce, kind=capability_channel, fixed=0x3e`, `endpoint1: produce, kind=capability_channel, fixed=0x3e` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 20 | `capability_channel_try_send` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count`, `dispositions: user_address`, `disposition_count: element_count` | — | `endpoint: Borrow, kind=capability_channel, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0`, `dispositions: Read, len=disposition_count elements, max-elements=16, element-size=24, record=capability_disposition, transactional-handles=(handle, rights, expected_kind, operation), common-rights=0x2, operations=[move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3], commit=AtomicOnOk; order=1` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 21 | `capability_channel_receive` | `endpoint: handle`, `deadline: u64`, `bytes: user_address`, `byte_capacity: byte_count`, `capability_slots: user_address`, `slot_count: element_count` | `actual_bytes: byte_count; also-on=buffer_too_small`, `actual_capabilities: element_count; also-on=buffer_too_small` | `endpoint: Borrow, kind=capability_channel, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0`, `capability_slots: ReadWrite, len=slot_count elements, max-elements=16, element-size=24, record=capability_receive_slot, typed-receive-slots=(handle, rights, expected_kind, flags), produce-transferred-handles, commit=AtomicOnOk; order=1` | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Capability` |
| 22 | `process_builder_create` | `factory: handle`, `group: handle`, `domain: handle`, `executable: handle` | `builder: handle` | `factory: Borrow, kind=task_factory, rights=0x100000`, `group: Borrow, kind=task_group, rights=0x4000000`, `domain: Borrow, kind=resource_domain, rights=0x8000000`, `executable: Borrow, kind=boot_file, rights=0x80`, `builder: produce, kind=process_builder, fixed=0xc2a` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
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
| 33 | `process_get_info` | `process: handle`, `info: user_address`, `info_size: byte_count` | — | `process: Borrow, kind=process, rights=0x8` | `info: Write, len=info_size bytes, max-bytes=32, record=process_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |

## Public records

| Name | Size | Alignment | Fields |
| --- | ---: | ---: | --- |
| `handle_info` | 16 | 8 | `object_kind: u32 @ 0`, `flags: u32 @ 4`, `rights: u64 @ 8` |
| `object_basic_info` | 16 | 8 | `koid: u64 @ 0`, `object_kind: u32 @ 8`, `reserved: u32 @ 12` |
| `object_wait_item` | 16 | 8 | `handle: u64 @ 0`, `signals: u64 @ 8` |
| `process_info` | 32 | 8 | `phase: u32 @ 0`, `terminal_reason: u32 @ 4`, `detail0: u64 @ 8`, `detail1: u64 @ 16`, `reserved: u64 @ 24` |
| `capability_disposition` | 24 | 8 | `handle: u64 @ 0`, `rights: u64 @ 8`, `expected_kind: u32 @ 16`, `operation: u32 @ 20` |
| `capability_receive_slot` | 24 | 8 | `handle: u64 @ 0`, `rights: u64 @ 8`, `expected_kind: u32 @ 16`, `flags: u32 @ 20` |
| `startup_handle` | 16 | 8 | `purpose: u32 @ 0`, `flags: u32 @ 4`, `handle: u64 @ 8` |
