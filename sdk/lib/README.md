<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Lib

HypeR Lib is the freestanding C foundation for applications compiled directly
for the HypeR Native ABI. It provides low-level syscall veneers and the small
set of C primitives required before a complete native runtime exists.

This component is deliberately not a Linux, FreeBSD, or POSIX compatibility
layer. Foreign binaries retain their original ABI and will run through separate
whole-personality supervisors. A future POSIX source runtime may build on HypeR
Native services, but its contracts do not belong in HypeR Lib.

## Current scope

- AArch64 and RV64 Native syscall entry using the published machine conventions;
- Native startup-stack parsing, CRT entry, and bootstrap-handle discovery;
- capability-scoped console and filesystem I/O, object wait, byte and capability
  channels, VMO/VMAR operations, staged Process construction, and core
  lifecycle wrappers;
- a shared process heap with `malloc`, `calloc`, `realloc`, `free`, and `aligned_alloc`;
- freestanding `memcpy`, `memmove`, `memset`, `memcmp`, and `strlen`;
- Clang-only cross compilation into `libhyper.a` and `libhyper.so`; and
- a public-interface-only Native application fixture for product integration.

Native userspace supports AArch64 and RV64GC/LP64D. Additional architecture
veneers are added only when the corresponding Kernel entry is
functional.

## Process heap

The loader initializes the shared runtime after relocation and before any
application or DSO constructors; CRT performs the same idempotent initialization
before calling `hyper_main` (including static applications), using the process's
bootstrap ROOT_VMAR capability. It reserves `[0xe0000000, 0xf0000000)` for the
heap, after the loader's shared-library range and below the user stack. This
is a Native address-layout contract; applications must not destroy or replace
that reservation. Initialization borrows ROOT_VMAR and retains its own child
VMAR, so normal startup-handle ownership remains with the application.

Reservation allocates no backing pages. Allocations map read/write, non-executable
VMOs in regions of at least 64 KiB, bounded by the 256 MiB virtual reservation
and the process's resource budget. First-fit blocks are split and neighboring
free blocks coalesced. Fully free regions are unmapped and their backing pages
released; partially occupied regions remain available for reuse. All metadata
is serialized by a process-local atomic lock, yielding while contended.
This is a general-purpose initial allocator, not a constant-time or real-time
allocation contract. It is not reentrant from asynchronous handlers.

`<stdlib.h>` exposes the C allocation subset. Failure returns NULL without
an errno facility; `calloc` detects multiplication overflow; `malloc(0)` returns
a freeable minimum allocation when memory is available; `free(NULL)` is a no-op;
`realloc(p, 0)` frees and returns NULL. Failed nonzero realloc preserves the old
allocation. `aligned_alloc` requires a power-of-two alignment and a size divisible
by it. `<hyper/heap.h>` exposes the arbitrary-size aligned allocation interface
used by Rust. Each pointer must be released through the same process heap.
Dynamic applications and their DSOs share the allocator in `libhyper.so`;
static applications use the same implementation from `libhyper.a`.

## Build

HypeR Lib consumes the generated C header from `sdk/abi/`. The include
directory is injected explicitly so the runtime does not carry a second ABI
definition.

```sh
cmake -S sdk/lib -B target/sdk-lib/aarch64 \
  -DCMAKE_C_COMPILER=clang \
  -DCMAKE_ASM_COMPILER=clang \
  -DCMAKE_AR=llvm-ar \
  -DCMAKE_RANLIB=llvm-ranlib \
  -DCMAKE_SYSTEM_NAME=Generic \
  -DCMAKE_SYSTEM_PROCESSOR=aarch64 \
  -DCMAKE_C_COMPILER_TARGET=aarch64-none-elf \
  -DCMAKE_ASM_COMPILER_TARGET=aarch64-none-elf \
  -DHYPER_LD=ld.lld \
  -DHYPER_ABI_INCLUDE_DIR="$PWD/sdk/abi/include"
cmake --build target/sdk-lib/aarch64
```

`sdk/toolchain/` owns SDK assembly, compiler-driver defaults, and the final
executable link contract. This component owns library semantics only.

## Testing

Library tests under `tests/unit` are host-executed unit tests for implementation
semantics. They can be run independently of the target archive:

```sh
cmake -S sdk/lib/tests/unit -B target/sdk-lib/unit \
  -DCMAKE_C_COMPILER=clang \
  -DHYPER_ABI_INCLUDE_DIR="$PWD/sdk/abi/include"
cmake --build target/sdk-lib/unit
ctest --test-dir target/sdk-lib/unit --output-on-failure
```

`test-app` is not part of the Lib unit-test suite. The top-level SDK check
compiles it against the assembled Native SDK to validate the public headers,
runtime, and compiler-driver integration.

## License

Licensed under the Apache License, Version 2.0. See
[the project license](../../LICENSE).

## Rust standard library bridge

The SDK also installs `libhyper-std.a` and `crt-std.o` for ordinary Rust
applications. Shared startup, stream buffering, and key-based thread-local
storage live in `libhyper`; the std archive carries stateless adapters.
See the [std runtime contract](../toolchain/rust-std/README.md) and the
[`hyper/std.h`](include/hyper/std.h) / [`hyper/thread.h`](include/hyper/thread.h)
interfaces. These are SDK interfaces, not additions to the kernel syscall ABI.
