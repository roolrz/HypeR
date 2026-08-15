# External Linux guest test payload

`make guest-assets` downloads the pinned Alpine Linux 3.23.5 AArch64 virtual
kernel and initramfs from the official Alpine mirror. It packages them as a
versioned inner VM CPIO and then builds the outer U-Boot ramdisk described in
`docs/vm-bundle.md`. Generated payload files are ignored by Git and are not
Apache-2.0 project source.

The script verifies SHA-256 checksums before extracting the standard
uncompressed AArch64 Linux `Image`. It replaces the distribution initramfs
entry point with the deterministic integration `/init` from `tools/guest`.
The QEMU tests pass `hypervisor-initrd.cpio` with `-initrd`; HypeR itself no
longer embeds a Linux binary. All generated files are placed under
`target/guest/`.

The Linux kernel is licensed under GPL-2.0-only. Alpine packages have their own
licenses. Do not redistribute generated payloads as part of an Apache-2.0-only
source release; retain upstream license and corresponding-source obligations
if binary artifacts containing them are distributed.
