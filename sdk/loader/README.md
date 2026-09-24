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

After relocating the initial dependency graph, the loader resolves
`hyper_runtime_start` directly in `libhyper.so` and passes the original startup
stack and a non-returning continuation. The shared runtime initializes the heap,
creates the final main stack, copies startup data, switches SP and releases the
kernel bootstrap stack. The continuation then runs constructors and enters the
application on the final stack. The interpreter never restores its abandoned
bootstrap frames, and its statically linked runtime primitives do not own a
second heap. Dynamic CRT uses the initialized runtime; static CRT performs the
same handoff itself. Later `dlopen` constructors use the existing heap.

## Architecture contracts

AArch64 supports RELATIVE, ABS64, GLOB_DAT and JUMP_SLOT relocations. RV64
requires LP64D ELF flags and supports RELATIVE, 64 and JUMP_SLOT; its JUMP_SLOT
calculation ignores the addend as required by the RISC-V psABI. COPY, TLS and
resolver relocations are rejected. Both backends share loading and lifetime
policy; only entry assembly and machine relocation rules differ.

## Host verification

Run `sh sdk/toolchain/scripts/check-loader-arch.sh` from the repository root.
The architecture probe checks machine flags and relocation formulas. A second
probe compiles the production `rtld.c` directly, supplies bounded in-memory ELF
views and records Native syscall effects. It covers malformed dynamic metadata,
file bounds, symbol indices, writable relocation targets, RELR cursor/work
limits, mapping failure cleanup and restoration of a failed load transaction.
The transaction retains pre-existing references and visibility while discarding
new dependency mappings in reverse order. It does not promise to undo arbitrary
constructor side effects.

Both ELF architectures run under UBSan; Linux CI also enables ASan. These tests
do not emulate page tables or prove the executable mapping/constructor path:
the Native dynamic-loading QEMU acceptance tests remain required.
