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
  advisory locks, optional RTC-backed UTC, a strict AArch64/RISC-V ELF64 process
  loader, and
  capability-relative userspace runtime linker for Native `/init` and its
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
- Linux guest FIT images delivered through the Native root initramfs;
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
guest isolation policy, cross-architecture asynchronous preemption, controlled
vCPU migration, automatic load balancing, broad hardware discovery, stable
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
a new notification epoch. Each storage guest currently has one vCPU.

The implementation includes capability-gated physical device claims, explicit
DMA translations, shared guest-memory grants, bounded asynchronous per-vCPU
configuration MMIO, control mailboxes, and direct kernel kick/call notification
bindings. Native management services do not forward individual disk requests.
AArch64 GICv2/GICv3 tests also exercise mailbox interrupts, cross-VM notifications,
peer exit, stale completion rejection and interrupted MMIO retirement.

Linux build, modules, services, rootfs assembly and source packaging live in
[HypeR-io-vm](https://github.com/roolrz/HypeR-io-vm). HypeR owns Native deployment,
DT generation and digest-pinned import. Default AArch64 `make run` keeps the
Native shell and starts an idle I/O VM under init, with a persistent QEMU disk
and no client VM or shared queues. `test-io-standby` checks shell responsiveness
and preservation of disk contents; `test-io-vm` remains the explicit full-path
writing fixture. Live client attachment, Native VFS block integration,
networking and Pi 5 controller assignment are not implemented by this baseline. QEMU does not qualify physical cache, interrupt or DMA behavior.

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
Native userspace VMM, services, and future compatibility supervisors
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

Read [the architecture guide](../kernel/docs/architecture.md) for the normative
boundary rules and migration constraints. The implemented Native contracts and
planned foreign-ABI boundary are specified separately in the [userspace and
syscall design](../kernel/docs/syscall-abi.md).

Kernel self-test images contain no Linux guest loader or default VM policy.
AArch64 and RISC-V Linux integration uses Native `vmm create/start/console` and
validates guest userspace startup, console input, named VM isolation and runtime-loss
retirement. RISC-V also retains a separate Native-authority guest fixture for
stop, WFI wakeup, virtual UART/PLIC, privilege isolation and retirement.
The x86-64 build gate does not establish a userspace guest-boot contract.

Pi 5 host bring-up prerequisites and recommended firmware configuration are
documented in [Raspberry Pi 5](../kernel/docs/rpi5.md). GICv2 Linux guest boot
and lifecycle tests run in QEMU; physical Pi 5 boot remains unverified.
The guest SMP acceptance target covers secondary CPU off/on cycles, concurrent
work, guest reboot/poweroff and reclamation on both GIC backends, including a
four-vCPU guest on a single-host-CPU configuration. This checks integration,
not physical cache, TLB or interrupt ordering.
