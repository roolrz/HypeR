<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Implementation status and design

[Project overview](../README.md)

This is the detailed inventory of implemented foundations and their current
acceptance boundaries. Planned work is tracked in the [roadmap](roadmap.md).

| Host architecture | Status | Current acceptance contract |
| --- | --- | --- |
| AArch64 | Tier 1 | QEMU `virt` with FEAT_VHE required; LSE; SMP; Linux guest reaches `/init` and provides an interactive console |
| RISC-V 64-bit | Supported | QEMU `virt`; kernel self-tests; UP/SMP Native applications and userspace-managed Linux guests, interactive console and VM retirement |
| x86-64 | Experimental | QEMU `q35`-targeted build and image validation; no public runtime contract yet |

On Pi 5 D0, hardware bring-up has confirmed the official EEPROM boot path,
four host CPUs, interrupt-driven debug UART, Linux I/O VM userspace and FAT
directory reads through the SDIO1/dm-linear/vhost-scsi path. A two-vCPU Alpine
guest has passed three CPU 1 off/on cycles, ordinary reboot with a synced ext4
file preserved, and ordinary poweroff with the I/O VM and `/data` still
available. Those lifecycle results precede the combined SDIO1/RP1 deployment.
On 2026-10-06, the combined deployment completed 1 GiB Native FAT and guest ext4
writes/overwrites with full readback, plus bidirectional guest TCP measurements
against a Mac peer. The [reference data](../README.md#exploratory-pi-5-io-measurements)
uses an informal environment and one run per case. Power-loss durability,
concurrent and sustained load, and physical DMA retirement under faults remain
unqualified and are tracked as
[deferred qualification](roadmap.md#deferred-hardware-qualification).
The current priorities are best-effort block/guest-network tuning without major
code or architecture rework, and capability/DMA isolation hardening around the
I/O VM. These are planned work, not additional isolation guarantees of the
current deployment. A [standard SMMUv3 driver and QEMU PCI DMA fixture](../kernel/docs/smmuv3.md)
now provide stage-2 translation, invalidation, IRQ-driven stream quarantine
and fail-closed controller handling; the current I/O VM
assignment path is not yet connected to those DMA domains.
See the [Pi 5 guide](../kernel/docs/rpi5.md) and
[image release contract](image-distribution.md).

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
  completion before target publication, including blocked waiters and affinity-driven vCPU migration;
- generation-tagged, allocation-free wait arbitration across notification,
  timeout, and cancellation, with counted Completion, sleeping Mutex and
  Semaphore primitives, and migration-safe deadline waits;
- an in-tree, schema-defined pre-release [HypeR Native ABI](../sdk/abi/docs/native.md)
  with checked Rust and C layouts, syscall metadata, and an auditable reference;
- a compiled capability foundation with fallible shared objects, schema-owned
  rights, 64-bit generation handles, detached unpublished slot transactions,
  deferred close, allocation-free iterative teardown, and weak global
  object/Process discovery with immutable, scope-derived task and object
  inspectors plus bounded pointer-free handle-graph snapshots;
- capability-backed Event objects with independent `WAIT` and `SIGNAL`
  authority, absolute-deadline waits, and exactly-once arbitration among
  signal, timeout, and Process cancellation;
- bounded byte-channel endpoints with FIFO messages and level signals, plus
  synchronous capability rendezvous with typed receive contracts;
- strong `ProcessImage`, `Process`, object-backed `UserThread`, and `TaskGroup`
  ownership with accounted construction, explicit publication, start/ready,
  stop/join, and acknowledged retirement;
- Native user-thread creation/start/stop, process-private atomic wait/wake and
  scheduler-backed sleep, with Rust std spawn/join, TLS cleanup and detached
  stack reclamation through the shared runtime;
- a writable kernel ramfs with rooted traversal, links, rename, metadata and
  advisory locks, optional RTC-backed UTC, a small kernel bootstrap contract,
  an AArch64/RISC-V userspace ELF64 mapper, and a capability-relative runtime linker for Native `/init` and its
  services, with eager relocation, W^X/RELRO enforcement, guarded stacks, and
  scheduler-owned Process publication;
- immutable file/VMO snapshots and private mappings with copy-on-write or
  eager allocation; complete executable source pages can be reused across
  processes while relocations and other writes remain private. Shared writable
  VMOs and guest memory retain stable physical backing; this is not live
  address-space cloning or fork;
- a manifest-driven Native init which constructs services transactionally and
  delegates monotonically attenuated capabilities;
- capability-scoped Native `ps` and `handle` tools with immutable task names,
  explicit Process-to-Thread ownership, decoded object purposes and rights,
  and KOIDs that remain diagnostic correlation values rather than authority;
- an AArch64 VHE native-EL0 proof which enters through a scheduler-owned
  user Thread, dispatches the initial handle, scheduling, lifecycle, and Event
  syscalls, contains a user fault, and retires the complete Process ownership
  graph;
- safe AArch64 IRQ-tail preemption, including deactivation and resumption of
  scheduler-owned vCPU continuations;
- IRQ domains and shared handler registration, host/guest GICv2 and GICv3, PLIC, x2APIC, host
  and virtual architectural timers;
- host PSCI and SBI CPU-power backends;
- AArch64 guest SMP with 1..8 vCPUs on GICv2/GICv3, per-vCPU timer and IPI
  state, and runtime-mediated PSCI CPU on/off and VM poweroff/reset; guest
  suspend remains unsupported, and RISC-V guests currently retain one vCPU;
- a compatibility-matched platform driver framework, PL011 and NS16550 UARTs,
  and reusable virtual-device models;
- board guest FIT images loaded from `/data`, with only the I/O VM FIT in the
  bootstrap initramfs; standalone Native acceptance archives retain guest fixtures;
- rollback-safe VM construction with one generational registry publication for
  guest memory, virtual interrupts, devices, and all configured dormant vCPUs;
- a capability-scoped Native VM manager contained by a bounded fleet resource
  domain, plus isolated per-VM runtimes which parse FIT images, own guest VMOs,
  construct Linux firmware data, and supervise installed VMs from userspace;
- a multi-client VM control plane and `/bin/vmm` lifecycle client, with one
  exclusive runtime-provided console connection per VM, caller-allocated shared
  output pages, and userspace retention independent of the physical Console;
- boot-relative, severity-tagged kernel log buffering, lock-independent Thread
  name snapshots for diagnostics, kallsyms, guarded kernel/IRQ/emergency
  stacks, and an optional allocation-free crash console.

This list describes implemented foundations, not a claim of production
completeness. In particular, general-purpose VMM resource policy, device assignment, strong
guest isolation policy, cross-architecture asynchronous preemption, automatic load balancing, broad hardware discovery, stable
management APIs, and a general-purpose virtual I/O stack are still under
development.

## Linux I/O VM baseline

The [I/O VM integration](io-vm.md) has an AArch64 QEMU cross-VM storage baseline.
Unmodified upstream Linux 6.18.51 and external modules run in a trusted I/O VM;
it directly drives a QEMU virtio-scsi disk and exports it through upstream
vhost-scsi/LIO iblock. A separate Linux guest uses its ordinary virtio-scsi
block driver. The Native fixture verifies 32 direct write/flush/read rounds,
host disk bytes, and restoration of Native memory access after both VMs retire.
Both GICv2 with one host CPU and GICv3 with four host CPUs pass driver
unbind/rebind followed by repeated I/O, exercising backend endpoint reset and
a new notification epoch. Each guest in this isolated storage fixture has one vCPU; board Alpine
deployment uses two on AArch64.

The implementation includes capability-gated physical device claims, explicit
DMA translations, shared guest-memory grants, bounded asynchronous per-vCPU
configuration MMIO, control mailboxes, and direct kernel kick/call notification
bindings. Native management services do not forward individual disk requests.
AArch64 GICv2/GICv3 tests also exercise mailbox interrupts, cross-VM notifications,
peer exit, stale completion rejection and interrupted MMIO retirement.

Linux build, modules, services, rootfs assembly and source packaging live in
[HypeR-io-vm](https://github.com/roolrz/HypeR-io-vm). HypeR owns Native deployment,
DT generation and digest-pinned import. Default AArch64 `make run` keeps the
Native shell and starts the resident I/O VM, whose shared queues back the FAT
configuration volume mounted at `/data`. The manager loads `/data/vms.json`;
`vmm start alpine` boots its configured disk-backed guest. `make` prepares these
artifacts, while `make run` only launches them. The optional `RUN_PROFILE=io`
standby profile deliberately omits Native storage and business clients.

`board-storage` tests persistent Native files, Alpine ext4 root, ordinary VM
disks, broker isolation and userspace device access. `test-io-vm` checks the
isolated two-VM data path; `test-io-standby` checks idle backend operation.
Pi 5 SDIO1 assignment is implemented in Native userspace. The complete RP1
PCI function is assigned through a generic PCI configuration/BAR/MSI-X
transport. Linux owns RP1 interrupts and peripheral drivers; the host retains
the BCM2712 PCIe bridge and physical MSI controller. This trusted I/O VM path
has no IOMMU DMA isolation. Basic physical guest networking has been exercised;
sustained/fault qualification, automatic recovery of quarantined devices and
fault-time DMA retirement remain unfinished.
QEMU does not qualify physical cache, interrupt or DMA behavior.

## Guest virtio-net implementation

The QEMU guest path uses a Native virtio-mmio frontend and Linux vhost-net/TAP,
bridged to an assigned virtio-net uplink and QEMU user networking. A VM can have
a disk, a network interface, or both. Both devices share one guest-memory grant
and mailbox, with separate notification bindings and reset epochs. All vhost
users must drain before the shared mapping can be released. Native client zero
reserves a future network identity without creating a frontend or link.

The reference GICv2/GICv3 controller exposes 256 interrupt IDs, and physical
assignment supports multiple controllers with common VM publication and
retirement. The network device has one RX/TX pair, a configured MAC and MTU
1500; offloads and multiqueue remain disabled.

Host configuration/state-machine tests and Linux control/retirement-failure
tests pass. Manual AArch64 QEMU TCG acceptance passes with both GICv2 and GICv3:

- A disk+network guest and a network-only guest obtain separate DHCP leases.
- Each downloads a 256 KiB + 137 byte host-served payload and verifies SHA-256.
  An additional public HTTP fetch checks DNS and outbound connectivity.
- Each network driver unbinds/rebinds and transfers again while the other VM
  remains running; both guests also pass stop/start and repeat transfers.
- The disk guest retains its proof file; the network-only guest has no SCSI
  device. All four VM stops receive `RELEASE_MEMORY: ok`.

Alpine now brings up its interfaces and runs a background DHCP client after
mounting its final root. Automatic address, route and DNS configuration, DHCP
client liveness, HTTP checksums and repeated start/stop have also passed on the
default GICv3 QEMU deployment. Separate guest boot checks confirm that a missing
NIC or DHCP server does not block the shell.

The QEMU results above are functional tests without performance measurements. The
Pi 5 development image also passed a physical smoke test after PCI host window
setup was corrected. The implementation assigns the whole RP1 PCI function to
Linux, with a separate noncoherent DMA bus and exclusive ownership of the RP1
shared infrastructure. The published pin subsequently served the 2026-10-06
physical storage and TCP performance survey. Full hardware qualification,
stress behavior and DMA retirement remain separate;
see [Ethernet qualification](../kernel/docs/rpi5.md#ethernet-backend-qualification).

This control plane uses protocol version 3 of the independent Linux appliance.
The package lock selects the published QEMU and Pi 5 generations from the
merged upstream source. Ordinary builds import these immutable GHCR digests.
See [build and validation](io-vm.md#build-and-validation) for the QEMU deployment
and package boundary.

## Design priorities

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

## Architecture

HypeR keeps policy above mechanism:

```text
Native userspace VMM, applications and services
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
actions only after policy returns. Cargo enforces the HAL-to-core dependency
direction, Rust privacy hides architecture internals, and CI checks that graph
and rejects direct private-backend imports.

Read [the architecture guide](../kernel/docs/architecture.md) for the normative
boundary rules and migration constraints. The Native contracts are specified in the
[userspace and syscall design](../kernel/docs/syscall-abi.md).

Kernel self-test images contain no Linux guest loader or default VM policy.
AArch64 and RISC-V Linux integration uses Native `vmm create/start/console` and
validates guest userspace startup, console input, named VM isolation and runtime-loss
retirement. RISC-V also retains a separate Native-authority guest fixture for
stop, WFI wakeup, virtual UART/PLIC, privilege isolation and retirement.
The x86-64 build gate does not establish a userspace guest-boot contract.

Pi 5 host bring-up prerequisites and recommended firmware configuration are
documented in [Raspberry Pi 5](../kernel/docs/rpi5.md). GICv2 Linux guest boot
and lifecycle tests run in QEMU; physical Pi 5 validation has established the
bounded hardware results listed above. Broader stress and fault qualification
remain outstanding.
The guest SMP acceptance target covers affinity-driven vCPU migration between host CPUs,
timer progress across migration, secondary CPU off/on cycles, concurrent
work, guest reboot/poweroff and reclamation on both GIC backends, including a
four-vCPU guest on a single-host-CPU configuration. This checks integration,
not physical cache, TLB or interrupt ordering.
