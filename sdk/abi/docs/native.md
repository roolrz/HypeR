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

## Object signals

| Object | Bit | Name |
| --- | ---: | --- |
| `event` | 0 | `signaled` |
| `channel` | 0 | `readable` |
| `channel` | 1 | `writable` |
| `channel` | 2 | `peer_closed` |
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
| `deadline_infinite` | `18446744073709551615` |
| `channel_disposition_same_rights` | `18446744073709551615` |
| `channel_max_message_bytes` | `65536` |
| `channel_max_message_handles` | `64` |
| `channel_max_queued_messages` | `16` |
| `channel_max_queued_bytes` | `1048576` |
| `channel_max_queued_handles` | `1024` |
| `console_max_transfer_bytes` | `4096` |

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
| 12 | `channel_create` | `options: u32` | `endpoint0: handle`, `endpoint1: handle` | `endpoint0: produce, kind=channel, fixed=0x3e`, `endpoint1: produce, kind=channel, fixed=0x3e` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 13 | `channel_write` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count`, `dispositions: user_address`, `disposition_count: element_count` | — | `endpoint: Borrow, kind=channel, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0`, `dispositions: Read, len=disposition_count elements, max-elements=64, element-size=24, record=channel_disposition, consume-handles=(handle, rights, expected_kind), required-rights=0x2; order=1` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 14 | `channel_read` | `endpoint: handle`, `options: u32`, `bytes: user_address`, `byte_capacity: byte_count`, `handles: user_address`, `handle_capacity: element_count` | `actual_bytes: byte_count; also-on=buffer_too_small`, `actual_handles: element_count; also-on=buffer_too_small` | `endpoint: Borrow, kind=channel, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=65536; order=0`, `handles: Write, len=handle_capacity elements, max-elements=64, element-size=8, produce-transferred-handles; order=1` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 15 | `console_read` | `console: handle`, `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` | `actual_bytes: byte_count; also-on=would_block` | `console: Borrow, kind=console, rights=0x10` | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |
| 16 | `console_write` | `console: handle`, `options: u32`, `bytes: user_address`, `byte_count: byte_count` | `actual_bytes: byte_count; also-on=would_block` | `console: Borrow, kind=console, rights=0x20` | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Capability` |

## Public records

| Name | Size | Alignment | Fields |
| --- | ---: | ---: | --- |
| `handle_info` | 16 | 8 | `object_kind: u32 @ 0`, `flags: u32 @ 4`, `rights: u64 @ 8` |
| `object_basic_info` | 16 | 8 | `koid: u64 @ 0`, `object_kind: u32 @ 8`, `reserved: u32 @ 12` |
| `channel_disposition` | 24 | 8 | `handle: u64 @ 0`, `rights: u64 @ 8`, `expected_kind: u32 @ 16`, `reserved: u32 @ 20` |
| `startup_handle` | 16 | 8 | `purpose: u32 @ 0`, `flags: u32 @ 4`, `handle: u64 @ 8` |
