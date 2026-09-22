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
shared memory and event-driven notifications. Storage uses standard virtio-scsi
with Linux vhost-scsi/LIO. QEMU remains the regression platform. Hardware
bring-up is part of this milestone, not a follow-up after the backend is done.

### Architecture decisions

- **Linux is a trusted I/O VM.** It supplies the device-driver ecosystem, while
  HypeR remains the host kernel and owns scheduling, memory, capabilities and VM
  lifecycle.
- **Physical devices are driven directly by Linux.** Device assignment must
  describe MMIO, interrupts, DMA addressing, firmware dependencies and reset
  ownership. An emulated device backed by a host driver is not this milestone.
- **Use standard storage queues.** Modern virtio-mmio and virtio-scsi define
  the guest interface; Linux vhost-scsi/LIO consumes the shared virtqueues.
  HypeR owns device negotiation, memory grants and event routing. Cross-VM
  setup must not forward individual requests through management services.
  Define version negotiation, buffer grants and event-driven notification.
  Design the data path for zero-copy where ownership, alignment and DMA rules
  allow it; measure remaining copies. A copy-first transport is not a required
  architectural stage.
- **Deliver Linux as an independent appliance.**
  [HypeR-io-vm](https://github.com/roolrz/HypeR-io-vm) owns the upstream LTS
  source lock, kernel configuration, external `.ko` drivers, Linux-side
  services, complete initramfs, tests and GHCR publication. Linux stays
  unmodified upstream. HypeR owns Native apps, DTS/DTB and launch policy;
  it imports a fixed package digest and does not build or patch the Linux rootfs.
  Each release carries matching source materials and notices.
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

Native apps and services remain first-class throughout this work, including
independently deployed power and resource-policy services. The I/O VM supplies
drivers, not a replacement for the Native application runtime. Keep existing
app/std functionality and service supervision covered while adding backend
capabilities; new privileged power operations require explicit Native authority.

### Ordered milestones and to-do

1. **Bring up the HypeR host on Pi 5.**
   - [x] Implement host GICv2, adapt the dedicated PL011 debug UART, and remove
     QEMU-specific early RAM/MMIO mapping assumptions.
   - [x] Add QEMU GICv2 UP/SMP acceptance and document the official EEPROM/PSCI boot setup.
   - [x] Validate physical boot, four online CPUs, timer-driven scheduling
     and interrupt-driven debug-UART input on Pi 5 D0.
   - [ ] Qualify prolonged idle/wakeup and load stress on hardware.
2. **Boot the Linux I/O VM on Pi 5.**
   - [x] Implement the GICv2 guest interrupt backend and its QEMU lifecycle tests.
   - [x] Implement Arm guest SMP (1..8 CPUs) and runtime-mediated PSCI CPU
     on/off, poweroff and reset; add GICv2/GICv3 QEMU acceptance.
   - [ ] Qualify the minimal upstream LTS configuration for Pi 5 in HypeR-io-vm.
   - [x] Publish the common appliance with corresponding source materials.
   - [x] Release the separate Pi 5 build profile after Linux CI qualification.
   - [x] Pin the common package and integrate Native deployment.
   - [ ] Adopt the separate Pi 5 package by immutable digest.
   - [x] Provide board/guest device trees, RAM reservations and VM configuration
     through the existing VMM and vm-runtime path.
   - [x] Validate two-vCPU Alpine boot and console, three CPU off/on cycles,
     ordinary reboot and poweroff on Pi 5 hardware.
   - [ ] Stress timer/IPI wakeups and cross-core cache/TLB retirement on hardware.
3. **Connect physical network and storage devices.**
   - [ ] Inventory the selected controllers' MMIO, IRQ, DMA, clock/reset and
     firmware dependencies; assign each resource one owner.
   - [ ] Add the required Native device/memory authorities and Linux handoff.
   - [ ] Verify direct network and storage operation inside the I/O VM, with
     HypeR-owned RAM excluded from the guest allocator.
4. **Expose I/O to Native clients.**
   - [x] Validate the AArch64 QEMU cross-VM virtio-scsi/vhost-scsi baseline,
     including a real disk, DMA translations, reset/rebind and VM retirement.
   - [x] Integrate storage with ordinary Native service deployment and mount
     the Pi 5 SD-backed configuration volume at `/data`; directory reads pass.
   - [ ] Qualify SD writes and persistence across reboot.
   - [ ] Define and implement the virtio-net frontend/backend contract; the
     storage choice does not by itself complete the network design.
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
[implementation status](status.md), [I/O VM delivery and integration](io-vm.md),
and [VFS boundaries](../kernel/docs/vfs.md).

## Later work

These remain directions rather than prerequisites for the Pi 5 milestone:

- IOMMU-backed device isolation and untrusted driver domains;
- transactional multi-vCPU management, richer VM supervision and accounting;
- broader Native ABI/std coverage, ABI stabilization and generated bindings;
- additional filesystems, cache/writeback policy and storage recovery;
- scheduler load balancing, power management and CPU hotplug;
- broader hardware support while preserving existing AArch64/RISC-V acceptance
  and x86-64 builds.
