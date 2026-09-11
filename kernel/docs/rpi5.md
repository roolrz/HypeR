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

The guest interrupt backend currently requires GICv3. GICv2 hosts skip guest
backend initialization instead of accessing unavailable ICH system registers.
This does **not** yet enable a Linux I/O VM on Pi 5. That needs a separate GICv2
virtualization backend, tracked in the [near-term roadmap](../../docs/roadmap.md). RP1 UARTs on the 40-pin header are not the console used
by this configuration.

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
  NATIVE_VM_CONFIG="$PWD/app/init/config/vms-empty.json" \
  NATIVE_SERVICE_MANIFEST="$PWD/app/init/config/services-native.json" \
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
```

CI runs both configurations, exercising Native services, std programs, shell
input and application startup. Host tests cover GIC SGI sender preservation,
CPU target masks, UP target-register behavior, spurious IDs, FDT matching,
nine-window devices, and low-RAM/high-MMIO mapping. The upstream Pi 5 DTB was
also checked locally for generic and essential-device discovery.

On hardware, record TF-A and HypeR logs, verify four online CPUs and INTID 153
console registration, then exercise slowly typed shell commands, idle wakeup,
and repeated `top` entry/exit. QEMU passing is not a substitute for these checks.
