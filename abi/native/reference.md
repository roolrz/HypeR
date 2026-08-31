<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Native ABI reference

This file is generated from `abi/native/schema.rs`. Do not edit it directly.

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

## Object signals

| Object | Bit | Name |
| --- | ---: | --- |
| `event` | 0 | `signaled` |

## Constants

| Name | Value |
| --- | ---: |
| `deadline_infinite` | `18446744073709551615` |

## Syscalls

| Number | Name | Arguments | Results | Capability effects | User memory | Execution | Audit |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 0 | `abi_query` | — | `revision: u64`, `features: u64` | — | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Abi` |
| 1 | `handle_close` | `handle: handle` | — | `handle: ConsumeOnCommit, any, rights=0x0` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 2 | `handle_duplicate` | `source: handle`, `requested_rights: rights` | `handle: handle` | `source: Borrow, any, rights=0x1`, `handle: produce, same-as(source), subset-from(requested_rights)` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 3 | `handle_replace` | `source: handle`, `requested_rights: rights` | `handle: handle` | `source: ConsumeOnCommit, any, rights=0x0`, `handle: produce, same-as(source), subset-from(requested_rights)` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 4 | `handle_get_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | — | `handle: Borrow, any, rights=0x0` | `output: Write, len=output_size, max=16, record=handle_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 5 | `object_get_basic_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | — | `handle: Borrow, any, rights=0x8` | `output: Write, len=output_size, max=16, record=object_basic_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |
| 6 | `thread_yield` | — | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` | `Task` |
| 7 | `thread_exit` | `status: i64` | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` | `Task` |
| 8 | `process_exit` | `status: i64` | — | — | — | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` | `Task` |
| 9 | `event_create` | `options: u32` | `handle: handle` | `handle: produce, kind=event, fixed=0x8000f` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 10 | `event_signal` | `event: handle`, `clear_mask: u64`, `set_mask: u64` | — | `event: Borrow, kind=event, rights=0x80000` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` | `Object` |
| 11 | `object_wait_one` | `object: handle`, `signals: u64`, `deadline: u64` | `observed: u64` | `object: Borrow, any, rights=0x4` | — | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` | `Object` |

## Public records

| Name | Size | Alignment | Fields |
| --- | ---: | ---: | --- |
| `handle_info` | 16 | 8 | `object_kind: u32 @ 0`, `flags: u32 @ 4`, `rights: u64 @ 8` |
| `object_basic_info` | 16 | 8 | `koid: u64 @ 0`, `object_kind: u32 @ 8`, `reserved: u32 @ 12` |
