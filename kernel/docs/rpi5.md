<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Raspberry Pi 5 bring-up

HypeR supports the host GICv2 MMIO interface and adapts its PL011 driver for
Pi 5's dedicated three-pin debug UART, including its smaller AXI register window. The intended first milestone is a
four-core Native shell with timer-driven scheduling and interrupt-driven input.
Physical Pi 5 boot has not yet been validated; QEMU GICv2 tests cover the host
interrupt path, not BCM2712 firmware, clocks, or electrical behavior.

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
runtime crashes. Physical Pi 5 boot, firmware resource descriptions, interrupt
ordering and device quiescence still need hardware validation. The Linux I/O
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

Use Pi EEPROM firmware followed by upstream TF-A's `PLAT=rpi5` BL31, then load
HypeR as an AArch64 Linux Image at EL2. TF-A supplies PSCI for secondary CPUs;
HypeR requires FEAT_VHE. This avoids adding a board-specific CPU-release
protocol to the kernel. See the [TF-A Pi 5 port documentation](https://tf-a.docs.trustedfirmware.org/en/latest/plat/rpi5.html).

Build TF-A in its own checkout with an AArch64 GNU toolchain:

```sh
CROSS_COMPILE=aarch64-linux-gnu- make PLAT=rpi5 DEBUG=1
```

Keep direct Linux boot enabled (the port default), and do not set fixed
`PRELOADED_BL33_BASE` or `RPI3_PRELOADED_DTB_BASE` addresses for this setup.
Copy `build/rpi5/debug/bl31.bin` to the boot partition.

Build HypeR and a Native-only initramfs from the HypeR repository root:

```sh
make image native-initramfs ARCH=aarch64 \
  NATIVE_GUEST_PREREQUISITES= NATIVE_GUEST_ENTRY= \
  NATIVE_VM_CONFIG="$PWD/app/init/config/native/vms.json" \
  NATIVE_SERVICE_MANIFEST="$PWD/app/init/config/native/services.json" \
  NATIVE_INITRAMFS="$PWD/target/app/aarch64/initramfs-rpi5.cpio"
```

Copy `kernel/target/aarch64-unknown-none/kernel/hyper.img` and
`target/app/aarch64/initramfs-rpi5.cpio` to the boot partition. Keep the current
Pi firmware's matching `bcm2712-rpi-5-b.dtb` there, so firmware can apply board
revision, RAM size and reserved-memory fixups. Do not substitute QEMU's DTB.

Suggested dedicated boot-partition `config.txt`:

```ini
[pi5]
armstub=bl31.bin
kernel=hyper.img
initramfs initramfs-rpi5.cpio followkernel
cmdline=cmdline.txt
uart_2ndstage=1
camera_auto_detect=0
display_auto_detect=0
[all]
```

Put this single line in `cmdline.txt`:

```text
earlycon=pl011,0x107d001000,115200n8 loglevel=7
```

The firmware's [config.txt reference](https://www.raspberrypi.com/documentation/computers/config_txt.html)
describes Image and initramfs placement. `initramfs` deliberately has no `=`.
HypeR reads the uncompressed newc archive directly. Firmware must describe its
bounds in `/chosen/linux,initrd-start` and `linux,initrd-end` and retain BL31's
reserved-memory range. Connect the dedicated debug port at **115200 8N1**.
The PL011 driver retains firmware's clock and baud setup.

## Hardware discovery and mapping

The [Pi 5 device tree](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b.dts)
and its BCM2712 includes provide the hardware addresses. HypeR translates bus
`ranges`; the interrupt driver does not hardcode these addresses:

| Resource | Physical address / interrupt |
| --- | --- |
| GIC distributor | `0x107fff9000` |
| GIC CPU interface | `0x107fffa000` |
| Dedicated debug PL011 | `0x107d001000`, SPI 121 / INTID 153 |
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

On hardware, record TF-A and HypeR logs, verify four online CPUs and INTID 153
console registration, then exercise slowly typed shell commands, idle wakeup,
and repeated `top` entry/exit. QEMU passing is not a substitute for these checks.

After host bring-up, use a four-vCPU guest image and verify all guest CPUs are
online, then repeatedly offline/online secondary CPUs through Linux sysfs.
Exercise per-CPU timer wakeups, SGI/IPI traffic, SPI routing, and console input
under concurrent load. Repeat guest `reboot -f` and `poweroff -f`, verify that
HypeR itself stays running, and check guest pages return to the stopped baseline.
Also terminate vm-runtime during a pending power request and with CPUs powered
off; every configured Thread and translation must retire before page/VMID reuse.
Record cold/warm boot logs and firmware-provided GICH/GICV/maintenance resources.
Real hardware must validate cross-core cache publication, TLB invalidation,
interrupt ordering, and device quiescence; QEMU cannot establish those properties.
Guest suspend is not supported. See [VM power and SMP](vm-bundle.md#guest-power-requests-and-smp).
