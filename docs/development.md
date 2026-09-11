<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Development guide

[Project overview](../README.md)

## Rust editor support

Open the repository root in VS Code with the `rust-lang.rust-analyzer`
extension installed. The shared `.vscode/settings.json` explicitly loads all
nine Cargo project roots, including the SDK workspace, Native applications,
host tests, and build tools. After updating these settings, run
**rust-analyzer: Reload Workspace** if the editor has not reloaded them.

The editor-only `.vscode/rust-analyzer.toml` supplies local SDK dependency
patches and the AArch64 kernel configuration to Cargo. It allows analysis
without first assembling an SDK; production Native builds continue to use
`hyper-cargo` and the assembled SDK. Other LSP clients can use the same
`linkedProjects` list and `cargo.configPath` setting, resolved from the
repository root. When adding an independent Cargo project, add its manifest
to the list; SDK workspace members are discovered automatically.

## Testing and CI

GitHub Actions separates source quality, architecture builds, image contracts,
Native SDK integration, and runtime acceptance. The AArch64 matrix exercises
VHE with different address-space geometries, UP/SMP, kernel self-tests, and
virtual interrupts and timers. A separate negative boot test verifies that
CPUs without FEAT_VHE reach the explicit rejection loop. Linux guest startup is
covered by Native userspace integration through the VMM tools. RISC-V Native integration runs the
same static/dynamic std, process, thread, filesystem, shell and userspace Linux
VM contracts on one and four harts. A separate `make test-vm-smoke ARCH=riscv64`
init fixture validates Native VM lifecycle, timer/serial WFI wakeups and
retirement independently of Linux loading.
Linux acceptance explicitly attaches `/bin/vmm` to the buffered guest serial port, requires
repeated initramfs timer wakeups, exercises guest-console RX, and detaches
through the local Ctrl-] menu. Reaching `/init` or delivering only the first
timer interrupt therefore cannot hide a stalled virtual timer. Both Native
integration suites also verify named VM isolation and repeated runtime crashes
with guest-memory reclamation. x86-64 has a build and image contract only.

Stable local equivalents live in `tests/ci/run.sh`:

```sh
sh tests/ci/run.sh quality
sh tests/ci/run.sh native
sh tests/ci/run.sh riscv64-native
sh tests/ci/run.sh aarch64-build
QEMU_CPU=max QEMU_CPUS=4 sh tests/ci/run.sh aarch64-qemu
sh tests/ci/run.sh riscv64-qemu
sh tests/ci/run.sh x86_64-build
```

QEMU tests track and terminate the processes they create. Failed CI runs retain
serial logs as artifacts. See [the CI contract](../tests/ci/README.md) for the
exact coverage and supported runtime expectations.

## Repository guide

| Path | Responsibility |
| --- | --- |
| `kernel/` | Independently buildable hypervisor kernel, configuration, documentation, tests, and build tooling |
| `kernel/src/arch/` | Architecture entry, context, page-table, exception, and virtualization mechanisms |
| `kernel/src/hal/` | Narrow architecture-neutral capability contracts |
| `kernel/src/kernel/` | Runtime ownership, policy, scheduling, IRQ, memory, devices, and VM orchestration |
| `kernel/src/vm/` | Reusable VM packages, guest-visible models, and architecture-neutral virtualization vocabulary |
| `kernel/src/drivers/` | Physical devices and firmware-interface drivers |
| `kernel/src/platform/` | Firmware parsing and immutable platform description |
| `kernel/src/mm/`, `kernel/src/sync/`, `kernel/src/time/` | Reusable allocation, synchronization, and timing mechanisms |
| `sdk/abi/` | Native ABI schema, generated Rust definitions, C header, and reference |
| `sdk/lib/` | Freestanding Native C runtime and architecture syscall veneers |
| `sdk/loader/` | Native userspace ELF interpreter, relocation, and runtime loading |
| `sdk/rust/` | Safe `no_std` Native OS binding, raw syscalls, and Rust runtime entry |
| `sdk/toolchain/` | Clang/Cargo drivers, linker contract, and transactional SDK assembly |
| `app/` | Native system applications built only against the assembled SDK |
| `kernel/tests/` | Kernel host tests, self-tests, image verification, and QEMU acceptance |
| `kernel/tools/` | Kconfig, kallsyms, and Linux guest-payload tooling |
| `tests/` | Repository-wide contracts and Kernel/SDK/application integration tests |
| `tools/` | Product-composition tooling shared across source domains |

Further documentation:

- [Architecture boundaries](../kernel/docs/architecture.md)
- [Native init contract](../kernel/docs/native-init.md)
- [Userspace and syscall architecture](../kernel/docs/syscall-abi.md)
- [Virtual filesystem architecture](../kernel/docs/vfs.md)
- [Native SDK contract](../sdk/README.md)
- [HypeR Native ABI reference](../sdk/abi/docs/native.md)
- [VM image format and boot ownership](../kernel/docs/vm-bundle.md)
- [RISC-V execution profile](../kernel/docs/riscv64.md)
- [x86-64 execution profile](../kernel/docs/x86_64.md)
- [Crash console](../kernel/docs/crash-console.md)
- [Security policy](../SECURITY.md)
- [External guest payload and licensing](../kernel/tools/guest/README.md)

## Contributing

HypeR welcomes contributions in architecture support, virtualization, memory
management, drivers, testing, documentation, and design review. The project is
still establishing core contracts, so structural changes should begin with the
invariant they enforce and the dependency direction they preserve—not only a
file move or a new abstraction.

Before opening a pull request:

1. Keep source code, comments, documentation, diagnostics, and commit messages
   in English.
2. Avoid `unwrap`, `expect`, implicit panic paths, and unnecessary unsafe code.
3. Document ownership, publication, synchronization, and hardware/ABI
   obligations that are not evident from the types.
4. Add host tests for portable mechanisms and architecture acceptance coverage
   for changed low-level paths.
5. Run `sh tests/ci/run.sh quality` and `make check` for every affected
   architecture. Run the relevant QEMU contract when runtime behavior changes.
6. Do not add GPL-derived implementation code. New dependencies require
   explicit `no_std`, maintenance, and license review.
7. Preserve the SPDX copyright and Apache-2.0 header in every new tracked text
   file. Cargo-generated lockfiles and the complete `LICENSE` text are exempt.

Small, coherent changes with a clear migration seam are preferred over broad
rewrites. [Open an issue](https://github.com/roolrz/HypeR/issues) before
starting work that changes a public format, architecture boundary, unsafe
ownership model, or guest-visible ABI. Issues are also the preferred place for
bug reports and focused design proposals.

## Guest artifacts and licensing

HypeR source code is licensed under the [Apache License 2.0](../LICENSE).

`make guest-assets` downloads external Linux and Alpine artifacts for testing.
They are checksum-verified, ignored by Git, and are not part of the
Apache-2.0-licensed source distribution. Linux is GPL-2.0-only and Alpine
packages carry their own licenses. Anyone redistributing generated guest
payloads must preserve the relevant upstream notices and source-availability
obligations. See [kernel/tools/guest/README.md](../kernel/tools/guest/README.md) for details.

## Native std incremental builds

The assembled SDK records a content fingerprint for its Rust standard-library
sources and Native target descriptions. `hyper-cargo` includes this identity in
std build flags, so changes to the installed std port invalidate cached
`build-std` artifacts in both check and release profiles. Unchanged content
retains the same identity regardless of file timestamps; app-only edits do not
invalidate std. Native libraries and linker tools retain their separate link
fingerprint.
