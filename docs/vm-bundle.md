# VM boot ramdisk format

## Firmware handoff

HypeR is intended to be selected as the kernel component of a signed FIT image.
The selected FIT configuration supplies a ramdisk component alongside HypeR and
the platform DTB. U-Boot must publish the resulting physical half-open ramdisk
range through the standard `/chosen/linux,initrd-start` and
`/chosen/linux,initrd-end` properties.

The bytes handed to HypeR must be an uncompressed SVR4 `newc` or `crc` CPIO
archive. A FIT implementation may store a compressed component only when
U-Boot decompresses it before setting the DTB range. HypeR deliberately does
not couple its early boot path to FIT parsing, a compression codec, or a
particular U-Boot storage backend.

Early boot validates that the complete ramdisk lies in ordinary DTB-described
RAM and does not overlap a `no-map` reservation. It reserves the range before
the buddy allocator receives memory, then accesses it through the permanent
linear map. The archive remains immutable for the lifetime of every loaded VM.

## Outer boot archive

The outer archive is a boot catalog and may contain multiple VM bundles:

```text
hypervisor/
  boot.conf
  vms/
    alpine.cpio
    service.cpio
```

`hypervisor/boot.conf` uses UTF-8 `key=value` records:

```text
format=hyper-boot-v1
default=alpine
```

Blank lines and lines beginning with `#` are ignored. Version 1 accepts exactly
`format` and `default`; duplicate and unknown keys are errors. A VM name is 1
to 64 ASCII alphanumeric, dot, underscore, or hyphen characters, but cannot be
`.` or `..`. The selected bundle path is `hypervisor/vms/<default>.cpio`.

This single-default policy is an initial boot policy rather than a limitation
of the archive layout. A later VM manager can add an explicitly versioned boot
catalog format for multiple autostart VMs without changing bundle v1.

## VM bundle

Each VM bundle is another uncompressed `newc` or `crc` CPIO archive. It keeps
large opaque payloads separate from policy metadata:

```text
manifest
kernel/Image
initramfs/initramfs.cpio.gz
```

The v1 manifest is strict UTF-8 `key=value` data:

```text
format=hyper-vm-v1
type=linux
architecture=aarch64
memory=134217728
vcpus=1
command_line=console=ttyAMA0 earlycon=pl011,mmio32,0x09000000 rdinit=/init
kernel=kernel/Image
initramfs=initramfs/initramfs.cpio.gz
```

All fields are required except `initramfs`, which may be absent or empty. Paths
must be relative and cannot contain empty, `.` or `..` components. `memory` is
a decimal byte count, at least 64 MiB, page aligned, and a power of two.
`vcpus` must be nonzero. The package parser preserves `type`, `architecture`,
and `vcpus` without applying the current machine's execution limits. This keeps
the storage ABI independent from a particular runner or host configuration.
The current AArch64, RISC-V, and x86-64 Linux runners support one vCPU and
contiguous RAM from 64 MiB through 1 GiB, and report distinct runtime errors
for other valid bundles.

The package layer verifies that selected archive entries are unique regular
files. The selected runner then validates guest-specific requirements; the
selected Linux runner requires the standard `ARMd` or RISC-V `RSC\x05` Image
magic, or a 64-bit relocatable x86 bzImage header with the configured preferred
load address. HypeR does not decompress the guest initramfs: Linux receives
those bytes and handles its own supported compression formats.

## Integrity and licensing

The CPIO layer provides bounds and structural validation, not authenticity.
Production deployments should use FIT hashes and signatures to authenticate
HypeR, the DTB, and the complete outer ramdisk as one boot configuration.
Future per-bundle signatures can be added through a new catalog or manifest
version without weakening FIT verification.

Guest kernels and userspace retain their upstream licenses. They are generated
test artifacts, ignored by Git, and are not part of HypeR's Apache-2.0 source.
