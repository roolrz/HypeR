<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Linux I/O VM

Keeping an independent I/O VM and preferring device drivers outside the HypeR
kernel is an [architectural decision](../kernel/docs/architecture.md#device-driver-placement),
not a temporary bring-up arrangement. Native userspace drivers are another
supported placement; applications consume their services through controlled
interfaces.

The I/O VM is a trusted Linux service VM. HypeR owns the Native applications,
virtual hardware, launch policy, and authoritative DTS/DTB. The separate
[HypeR-io-vm repository](https://github.com/roolrz/HypeR-io-vm) owns everything
needed to produce the Linux appliance: pinned upstream Linux, configuration,
external modules, Linux userspace services, initramfs assembly, tests, and
package publication. This repository does not build Linux or modify appliance binaries. Board
deployment adds a separate configuration archive to the Linux initramfs.

The AArch64 QEMU baseline runs two Linux VMs: the I/O VM directly drives a
QEMU virtio-scsi disk, and a business VM reaches it through standard virtio-scsi
and upstream vhost-scsi/LIO. A Native test deployment owns both VMs, their DTBs,
shared memory and control transactions. It verifies disk contents independently
on the host and checks DMA memory admission after both VMs retire.
See [implementation status](status.md#linux-io-vm-baseline) for the acceptance
boundary. Pi 5 device assignment has basic storage/network measurements;
extended hardware qualification remains deferred.
The board deployment path is described in [board storage](board-storage.md).

## Selected backend interfaces

The I/O VM architecture selects three interface families:

| Interface | Purpose | Current status |
| --- | --- | --- |
| virtio-scsi | Storage for Native clients and guest disks | Implemented with vhost-scsi/LIO |
| virtio-net | Guest network connectivity | Implemented with vhost-net/TAP; QEMU GICv2/GICv3 acceptance and an initial Pi 5 TCP survey completed |
| vfio-user | General device backends beyond the storage and network interfaces | Planned; cross-VM integration remains to be designed |

Storage and guest networking have implementations; vfio-user remains a design
direction. Other device models are assessed against actual requirements;
there is no promise of universal support. Native userspace services remain an
option for devices that do not fit these interfaces.

The third family is **vfio-user**, not virtio-user or vhost-user. Its integration
must define device presentation, cross-VM transport, memory authorization,
interrupts, DMA and reset/retirement. The existing storage bridge is not a
vfio-user implementation, and this decision does not claim wire compatibility
or automatic physical-device passthrough.

## Resident startup

`make run` on AArch64 uses the QEMU board configuration, keeps the HypeR shell,
and starts `/svc/io-runtime` from the generated service manifest. Init explicitly delegates the physical-device
capability only to this service and charges its VM to the bounded VM fleet.
The service owns the VM, assigned physical controllers and control mailboxes. Guest power-off,
control failure or runtime exit initiates VM retirement; failed physical reset
continues to use the kernel's existing quarantine semantics. There is no
automatic restart of an uncertain device owner.

The board profile prepares dm-linear volumes and mounts HypeR's configuration
volume at `/data` before init provisions the configured business VMs. Business
VMs are started only when their configuration requests autostart or through
`vmm start`. `make run RUN_PROFILE=io` retains the explicit standby diagnostic
profile: it boots a resident backend without mounting Native storage or
provisioning business clients.

Building the default board profile creates `target/board/qemu/disk.img` once
and preserves existing disks. `make run` validates the disk's GPT against the
selected board configuration, then launches the existing kernel, ramdisk and
disk. `BOARD_IMAGE` selects another board disk. The diagnostic `RUN_PROFILE=io` instead creates
`target/app/aarch64/io-disk.img`; `IO_VM_DISK` overrides that diagnostic disk.
`RUN_PROFILE=native` selects the previous boot profile. `IO_VM_PACKAGE` can override the downloaded
package with a complete, locally qualified boot generation. The importer also
accepts `IO_VM_REFERENCE` for an explicit digest and `IO_VM_ORAS` for ORAS.

`make test-io-standby` verifies backend readiness, slow interactive shell input,
continued idle operation and an unchanged, preexisting physical disk. The
`test-io-vm` acceptance remains separate and deliberately writes its own
unique disposable disk through the full two-VM virtio-scsi path.

## Repository and package ownership

| HypeR | HypeR-io-vm |
| --- | --- |
| Native apps, VM lifecycle, grants, notifications | Upstream Linux build and out-of-tree drivers |
| Guest virtio device models and negotiation | Linux-side services, vhost-scsi/LIO and vhost-net/TAP |
| DTS/DTB and device assignment policy | Kernel configuration and complete Linux initramfs |
| Digest-pinned package download and validation | GHCR package publication and corresponding source materials |
| Hyper integration and board tests | Linux appliance build and backend tests |

The Linux service is not a Native app or a member of the Native app workspace.
Linux ioctl and kernel-module ABI details stay in the I/O VM repository.
HypeR keeps its Apache-2.0 source license. Linux and its modules retain their
own licenses; distributing their binaries requires the corresponding source
materials and notices regardless of which repository contains the build scripts.

The delivery boundary is an OCI artifact in GitHub Container Registry (GHCR),
not a container to execute. A boot package contains `Image`, a complete
`initramfs.cpio.gz`, a versioned manifest, and corresponding source materials.
HypeR downloads the runtime payload by immutable `sha256` digest; tags are
human-facing release names and must never select an implicit moving upgrade.
Source materials remain available with the same package generation, but are
not included in the HypeR boot ramdisk.

Release adoption requires an explicit digest update. An incompatible package format, wrong
architecture/platform, missing artifact, or failed checksum aborts import.
Download and verification finish before a new generation becomes visible;
failed updates leave the previously selected generation intact.
Separate QEMU and Pi 5 AArch64 appliances are published as prereleases in GHCR.
Each entry in [io-vm.lock.json](../scripts/io-vm.lock.json) pins its platform's
immutable reference, source revision and successful qualification/publication
runs. Platform metadata identifies the build profile; it does not certify Pi
hardware operation. The development image with the combined SDIO1/RP1
deployment passed a physical Pi 5 smoke test before publication. The published
Pi 5 pin was then used for the 2026-10-06 physical storage and guest TCP
[performance survey](../README.md#exploratory-pi-5-io-measurements). This is basic
functional/reference evidence, not full stress, durability and DMA retirement
qualification; the lock entry therefore retains `hardware_qualified: false`.
Final boot images follow the [image release contract](image-distribution.md).
The import command also accepts `IO_VM_REFERENCE` explicitly. The qualified QEMU
fixture consumes the imported generation. Without an override, the ordinary
AArch64 `make` build also selects the committed protocol version 3 lock.
No local appliance checkout or package override is required. `make run` starts the artifacts from the
last build and does not fetch or rebuild the appliance.

The I/O VM repository owns the pinned upstream LTS version and builds each
external module against the exact kernel configuration and release. HypeR
consumes the resulting coherent image and does not independently replace a
module. Bridge protocol compatibility is negotiated separately from Linux's
internal module ABI.

DTBs are generated by HypeR from admitted hardware and mapping facts. A Linux
package must not supply the authoritative DTB. A host DTB must not be copied
wholesale into a VM: that would advertise resources the VM does not own.
`hyper-vm-image` remains the owner of the runtime-generated guest DTB.

## Board-managed clients

`make board-run BOARD=qemu` uses the board-generated client table. Client zero
is HypeR's configuration volume, mounted by the kernel at `/data`; init waits
for an explicit readiness record before opening `/data/vms.json`. This is also
the default AArch64 `make run` profile.

A VM definition may include `"disk": {"client": 1, "volume": "alpine"}` and/or
`"network": {"client": 1, "network": "default", "mac": "02:48:59:00:00:01"}`.
Both devices use the same client ID. The manager creates one capability session
for the runtime; the I/O runtime checks its complete device policy against
`/etc/hyper/io-clients.conf`, generated from the same board JSON as the Linux
client, volume and network tables. Duplicate active bindings are refused.
The board source assigns client IDs when it generates the runtime definitions;
see [the QEMU board](../boards/qemu.json) for the source format.
`vmm create NAME --config FILE` reads disk/network assignments and runtime settings from
the named definition in the configuration file; `--from SOURCE` selects a template.

The resident I/O VM has 128 MiB of ordinary RAM for Linux, services and
vhost queue allocations; shared business-guest pages are additional mappings,
not allocator memory for Linux. The isolated two-VM fixture below retains its
64 MiB per-VM layout.

The fleet has a finite allowance for eight business VMs plus the resident I/O
VM and manager overhead. Physical memory admission remains fallible; the quota
does not reserve backing RAM in advance.

The I/O runtime owns one control mailbox per client and a separate notification
capability for each configured device. The
per-VM runtime handles virtio configuration MMIO and exchanges only negotiation
and notification enable/disable commands over its dedicated channel. Virtqueue
kicks, completions, disk data and packet buffers use direct kernel routes and
shared pages. Neither Native service relays individual I/O requests or packets.

Normal guest RAM remains noncontiguous. A single guest-memory grant is mapped
into the frontend and admitted into the I/O VM using an immutable, token-scoped
extent table. Dynamic aliases use the fixed physical-address translation;
static I/O RAM and the configuration client's ranges are excluded from that
translation to avoid ambiguous DMA reverse mappings.

Closing a runtime session revokes its connection: disable new notifications,
reset and drain every configured vhost endpoint, release the Linux mappings, obtain the kernel's
quiescence proof, then remove the old notification routes. A new binding gets
a new generation. Any failure to establish quiescence stops the I/O VM and
retains uncertain DMA storage until actual device retirement. A timeout alone
never authorizes memory reuse.

## Device protocol and control ownership

The guest-facing devices use modern virtio-mmio with standard virtio-scsi and
virtio-net. Linux vhost-scsi/LIO and vhost-net consume the shared virtqueues.
Neither vm-runtime nor the Linux management service forwards individual disk
requests or packets.

Configuration MMIO uses a bounded per-vCPU request, published after hardware
detach and completed through the owning vCPU capability. Reading a request is
not acknowledgement. Completion matches the exact generation and advances the
instruction exactly once. Administrative stop cancels the continuation.
Only registered device ranges may use this path; unknown MMIO retains its
diagnostic terminal disposition.

Queue kicks and completion interrupt status/acknowledgement use prevalidated
kernel notification bindings. They do not enter the userspace device loop.
Durable pending state and virtqueue indices are authoritative; a physical IPI
is only a prompt to a remote CPU. Idle backends block, and notification arming
must recheck pending work to avoid lost wakeups.

Virtio features are the intersection of the guest transport, bridge, and Linux
backend capabilities. Queue activation commits only after mappings and backend
configuration have succeeded. Device reset first disables new submissions,
quiesces the backend, and then releases resources. Linux vhost ioctl numbers
remain Linux-local: the cross-VM bridge is not an ioctl forwarding ABI.

A notification binding exposes a Native `PEER_CLOSED` signal when either VM
retires. The surviving guest can still read and acknowledge its transport
interrupt state; losing the backend does not turn those accesses into terminal
MMIO faults. The runtime backend API can mark the device `DEVICE_NEEDS_RESET`
and raise a configuration-change interrupt. The current acceptance fixture
stops both VMs on backend failure instead of keeping the business guest running.
A closed binding cannot be enabled
again, and late backend completions cannot reactivate it. Queue descriptions
and pending transactions remain retained until independent CPU/DMA quiescence
has been established; failure notification is not permission to release pages.

The AArch64 reference controller exposes 256 interrupt IDs on GICv2 and GICv3.
Assigned physical devices and I/O notification/control endpoints admit shared
interrupt IDs 40 through 255, with collision checks against existing routes.
The I/O VM may own both storage and network controllers; activation rollback
and final retirement must quiesce every assigned controller before RAM reuse.

## Guest networking

The QEMU path is guest virtio-net → Linux vhost-net/TAP → Linux bridge → assigned
virtio-net uplink → QEMU user networking. The bridge has no IP address; QEMU
supplies DHCP and NAT. Native HypeR does not establish a network link. Client
zero remains reserved for future Native networking, but no Native network
frontend, TAP or network notification binding is created.

Appliance startup creates the configured bridge (by default `hbr0`), attaches
the uplink and brings both interfaces up. Each guest gets its own nonpersistent
TAP when its virtio-net queues activate; reset or retirement removes that TAP
after vhost drains. Guests on the same bridge share a layer-2 segment. QEMU's
NAT sits beyond the uplink; the I/O VM does not provide DHCP or IP routing.

The board's `io-vm.network-device` selects the physical controller and
`io-vm.networks` names the bridge/uplink. Each business VM's optional `network`
entry selects a network and stable locally administered unicast MAC. Disk and
network are independent options: a network-only VM does not receive a hidden
storage endpoint or LIO target. The shared Ethernet segment does not provide
anti-spoofing or per-VM network isolation policy.

The guest device uses modern virtio-mmio, RX queue 0 and TX queue 1, with at most
128 descriptors per queue, with MTU 1500. It offers `VIRTIO_F_VERSION_1` and
`VIRTIO_NET_F_MAC`. Guest TX checksum completion (`VIRTIO_NET_F_CSUM`) and TCP
segmentation (`VIRTIO_NET_F_HOST_TSO4/HOST_TSO6`) are offered only when the
backend's NETWORK_HELLO advertises them; TSO additionally requires CSUM in the
guest's negotiated features. RX offloads, mergeable RX buffers, a control
virtqueue and multiqueue are not offered.

Modern virtio uses a 12-byte packet header even without mergeable RX buffers.
An offload-capable backend preserves that header through vhost-net to an
`IFF_VNET_HDR` TAP, with a 12-byte little-endian header. TAP converts guest TX
metadata into Linux checksum/GSO state; the uplink can complete it in hardware
or Linux can fall back to software. `VHOST_NET_F_VIRTIO_NET_HDR` stays clear in
this mode. TAP's `TUNSETOFFLOAD` flags remain zero: those describe packets sent
*to* the guest, whose receive path still requires ordinary MTU-sized frames.

The pinned QEMU and Pi 5 appliances support these optional TX offloads. Ordinary
builds import their immutable upstream OCI digests from `scripts/io-vm.lock.json`;
no local appliance override is required. The pinned Linux build also orders vhost
work-node reuse before concurrent requeue, preventing pending work from being
lost. Older backends and guests may still negotiate VERSION_1 and MAC alone.

Each client has one memory owner, binding identity and control transaction
sequence. Storage and network have independent queue epochs and can reset
independently. The primary endpoint is storage when present, otherwise network;
its HELLO admits the binding and its epoch labels PREPARE_MEMORY/RELEASE_MEMORY.
NETWORK_HELLO returns 80 bytes, including MAC and MTU; NETWORK_ACTIVATE uses two
queue records in a 112-byte request. Only the shared memory owner may acknowledge
RELEASE_MEMORY, after both backends have drained. A failed drain retains the
mapping and withholds the retirement proof.

The Pi 5 profile uses the same guest/frontend/backend contract with RP1 GEM as
the physical uplink. `boards/rpi5.json` combines SDIO1 storage with RP1
Ethernet. The Linux bridge forwards guest MAC addresses onto the attached LAN;
the LAN supplies DHCP and routing. Neither Native HypeR nor the I/O VM bridge
has an IP address.

The assignment unit is the complete RP1 PCI function (`pci-function`, selected
by PCI vendor/device ID `1de4:0001`). HypeR owns the BCM2712 host bridge,
firmware-initialized PCI windows, exclusive BAR ownership and physical MSI
routing. Linux sees a generic ECAM host bridge, all admitted BARs and a GICv2m
MSI frame. PCI configuration and MSI-X table accesses are mediated: BAR probes
and relocations affect only the guest view, and messages can target only the
assignment's guest SPI range. Linux owns RP1's interrupt controller, clocks,
GPIO, GEM, PHY and other enabled function devices. Native projects the firmware
subtree and its admitted dependencies without operating RP1 function registers.

RP1 uses a separate noncoherent DMA bus with a 64 GiB bus-address offset;
SDHCI retains its own DMA translation. The current PCI host transport admits one
firmware-initialized, single-function endpoint with memory BARs and MSI-X.
It has no IOMMU isolation and is intended only for the trusted I/O VM.
Whole BARs can include vendor aliases for controller configuration; standard
PCI mediation alone does not contain a malicious function driver. RP1
peripherals share that owner and cannot be assigned independently. Stopping an
activated I/O VM quarantines its physical devices and memory; clearing PCI bus
mastering is not a DMA-drain proof. Ordinary business-VM stop/start still uses
the shared backend retirement protocol.

See the [Pi 5 Ethernet bring-up procedure](../kernel/docs/rpi5.md#ethernet-backend-qualification).
Host tests and QEMU acceptance do not qualify the physical RP1 link, cache
maintenance or interrupt ordering. The physical TCP survey records basic
operation; sustained traffic, concurrent I/O and fault coverage remain
[deferred qualification](roadmap.md#deferred-hardware-qualification).
The [manual network tools](../tests/hardware/network/README.md) are opt-in guest
additions and do not change the default Alpine rootfs or I/O VM appliance.

## Shared pages and DMA

Direct queue consumption requires Linux to access queue metadata, responses,
and every guest data buffer referenced by descriptors. The trusted deployment
currently grants the entire RAM VMO of each business VM with an I/O-backed disk
or network device. Both devices share one grant and Linux mapping.
The Native configuration-volume client
uses a dedicated 1 MiB shared I/O pool. Bridge protocol version 3 retains six storage
split queues: control, event, and four request queues. The Native client submits
up to four 128 KiB reads before issuing one combined notification. Each request
queue owns its descriptor chain and data buffer until completion. Writes and
flushes remain ordered by the device session. Unretired requests poison the
session on failure; their memory lease remains retained until backend quiescence.
The rest of HypeR memory is not exported.

### Allocation and zero-copy scope

The business disk path shares the original guest pages: Linux vhost-scsi reads
virtqueue descriptors and accesses payload buffers through an I/O VM mapping
of those same physical pages. HypeR and vm-runtime do not copy each disk payload
through an intermediate buffer. This is cross-VM zero-copy, not a guarantee
that every Linux block driver or physical device avoids bounce buffers.

Guest RAM starts as a sparse VMO. However, admitting that VMO to the I/O VM
currently populates **every page**, before the backend starts serving requests.
`Mapping::prepare` in `kernel/src/kernel/vm/memory/live.rs` walks the full VMO,
calls `populate_page`, obtains physical addresses and constructs the shared
extents. The current alias address is derived from the host physical address,
so the complete extent description requires physical pages to exist first.
Thus attaching a disk or network device removes the memory-saving benefit of demand allocation
for that guest, even if stage-2 entries are subsequently installed on faults.
Guests without this backend admission can retain sparse backing; their actual
resident footprint still depends on image loading and guest accesses.

The resident I/O VM has a separate allocation policy: its own 128 MiB RAM is
allocated eagerly as one physically contiguous VMO for the current physical
device DMA layout. The Native configuration client also eagerly allocates its
1 MiB pool. Imported business-guest pages are shared data, not free RAM that
Linux can use for its own allocator or vhost metadata.

On AArch64, Stage-2 keeps the 4 KiB translation granule but can install a
2 MiB block for an aligned, physically contiguous range of already-resident
RAM. Primary guest RAM must belong to one backing region, have no previously
installed page mappings in that block, and have uniform instruction-publication
state. Dynamic I/O aliases must fit wholly inside one admitted grant extent.
Preallocation alone does not guarantee physical contiguity: the I/O VM's
contiguous RAM benefits directly, while scattered business-guest backing falls
back to 4 KiB mappings. The optimization does not populate additional RAM,
move shared pages, or change DMA addresses.

Device mappings retain 4 KiB granularity. A page-specific execute-permission
change splits a RAM block into 4 KiB descriptors naming the original physical
pages, with architectural break-before-make and translation invalidation.
Complete grant extents can be revoked without allocating split tables; backing
remains retained until the existing CPU/DMA retirement protocol completes.
Split tables are not automatically promoted back into blocks. This reduces
translation-table storage for eligible dense RAM; actual TLB and throughput
benefits depend on the processor and workload.

Whole-guest preallocation is a property of this implementation, **not a virtio
or vhost protocol requirement**. Supporting first-touch allocation from either
VM would require coordinated allocation from the same backing object, an import
address model that does not require a physical address in advance, and DMA
admission that makes the relevant pages resident and stable before submission.
CPU stage-2 faults alone cannot service a physical device's DMA access.
There is currently no configurable buffered-copy alternative; a bounded shared
pool would also require descriptor translation, backpressure and safe request
cancellation/reset handling.

### Memory observations and queue overhead

`vmm` reports two different quantities:

- `RAM capacity` is the configured guest RAM size.
- `allocated VM backing` counts resident physical backing in the VM's primary
  backing regions, deduplicating overlapping ranges of the same VMO within that
  snapshot. It does not count installed stage-2 entries, Linux's internal used
  memory, or all VM overhead such as page tables and vCPU stacks. It is not an
  exhaustive census of dynamically imported device mappings or a system-wide
  exclusive-memory accounting value.

For the resident I/O VM, 128 MiB RAM plus the 1 MiB configuration-client pool
produces 129 MiB (135,266,304 bytes). A disk-backed 256 MiB Alpine guest can
report the full 256 MiB even when idle, because backend admission populated
its RAM. Neither observation measures Linux `MemAvailable`. Shared backing
must not be added across owners as if each mapping were another allocation.

Virtio-scsi permits multiple request queues but does not require one per guest
CPU. The guest driver chooses how many to use within the advertised limit of
four. The pinned Linux vhost-scsi backend defaults to 2,048 preallocated data
scatterlist entries per command. At a queue depth of 128 and a 32-byte entry,
that alone is approximately 8 MiB per enabled request queue, excluding command
metadata and other allocations. This is an implementation-dependent estimate,
not a fixed per-vCPU cost or measured peak. Queue depth, enabled queue count and
concurrent clients must all inform the I/O VM's RAM budget. These kernel
allocations cannot be replaced with swap; shared pages must also remain stable
through backend CPU/DMA use and retirement.

Business guests may enable one through four request queues; unused optional
queues have canonical zero entries in activation records. Protocol versions 1
and 2 are rejected; both repositories must use a matching version 3 generation. Image
loading pipelines two 512 KiB userspace buffers between a scoped reader and the
guest-memory writer. Read-ahead stays inside the selected payload and the reader
is joined on success or failure. There is no idle polling or unbounded data cache.

The optional `hyper-vm-runtime/startup-profile` Cargo feature reports image
validation, payload read/write, memory preparation, and installation timings.
Read and write durations overlap in the pipeline; their sum is not wall time.
The existing startup total ends at vCPU start submission, before guest entry.
Ordinary builds omit the detailed phase measurements.

The same physical pages must remain owned and stable until all backend CPU and
DMA users have retired. Guest physical, I/O VM physical, Linux virtual, host
physical, and device DMA addresses are distinct domains. A stage-2 alias does
not translate a physical device's DMA transaction.

The Native guest-memory grant freezes an explicitly supplied writable VMO and
may be mapped into multiple pending VMs. Mapping regions are page aligned,
non-overlapping, and must cover the configured RAM before sealing. Closing the
grant handle does not release installed mappings; Native access remains
excluded until the last retained hardware lease retires. The grant may be
delegated through startup capabilities or a capability-channel rendezvous.

`vmo_create_contiguous` provides eagerly allocated, zeroed physical backing for
power-of-two sizes from one page through the ABI's current 128 MiB limit. The
whole allocation is charged to its resource domain and retained until the last
derived page owner retires. This is an explicit alternative to sparse VMOs,
not implicit DMA authority: device assignment, DMA address translation, and
quiescence still require their own ownership and validation.

Static imports, including the configuration client's pool and the legacy test
fixture, appear as Linux memory and reserved-memory nodes without `no-map` or
`reusable`. Linux creates `struct page` metadata but cannot allocate those pages.
Dynamic business clients instead advertise an aperture, without declaring it
as RAM. After token admission, the external module registers device-page
metadata and maps only the authorized scatter extents into a contiguous
userspace view. The vhost memory table translates frontend GPAs into that view.
Release drains vhost and drops Linux mappings and page references before the
module reports quiescence. The physical device bus has explicit `dma-ranges` tuples
(HypeR HPA, I/O VM GPA, length), covering both ordinary and imported RAM.

The QEMU test exercises imported-page I/O with nonidentity DMA translation and
`ACCESS_PLATFORM`. Shared queues and payload pages are not copied through the
Native control service. This does not promise that every Linux block-layer or
device alignment case is free of bounce buffers.

Stopping I/O VM vCPUs does not stop physical DMA. On a platform without DMA
isolation, pages cannot be reclaimed after a crash until the assigned hardware
has been reset and outstanding transactions have been drained. If quiescence
cannot be established, quarantine the pages; a timeout never grants permission
to reuse them.

## Build and validation

### Protocol version 3 appliance requirement

The [package lock](../scripts/io-vm.lock.json) selects separate protocol version 3
QEMU and Pi 5 appliances published from the merged HypeR-io-vm source. Default
builds and I/O VM acceptance import and verify these immutable GHCR digests.
The appliance boot banner and `/etc/hyper-io-version` identify the source
revision, dirty state, build profile and UTC build time.

`IO_VM_PACKAGE` remains an explicit development override for a complete boot
generation; ordinary builds do not need it. A completed SD image already
contains its appliance and does not need access to GHCR when booting.

Adopt future generations in this order:

1. Land the I/O VM changes in HypeR-io-vm and pass its appliance build/tests.
   The successful main build publishes separate QEMU and Pi 5 prerelease packages.
2. Import and test those exact immutable digests with the matching HypeR
   control plane. Keep physical Pi 5 results separate from Linux/QEMU checks.
3. Update both entries in `scripts/io-vm.lock.json`, including their source and
   workflow identities, and pass HypeR integration checks using those pins.
4. After HypeR merges, advance the image-distribution repository's HypeR pin;
   it inherits the dependency locks from that revision.

The default QEMU board includes storage and networking for Alpine:

```sh
make board-build ARCH=aarch64 BOARD=qemu
make board-run ARCH=aarch64 BOARD=qemu
```

`boards/qemu.json` owns the assigned network controller and guest interface;
the launcher creates the matching QEMU user-network uplink. Alpine automatically
loads virtio-net, brings up its interfaces and starts a background DHCP client
which configures addresses, routes and DNS and renews leases. A missing DHCP
server does not block the guest shell; client logs are in
`/var/log/udhcpc-<interface>.log`. This applies to both RAM and disk roots.
Existing persistent disks keep their on-disk VM definitions, guest FITs and
root filesystems across builds. Follow
[updating existing deployments](board-storage.md#updating-existing-deployments)
when changing device policy or guest startup files.

### Startup diagnostics

- `hyper-io-bridge ... failed with error -17`: an old appliance can treat a
  network notification as a storage notification and attempt to register the
  same device name twice. Verify that the packaged appliance recognizes
  `hyper,device-kind` and uses protocol version 3. Rebuild the bootstrap from
  the current appliance pin; changing only the guest disk does not
  replace the I/O VM.
- `vm-runtime: failed: OperatingSystem(Status(Status(-15)))`: `-15` means
  `PEER_CLOSED`. Inspect preceding io-runtime diagnostics for the cause. A
  `rejected client ... does not match the board I/O policy` message means the
  requested volume, network or MAC differs from the bootstrap policy. Compare
  `/data/vms.json` with `/etc/hyper/io-clients.conf`; an existing disk can still
  contain a storage-only VM definition after a network-enabled bootstrap rebuild.
- Alpine reaches its shell but has no address: check `ip addr`, `ip route` and
  `/var/log/udhcpc-eth0.log`. The appliance's `network default ready on eth0`
  message confirms bridge setup, not a DHCP lease or a working external link.
  QEMU supplies DHCP through user networking; Pi 5 needs an external LAN DHCP server.

### Network acceptance

The [network verifier](../tests/qemu/verify-network.py) derives a test fixture
from the default board and adds a second, network-only guest with a distinct
MAC address. This second guest belongs to the test, not the default deployment.
`test-board-network` prepares and runs this fixture with a fresh disposable disk
under `target/board-tests/network.*`. It retains the configuration and log for
diagnosis and writes a small proof file in the disk guest's `/root`:

```sh
make test-board-network ARCH=aarch64 \
  QEMU_MACHINE=virt,virtualization=on,gic-version=3
make test-board-network ARCH=aarch64 \
  QEMU_MACHINE=virt,virtualization=on,gic-version=2
```

The **HypeR / I/O VM integration** workflow runs separate GICv2 and GICv3 network
jobs on pull requests and main pushes. Its local suite is
`sh tests/ci/run.sh io-network`; `QEMU_MACHINE` selects the GIC version.
The suite imports the committed appliance pin unless `IO_VM_REFERENCE` selects
another immutable generation. The Make target also accepts `IO_VM_PACKAGE`.

The test serves a random 256 KiB + 137 byte payload on localhost, verifies its
SHA-256 inside each guest, then uploads it and checks the server's independently
computed SHA-256 receipt. It repeats both directions after reset and restart.
`NETWORK_TX_OFFLOAD=enabled` requires CSUM and both HOST_TSO bits in the guest's
negotiated state; `disabled` verifies fallback with an older appliance selected
through `IO_VM_REFERENCE`. The default accepts either, while always rejecting
unsupported RX offloads. CI needs no public HTTP endpoint. A direct
`verify-network.py run` invocation can add `--external-url http://...` with a
small public file to also check DNS and outbound HTTP; the external fetch
checks for a successful, nonempty response, while the local payload provides
the integrity check. The guest image must include its `virtio_net` module and automatic DHCP
startup. The verifier checks the address, default route, DNS configuration and
live DHCP client before issuing any manual network setup. Manual setup is used
only after the intentional driver unbind/rebind. Direct verifier invocations
must use the same generated board configuration for image construction and
execution; rebuild their disposable disk when guest files change.

Both GIC variants have passed DHCP, payload integrity, network driver
unbind/rebind, and stop/start followed by another transfer for a disk+network
guest and a network-only guest. The disk proof survives network reset and VM
restart; the network-only guest has no SCSI device. Both guests remain usable
after the other's network reset, and the verifier waits for all four
`RELEASE_MEMORY: ok` acknowledgements. An optional fetch from Alpine's public
HTTP server also passed for both guests, covering DNS and outbound connectivity.
The existing isolated storage acceptance continues to pass.

These are functional tests without performance thresholds. Concurrent
disk/network load, sustained traffic and fault stress require further coverage;
these results do not qualify physical RPi5 networking or a remote appliance
release. Backend protocol and injected retirement-failure tests separately
exercise shared memory ownership and device epochs.

### Package import and storage acceptance

Import the pinned, anonymously downloadable QEMU package (ORAS is discovered
or downloaded automatically):

```sh
make io-vm-fetch
```

The standalone fetch target does not forward `IO_VM_ORAS`. To select an ORAS
executable for this step, run
`python3 -B scripts/fetch-io-vm.py --platform qemu --oras /path/to/oras`.
Board and I/O initramfs composition targets accept `IO_VM_ORAS`.

The command prints the verified generation directory under
`target/io-vm/packages/`. Repeating it verifies and reuses that directory without
network access. A private GHCR package requires `oras login ghcr.io` first;
credentials belong to the local registry configuration, never to the repository.
Only the runtime payload is fetched. The OCI manifest also identifies the
matching configuration and corresponding-source archive for redistribution.
To qualify another generation, pass
`IO_VM_REFERENCE=ghcr.io/OWNER/PACKAGE@sha256:DIGEST` explicitly.
`make io-vm-fetch IO_VM_PLATFORM=rpi5` downloads and verifies the separately
built Pi package against its platform metadata. The lock explicitly records `hardware_qualified: false`.

Linux build instructions and the vhost-scsi reserved-page acceptance test live
in [HypeR-io-vm](https://github.com/roolrz/HypeR-io-vm). They run in that
repository. The HypeR fixture defaults to the pinned QEMU package:

```sh
make test-io-vm ARCH=aarch64
make test-io-vm ARCH=aarch64 \
  IO_VM_TEST=reset QEMU_CPUS=1 QEMU_MACHINE=virt,virtualization=on,gic-version=2
```

The fixture constructs identical FIT payloads from the same appliance. Separate
JSON configurations supply `hyper.role=io` and `hyper.role=business`; the runtime
adds HypeR-owned device trees.
It boots `hyper-io-smoke` as a privileged Native `/init`; it does not replace
the normal init/service configuration. Each guest has 64 MiB of ordinary RAM;
the I/O VM additionally maps the business VM's RAM as reserved shared pages.
The reset case unbinds and rebinds the business virtio-scsi driver between two
32-round direct-write/flush/read runs.

Every run creates a fresh disposable QEMU disk. The harness verifies the final
sector and all untouched bytes after stopping its QEMU process, retaining the
disk and `result.json` beside the log. This uses LIO iblock, not a RAM disk.
A Linux-only successful appliance test is a separate prerequisite.

The **HypeR / I/O VM integration** GitHub workflow runs both reset
configurations on pull requests and main pushes using the pinned package.
Manual dispatch can override the public immutable reference. Its local
equivalent is `sh tests/ci/run.sh io-vm`; set `IO_VM_REFERENCE` to qualify
another generation explicitly.
The locked version 3 packages passed the upstream appliance build/tests and
publication verification. The board-managed Native FAT and business-client tests
are additional HypeR acceptance paths, documented in [board storage](board-storage.md).
Updating the lock requires qualifying the exact new build.

Physical virtio discovery accepts the trigger declared by firmware, including
QEMU's edge-triggered SPIs. Active physical edge sources remain enabled;
level sources are masked until guest acknowledgement. The guest-facing line
is level-triggered and follows durable virtio interrupt status.

## Compatibility and qualification

Before release, bridge layouts must define explicit widths, endianness, lengths, capability
bits, and generation-tagged identities. Extensions are negotiated; existing
fields and operation numbers do not change meaning. Unsupported mandatory
features fail connection setup. The Native syscall ABI and Linux-local UAPI
are separate interfaces and do not inherit bridge version numbers.

The storage baseline passes with one vCPU per guest, under both GICv3 with
four host CPUs and GICv2 with one host CPU. It covers direct read/write/flush,
driver unbind/rebind, exact host disk contents, nonidentity DMA translations,
and memory admission after retirement. Separate Native guest tests cover
mailbox/notification interrupts, backend exit, stale completions and interrupted
MMIO retirement.

Sustained multi-vCPU storage load, concurrent reset during I/O, forced I/O VM exit with
physical requests in flight, and physical read/write/flush failure injection
still need dedicated qualification. Pi 5 must additionally validate DMA address
translation, cache maintenance, interrupt ordering, device reset, and measured
image sizes. Failed physical quiescence quarantines the owner and its pages;
this baseline does not provide automatic recovery of quarantined devices.

### Native mapping release retries

`guest_mapping_release` reports `WOULD_BLOCK` when another operation owns the
all-CPU translation synchronization transport. No mapping state has changed;
retry the same release without repeating Linux RESET/RELEASE. `BUSY` instead
means that the backend has not yet supplied the mapping's quiescence proof.
Other failures are not treated as proof or silently retried. The I/O broker
bounds transport retries to five seconds, sleeps between attempts, and keeps
processing backend console and power notifications. A failed retirement keeps
its mapping ownership and enters the backend shutdown/quarantine path.

### Forwarded appliance logs

The Native I/O runtime prefixes each forwarded guest console line with
`HypeR IO VM:`, including lines split across serial reads. Native runtime messages
retain their own service name. Prefixing does not buffer a whole line or alter
its bytes; a partial line remains visible, so output from other services can
still interleave before the guest emits its next newline.

An appliance message such as `HypeR I/O [naa.5001405000000001]` identifies its
LIO/vhost-scsi target. This NAA-format world-wide name binds the vhost endpoint
to its storage target; it is not a VM ID, memory address, or disk capacity.
The appliance currently constructs target names from the configured volume
ordinal (the first volume uses the suffix `000000001`). These locally generated
names do not assert a globally assigned hardware identity.
