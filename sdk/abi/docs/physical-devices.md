<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Userspace physical-device assignments

Profiles identify the admitted transport: virtio-mmio SCSI (1), a generic
userspace-controlled resource bundle (2), virtio-mmio network (3), or a mediated
PCI function (4). The kernel does not implement SDHCI, clock, GPIO or network
controller protocols for the generic bundle.

The 32-byte profile record contains `profile` at offset 0, `interrupt_count` at
4, `resource_count` at 8, `pci_identity` at 12, `dma_bus_offset` at 16 and
`aperture_size` at 24. Non-PCI profiles have zero PCI identity/DMA offset and a
64 KiB aperture. Userspace bundles have zero or one physical IRQ, but assignment
reserves one guest IRQ even when the physical IRQ count is zero.

## Capability boundaries

| Object | Operation | Required right |
| --- | --- | --- |
| DeviceAssignmentAuthority | Firmware inspection | `INSPECT` |
| DeviceAssignmentAuthority | Claim by index, match or resource bundle | `ASSIGN_DEVICE` |
| DeviceAssignmentAuthority | Inspect a resident VMO's DMA extent | `MAP_DMA` (also `READ\|MAP` on the VMO) |
| PhysicalDevice | Metadata and resource descriptions | `INSPECT` |
| PhysicalDevice | Bind to a pending VM | `ASSIGN_DEVICE` (also `WRITE` on the VM) |
| PhysicalDevice | Read / write a userspace register | `READ` / `WRITE` |
| PhysicalDevice | Inspect a pending IRQ sequence | `WAIT` |
| PhysicalDevice | Acknowledge/rearm or assert the guest IRQ | `ACK_INTERRUPT` |

Claim-by-index/match returns `TRANSFER|DUPLICATE|INSPECT|ASSIGN_DEVICE`.
A userspace resource bundle additionally grants `READ|WRITE|WAIT|ACK_INTERRUPT`.
Delegation can only preserve or reduce rights. An `INSPECT` duplicate cannot
claim hardware, attach a device, query VMO physical addresses or operate an IRQ.
Register reads may have side effects: `READ` is device operation authority and
still requires an active assignment. These rights do not establish DMA isolation.

Init retains root discovery/assignment/address authority and grants precisely
`INSPECT|ASSIGN_DEVICE|MAP_DMA` to io-runtime. Its register/IRQ worker receives only
`READ|WRITE|WAIT|ACK_INTERRUPT` on the already claimed device, with no assignment or
further delegation permission. The Linux I/O VM receives mediated resources,
not a Native assignment-authority handle.

## Firmware and register access

The existing `physical_device_info` record reports `device_id = 0` and
`transport_version = 0` for a userspace-managed bundle, whose register protocol
the kernel does not identify. Its `mmio_size` remains nonzero. Virtio assignments
continue to report their device identifier and transport version.

`device_firmware_read` (143) borrows an assignment authority and a 24-byte query:
`node: u32`, `field: u32`, `name_address: u64`, `name_length: u64`. Node indices
start at zero and are contiguous; `NOT_FOUND` ends enumeration. Fields are:

| Field | Result |
| --- | --- |
| 0 | 32-byte info: flags, register count, physical IRQ number, IRQ trigger (four u32), then two reserved zero u64 |
| 1 | Full FDT path, UTF-8 without trailing NUL |
| 2 | Original NUL-separated compatible strings |
| 3 | Translated register ranges, little-endian u64 base/length pairs |
| 4 | Named raw FDT property bytes |
| 5 | NUL-separated names of all immutable firmware properties |

Info flag bit 0 means kernel-owned; other bits are reserved. IRQ trigger is zero
for absent, 1 for level, or 2 for edge. Only field 4 accepts a name; it must be
nonempty UTF-8 without NUL, at most 128 bytes. Other queries use zero name length.
Missing nodes/properties return `NOT_FOUND`. Capacity zero queries the required
size in result value 0. Nonzero capacity copies the whole field or fails; output
is bounded to 64 KiB. No physical addresses supplied by applications authorize
access: firmware inspection and resource claims are separate operations.

`device_claim_bundle` (144) takes up to eight 16-byte entries (`node: u32`,
`resource: u32`, guest-aperture `offset: u64`) and an IRQ node index. The kernel
checks exact firmware resources, overlap, kernel ownership, existing claims,
and the 64 KiB aperture. This version supports exclusive level-triggered SPIs.
The returned PhysicalDevice has the userspace bundle rights listed above.

After installing a generic assignment, the VM owner registers its exact 64 KiB
assigned aperture with `virtual_machine_register_mmio` before starting any vCPU.
This permission belongs only to that VM's assignment; it does not enlarge the
platform's ordinary userspace MMIO window or expose legacy kernel-handled devices.

`device_mmio` (145) accepts device, offset, width, operation, and value. Widths
are 1, 2, or 4 bytes. Operation 0 reads and requires value zero; operation 1 writes.
Both reads and writes require an active assignment with retained DMA backing:
even a register read can have device-specific side effects. Capability checks
therefore occur after installation and before starting the guest. Kernel validation
covers resource bounds, alignment, rights and lifecycle; register semantics belong
to userspace.

The PhysicalDevice remains the authority for its existing bound VM/SPI route.
`device_irq_pending` (146) returns zero when idle, otherwise a nonzero monotonic
sequence. READABLE (bit 0) indicates a newly delivered notification. Physical
delivery masks the source; notification publication and the final
hardware mask belong to one IRQ registry transaction. Rearm is serialized after
that transaction, so userspace acknowledgement cannot be overwritten by a late
delivery mask. `device_irq_complete` (147) validates
the sequence and sets the guest IRQ level. Asserted=true keeps the physical source
masked and its sequence pending, but consumes READABLE to prevent a busy loop.
Userspace can query and reuse that sequence after a guest register write.
Asserted=false clears the pending record and
rearms the source. Sequence zero is accepted only while no IRQ is pending, for
register-write resampling. A stale sequence returns `BUSY`; reread pending state.

Neither IRQ acknowledgement nor a userspace reset report proves DMA quiescence.
Without a kernel-verifiable isolation/reset boundary, active userspace assignments
remain quarantined on teardown and retain their DMA backing. Removing CPU access
or stopping the driver does not permit releasing pages still reachable by DMA.
