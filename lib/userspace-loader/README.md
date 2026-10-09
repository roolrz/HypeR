<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Userspace executable loader

`lib/userspace-loader` builds `/lib64/userspace-loader-hyper-aarch64` and
`/lib64/userspace-loader-hyper-riscv64`. Every Native process, including `/init`,
starts here. This program maps the executable and its `PT_INTERP`, then builds
an LP64 startup stack. It has no allocator, dependencies, TLS, symbol lookup,
relocation, constructors or service process. Dynamic linking remains in
[`lib/dynamic-loader`](../dynamic-loader/README.md).

## Kernel entry contract

The kernel admits this installed bootstrap ELF once, retains its immutable
snapshot, and reuses its read-only pages across process starts. The accepted
subset is branded little-endian ELF64 `ET_DYN`, matching the host architecture,
with at most eight nonoverlapping `PT_LOAD` segments and a 1 MiB image span.
Their page-rounded mappings must cover one contiguous range so the runtime
can retire the bootstrap with one unmap operation.
It rejects interpreters, dynamic tables, TLS and executable stacks. It does not
read the symbol table or apply relocations. Linker assertions require no runtime
relocations and no writable global state in the shipped bootstrap program.

The initial thread receives an empty guarded stack and a ByteChannel handle in
its first argument register (`x0` or `a0`). One queued message contains the
fixed `loader_startup` ABI record, `handle_count` tagged handle records, then
`data_size` opaque bytes. The fixed header supplies:

- immutable executable VMO and exact file length;
- root and temporary-stack VMAR handles;
- temporary-stack and bootstrap-image mapping bounds;
- a bootstrap-only read/execute Directory scoped to `/lib64`;
- whether the caller expects a startup result.

All handle values belong to the child and are installed before the initial
thread becomes runnable. The parent cannot acquire the child's VMAR authority
through this operation. Normal `ProcessBuilder` starts atomically consume the
builder and return a supervisor Process plus a READ/WAIT result endpoint.
A failed syscall preserves the sealed builder. A later loader failure is a
committed child failure, not a retryable builder operation.

The kernel limits and copies opaque startup data but does not interpret argv,
environment, auxv or application ELF metadata. Capability transfers keep their
existing prepare/commit and accounting rules. The kernel remains responsible
for W^X, executable provenance, COW, cache/TLB coherence and process retirement.

## SDK policy

The SDK encodes an argument/environment count pair, little-endian string offsets
relative to that pair, and NUL-terminated strings. Counts are bounded at 64 each,
strings at 4096 bytes, and the complete payload at the kernel's 16 KiB bound.
An empty initial payload selects `/init` as argv[0]. Runtime-private auxv tags
are declared in `hyper/launch.h`; they are not part of the kernel entry contract.

The loader validates the full segment plan before mapping. It uses private COW
views of the immutable executable VMO and rejects page overlap, W+X, malformed
ranges, nonempty ELF TLS and executable stacks. Applications start at or above 4 MiB;
`PT_INTERP` is a direct file under `/lib64`, mapped at 256 MiB. Shared libraries
remain in the dynamic loader's range starting at 512 MiB. Neither interpreter
lookup nor dependency lookup falls back to an ambient filesystem namespace.

The bootstrap-only directory and executable VMO handles close before transfer
to the interpreter or static CRT. The interpreter self-relocates before using
its initialized state, then loads and relocates dependencies. Static CRT
self-relocates instead. Both enter the runtime's common final-stack handoff.
After copying startup data and switching stacks, the runtime releases the
bootstrap image and stack, acknowledges successful startup, and invokes
constructors/application entry. The SDK waits for that acknowledgement; on
failure it requests child stop and waits for termination. A queued success
wins over subsequent peer closure, so very short-lived programs still spawn
successfully.

Readiness confirms mapping, relocation and runtime setup, not successful
constructors or `main`. Those can still fail after the parent receives success.
Init has no waiting parent: its launch flags omit the reply request, and the
userspace loader closes that channel after reading the bootstrap message.

## Interpreter handoff

Both loaders execute on the initial thread in the application's address space.
There is no loader service process or IPC round trip between them. If the main
image has `PT_INTERP`, this loader maps the interpreter and branches to its ELF
entry with SP pointing at the constructed startup vector. Without `PT_INTERP`,
it branches directly to the application's CRT entry. Neither transfer returns.

The vector uses the LP64 System V layout: `argc`, `argv`, a null pointer, `envp`,
a null pointer, and terminated auxv pairs. Key entries are:

| Entry | Meaning |
| --- | --- |
| `AT_PHDR`, `AT_PHENT`, `AT_PHNUM` | Main application's mapped program headers |
| `AT_ENTRY` | Main application's entry, even when execution first enters ld |
| `AT_BASE` | Interpreter load bias, or zero for a static application |
| `AT_PAGESZ` | Current Native ELF mapping granule, 4096 bytes |
| `HYPER_AUXV_*` | SDK handles, temporary mapping bounds, main-stack request and startup result channel |

The dynamic loader uses those headers to discover dependencies, performs eager
relocation, and calls the shared runtime for the final-stack handoff. Its
continuation runs constructors and branches to `AT_ENTRY`. Static CRT performs
its own relative relocation and calls the same runtime implementation linked
into the application. The userspace loader performs neither kind of relocation.
Auxv is an SDK protocol, not the kernel's process-entry contract.

## Address layout

The current layout below applies to both supported Native architectures. All
addresses are process virtual addresses; a window is not a fully backed mapping.

| Region | Current placement | Owner |
| --- | --- | --- |
| Null page | Unmapped; outside ROOT_VMAR | Kernel |
| Userspace loader | Starts at `0x00100000`, admitted span at most 1 MiB | Kernel bootstrap |
| Temporary stack | `[0x003df000, 0x003ff000)`, 128 KiB, with a 4 KiB unmapped guard at each end | Kernel bootstrap |
| Application ELF | `[0x00400000, 0x10000000)` window | Userspace loader |
| Interpreter ELF | `[0x10000000, 0x20000000)` window | Userspace loader |
| Shared libraries | `[0x20000000, 0xe0000000)` window | Dynamic loader |
| Heap | Low-end hint `0xe0000000`; initial VA reservation is one eighth of the application address limit | libhyper |
| Final main and worker stacks | Prefer the top of the application range; independent guarded VMARs | libhyper |

For `ET_DYN`, the lowest mapped page starts at the beginning of the selected
application/interpreter window; `ET_EXEC` retains its linked addresses within
the application window. Each main/interpreter image is limited to a 64 MiB span.
The normal SDK produces PIE applications; dynamic main images must expose the
program-header mapping used by the interpreter through `PT_PHDR`.

The kernel delivers actual bootstrap bounds in `loader_startup`; the SDK does
not recover them from an assumed SP. Its current entry assembly requires at
least 128 KiB of temporary stack, and its application windows assume the
bootstrap fits below 4 MiB. Moving bootstrap mappings outside that window or
reducing the temporary stack therefore requires coordinated kernel/loader
changes. These are current layout constraints, not a promise of arbitrary
bootstrap placement. Userspace address randomization is not implemented.

The kernel grants ROOT_VMAR from one page to the architecture's application
limit. The runtime queries that limit instead of assuming identical address
widths. Final stack size follows the main image's `PT_GNU_STACK` request,
defaulting to 256 KiB, with enough room for the entry vector and handoff frames.
Stacks reserve at least 256 MiB of VA but initially back only their usable
extent. Heap backing is likewise allocated as needed; a large reservation does
not consume the same amount of RAM. VMAR overlap and authority checks remain
kernel responsibilities. See [heap](../hyper/README.md#process-heap) and
[stack lifetime and explicit growth](../hyper/README.md#guarded-growable-stacks).

## Validation

`sh sdk/toolchain/scripts/check-userspace-loader.sh` exercises the production
ELF parser, stack handoff and self-relocator with syscall substitutes, for both
architectures. Kernel host tests cover the smaller bootstrap admission subset.
QEMU acceptance exercises the installed SDK, init, static/dynamic applications,
and supervisor lifecycle. Startup latency measurements are local manual tests,
not CI performance thresholds.
