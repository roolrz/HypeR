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
- QEMU for the selected architecture; AArch64 uses `-cpu max` by default and
  requires FEAT_VHE (hardware without it is unsupported);
- `curl`, `cpio`, `gzip`, `tar`, and SHA-256 tooling for the Linux guest assets;
- dosfstools and mtools for the default board FAT image; e2fsprogs and
  squashfs-tools for the Alpine root disk;
- ORAS for digest-pinned I/O appliance downloads: the importer uses `PATH` or
  downloads a checksum-pinned ORAS 1.3.0 automatically on supported hosts.
  `IO_VM_ORAS` can select an executable; verified packages are reused offline;
- `dtc` when building the x86-64 QEMU platform description.

Build the AArch64 kernel, Native SDK, and initramfs, then run the complete
system:

```sh
make defconfig
make
make run
```

`make` builds the Rust std-based init, direction-attenuated Console workers,
session manager, capability-scoped shell, VM manager, and isolated VM runtime
only through the assembled SDK under `target/sdk/aarch64`. It also downloads
the checksum-pinned AArch64 Linux inputs and, on first build, packages the guest
FIT and root disk into the board disk. Later builds preserve that disk.
`make run` only starts existing artifacts; `make rebuild` repacks the whole disk. The applications do not
include private kernel or SDK source paths. Native applications are dynamic
PIEs by default and share the capability-loaded `libhyper.so` runtime through
the in-tree AArch64 ELF interpreter. SDK consumers can select a self-contained
static PIE backed by the matching `libhyper.a` with `HYPER_LINK_MODE=static`.
Pass `INITRAMFS=/path/to/archive.cpio` to test another Native userspace image.

The default AArch64 board profile retains the HypeR shell and starts a resident
Linux I/O VM with its assigned QEMU virtio-scsi disk. Its Native owner is
`io-runtime`, visible in `ps`. It mounts the configuration volume at `/data`
and provides disks to configured guest VMs. The persistent disk is
`target/board/qemu/disk.img`; `BOARD_IMAGE` selects another board disk.

For an idle-backend diagnostic without mounted Native storage, build with
`make RUN_PROFILE=io`, then launch with `make run RUN_PROFILE=io`. Its separate
64 MiB disk is created once during the build. Set `IO_VM_DISK=/path/to/disk.img`
on both commands to select another diagnostic disk. See [I/O VM](io-vm.md).

Use `make RUN_PROFILE=native` followed by `make run RUN_PROFILE=native`
for the Native/Alpine profile without
an I/O appliance or physical disk. Explicit `INITRAMFS=...` also defaults to
that profile. RISC-V retains its existing Native run profile.

Run the standalone kernel mechanism tests separately:

```sh
make test-qemu ARCH=aarch64
```

This builds the kernel with `kernel-self-test` and starts a four-CPU QEMU
`virt` machine. Kernel self-tests do not load Linux or create a default VM.
Use `make test-native` for userspace-managed Linux guest integration. Its guest
downloads are cached under the platform temporary directory and generated
payloads remain under `kernel/target/guest/`.

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
| `make` / `make run ARCH=aarch64` | Build while preserving persistent disk contents / start existing artifacts |
| `make rebuild` | Repack the board disk, resetting persistent contents |
| `make test-native` | Verify the Native service graph, command execution, and managed Linux VM console attach/detach under QEMU |
| `make guest-assets ARCH=<arch>` | Download and package the pinned Linux guest inputs |
| `make check ARCH=<arch>` | Run target checks and Clippy, including kernel self-test builds |
| `make test ARCH=<arch>` | Run kernel host, Kconfig, and kallsyms tests |
| `make test-image ARCH=<arch>` | Verify the ELF, relocation, image, and architecture contract |
| `make test-qemu ARCH=<arch>` | Run the architecture's QEMU acceptance test where supported |
| `make verify ARCH=<arch>` | Run kernel checks, tests, supported QEMU acceptance, and release/image validation |
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

See [Native applications](applications.md) for file tools, process monitoring,
named VM commands, and `/etc/hyper/vms.json` configuration.

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

See [Incremental builds](incremental-builds.md) for cache behavior and recovery.

For physical Pi 5 bring-up, see the [official firmware boot and debug UART setup](../kernel/docs/rpi5.md).
