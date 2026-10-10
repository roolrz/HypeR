<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# I/O authority and DMA memory ownership

The implemented boundaries below improve authority separation and memory
lifetime. They do **not** establish complete isolation of the current Linux
I/O VM. Production assignment still lacks an IOMMU domain binding; the default
QEMU virtio-mmio devices bypass the PCI SMMU, and the Pi 5 deployment remains
trusted. Do not mark the security roadmap complete on the strength of the
standalone SMMUv3 fixture.

## Device operations

An assignment authority separates firmware inspection (`INSPECT`), physical
claims (`ASSIGN_DEVICE`) and VMO physical-address disclosure (`MAP_DMA`). An
inspection-only duplicate cannot acquire a device or inspect a VMO's physical
extent. Resolving a DMA extent also requires `READ|MAP` on the supplied VMO; it
does not install a DMA mapping or certify device quiescence.

Each successful claim owns a concrete PhysicalDevice kobject and its exact
firmware-derived resources. Binding it requires `ASSIGN_DEVICE`; userspace
register access requires `READ` or `WRITE`; IRQ sequence observation requires
`WAIT`; completion/rearm requires `ACK_INTERRUPT`. A register read can have
hardware side effects, so it remains an operation on an active assignment.

Init grants io-runtime the three authority operations explicitly in the service
manifest. A generic register/IRQ worker receives only the four operation rights
it needs on its PhysicalDevice. It cannot duplicate that handle or assign it to
another VM through that handle. Workers share io-runtime's process and handle
table, so attenuation is not a security boundary between those threads.
Linux receives mediated device resources, not a Native authority
handle. Existing handle attenuation, generational KOIDs, exact register-window
checks, exclusive resource claims and host-owned PCI/MSI mediation still apply.
See the [physical-device ABI](../../sdk/abi/docs/physical-devices.md).

## Backend memory lease

`BackendMemoryLease` is a kernel-only kobject, visible by kind/KOID to authorized
object inspection but never publishable as a userspace handle. Its initial
owner uses the existing `KernelService` reference class. Every cloned owner
retains the stable VMO pages and exclusive hardware-write lease. The object and
its page-address snapshot are charged to the creating resource domain.

Production live backend mappings now retain this lease. A future DMA mapping
can retain the same owner independently of the CPU mapping or source handles.
Closing a handle or removing a CPU mapping cannot release a page still held by
the DMA domain. Final destruction uses the ordinary object reaper and performs
no device register access, reset, or IOMMU command.

This ownership mechanism does not make a guest's RELEASE_MEMORY acknowledgement
a physical DMA fence. The current unprotected backend still relies on its
trusted quiescence protocol; active PCI/userspace assignments remain quarantined
when the kernel cannot prove complete retirement.

## SMMUv3 data and control memory

The driver distinguishes exclusive coherent control allocations (`DmaMemory`)
from retained guest/shared payloads (`DmaBuffer`). Only the former exposes a CPU
address for table and queue operations. The payload contract provides a stable
length and physical page addresses, allowing noncontiguous RAM without giving
the driver direct access to its bytes.

`map_buffer` validates the complete IOVA/physical range, excludes overlap and
allocates intermediate tables before publishing any leaf. A single owner covers
the complete buffer. A single completed domain invalidation covers publication.
An allocation failure may leave empty intermediate tables, which grant no DMA
access. A command failure after publication retains the buffer in the failed
controller; an error does not imply rollback.

`unmap_buffer` accepts the original mapping base and revokes the complete range.
It returns the owner only after TLBI and CMD_SYNC complete. A timeout retains the
owner even if leaf entries have already been cleared. Page helpers accept only
one-page buffers. Revoked IOVA reuse still requires the existing device and
stream lifecycle rules; this API is not a device reset.

The real QEMU PCI EDU fixture now maps VMO-backed leases after dropping their
original VMO/backing owners. It checks both DMA directions, host guard pages,
different domains, permissions, revocation/reassignment and fault/timeout
quarantine. Host fault-injection tests additionally cover scattered buffers,
cross-table ranges, overlapping mappings, allocation rollback and failed
revocation without premature owner release. They walk the actual stage-2 leaf
entries after partial allocation failure and an invalid final physical page,
and check that failed revocation clears leaves while retaining their payload
owners. See [SMMUv3](smmuv3.md).

## Pi 5 coverage and remaining integration

Host ownership now reserves every firmware translation provider identified by
`#iommu-cells`, including BCM2712 IOMMU controls, even when no driver is bound.
The catalogue's existing page/IRQ alias checks also exclude aliases of those
controls from generic userspace claims. Reserving controls is necessary but is
not evidence that every RP1 master passes through that controller.

Raspberry Pi's [Pi 5 device tree](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b.dts)
connects selected RP1 display/camera masters to `iommu5`; that association does
not establish Ethernet coverage. The [SoC device tree](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm64/boot/dts/broadcom/bcm2712.dtsi)
describes SDIO1/SDHCI separately and does not give it an `iommus` association.
No verified hardware containment for that path has been established here.
The [RP1 peripheral specification](https://datasheets.raspberrypi.com/rp1/rp1-peripherals.pdf)
must be read alongside the host bridge and SoC routing: a translation mechanism
covering selected masters cannot be assumed to cover the whole PCI function.

The current BCM2712 inbound DMA windows provide address translation over broad
RAM ranges. They have not been converted into per-owner isolation windows or
proven to supply the required retirement fence. No speculative programming of
those live physical windows or SDHCI DMA controls is included in this change.
SDHCI remains the explicit trusted exception; HypeR does not add an SD driver.

The remaining production work is to connect enumerated requester IDs and exact
shared-memory grants to owned domains before enabling bus mastering, serialize
live grant removal with completed DMA invalidation, and test faults through
actual assigned block/network devices. QEMU needs a physical PCI transport path
for those devices; enabling `iommu=smmuv3` alongside existing virtio-mmio is
insufficient. Pi 5 requires verified coverage and retirement for the applicable
bridge/IOMMU paths before any deployment can claim I/O VM containment.
