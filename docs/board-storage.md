<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Board deployment and storage

`boards/qemu.json` and `boards/rpi5.json` describe deployment policy for the
same AArch64 kernel and applications. Hardware addresses and interrupt topology
come from the host and guest device trees. Selecting a board does not produce
a different kernel or app ABI.

## Disk ownership

The image builder creates an outer GPT with a 1 GiB FAT32 configuration volume
and one opaque partition per configured VM. The configuration volume contains
the boot payloads, `board.json`, `vms.json`, `volumes.json`, and guest ITBs.
The I/O VM image lives only at `/vm/io.itb` inside `bootstrap.cpio`; no separate
`/data/vm/io.itb` is packaged. Business guest images remain on the configuration
volume.
QEMU loads `bootstrap.cpio` directly from the host with `-initrd`, so its
configuration volume omits this archive. Pi 5 firmware loads the archive from
the FAT boot/configuration partition; that required boot file remains visible
as `/data/bootstrap.cpio` after HypeR mounts the partition.
VM partitions contain whole virtual disks: their first byte is the guest's
sector zero, so a guest GPT is contained within that VM's volume.

The trusted Linux I/O VM alone drives the physical controller. Its bootstrap
manifest identifies outer partitions by PARTUUID and exact sector count. It
validates the GPT and creates a separate `dm-linear` device for every volume,
including the HypeR configuration volume. It neither mounts filesystems nor
recursively discovers partitions inside guest disks. LIO exposes each mapper
through an independently authorized vhost-scsi target. Client zero owns only
the configuration volume; other clients cannot request that target.

HypeR's Native block initiator uses standard virtio-scsi split queues and shared
pages. The kernel mounts the configuration volume as a real FAT filesystem at
`/data` beneath the initramfs root. File reads access that volume; they do not
populate a second ramfs copy. The block transport can sleep, and the filesystem
holds a sleepable mutex rather than an IRQ-disabling lock while waiting for I/O.
See [FAT semantics and durability](../kernel/docs/fat.md).

## Bootstrap and VM services

The bootstrap initramfs contains enough to start init, the Native services,
and the Linux I/O VM before `/data` exists. The image builder projects the same
board JSON into bounded Native and Linux client manifests. Linux kernel and
module binaries remain digest-verified appliance artifacts; HypeR contributes
a separate configuration archive to the Linux initramfs.

The shell starts independently. An explicit readiness channel gates init's
loading of `/data/vms.json` until the configuration filesystem is mounted.
If no VM is configured for autostart, vm-manager reports `NoAutostart` to init;
this completes boot supervision without claiming that a VM stopped. The I/O VM
continues running under io-runtime independently.
Guest lifecycle management remains in vm-manager and each guest's vm-runtime.
The manager retains each disk session endpoint until the runtime reports its VM
installed, then transfers it to the I/O broker through the supervisor wait set.
Image loading does not consume the bounded disk-handshake deadline or occupy
the shared broker listener. Pending endpoints retire with their owning instance.
Disk connection setup uses a capability channel to io-runtime. Only negotiation,
reset and release commands pass through that service; queue notifications and
shared-page data transfers do not.

Ordinary guest memory need not be physically contiguous. A dynamic backend
mapping retains the exact guest-memory grant and exposes a kernel-authorized
extent table to the Linux module. Admission binds one generation token to one
client notification route. After vhost drains and Linux drops its mappings and
page references, the module reports quiescence on that route. HypeR then clears
the aliases and waits for every CPU's translation invalidation acknowledgment
before releasing the old storage. A timeout or process exit is not proof that
DMA has stopped.

## Build and run

Interactive `make run` / `make board-run` uses QEMU's multiplexed serial console.
Press `Ctrl+A`, then `X` to exit QEMU, or `Ctrl+A`, then `C` to switch to the
QEMU monitor. `Ctrl+C` is passed to the guest.

The image builder needs `mkfs.fat` from dosfstools and `mcopy` from mtools,
in addition to the normal build prerequisites. It searches `PATH` first,
then queries `brew --prefix` for each package's `bin` and `sbin` directories
when Homebrew is available. Explicit `--mkfs` and `--mcopy` executable paths
override automatic discovery.

```sh
make board-plan BOARD=qemu
make board-image BOARD=qemu
make board-run BOARD=qemu
```

The default imports the digest-pinned appliance. `IO_VM_PACKAGE` optionally
selects a complete verified local generation.
When importing an uncached package, the fetcher uses ORAS from `PATH` or
automatically downloads ORAS 1.3.0 into `target/tools` for macOS/Linux on
ARM64/x86-64. Downloads require `curl` and are checked against pinned SHA-256
digests before installation. Set `IO_VM_ORAS` to use a specific executable.
Verified cached packages need neither ORAS nor network access.

The board bootstrap contains only the I/O VM's ITB. Ordinary guest images such
as Alpine are stored in the configuration partition and loaded through `/data`;
they are not duplicated in the bootstrap ramfs. Standalone Native test archives
still carry their guest fixtures.

`BOARD_CONFIG` selects a custom JSON file, `BOARD_OUTPUT` selects staging, and
`BOARD_IMAGE` selects the resulting raw image. `board-image` always refuses to
overwrite an existing image. To create another generation, choose a new output
name; rebuilding applications must not silently erase persistent disk contents.
AArch64 `make run` selects this QEMU deployment profile. On first use,
`board-run` creates the configured image only if its output does not exist;
the image publisher still refuses replacement. Existing images must pass GPT
validation against the selected configuration. QEMU uses the freshly built kernel/bootstrap
while persistent files and VM images remain those stored on the disk.

An optional VM `disk-image` names an explicit `--artifact NAME=PATH` input. It
must exactly match the declared volume size, preserving any guest backup GPT.
Without this input, the newly created VM disk is blank. Additional deployment
inputs can be passed through `BOARD_ARTIFACTS`.

`make test-board-storage IO_VM_PACKAGE=/path/to/verified/appliance` creates an
isolated disk under `target/board-tests/` and boots it twice. Its test-only
application checks multi-megabyte file contents, extension gaps, timestamps,
rename, copy, directory enumeration and explicit synchronization. The second
cold boot checks persisted contents; both boots also exercise paced shell input
and, in stack-audit builds, require at least 2 KiB of measured stack reserve
and no more than 12 KiB of measured use (`STACK_MAXIMUM_USED`).
The fixture keeps its disk and logs for diagnosis and never uses `BOARD_IMAGE`
as a scratch disk.

The bootstrap board document selects the physical controller explicitly through
`io-device`: `profile` selects the I/O runtime's device policy and
exactly one of `compatible` or `path` identifies its firmware node. QEMU uses
`virtio-mmio-scsi` with `virtio,mmio`. Multiple eligible matches are rejected;
use the full canonical FDT path when a machine exposes several controllers.
Claiming a device never falls back to another match because one is busy. Up to
eight business VM disks plus the configuration volume are supported per I/O VM.
The finite fleet quota includes eight business VM allowances and one resident
I/O VM allowance, plus manager overhead. This is an admission ceiling, not
preallocated RAM or a guarantee that all eight guests fit the host.

For Pi 5, provide board-specific `tfa` and `host-dtb` artifacts. Both board
profiles use the same pinned AArch64 appliance; its Pi driver configuration is
build-verified, but physical Pi qualification remains outstanding. The FAT boot
volume contains the firmware configuration, TF-A, kernel and bootstrap archive.
The userspace `bcm2712-sdhci` profile validates the upstream C0/D0 SDIO1
resource graph: host/config registers, fixed clock, main/AON pinctrl and AON
GPIO supplies/card detection using the kernel's immutable firmware catalogue.
The board selects the full firmware node path,
because Wi-Fi SDIO2 has the same compatible string. Only named register bytes
are trapped through to hardware; adjacent firmware interrupt registers are not
mapped into Linux. The GPIO/pinctrl controllers are claimed exclusively against
other HypeR assignments, and card detection keeps the upstream polling fallback.
The kernel atomically claims the requested register bundle and level interrupt;
it does not interpret SDHCI registers, clocks, pinmux or GPIO policy. A separate
I/O runtime worker handles trapped MMIO and physical IRQ notifications, so the
main runtime can wait for Linux storage without blocking its device service.
The worker reads SDHCI status and signal-enable registers to update the guest
interrupt level. Native calls 143–147 provide firmware reads, bundle claims,
bounded 1/2/4-byte MMIO, pending IRQ tokens and sequence-checked IRQ completion.

The profile preserves noncoherent DMA and requires the controller's actual
64-bit DMA capability; there is no implicit 32-bit bounce pool. Device retirement
currently cannot prove SD DMA has stopped: after the I/O VM has run, stopping it
quarantines its device and all own/imported memory until host reboot. A timeout
never releases potentially active DMA storage. These are explicit deployment
constraints, not a claim of successful hardware qualification. The common
appliance must include the upstream SDHCI/GPIO/pinctrl/regulator drivers, and the
[Pi 5 bring-up checks](../kernel/docs/rpi5.md) remain required.

`make test-userspace-device` uses a test-only virtio worker and a real QEMU disk
with a level interrupt to exercise the same generic bundle/MMIO/IRQ mechanism.
This is a validation fixture, not evidence that Pi 5 SDIO or DMA has passed
hardware qualification.

The I/O control loop keeps at most one pending backend transaction per client
and advances clients independently. Absolute deadlines are checked even when
unrelated objects remain ready. Losing the manager admission endpoint closes
new admissions without stopping existing clients or `/data`. A request already
sent to Linux must receive its matching reply, or the I/O VM must be safely
retired, before its DMA mapping can be reclaimed.

`make test-board-business` exercises ordinary scatter-backed VM start, stop,
and restart against a real QEMU disk partition. `make test-board-broker` uses
separately built test applications to withhold client A's reply after its real
HELLO reaches Linux. Client B must complete RESET, RELEASE, kernel-proven mapping
retirement, and notification disconnection before A can receive that reply.
The fixture checks client and generation identities, closes the actual manager
admission endpoint, and verifies both VM disks and `/data`.
Production applications are never rebuilt with these fault-injection features.
