<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Getting started

[Project overview](../README.md)

Commands in this guide run from the repository root.

## Prerequisites

- Rust and rustup; `rust-toolchain.toml` selects the pinned compiler and
  components;
- Clang/LLVM;
- GNU Make;
- Python 3.11+ for SDK source preparation and workspace contract checks;
- QEMU for the selected architecture;
- `curl`, `cpio`, `gzip`, `tar`, and SHA-256 tooling for the Linux guest assets;
- `dtc` when building the x86-64 QEMU platform description.

Build the AArch64 kernel, Native SDK, and initramfs, then run the complete
system:

```sh
make defconfig
make run
```

`make run` builds the `no_std` Rust init, direction-attenuated Console workers,
session manager, capability-scoped shell, VM manager, and isolated VM runtime
only through the assembled SDK under `target/sdk/aarch64`. It also downloads
the checksum-pinned AArch64 Linux inputs, packages the guest FIT, and places it
in the Native initramfs for userspace-managed boot. The applications do not
include private kernel or SDK source paths. Native applications are dynamic
PIEs by default and share the capability-loaded `libhyper.so` runtime through
the in-tree AArch64 ELF interpreter. SDK consumers can select a self-contained
static PIE backed by the matching `libhyper.a` with `HYPER_LINK_MODE=static`.
Pass `INITRAMFS=/path/to/archive.cpio` to test another Native userspace image.

The separate Kernel self-test guest path remains available as an integration
test:

```sh
make test-qemu ARCH=aarch64
```

This downloads checksum-pinned Alpine Linux inputs, constructs a versioned VM
bundle, builds the kernel with `kernel-self-test`, and starts a four-CPU QEMU
`virt` machine. Guest downloads are cached under the platform temporary
directory and generated payloads remain under `kernel/target/guest/`.

Select another architecture explicitly:

```sh
make defconfig ARCH=riscv64
make test-qemu ARCH=riscv64

make defconfig ARCH=x86_64
make image ARCH=x86_64
make test-image ARCH=x86_64
```

The default x86-64 QEMU configuration uses TCG, which cannot execute VMX.
Hardware-assisted guest execution requires a suitable KVM/nested virtualization
environment and is not yet a public CI contract.

Useful targets:

| Command | Purpose |
| --- | --- |
| `make image ARCH=<arch>` | Build the canonical ELF and delivery image |
| `make sdk` | Assemble the AArch64 Native SDK under `target/sdk/aarch64` |
| `make sdk-check` / `make sdk-test` | Verify SDK generation, publication, compilation, and portable runtime behavior |
| `make native-initramfs` | Build Native `/init` through the SDK and package it as deterministic `newc` |
| `make run ARCH=aarch64` | Build and start the complete Native system |
| `make test-native` | Verify the Native service graph, command execution, and managed Linux VM console attach/detach under QEMU |
| `make guest-assets ARCH=<arch>` | Download and package the pinned Linux guest inputs |
| `make check ARCH=<arch>` | Run target checks and Clippy, including kernel self-test builds |
| `make test ARCH=<arch>` | Run kernel host, Kconfig, and kallsyms tests |
| `make test-image ARCH=<arch>` | Verify the ELF, relocation, image, and architecture contract |
| `make test-qemu ARCH=<arch>` | Run the architecture's QEMU acceptance test where supported |
| `make verify ARCH=<arch>` | Run the complete local contract for the selected architecture |
| `make verify-all` | Verify the AArch64 kernel, SDK, and Native system together |

The default AArch64 image is written to:

```text
kernel/target/aarch64-unknown-none/kernel/hyper
kernel/target/aarch64-unknown-none/kernel/hyper.img
```

`make release` strips debugger-only sections from the canonical ELF without
recompiling it, then verifies that the resulting raw image is byte-identical.

`make native-initramfs` copies its payloads into a temporary staging directory
and runs `llvm-strip --strip-debug` on the ELF copies before packaging. Packaged
applications and libraries retain their symbol tables; original application and
SDK build products retain all debug information. Set `LLVM_STRIP` to override
the tool path.

## Configuration

HypeR uses an in-tree, dependency-free Kconfig-like tool. It reads
`kernel/Kconfig`, writes `kernel/.config`, validates dependencies and ranges,
and exports declared symbols as checked Rust `cfg` values and typed constants.

```sh
make config       # interactive configuration
make olddefconfig # accept defaults for newly introduced symbols
make defconfig    # restore the selected QEMU architecture defaults
```

`CONFIG_FILE` selects an alternate complete configuration without replacing a
developer's `.config`:

```sh
make image ARCH=aarch64 CONFIG_FILE=kernel/configs/qemu_aarch64_defconfig
```
