<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native SDK contract

The HypeR Native SDK is the supported build boundary between this source tree
and Native EL0 applications. Kernel, ABI, runtime, toolchain, and application
changes are reviewed and tested in one commit; SDK releases are derived from
that coherent commit rather than assembled from independently moving
repositories.

## Source ownership

The SDK is produced from three independently owned source components:

| Path | Responsibility |
| --- | --- |
| `sdk/abi/` | Machine-visible values, layouts, syscall metadata, and generated interfaces |
| `sdk/lib/` | Freestanding C runtime, startup code, and architecture syscall veneers |
| `sdk/toolchain/` | Clang driver, linker script, ELF branding, and transactional SDK assembly |

The kernel consumes `sdk/abi/` as a dependency-free `no_std` path dependency.
Native applications consume only the assembled SDK. They must not include
headers from `sdk/abi/` or `sdk/lib/` directly, link source-tree archives, or
depend on private kernel modules.

## Build and installed layout

Run:

```sh
make sdk
```

The default output is `target/sdk/aarch64`:

```text
bin/hyper-clang
bin/hyper-brand-elf
include/hyper/native.h
include/hyper/startup.h
include/hyper/syscall.h
include/string.h
lib/crt1.o
lib/libhyper.a
lib/hyper/aarch64/hyper-native.ld
share/hyper/manifest
```

`hyper-clang` accepts `HYPER_SYSROOT` to select another installed SDK and
`HYPER_CLANG` or `HYPER_LD` to select explicit LLVM tools. The SDK currently
targets AArch64 Native applications. `hyper-brand-elf` is a host executable,
so a published archive is identified by both its host and target platform.
The manifest records the SDK version, source revision, host, target, and Native
ABI revision. Local builds use a dirty-aware Git description; release jobs set
an explicit SDK version and source identity.

`make sdk-check` validates generated ABI output, lints the Rust ABI crate,
builds the SDK transactionally, and compiles and links a public-interface-only
application. `make sdk-test` runs ABI layout tests and portable C runtime unit
tests.

## Application integration

`make app` first assembles the SDK and then builds `app/init` with
the installed `bin/hyper-clang`. `make native-initramfs` packages that result
as a deterministic `newc` archive, and `make test-native` boots the kernel and
verifies startup, blocking console input, and echo under QEMU.

This enforced producer-consumer path is also the release boundary: a source
change which works only through an undeclared source-tree include cannot pass
the Native integration job.

## Release model

Until the first supported SDK release, the Native ABI revision remains zero
and interfaces may change with the repository. A future GitHub release will
publish host-specific SDK archives built from the release tag. Each archive
will record the repository commit, host platform, target architecture, and
LLVM compatibility range. SDK release versioning is separate from the ABI
revision; publishing a toolchain package does not by itself declare ABI
stability.

Linux, FreeBSD, POSIX, and other foreign interfaces are not part of this SDK.
They remain separately versioned compatibility personalities built above the
HypeR Native capability boundary.
