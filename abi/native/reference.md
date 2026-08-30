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

## Syscalls

| Number | Name | Arguments | Results | Capability effects | User memory | Execution | Audit |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 0 | `abi_query` | — | `revision: u64`, `features: u64` | — | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Abi` |
| 1 | `handle_close` | `handle: handle` | — | `handle: ConsumeOnCommit, any, rights=0x0` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 2 | `handle_duplicate` | `source: handle`, `requested_rights: rights` | `handle: handle` | `source: Borrow, any, rights=0x1`, `handle: produce, same-as(source), subset-from(requested_rights)` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 3 | `handle_replace` | `source: handle`, `requested_rights: rights` | `handle: handle` | `source: ConsumeOnCommit, any, rights=0x0`, `handle: produce, same-as(source), subset-from(requested_rights)` | — | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 4 | `handle_get_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | — | `handle: Borrow, any, rights=0x0` | `output: Write, len=output_size, max=16, record=handle_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Capability` |
| 5 | `object_get_basic_info` | `handle: handle`, `output: user_address`, `output_size: byte_count` | — | `handle: Borrow, any, rights=0x8` | `output: Write, len=output_size, max=16, record=object_basic_info; order=0` | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` | `Object` |

## Public records

| Name | Size | Alignment | Fields |
| --- | ---: | ---: | --- |
| `handle_info` | 16 | 8 | `object_kind: u32 @ 0`, `flags: u32 @ 4`, `rights: u64 @ 8` |
| `object_basic_info` | 16 | 8 | `koid: u64 @ 0`, `object_kind: u32 @ 8`, `reserved: u32 @ 12` |
