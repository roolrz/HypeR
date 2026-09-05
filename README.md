<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR

[![CI](https://github.com/roolrz/HypeR-Kernel/actions/workflows/ci.yml/badge.svg)](https://github.com/roolrz/HypeR-Kernel/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

HypeR is an experimental type-1 hypervisor and kernel written in Rust. It is
being built as a long-lived systems project: architecture mechanisms, host
kernel policy, virtual-machine services, and device models have explicit
boundaries so that each can evolve without turning the kernel into a collection
of platform-specific shortcuts.

AArch64 is the Tier-1 architecture. RISC-V 64-bit is a supported secondary port;
x86-64 remains experimental. Both expose accidental coupling and continuously
test the kernel's architecture boundaries. Today, HypeR can bring up SMP hosts
in QEMU, mount a firmware-provided initramfs, load a constrained Native ELF
`/init` on AArch64, and boot Linux guests to initramfs `/init` in repository
acceptance tests on AArch64 and RISC-V.

> [!IMPORTANT]
> HypeR is under active development. It is not yet suitable for production,
> untrusted workloads, or systems where compatibility and data integrity are
> required. Interfaces and on-disk formats may change while the architecture is
> being established.

## Why HypeR?

- **Rust at the kernel boundary.** The kernel is `no_std` and `no_main`.
  Assembly is limited to architectural entry, exception and context
  transitions, and instructions Rust cannot express. Panic-like convenience
  paths such as `unwrap` and `expect` are rejected by project lints.
- **Architecture is a first-class design constraint.** AArch64 remains the
  semantic priority, while RISC-V and x86-64 expose accidental coupling early.
  Common interfaces must preserve architecture-specific correctness rather
  than reducing every machine to the smallest shared abstraction.
- **Explicit ownership and lifecycle.** Page ownership, thread state, IRQ
  registration, vCPU context, publication, and rollback are designed as local
  contracts. Unsafe code is treated as an auditable implementation boundary,
  not a substitute for missing APIs.
- **Hardware-realistic foundations.** Cache maintenance, barriers, interrupt
  state, TLB invalidation, CPU startup, exception entry, and guest world
  switches are implemented with real-hardware ordering requirements in mind,
  even when QEMU cannot expose every failure mode.
- **Inspectable failure paths.** The kernel includes structured logging,
  allocation-free symbol lookup, architecture register dumps, guarded stacks,
  and an optional crash console for post-failure inspection.
- **Reproducible acceptance contracts.** Host tests, image verification, and
  QEMU guest-boot tests are normal project interfaces. Guest artifacts are
  checksum-pinned, generated under the ignored `target/` tree, and never
  embedded in the repository.

## Project status

| Host architecture | Status | Current acceptance contract |
| --- | --- | --- |
| AArch64 | Tier 1 | QEMU `virt`; nVHE and VHE; LL/SC and LSE; SMP; Linux guest reaches `/init` and completes repeated timer wakeups |
| RISC-V 64-bit | Supported | QEMU `virt`; OpenSBI, H extension, PLIC and SSTC; Linux guest reaches `/init` |
| x86-64 | Experimental | QEMU `q35`-targeted build and image validation; no public runtime contract yet |

The current foundation includes:

- position-independent boot images and Linux-compatible architecture entry;
- FDT-based platform discovery on current QEMU hosts, with Linux-compatible
  architecture handoffs including x86 boot parameters;
- KASLR with RELA/RELR relocation on AArch64;
- permanent stage-1 mappings, guest stage-2 translation, boot allocation,
  buddy allocation, slab allocation, and a bounded per-CPU fast path behind
  Rust's global allocator interface;
- SMP startup with a scheduler-owned idle thread on every admitted CPU;
- class-aware, intrusive ready queues with RT FIFO and a replaceable Fair
  class whose initial backend is time-sliced round-robin;
- explicit affinity-controlled kernel-Thread migration with source-context
  completion before target publication, including blocked waiters;
- generation-tagged, allocation-free wait arbitration across notification,
  timeout, and cancellation, with counted Completion, sleeping Mutex and
  Semaphore primitives, and migration-safe deadline waits;
- an exactly pinned, pre-release [HypeR Native ABI](https://github.com/roolrz/HypeR-ABI)
  with checked Rust and C layouts, syscall metadata, and an auditable reference;
- a compiled capability foundation with fallible shared objects, schema-owned
  rights, 64-bit generation handles, detached unpublished slot transactions,
  deferred close, allocation-free iterative teardown, and weak global
  object/Process discovery with bounded pointer-free handle-graph snapshots;
- capability-backed Event objects with independent `WAIT` and `SIGNAL`
  authority, absolute-deadline waits, and exactly-once arbitration among
  signal, timeout, and Process cancellation;
- bounded Channel endpoints with FIFO messages, level signals, transactional
  user copies, and atomic capability-transfer publication;
- strong `ProcessImage`, `Process`, object-backed `UserThread`, and `TaskGroup`
  ownership with accounted construction, explicit publication, start/ready,
  stop/join, and acknowledged retirement;
- an immutable indexed ramfs and strict AArch64 ELF64 loader for the first
  Native `/init`, with static PIE relocation, W^X enforcement, guarded stack,
  and scheduler-owned Process publication;
- an AArch64 VHE/nVHE native-EL0 proof which enters through a scheduler-owned
  user Thread, dispatches the initial handle, scheduling, lifecycle, and Event
  syscalls, contains a user fault, and retires the complete Process ownership
  graph;
- safe AArch64 IRQ-tail preemption, including deactivation and resumption of
  scheduler-owned vCPU continuations;
- IRQ domains and shared handler registration, GICv3/vGIC, PLIC, x2APIC, host
  and virtual architectural timers;
- PSCI and SBI CPU-power backends;
- a compatibility-matched platform driver framework, PL011 and NS16550 UARTs,
  and reusable virtual-device models;
- versioned CPIO VM bundles delivered through the firmware ramdisk;
- rollback-safe VM construction with one generational registry publication for
  guest memory, virtual interrupts, devices, and the dormant boot vCPU;
- boot-relative, severity-tagged kernel log buffering, lock-independent Thread
  name snapshots for diagnostics, kallsyms, guarded kernel/IRQ/emergency
  stacks, and an optional allocation-free crash console.

This list describes implemented foundations, not a claim of production
completeness. In particular, a general-purpose Native runtime, an EL0 VMM, a
published capability syscall ABI, device assignment, strong
guest isolation policy, cross-architecture asynchronous preemption, controlled
vCPU migration, automatic load balancing, broad hardware discovery, stable
management APIs, and a general-purpose virtual I/O stack are still under
development.

## Architecture

HypeR keeps policy above mechanism:

```text
native EL0 VMM, services, and future compatibility supervisors
    -> schema-defined pre-release syscall and capability boundary
    -> kernel user-entry adapters and services
    -> kernel policy: task, IRQ, time, memory, crash, device
    -> kernel VM policy: lifecycle, vCPU orchestration, resource ownership
    -> reusable VM formats, guest ABI, events, and device models
    -> architecture-neutral mechanisms and HAL capabilities
    -> selected architecture, firmware, and physical drivers
    -> registers, instructions, MMIO, and assembly
```

HypeR exposes the selected architecture through enforced topical context, CPU,
exception, guest ABI, IRQ, memory, platform, time, and virtualization facades.
Architecture backends own machine context, register and page-table formats,
exception entry, world switching, and hardware virtualization. Kernel and VM
services own policy, resource publication, scheduling, virtual-device binding,
and failure decisions.

Exceptions and VM exits necessarily travel upward. Named entry adapters confine
that transition, copy architecture-private state into owned typed events,
invoke immutable registered kernel services, and encode exhaustive completion
actions only after policy returns. CI rejects direct architecture-to-kernel
policy dependencies outside the three non-returning bootstrap transfers.

Read [the architecture guide](docs/architecture.md) for the normative boundary
rules and migration constraints. The planned process, capability, syscall, and
foreign-ABI boundary is specified separately in the [userspace and syscall
design](docs/syscall-abi.md).

## Quick start

### Prerequisites

- Rust and rustup; `rust-toolchain.toml` selects the pinned compiler and
  components;
- Clang/LLVM;
- GNU Make;
- QEMU for the selected architecture;
- `curl`, `cpio`, `gzip`, `tar`, and SHA-256 tooling for the Linux guest assets;
- `dtc` when building the x86-64 QEMU platform description.

Build the standalone AArch64 kernel and run it with an uncompressed `newc`
initramfs containing a Native `/init`:

```sh
make defconfig
make run INITRAMFS=/path/to/hyper-initramfs.cpio
```

The integrated HypeR build supplies this archive from the Native userspace
repositories. The Kernel repository validates the loader and ramfs with host
tests; it does not carry or synthesize a production `/init` program.

Linux guest construction and boot remain Kernel integration tests:

```sh
make test-qemu ARCH=aarch64
```

This downloads checksum-pinned Alpine Linux inputs, constructs a versioned VM
bundle, builds the kernel with `kernel-self-test`, and starts a four-CPU QEMU
`virt` machine. Guest downloads are cached under the platform temporary
directory and generated payloads remain under `target/guest/`.

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
| `make run ARCH=aarch64 INITRAMFS=<path>` | Start the production boot path with a Native initramfs |
| `make guest-assets ARCH=<arch>` | Download and package the pinned Linux guest inputs |
| `make check ARCH=<arch>` | Run target checks and Clippy, including kernel self-test builds |
| `make test ARCH=<arch>` | Run host, Kconfig, kallsyms, and Native ABI tests |
| `make test-image ARCH=<arch>` | Verify the ELF, relocation, image, and architecture contract |
| `make test-qemu ARCH=<arch>` | Run the architecture's QEMU acceptance test where supported |
| `make verify ARCH=<arch>` | Run the complete local contract for the selected architecture |

The default AArch64 image is written to:

```text
target/aarch64-unknown-none/kernel/hyper
target/aarch64-unknown-none/kernel/hyper.img
```

`make release` strips debugger-only sections from the canonical ELF without
recompiling it, then verifies that the resulting raw image is byte-identical.

## Configuration

HypeR uses an in-tree, dependency-free Kconfig-like tool. It reads the root
`Kconfig`, writes a Linux-style `.config`, validates dependencies and ranges,
and exports declared symbols as checked Rust `cfg` values and typed constants.

```sh
make config       # interactive configuration
make olddefconfig # accept defaults for newly introduced symbols
make defconfig    # restore the selected QEMU architecture defaults
```

`CONFIG_FILE` selects an alternate complete configuration without replacing a
developer's `.config`:

```sh
make image ARCH=aarch64 CONFIG_FILE=configs/qemu_aarch64_defconfig
```

## Testing and CI

GitHub Actions separates source quality, architecture builds, image contracts,
and runtime acceptance. The AArch64 matrix exercises baseline and feature-rich
CPU models, multiple host modes and atomic backends, address-space geometries,
SMP, kernel self-tests, virtual interrupts and timers, and Linux guest startup.
It requires repeated initramfs timer wakeups before exercising guest-console
RX, so reaching `/init` or delivering only the first timer interrupt cannot
hide a stalled virtual timer. RISC-V must also reach guest `/init`. x86-64
currently has a build and image contract only.

Stable local equivalents live in `tests/ci/run.sh`:

```sh
sh tests/ci/run.sh quality
sh tests/ci/run.sh aarch64-build
QEMU_CPU=max QEMU_CPUS=4 sh tests/ci/run.sh aarch64-qemu
sh tests/ci/run.sh riscv64-qemu
sh tests/ci/run.sh x86_64-build
```

QEMU tests track and terminate the processes they create. Failed CI runs retain
serial logs as artifacts. See [the CI contract](tests/ci/README.md) for the
exact coverage and supported runtime expectations.

## Roadmap

The roadmap describes direction rather than release promises. Adjacent work may
overlap, but a later stage must not bypass an ownership or isolation prerequisite
from an earlier one. Every stage keeps AArch64 healthy and preserves buildable
secondary architectures.

### 1. Establish native EL0 and the capability ABI

- extend the current loader-backed AArch64 VHE host-EL0 and nVHE
  stage-2-only Native path into a service runtime before publishing a binary
  ABI, retaining a minimal EL1 relay only as a justified compatibility
  fallback;
- extend the implemented Process, UserThread, ProcessImage, TaskGroup,
  ResourceDomain, and address-space lifecycle with multi-Thread race coverage
  and atomic exec quiescence;
- expand the checked schema's current Rust values, C header, layouts, metadata,
  and reference into generated dispatch wrappers, architecture stubs, and vDSO
  exports;
- expose the typed VMO/VMAR capability core through Native syscalls, then
  extend the implemented Event, Channel, and single-object wait foundation with
  EventPair, WaitSet, clock/timer, and atomic-wait primitives sufficient for a
  real service runtime;
- add temporary pre-release debug output, then extend the implemented blocking
  cancellation with multi-Thread Process qualification before atomic exec.

### 2. Extend VM lifetime and topology

- replace the current non-removable binding with allocator-safe VM leases and
  cross-CPU vCPU retirement;
- add stage-2/TLB teardown and hardware VMID retirement before registry reuse;
- install and start multi-vCPU groups transactionally without parallel global
  registries;
- add explicit pause, resume, shutdown, and resource-accounting lifecycles.

### 3. Move VMM policy to native userspace

- expose VM, vCPU, guest-memory, interrupt, and lifecycle operations through
  typed capabilities without duplicating the kernel VM registry;
- start a native EL0 VMM and move bundle selection, VM construction policy, and
  management orchestration out of EL2;
- retain architecture entry, translation, IRQ, and world-switch mechanisms
  behind the selected HAL.

### 4. Build device-isolation resources

- extend the existing owned MMIO/IRQ model to DMA, IOMMU, firmware, and
  physical-memory authorities;
- implement revocable MemoryGrant, DeviceLease, DmaMapping, and
  InterruptSession lifecycles with fail-closed teardown;
- resolve generic FDT phandles, `interrupt-map`, and `interrupts-extended`
  without moving binding policy into the parser.

### 5. Add the transitional Linux driver domain

- run Linux as an untrusted driver-domain VM, never as HypeR's host OS;
- introduce a bounded copy-based backend transport before shared zero-copy
  queues, then add virtio block and network frontends;
- require stage-2 and IOMMU confinement before assigning physical devices or
  permitting DMA into granted memory.

### 6. Mature kernel execution

- qualify IRQ-tail and vCPU preemption on secondary architectures, then add
  controlled migration and load balancing over existing affinity metadata;
- strengthen power-management, suspend/resume, and CPU hotplug lifecycles;
- expand diagnostics, tracing, crash analysis, and management interfaces;
- validate ordering, cache maintenance, and interrupt behavior on physical
  AArch64 hardware.

### 7. Add foreign binary personalities

- add a supervised execution route which is immutable for each installed
  ProcessImage and prove it first with a small alternate test ABI;
- implement Linux and FreeBSD compatibility supervisors with private fd,
  signal, credential, VFS, restart, auxiliary-vector, and vDSO policy;
- keep the route extensible for a future whole-personality in-kernel engine,
  while rejecting per-syscall mixing of kernel and supervisor semantics.

## Repository guide

| Path | Responsibility |
| --- | --- |
| `src/arch/` | Architecture entry, context, page-table, exception, and virtualization mechanisms |
| `src/hal/` | Narrow architecture-neutral capability contracts |
| `src/kernel/` | Runtime ownership, policy, scheduling, IRQ, memory, devices, and VM orchestration |
| `src/vm/` | Reusable VM packages, guest-visible models, and architecture-neutral virtualization vocabulary |
| `src/drivers/` | Physical devices and firmware-interface drivers |
| `src/platform/` | Firmware parsing and immutable platform description |
| `src/mm/`, `src/sync/`, `src/time/` | Reusable allocation, synchronization, and timing mechanisms |
| `tests/` | Host tests, kernel self-tests, image verification, QEMU acceptance, and CI contracts |
| `tools/` | Kconfig, kallsyms, and guest-payload tooling |

Further documentation:

- [Architecture boundaries](docs/architecture.md)
- [Native init contract](docs/native-init.md)
- [Userspace and syscall architecture](docs/syscall-abi.md)
- [HypeR Native ABI](https://github.com/roolrz/HypeR-ABI)
- [VM bundle format](docs/vm-bundle.md)
- [RISC-V execution profile](docs/riscv64.md)
- [x86-64 execution profile](docs/x86_64.md)
- [Crash console](docs/crash-console.md)
- [Security policy](SECURITY.md)
- [External guest payload and licensing](tools/guest/README.md)

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
rewrites. [Open an issue](https://github.com/roolrz/HypeR-Kernel/issues) before
starting work that changes a public format, architecture boundary, unsafe
ownership model, or guest-visible ABI. Issues are also the preferred place for
bug reports and focused design proposals while a fuller contributor guide is
being prepared.

## Guest artifacts and licensing

HypeR source code is licensed under the [Apache License 2.0](LICENSE).

`make guest-assets` downloads external Linux and Alpine artifacts for testing.
They are checksum-verified, ignored by Git, and are not part of the
Apache-2.0-licensed source distribution. Linux is GPL-2.0-only and Alpine
packages carry their own licenses. Anyone redistributing generated guest
payloads must preserve the relevant upstream notices and source-availability
obligations. See [tools/guest/README.md](tools/guest/README.md) for details.

---

HypeR is being built in public for developers who care about hypervisors,
kernel architecture, and low-level Rust. If that sounds like your kind of
problem, contributions and rigorous reviews are welcome.
