<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Roadmap

[Project overview](../README.md)

## Near-term priorities

The Pi 5 functional bring-up and initial I/O performance survey are complete.
The current phase focuses on **best-effort block and guest network performance
tuning** and **security hardening around the I/O VM**. Extended hardware
qualification is retained in the [deferred TODO](#deferred-hardware-qualification),
not a prerequisite for starting this phase.

### 1. Block and guest network performance

Improve the existing paths without major code or architecture changes. Use
focused profiling, tuning and local refactoring where measurements justify
them. Work on a best-effort basis, with no required Linux parity or fixed
throughput target; changes that need a substantial redesign belong in a
separate proposal.

- [ ] Investigate Native storage write throughput and synchronous-write latency,
  then reduce measured overhead in the existing block/VFS/backend path.
- [ ] Investigate guest network transmit throughput, then tune the existing
  virtio-net/vhost-net path and notification handling where useful.
- [ ] Check read performance and guest network receive performance for useful
  local improvements and regressions alongside those changes.
- [ ] Record before/after throughput and latency on physical Pi 5, comparing
  against native Raspbian with the manual storage and network tools. Keep
  correctness checks and disclose differences in filesystem, card state,
  workload and network setup. Do not add QEMU or CI performance thresholds.

The [2026-10-06 survey](../README.md#exploratory-pi-5-io-measurements) supplies
starting reference data, not a controlled benchmark or a diagnosis of the gaps.
Each optimization should have measured benefit and preserve existing I/O,
memory ownership and flush semantics. Report remaining gaps when further gains
would require major rework. Native networking remains outside this phase.

### 2. Capability and DMA isolation hardening

The goal is to protect HypeR's memory and authority from a faulty or compromised
I/O VM. Guests using its storage or network backend still depend on that VM:
this work does not promise guest I/O availability or trustworthy backend data
after the backend is compromised. The current Pi 5 deployment still trusts the
I/O VM; reserving HypeR RAM in Linux and restricting guest CPU mappings do not
provide physical DMA isolation.

- [ ] Refine capability scope and rights for device discovery, assignment,
  MMIO/configuration access, interrupt control, reset and DMA mappings. Enforce
  device/resource boundaries in the kernel and allow delegation only with the
  same or reduced authority. Audit grants from init through io-runtime to the
  I/O VM; board configuration alone is not an access-control boundary.
- [ ] Introduce host-owned DMA domains and SMMU/IOMMU drivers on supported
  hardware. Keep translation tables and isolation controls outside I/O VM
  access. Admit only authorized RAM and shared buffers, with supported access
  permissions, explicit mapping lifetimes and completed invalidation before
  reuse. Unsupported hardware must report its actual isolation limits.
  The [standard SMMUv3 mechanism and QEMU PCI DMA fixture](../kernel/docs/smmuv3.md)
  include stage-2 mappings, IRQ-driven stream quarantine, failure containment
  and fault injection. Assignment/lease integration and physical qualification
  remain open, so this broader item is not complete.
- [ ] Implement the applicable RP1/BCM2712 DMA isolation mechanisms after
  establishing their coverage and enforcement properties. Audit bus masters,
  address aliases, bypass routes, PCI configuration/BAR access and MSI targets;
  Linux must not be able to reprogram or bypass the host protection boundary.
- [ ] Assess SDIO1/SDHCI separately from RP1. If an assigned device cannot be
  contained by the available hardware, document that gap and select a different
  ownership/access arrangement before claiming full I/O VM DMA containment.
- [ ] Validate denied operations, restricted delegation and out-of-domain DMA
  on supported paths, including the mapping lifecycle. Keep focused correctness
  checks with each change; the extended fault and stress campaign below remains
  deferred.

Pi 5 hardware coverage must be established per path. Raspberry Pi's
[Pi 5 device tree](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b.dts)
associates `iommu5` with selected RP1 display/camera masters, and its Linux tree
contains a platform-specific
[BCM2712 IOMMU driver](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/drivers/iommu/bcm2712-iommu.c).
That does not establish protection for Ethernet, SDHCI or every DMA path exposed
by assigning the whole RP1 function. The plan must not assume Pi 5 has a generic
Arm SMMU covering all devices.

The limited-change constraint above applies to performance tuning. Security
hardening may require new drivers and changes to capability and assignment
contracts; containment claims require evidence for the covered paths.

## Completed foundation

- [x] Boot HypeR on Pi 5 D0 with four CPUs, timer-driven scheduling and the
  interrupt-driven debug UART; retain QEMU GICv2/GICv3 coverage.
- [x] Run the Linux I/O VM on Pi 5 and consume independently published,
  digest-pinned protocol version 3 appliances from
  [HypeR-io-vm](https://github.com/roolrz/HypeR-io-vm).
- [x] Provide board/guest device trees, RAM reservations and VM configuration;
  validate two-vCPU Alpine boot, console, CPU off/on, ordinary reboot with a
  synced file preserved, and poweroff on Pi 5.
- [x] Hand SDIO1 resources and the RP1 PCI function to Linux with resource
  validation and PCI/BAR/MSI mediation; keep physical device drivers in Linux.
- [x] Mount the SD-backed `/data` volume through virtio-scsi/vhost-scsi and
  integrate the Native block frontend with HypeR VFS/FAT.
- [x] Implement guest virtio-net with Linux vhost-net/TAP and independent
  storage/network reset epochs; reserve an inactive Native network endpoint.
- [x] Validate QEMU cross-VM disk I/O, guest DHCP and HTTP integrity, backend
  reset, VM restart and memory-release acknowledgements.
- [x] Verify 1 GiB Native FAT and guest ext4 writes/overwrites with full readback
  on Pi 5, exercise bidirectional guest TCP through RP1, and record initial
  comparisons with native Raspbian.

These establish basic end-to-end operation. The lifecycle tests predate the
combined SDIO1/RP1 deployment, while the 2026-10-06 I/O survey used that combined
deployment. Neither set of results closes the deferred qualification items.

## Architecture boundaries

- HypeR owns scheduling, memory, capabilities, VM lifecycle and device isolation.
  Native apps and service supervision remain first-class. Linux supplies the
  physical device-driver ecosystem as an I/O VM, not as the host kernel.
- Storage uses virtio-scsi and Linux vhost-scsi/LIO; guest networking uses
  virtio-net and Linux vhost-net/TAP. HypeR owns negotiation, shared-memory
  grants and event routing. Management services do not forward individual I/O
  requests.
- HypeR-io-vm owns the upstream Linux source lock, kernel configuration, external
  modules, Linux services, initramfs and publication with corresponding source
  materials. HypeR imports immutable package digests and owns Native apps,
  DTS/DTB and launch policy.
- Namespace, open-file semantics and cache policy stay in HypeR. Linux exports
  block I/O; it must not independently mount or modify a block range exclusively
  owned by HypeR.
- Submitted buffers remain owned until completion or proven device quiescence.
  A crashed VM does not prove physical DMA has stopped. Deferring qualification
  does not permit recycling possibly DMA-visible memory: uncertain retirement
  must retain affected memory rather than claim successful release.

## Deferred hardware qualification

These are open TODOs, deliberately outside the current phase. Existing manual
tools remain available; deferral does not mean the checks have passed.

- [ ] Power-loss durability and reboot persistence on the combined SDIO1/RP1
  deployment, with an external record of acknowledged writes.
- [ ] Backend failure, shutdown, recovery and physical DMA retirement: fence
  sessions, fail outstanding requests, establish device quiescence/reset before
  memory reuse, and reject stale completions after reconnect.
- [ ] Concurrent storage/network traffic under memory and queue pressure.
- [ ] Prolonged load and idle/wakeup stress, including timer/IPI wakeups and
  cross-core cache/TLB retirement on hardware.
- [ ] Extended qualification of the minimal Pi 5 appliance, repeatability,
  system-wide CPU consumption and controlled environment metadata.

See the [manual storage procedure](../tests/hardware/storage/README.md) and
[network exercise](../tests/hardware/network/README.md). Current measurements
remain exploratory and do not establish production readiness.

## Later work

- Native network APIs and a connection for the reserved Native endpoint;
- transactional multi-vCPU management, richer VM supervision and accounting;
- broader Native ABI/std coverage, ABI stabilization and generated bindings;
- additional filesystems, cache/writeback policy and storage recovery;
- scheduler load balancing, power management and CPU hotplug;
- broader hardware support while preserving existing AArch64/RISC-V acceptance
  and x86-64 builds.

See [Pi 5 bring-up](../kernel/docs/rpi5.md),
[implementation status](status.md), [I/O VM delivery and integration](io-vm.md),
and [VFS boundaries](../kernel/docs/vfs.md).
