<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native init contract

The production kernel boot path starts one Native userspace process from the
firmware-provided initial ramdisk. Linux guest autostart belongs exclusively to
images built with the `kernel-self-test` Cargo feature.

## Initial ramdisk

Firmware must describe one initial ramdisk through the architecture boot
protocol. The complete range must lie in discovered RAM, remain disjoint from
the kernel and DTB, and fit in the permanent linear map. The kernel reserves
that physical range for the lifetime of the mounted filesystem.

The archive is an uncompressed SVR4 `newc` or `crc` CPIO stream. Mounting
validates the complete archive and builds a fallible, sorted metadata index;
file names and contents remain borrowed from the immutable ramdisk. Paths are
relative, nonempty UTF-8 names. Empty components, `.` components, `..`, leading
or trailing slashes, duplicate canonical names, and a non-directory root entry
are rejected. A conventional `.` directory entry is accepted but is not
indexed.

The archive must contain `/init` as a regular file with at least one executable
mode bit. Missing, non-regular, or non-executable init entries stop kernel
startup.

## Executable image

The initial loader currently accepts little-endian AArch64 ELF64 images branded
with HypeR ELF OSABI 63, ABI version 0, and either `ET_EXEC` or `ET_DYN` type. Every load segment
must be readable, page-congruent with its file offset, nonempty in memory, and
free of page-level overlap. Writable and executable permissions are mutually
exclusive, and the entry point must lie in executable segment memory.

The loader rejects an interpreter, dynamic dependencies, text relocations,
nonempty TLS segments, an executable stack, symbol-based relocations, and
unsupported relocation tables. Static PIE images may use
`R_AARCH64_RELATIVE` RELA entries and AArch64 RELR entries. Relocation targets
must be aligned, unique, and contained in writable declared segment memory;
relocations can never modify code or a read-only segment.
`PT_GNU_RELRO` subranges are not split from their containing `PT_LOAD`
mapping yet, so the userspace toolchain must not treat RELRO as an enforced
permission boundary.

The current process layout reserves the user range from 1 MiB through 4 GiB.
An `ET_DYN` image is biased so its lowest mapped page begins at 2 MiB. Total
segment mappings and input image size are each limited to 64 MiB. The initial
Thread receives a 256 KiB read/write stack below `0xffff0000`, separated from
the image by an unmapped guard page. Its 16-byte-aligned entry stack follows
the LP64 System V ordering for `argc`, `argv`, `envp`, and `auxv`. HypeR-private
auxiliary entries point to a bounded array of generated, fixed-width startup
handle records. TLS starts at zero.

Executable bytes are copied into writable unpublished staging memory,
relocated, then snapshotted into immutable instruction-coherent storage before
their RX mapping is published. Non-executable segments are installed with
their final read-only or read/write permissions. The address space, Process,
TaskGroup, and initial UserThread become visible only after loading succeeds.

## Bootstrap capabilities

The capability substrate defines the typed objects required by the initial
process: `Process`, `TaskGroup`, `ResourceDomain`, `TaskFactory`,
`ExecutableAuthority`, VMO, and VMAR. Long-lived lifecycle objects and the root
VMAR have one concurrency-safe object-publication identity. Handles remain
process-local generation values, rights may only decrease, and a heterogeneous
initial set can use the existing batch reservation and publication transaction.

The production init transaction always reserves and writes five tagged
authorities: the root `ResourceDomain`, root `TaskGroup`, `TaskFactory`,
`ExecutableAuthority`, and root VMAR. When the platform selected a runtime
system console, the same transaction includes a sixth Console authority.
Future handle values are encoded while unresolved and the complete batch is
published before the initial Thread can run. The Console exposes nonblocking,
capability-checked reads and writes; `READABLE` and `WRITABLE` levels compose
with the ordinary object-wait syscall. No self-Process handle is installed in
its own table.

## Validation boundary

Host tests validate archive indexing, path rejection, ELF permissions, layout,
entry points, and supported relocation decoding. Kernel QEMU tests use the
test-only Linux guest path. The `test-native` contract separately builds
`app/init` through the assembled SDK, constructs the production
initramfs, and verifies Native startup and console I/O end to end.
