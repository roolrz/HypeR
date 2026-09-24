<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Development guide

[Project overview](../README.md)

## Rust editor support

Open the repository root in VS Code with the `rust-lang.rust-analyzer`
extension installed. The shared `.vscode/settings.json` explicitly loads all
Cargo project roots, including the SDK workspace, Native applications,
and build tools. Run `make sdk ARCH=aarch64` once to prepare the Native target
and patched std sources, then **Developer: Reload Window** after updating editor
settings. Continue opening
the repository root; no separate editor workspace or generated configuration
is needed.

Reload the VS Code window, not just the language server, if new settings are
ignored. On some mounted volumes VS Code refuses to watch `.vscode` for file
changes; restarting rust-analyzer then reuses the cached client configuration.
The extension's Output log should show the configured `cargo.extraEnv` and
`check.overrideCommand`, rather than `{}` and `null`.

The editor-only `.vscode/rust-analyzer.toml` supplies the AArch64 kernel
configuration, without dependency patches. Applications and the smoke programs
keep SDK source patches and editor targets in their workspace-local Cargo
configuration. This prevents editor discovery from adding `patch.unused` records
to kernel, ABI and host-tool lockfiles that ordinary builds would remove.
The production `hyper-cargo` driver overrides source paths with installed SDK
paths; save-time editor checks explicitly select the local configuration.
The freestanding Rust smoke program selects `aarch64-unknown-none`;
kernel and host projects retain their host target. The shared std source
tree contains both HypeR and upstream host implementations; each project selects
its platform through the actual target cfgs. This resolves `File::into_std()`
and `std::os::hyper` without pretending that host code targets HypeR.

Build-script discovery and save-time diagnostics both use
`scripts/rust-analyzer-check.sh`: Native projects use the existing `hyper-cargo`
driver with repository SDK source patches, while kernel/host projects keep
ordinary Cargo checks including test targets. Editor artifacts are separate
from production outputs under `target/rust-analyzer`. All configured paths are
repository-relative or resolved at invocation time, so relocating the checkout
does not require regenerating settings. Root Make commands still explicitly
select their production architecture and do not load the app-local Cargo config.

The editor defaults to AArch64, like the existing kernel editor configuration;
this does not change RISC-V builds. Rerun `make sdk ARCH=aarch64` and restart
rust-analyzer after changing std/toolchain sources. Other LSP clients can reuse
the settings in `.vscode/settings.json`, with the `rust-analyzer.` prefix removed
and the repository root as their root directory. When adding an independent
Cargo project, add its manifest to `linkedProjects`; kernel and SDK workspace
members are discovered automatically. The kernel workspace includes the binary,
`hyper-core`, and `hyper-hal`.

## Testing and CI

Use these repository-root commands for short feedback loops:

| Change | First check |
| --- | --- |
| App parser, state machine, or policy library | `make app-test APP_TEST_ARGS='test_name'` |
| Native ABI and SDK source behavior | `make sdk-test` |
| Installed SDK consumption by apps | `make app-sdk-test ARCH=aarch64` |
| Portable kernel mechanisms | `make -C kernel test ARCH=aarch64` |
| App deployment and packaging | `sh tests/ci/run.sh scripts` |
| Kernel or app runtime behavior | The relevant QEMU target below |

`app-test` uses SDK source patches within the existing app workspace, an explicit
host target, and ordinary Cargo. It does not assemble an SDK or require patched
std; the first invocation can download missing Cargo dependencies. `APP_TEST_PACKAGE=hyper-init` selects a single package;
`APP_TEST_ARGS=supervision` filters its test names.
Run it from the repository root so Native editor-target defaults are not loaded.
`app-sdk-test` separately tests the assembled SDK contract. Production app
manifests and builds continue to consume installed SDK sources.

`make app` builds and installs ordinary system applications, including the I/O
runtime. `make app-fixtures` additionally builds static/dynamic linking probes
and Rust std smoke programs. Specialized VM smoke binaries remain owned by their
acceptance targets.

[app/deployment.json](../app/deployment.json) is the shared installation and
initramfs payload manifest. A binary entry defines its Cargo binary, staged
filename, archive destination, mode, and image membership. Cargo workspace
membership remains a build concern; service manifests still own startup and
capabilities, and board JSON still owns storage and device deployment.

`NATIVE_IMAGE_PROFILE=development` is the default and preserves the existing
apps and test programs. `NATIVE_IMAGE_PROFILE=system` includes all ordinary apps
and runtime libraries without acceptance probes; it also avoids building those
probes. The I/O service is included by the existing I/O/board boot profiles.
Changing image membership does not delete applications or their capabilities.
Packaging still copies ELF files before stripping only debug information and
preserves unchanged output timestamps. Use a separate `NATIVE_INITRAMFS` or
`BOARD_OUTPUT` when comparing profiles.

Before packing, `services.json` is checked on the host using init's production
parser, bootstrap authority policy, and supervision rules. Invalid syntax,
dependencies, capability declarations, unsupported restart policies, or missing
service images reject the build without replacing the existing ramdisk. The
check also runs before an incremental packaging cache hit.
For a standalone configuration check (without image membership checks), run:

```sh
python3 scripts/check-service-manifest.py app/init/config/services.json
```

This uses the source workspace and host Rust toolchain; it does not need a built
HypeR SDK. Live handle availability and kernel authorization remain boot-time
checks. `python3 -B tests/build/service-manifest.py` runs the host regression
suite, also included in Native CI.

Plain `make` (or `make all`) builds the kernel and the selected profile's ramdisk.
The default AArch64 board profile creates a disk only if missing; subsequent
builds preserve `/data` and guest disks while updating the host-side kernel and
bootstrap loaded by QEMU. `make run` only starts existing artifacts and fails if
any are missing; it never builds or packages them.

`make rebuild` (or `make board-rebuild` for the board profile) repacks the full
board disk, resetting persistent contents after successful packing. Use
`make clean && make rebuild` to compile from a clean build tree as well.
`make clean` deletes all of `target/`, including default persistent disks;
disks that must survive cleaning need an external `BOARD_IMAGE` or `IO_VM_DISK`.
`make board-image` still refuses an existing output unless explicitly replaced.
For Native/standby profiles, select the same `RUN_PROFILE` for build and run.

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
Linux acceptance explicitly attaches `/bin/vmm` to the buffered guest serial port, exercises guest-console RX, and detaches
through the local Ctrl-] menu. Both Native
integration suites also verify named VM isolation and repeated runtime crashes
with guest-memory reclamation. x86-64 has a build and image contract only.

Stable local equivalents live in `tests/ci/run.sh`:

```sh
sh tests/ci/run.sh scripts
sh tests/ci/run.sh quality
sh tests/ci/run.sh native                 # all four AArch64 Native shards, serially
sh tests/ci/run.sh native-gicv3           # one independent CI shard
sh tests/ci/run.sh board-storage
sh tests/ci/run.sh io-vm
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
| `kernel/hal/` | `hyper-hal` crate: public machine capabilities and private architecture implementations in `src/arch/` |
| `kernel/core/` | `hyper-core` manifest for reusable mechanisms in `kernel/src/lib.rs`; no dependency on kernel policy or the selected HAL |
| `kernel/src/hal/` | Portable HAL contracts in `hyper-core` |
| `kernel/build_support/` | Shared build-time configuration parsing |
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

- [Kernel stack budgets and workload checks](../kernel/docs/stack-budgets.md)
- [Architecture boundaries](../kernel/docs/architecture.md)
- [Native init contract](../kernel/docs/native-init.md)
- [Userspace and syscall architecture](../kernel/docs/syscall-abi.md)
- [Virtual filesystem architecture](../kernel/docs/vfs.md)
- [Native SDK contract](../sdk/README.md)
- [HypeR Native ABI reference](../sdk/abi/docs/native.md)
- [VM image format and boot ownership](../kernel/docs/vm-bundle.md)
- [Linux I/O VM ownership and package delivery](io-vm.md)
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
5. Run `git diff --check`, `sh tests/ci/run.sh quality`,
   `sh tests/ci/run.sh scripts`, and `make check` for every affected
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

The Linux I/O appliance has a separate delivery path: HypeR-io-vm builds and
publishes the complete kernel/initramfs as a GHCR package, while HypeR imports
an immutable digest and supplies its own DTS/DTB and launch policy. Its Linux
sources, configuration, modules, services and rootfs assembly live in that
repository. Corresponding source materials travel with the published package;
they are not added to the HypeR boot ramdisk. See the
[I/O VM contract](io-vm.md) for ownership, import instructions and current limits.

## Native std incremental builds

The assembled SDK records a content fingerprint for its Rust standard-library
sources and Native target descriptions. `hyper-cargo` includes this identity in
std build flags, so changes to the installed std port invalidate cached
`build-std` artifacts in both check and release profiles. Unchanged content
retains the same identity regardless of file timestamps; app-only edits do not
invalidate std. Native libraries and linker tools retain their separate link
fingerprint.

New project-authored files must carry an `SPDX-FileCopyrightText` declaration
with the contributor or copyright holder and a year (or year range), together
with `SPDX-License-Identifier: Apache-2.0`. Preserve existing and upstream
attribution; the header check does not require the repository owner's name.

### Dependency maintenance

Dependabot proposes immutable Action updates and Cargo updates for the kernel,
its host tests/kallsyms tool, Rust SDK and FIT tool. Dependency-free tool roots
need no registry updates. Application and SDK smoke workspaces intentionally
consume unpublished, installed SDK crates: their updates remain manual using
the normal SDK preparation and consumer checks, rather than committing a source
patch overlay into their production manifests. The repository owner reviews
these updates and new external inputs.

`sh scripts/audit-dependencies.sh` runs cargo-audit 0.22.2 against every tracked
Cargo lockfile, including those consumers. CI runs it on lockfile changes and
weekly so newly published advisories are noticed without a code change. It checks
known RustSec advisories, not reachability or non-Rust appliance packages. Any
future advisory exception must name its ID, applicability rationale, reviewer
and review date in `.cargo/audit.toml`; no blanket advisory suppression is used.

The SPDX source check preserves project and vendored notices. New dependency
licenses still require owner review of the actual license text and distribution
obligations; passing an advisory scan is not license approval. External Linux
appliance and firmware distribution retain their separate provenance and
corresponding-source requirements.

The root Makefile retains shared defaults and command orchestration; component
recipes live in `mk/sdk-app.mk`, `mk/native-tests.mk` and `mk/boards.mk`. The
console, application and runtime-crash QEMU tests share subprocess ownership,
serial buffering and deadlines through `tests/qemu/session.py`; their scenario
assertions stay in each test. Editor metadata and save-time checks use Native
workspace-local SDK patches; the shared configuration contains no dependency
patches. Kernel and host-tool lockfiles therefore do not acquire Native
`patch.unused` records through the editor configuration.

Host-test workspaces with ignored lockfiles are compiled and tested separately;
the tracked-lock audit does not assert their exact independently resolved graph.

For focused maintenance feedback, `make app-test APP_TEST_PACKAGE=hyper-vm-manager`
executes the same fleet decisions used by the service, with explicit event time
and recorded resource effects. `make app-test APP_TEST_PACKAGE=hyper-vm-support`
checks the shared image/device/protocol mechanisms. `make sdk-test` includes
C/Rust transport capture tests; `sh sdk/toolchain/scripts/check-loader-arch.sh`
executes the production loader's parser, relocator and rollback paths against
host fixtures (UBSan on all hosts, ASan additionally on Linux). These checks
complement, rather than replace, Native dynamic-linking and cross-VM acceptance.
