<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Raspberry Pi 5 bring-up

HypeR supports the host GICv2 MMIO interface and adapts its PL011 driver for
Pi 5's dedicated three-pin debug UART, including its smaller AXI register window. The intended first milestone is a
four-core Native shell with timer-driven scheduling and interrupt-driven input.
Native boot, bidirectional UART and diskless Linux guest userspace were
confirmed on a Pi 5 D0 board. Physical storage and DMA still require validation.

GICv2 guest virtualization is implemented and tested with Linux on QEMU.
Firmware must describe the GICH and GICV register windows plus the maintenance
PPI in the GIC node. Hosts without those resources retain Native application
support and reject VM admission. The current backend requires five priority
and preemption bits and supports up to 64 list registers.

Each vCPU owns its saved GICH control, VMCR, APR and list-register state. Guest
stage-2 maps only the banked GICV CPU interface as Device memory; GICC and GICH
remain host-only. The emulated distributor exposes 1..8 vCPUs and 64 interrupt
IDs using GICv2 without security extensions. The Native platform-info query reports the
GIC revision so vm-runtime emits matching guest firmware. GICv3 hosts retain
their existing guest profile.

QEMU covers guest boot, timer/console wakeups and VM retirement, including
runtime crashes. Physical host and diskless Linux boot are confirmed. Interrupt ordering under
load and physical device quiescence still need hardware validation. The Linux I/O
backend has a board-configured deployment path; its Pi 5 hardware qualification
remains in the [roadmap](../../docs/roadmap.md). RP1 UARTs on the 40-pin header are not the
console used by this configuration.

The I/O appliance is built and published by the separate HypeR-io-vm repository.
HypeR consumes its pinned GHCR package and provides the guest DTS/DTB and
launch configuration. The firmware-provided host DTB used below is not a guest
device-assignment description. The common AArch64 package includes the upstream
Pi storage drivers, but has not been qualified on physical Pi 5 hardware. See
the [I/O VM contract](../../docs/io-vm.md) and
[board storage deployment](../../docs/board-storage.md).

The Native I/O runtime validates the C0/D0 SDIO1 dependency graph and runs a
separate userspace MMIO/IRQ worker. The kernel supplies immutable firmware
facts, atomic exclusive resource claims, bounded register access and IRQ
notification/completion; it contains no SDHCI register or pinmux policy.
Retiring an I/O VM cannot yet prove SD DMA has stopped, so the device and all
own/imported DMA backing remain quarantined until host reboot. QEMU's generic
device test does not establish physical SD reset, cache or DMA correctness.

## Recommended boot chain

Use the official Pi 5 EEPROM firmware and its built-in BL31 to load HypeR
as an AArch64 Linux Image at EL2. Leave `armstub` unset: no custom TF-A binary
is built or installed. BL31 supplies PSCI; HypeR requires FEAT_VHE.
The [official EEPROM release notes](https://github.com/raspberrypi/rpi-eeprom/blob/master/firmware-2712/release-notes.md)
track the bundled BL31 updates. Record the installed EEPROM version; the disk
image does not pin or update EEPROM.

## Native-only image integration

HypeR downloads official prebuilt DTBs and overlays using
`scripts/rpi5-boot.lock.json`, including SHA-256 checksums and a pinned
corresponding Linux source archive. Python 3.11+ is required; no firmware
compiler or separate boot-build repository is needed. Downloads are cached
under `target/rpi5-boot/` and verified before reuse. External sources and binaries
remain outside HypeR Git and retain upstream licenses.

From a HypeR checkout:

```sh
make rpi5-bringup ARCH=aarch64
```

`RPI5_BOOT_PACKAGE=/path/to/package` remains an optional local development
override for a verified v2 package. Old custom-TF-A v1 packages are not accepted.

This builds the same architecture-wide kernel/application binaries and a
Native-only initramfs using `app/init/config/native/`. It validates every
external package checksum, then creates
`target/board/rpi5-native/disk.img` from `boards/rpi5-native.json`. The image has
a 64 MiB FAT32 GPT boot partition; no Linux I/O VM, Alpine download or VM disk
is needed to qualify the kernel and shell. The unused device selector in this
board policy records the later storage assignment; Native-only init does not
start the I/O service or claim the SD controller. This is separate from the
full `boards/rpi5.json` deployment with its 1 GiB configuration volume and guest
disks. Full I/O deployment still needs physical SD/DMA qualification.

Existing image outputs are refused. To explicitly replace this generated
image, add `BOARD_IMAGE_REPLACE=--replace`. This command never writes a physical
device. Preserve the working Pi OS medium and flash a spare medium with an image
writer after checking its device identity. Keep `manifest.json`,
`boot-notices.txt` and `sources.tar.gz` with any redistributed image; verify
upstream license obligations for the particular distribution.

The generated FAT partition contains these settings:

```ini
[pi5]
kernel=hyper.img
initramfs bootstrap.cpio followkernel
cmdline=cmdline.txt
uart_2ndstage=1
camera_auto_detect=0
display_auto_detect=0
[all]
```

`cmdline.txt` selects `earlycon=pl011,0x107d001000,115200n8 loglevel=7`.
No fixed BL33 or DTB load addresses are configured. EEPROM firmware patches the
host DTB for actual board revision, RAM size and reserved memory; the built-in BL31 supplies
PSCI and EL2 handoff. Record the installed EEPROM version alongside the boot
package manifest. Never substitute QEMU's DTB or a guest-assignment DTB.

The firmware's [config.txt reference](https://www.raspberrypi.com/documentation/computers/config_txt.html)
describes Image and initramfs placement. `initramfs` deliberately has no `=`.
HypeR reads the uncompressed newc archive directly. Firmware must describe its
bounds in `/chosen/linux,initrd-start` and `linux,initrd-end` and retain BL31's
reserved-memory range. Connect the dedicated debug port at **115200 8N1**.
The PL011 driver retains firmware's clock and baud setup. The GPIO14/15 RP1
UART is not an alternative early console for this image.

## Hardware discovery and mapping

The [Pi 5 device tree](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b.dts)
and its BCM2712 includes provide the hardware addresses. HypeR translates bus
`ranges`; the interrupt driver does not hardcode these addresses:

| Resource | Physical address / interrupt |
| --- | --- |
| GIC distributor | `0x107fff9000` |
| GIC CPU interface | `0x107fffa000` |
| Dedicated debug PL011 | `0x107d001000`, SPI 121 / INTID 153 (C0/C1); SPI 120 / INTID 152 (D0) |
| EL2 physical timer | PPI 10 / INTID 26 |
| CPU power | PSCI, SMC conduit |

Before enabling the MMU, allocation-free FDT discovery builds the temporary
identity map from firmware RAM. It maps RAM as Normal memory and other blocks
as Device/XN, including peripherals above 4 GiB. This transient map covers
512 GiB with 1 GiB blocks and rejects RAM/MMIO sharing a block or resources
outside that range. The permanent map remains page-granular and applies the
usual RAM, reserved-memory and MMIO validation. No Pi-specific memory addresses
are embedded in the bootstrap.

## Validation

```sh
make test-native-gicv2 QEMU_CPUS=4
make test-native-gicv2 QEMU_CPUS=1
make test-guest-smp QEMU_MACHINE=virt,virtualization=on,gic-version=2
```

CI runs both configurations, exercising Native services, std programs, shell
input and userspace-managed Linux guest startup. It also runs four-core GICv2
kernel self-tests, paced guest-console input and vm-runtime crash recovery.
Host tests cover guest distributor decoding, LR encoding, delivery gates,
pending-state preservation, GIC SGI sender preservation,
CPU target masks, UP target-register behavior, spurious IDs, FDT matching,
nine-window devices, and low-RAM/high-MMIO mapping. The upstream Pi 5 DTB was
also checked locally for generic and essential-device discovery.

Native boot to the shell and bidirectional debug-UART input were confirmed on
a Pi 5 D0 board on 2026-09-20 after installing the stepping overlay. This does
not qualify the guest or physical storage paths.

On hardware, record firmware and HypeR logs, verify four online CPUs and the matching
console IRQ (INTID 153 for C0/C1, 152 for D0), then exercise slowly typed shell
commands, idle wakeup,
and repeated `top` entry/exit. QEMU passing is not a substitute for these checks.

After host bring-up, use a four-vCPU guest image and verify all guest CPUs are
online, then repeatedly offline/online secondary CPUs through Linux sysfs.
Exercise per-CPU timer wakeups, SGI/IPI traffic, SPI routing, and console input
under concurrent load. Repeat guest `reboot -f` and `poweroff -f`, verify that
HypeR itself stays running, and check guest pages return to the stopped baseline.
Also terminate vm-runtime during a pending power request and with CPUs powered
off; every configured Thread and translation must retire before page reuse.
Include VMID rollover while guests run and resume on another CPU, checking that
new epochs flush old translations before recycled tags enter hardware.
Record cold/warm boot logs and firmware-provided GICH/GICV/maintenance resources.
Real hardware must validate cross-core cache publication, TLB invalidation,
interrupt ordering, and device quiescence; QEMU cannot establish those properties.
Guest suspend is not supported. See [VM power and SMP](vm-bundle.md#guest-power-requests-and-smp).

### Firmware handoff and console constraints

The Pi 5 packer pads the staged kernel to its Image-header runtime size so
`followkernel` cannot place initramfs in its NOLOAD BSS, tables or boot stack.
The original build artifact is unchanged.

The firmware may add nonstandard device metadata such as `/axi/vc_mem`.
Malformed device `reg`, `interrupts`, or `ranges` encodings are quarantined per
node: no MMIO or interrupt capability is published from that node. A malformed
bus translation also quarantines descendant resources. Valid generic resources
remain discoverable for userspace drivers even without a kernel driver.
Recognized essential controllers/timers reject quarantined resources, as does
platform driver binding. RAM, CPU and reserved-memory errors remain fatal;
structural FDT bounds checks are never bypassed. No Pi-specific node-name
exception is used.

BCM2712 describes its debug PL011 with a 512-byte `reg` window. Early console
validation and permanent binding use the driver's operational register extent,
not a 4 KiB page or the optional PrimeCell identification window. Page rounding
belongs to the mapper and does not enlarge the firmware-granted resource.

The Native return frame retains the full 64-bit HCR_EL2 in ordinary stack
storage, as the guest return path does. ESR_EL2 is not a general-purpose scratch
register: reserved upper syndrome bits cannot preserve HCR_EL2.E2H at bit 34.
An emulator that retains those bits can conceal a loss of the VHE translation
regime immediately before EL0 entry on hardware. The Native boundary check
rejects reintroducing this scratch-register use.

Console and kernel log output preserve LF line endings without inserting CR.
Configure the serial terminal to display LF as a new line at column zero if
needed. Interactive input accepts CR, LF and CRLF and normalizes them to LF.

The boot partition includes both `bcm2712-rpi-5-b.dtb` and
`bcm2712d0-rpi-5-b.dtb`, plus `overlays/bcm2712d0.dtbo` and
`overlays/overlay_map.dtb`, from the same pinned vendor firmware release.
Leave `device_tree` unset and do not force the D0 overlay on all boards.
Firmware can load the base DTB and automatically apply the D0 stepping overlay;
supplying a separate D0 DTB alone is not sufficient for that boot path.
The overlay changes UART10 from SPI 121 (INTID 153) to SPI 120 (INTID 152), along
with other silicon differences. Missing it can leave a working shell prompt
with no UART RX interrupts. These differences belong in vendor device-tree
inputs, not kernel IRQ overrides. All DTB/overlay binaries and corresponding
sources remain in the external boot package.

The FAT32 boot/configuration volume uses the GPT Basic Data type, not the EFI
System Partition type. Its volume label is `HYPER`; firmware files and HypeR
configuration continue to share this volume. Flash the complete `disk.img` to
the whole device, then eject and reconnect it for the host to discover the new
partition table. No UEFI boot manager is involved.

The default EEPROM-stub image was subsequently confirmed working on hardware
on 2026-09-20. Keep the installed EEPROM version with qualification records.

## Diskless I/O VM bring-up

Separate QEMU and Pi 5 appliances are published from the same pinned upstream
Linux source. `scripts/io-vm.lock.json` selects each platform's immutable OCI
digest, source revision and successful build/publication runs. HypeR owns guest
FIT composition and Native VM policy.

The bring-up target downloads the pinned Pi 5 package by default and selects
the explicit `hyper.mode=bringup` init policy:

```sh
make rpi5-io-bringup ARCH=aarch64 BOARD_IMAGE_REPLACE=--replace
```

`IO_VM_PACKAGE=/path/to/rpi5/appliance` remains an optional development override.
The released profiles passed Linux CI and QEMU acceptance; earlier physical Pi
results used a development reassembly. The newly pinned package still requires
hardware requalification.

This replaces `target/board/rpi5-native/disk.img`, retaining the official
firmware boot chain and HypeR shell. It registers `io-bringup` for manual startup through
vm-manager/vm-runtime without io-runtime, device assignment or a /data mount.
Use `vmm start io-bringup`, `vmm status io-bringup` and
`vmm console io-bringup`. Manual startup preserves the Native shell if guest
bring-up fails; inspect `dmesg` for the terminal vCPU syndrome and address.
The guest reports `bring-up ready; no devices assigned, storage service disabled`.
It waits under VM supervision; it does not provide an interactive Linux shell
or claim backend readiness. Stop/restart use the normal VMM commands.

The full storage deployment remains `boards/rpi5.json`; enabling it still
requires physical storage/DMA qualification. Do not supply its volume/client
configuration to diskless bring-up. QEMU GICv2 can exercise this guest boot path
but cannot validate the board's device drivers.


## SD-card backend qualification

`boards/rpi5-sd.json` selects the physical BCM2712 SDIO1 controller by firmware
path and contains a 1 GiB FAT configuration partition plus a 2 GiB Alpine ext4 root
partition, matching the QEMU guest layout. Alpine uses the minimal rootfs and
remains stopped until `vmm start alpine`. Its FIT lives at `/data/vm/alpine.itb`,
not in the Native initramfs. Its root disk is exported separately through
dm-linear and vhost-scsi; use `vmm console alpine` after starting it.
Build its complete flash image from this checkout with the pinned I/O VM,
official boot inputs and Alpine downloads (no additional build checkout):

```sh
make rpi5-sd ARCH=aarch64 \
  BOARD_IMAGE_REPLACE=--replace
```

The output is `target/board/rpi5-native/disk.img`, matching the earlier bring-up
path. Flashing it to the whole SD card replaces existing partitions and data.
The build itself only writes this generated image, never the physical card.

Native io-runtime claims the SDIO1 dependency graph and supplies a guest DT;
Linux drives SDHCI, GPIO and pinctrl. The appliance selects the configuration
partition by PARTUUID and maps it through dm-linear as `hyper-config`.
Linux exports that block device through LIO/vhost-scsi. HypeR mounts its FAT
filesystem at `/data`; Linux must not mount the same filesystem concurrently.
The boot files remain ordinary firmware-readable FAT files in this volume.

Expected readiness is `configuration volume mounted at /data`. Check
`vmm status io`, `ls /data`, and a small file write/read after it appears.
The configuration mount and directory reads have passed on Pi 5 D0.

The 2026-09-22 hardware run also verified a 256 MiB, two-vCPU Alpine guest
with a 128 MiB resident I/O VM: both CPUs online, three CPU 1 off/on cycles,
one ordinary `reboot`, and one ordinary `poweroff`. A file written and synced
on the SD-backed ext4 root survived the reboot. After poweroff, Alpine reached
`stopped`, the I/O VM remained `running`, `/data` remained readable, and the
backend reported successful memory release. This establishes basic guest
lifecycle operation, not repeated power-cycle durability or DMA retirement
under faults. Those stress and failure cases remain unqualified. The kernel retains generic device ownership and
notification mechanisms; SDHCI policy remains in userspace.

## Release boundary

These Make targets are local integration tools. The separate image distribution
repository will pin merged HypeR and I/O VM revisions plus immutable binary and
source artifacts; see [image distribution](../../docs/image-distribution.md).
Do not promote local reassemblies or uncommitted builds to release pins.

On AArch64, stage-2 permission faults outside a stage-1 walk do not provide a
generally reliable HPFAR IPA. The exception backend recovers it with a guest
stage-1 address translation, preserving PAR and restoring the host HCR before
accessing host memory. This fixes first execution of Linux userspace pages on
Pi 5; an unresolved instruction abort is a memory fault, never an MMIO request.
