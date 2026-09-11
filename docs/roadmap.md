<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Roadmap

[Project overview](../README.md)

## Current goal: a Linux I/O backend on Raspberry Pi 5

The near-term milestone is an end-to-end system on **physical Raspberry Pi 5**:
HypeR runs Native applications and a trimmed Linux I/O VM; Linux drives the
physical network and storage devices and exports their services to HypeR over
an explicit native protocol. QEMU remains the regression platform. Hardware
bring-up is part of this milestone, not a follow-up after the backend is done.

### Architecture decisions

- **Linux is a trusted I/O VM.** It supplies the device-driver ecosystem, while
  HypeR remains the host kernel and owns scheduling, memory, capabilities and VM
  lifecycle. It is separate from any future foreign-ABI compatibility service.
- **Physical devices are driven directly by Linux.** Device assignment must
  describe MMIO, interrupts, DMA addressing, firmware dependencies and reset
  ownership. An emulated device backed by a host driver is not this milestone.
- **Use an explicit shared-memory protocol.** Define bounded request/completion
  queues, version negotiation, buffer grants and event-driven notification.
  Design the data path for zero-copy where ownership, alignment and DMA rules
  allow it; measure remaining copies. A copy-first transport is not a required
  architectural stage.
- **Memory ownership is explicit.** A submitted buffer remains owned by the
  in-flight operation until completion or proven device quiescence. Define
  cache visibility, barriers, cancellation and queue generations before reuse.
  A crashed VM does not prove that physical DMA has stopped.
- **Pi 5 initially uses a trusted-driver model.** Exclude HypeR-owned RAM from
  Linux's allocatable memory and reserve shared/device-visible ranges explicitly.
  Retain stage-2 CPU access controls. Reservation is an allocation contract, not
  DMA isolation; this milestone does not claim containment of malicious Linux.
  IOMMU-backed untrusted device domains remain a later hardening goal.
- **Keep the VFS boundary intact.** Namespace, open-file semantics and cache
  policy stay in HypeR; ramfs remains entirely in the kernel. Linux exports block
  I/O rather than taking ownership of HypeR's VFS. Give HypeR exclusive ownership
  of each exported block range; Linux must not independently mount or modify it.
  Define completion/flush durability and cache invalidation across reconnects.

### Ordered milestones and to-do

1. **Bring up the HypeR host on Pi 5.**
   - [x] Implement host GICv2, adapt the dedicated PL011 debug UART, and remove
     QEMU-specific early RAM/MMIO mapping assumptions.
   - [x] Add QEMU GICv2 UP/SMP acceptance and document a TF-A/PSCI boot setup.
   - [ ] Validate physical boot, four online CPUs, timer-driven scheduling,
     interrupt-driven serial input and idle wakeup.
2. **Boot the Linux I/O VM on Pi 5.**
   - [ ] Implement the GICv2 guest interrupt backend and its lifecycle tests.
   - [ ] Build a minimal Linux configuration and reproducible guest artifacts.
   - [ ] Provide board/guest device trees, RAM reservations and VM configuration
     through the existing VMM and vm-runtime path.
   - [ ] Validate guest boot, console, timer wakeups and shutdown on hardware.
3. **Connect physical network and storage devices.**
   - [ ] Inventory the selected controllers' MMIO, IRQ, DMA, clock/reset and
     firmware dependencies; assign each resource one owner.
   - [ ] Add the required Native device/memory authorities and Linux handoff.
   - [ ] Verify direct network and storage operation inside the I/O VM, with
     HypeR-owned RAM excluded from the guest allocator.
4. **Expose I/O to Native clients.**
   - [ ] Specify the shared queue ABI, operations, buffer ownership, errors,
     backpressure, ordering and event notification.
   - [ ] Implement the Linux backend and HypeR Native frontend services/adapters.
   - [ ] Integrate block I/O at the VFS backend boundary and provide the Native
     network APIs needed by the first end-to-end applications.
   - [ ] Prove teardown: fence sessions, fail outstanding requests, quiesce DMA
     before releasing memory, and prevent stale completions after reconnect.
5. **Qualify the complete system on physical Pi 5.**
   - [ ] Run Native network traffic and block read/write with data verification,
     then concurrent I/O under memory and queue pressure.
   - [ ] Test backend failure and shutdown. Restart is allowed only after device
     quiescence/reset is established; otherwise keep affected memory pinned and
     require recovery rather than recycling potentially DMA-visible pages.
   - [ ] Record throughput, latency, CPU consumption and idle behavior alongside
     reproducible build, deployment and test instructions.

The milestone is complete when Native clients use both physical network and
storage through the Linux VM on Pi 5, correctness and failure tests pass, and
measured results can be reproduced. A Linux boot banner or QEMU-only I/O is not
sufficient. This is a development milestone, not a production-readiness claim.

See [Pi 5 bring-up](../kernel/docs/rpi5.md),
[implementation status](status.md), and [VFS boundaries](../kernel/docs/vfs.md).

## Later work

These remain directions rather than prerequisites for the Pi 5 milestone:

- IOMMU-backed device isolation and untrusted driver domains;
- transactional multi-vCPU management, richer VM supervision and accounting;
- broader Native ABI/std coverage, ABI stabilization and generated bindings;
- additional filesystems, cache/writeback policy and storage recovery;
- scheduler load balancing, power management and CPU hotplug;
- broader hardware support while preserving existing AArch64/RISC-V acceptance
  and x86-64 builds;
- supervised Linux/FreeBSD binary personalities, separate from the I/O VM.
