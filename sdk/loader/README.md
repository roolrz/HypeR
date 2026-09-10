<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR runtime linker

This component provides the Native ELF interpreter installed as
`/lib/ld-hyper-aarch64.so` or `/lib/ld-hyper-riscv64.so`. The kernel maps only the main image and this
interpreter. Dependency policy, symbol lookup, relocation, constructors,
RELRO, and runtime loading remain userspace responsibilities.

Library lookup is capability-relative: the interpreter receives a Directory
handle and accepts exact relative object names, never ambient search paths.
Initial dependencies are global; `hyper_dlopen_at` additionally supports local
symbol scopes. Loading is eager and bounded, with `RELA` and `RELR`
support and no writable-executable mapping transition.

`hyper_dlclose` currently releases a logical caller reference but deliberately
does not run destructors or reclaim mappings. This conservative lifetime rule
avoids invalidating code while other threads may still execute it; unload will
require an explicit process-wide quiescence protocol.

## Runtime initialization

After relocating the initial dependency graph and before running constructors,
the loader resolves `hyper_runtime_initialize` directly in `libhyper.so` and
passes it the original startup stack. This initializes the shared process heap
before a constructor can allocate. The interpreter's statically linked runtime
primitives do not own a second heap. CRT repeats initialization idempotently
before application entry; later `dlopen` constructors use the existing heap.

## Architecture contracts

AArch64 supports RELATIVE, ABS64, GLOB_DAT and JUMP_SLOT relocations. RV64
requires LP64D ELF flags and supports RELATIVE, 64 and JUMP_SLOT; its JUMP_SLOT
calculation ignores the addend as required by the RISC-V psABI. COPY, TLS and
resolver relocations are rejected. Both backends share loading and lifetime
policy; only entry assembly and machine relocation rules differ.
