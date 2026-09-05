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

- AArch64 Native syscall entry using the published machine convention;
- Native startup-stack parsing, CRT entry, and bootstrap-handle discovery;
- capability-scoped console I/O, object wait, and core lifecycle wrappers;
- freestanding `memcpy`, `memmove`, `memset`, `memcmp`, and `strlen`;
- Clang-only cross compilation into `libhyper.a`; and
- a public-interface-only Native application fixture for product integration.

Native userspace is currently implemented only on AArch64. Additional
architecture veneers will be added only when the corresponding Kernel entry is
functional.

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
