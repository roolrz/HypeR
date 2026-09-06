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

The production init transaction reserves and writes only the authorities init
currently consumes: the root `ResourceDomain`, root `TaskGroup`,
`TaskFactory`, BootFs, and (when available) Console. Future handle values are
encoded while unresolved and the complete batch is published before the
initial Thread can run. No self-Process handle is installed in its own table.
Executable-memory and root-VMAR purposes remain part of the Native ABI, but
the kernel does not delegate dormant authority to init before a corresponding
userspace operation exists.

The Kernel loads only `/init`; it neither interprets the service manifest nor
preloads system services. Init reads `/etc/hyper/services.json` through BootFs,
validates the complete dependency and capability graph, and then uses the
one-shot `ProcessBuilder` object to construct each child. A builder owns every
staged startup capability immediately after successful insertion. Starting a
sealed builder publishes the child, atomically replaces the consumed builder
authority with a supervisor Process handle in the parent, and only then makes
the initial child Thread runnable. Init retains `REQUEST_STOP` on every
supervisor and requests termination of all already-started children if graph
launch or later critical supervision fails.

Init retains physical Console management authority. Separate input and output
workers receive only the physical direction and raw byte-channel direction
they require. The session manager owns the peer data endpoints and receives no
physical Console capability. None of the children receives duplication or
onward-transfer authority. This preserves duplex, blocking I/O without polling
and leaves later foreground-session handoff to capability rendezvous without
changing physical Console ownership.

The manifest format reserves restart policies, but the current runtime accepts
only `never` and exactly one critical service. Init blocks on that Process's
termination signal without polling. Multiple critical services require
WaitSet observation; reliable restart additionally requires an observable exit
reason and a monotonic backoff facility. Unsupported supervision graphs are
rejected during preflight before any child is started.

## Validation boundary

Host tests validate archive indexing, path rejection, ELF permissions, layout,
entry points, and supported relocation decoding. Kernel QEMU tests use the
test-only Linux guest path. The `test-native` contract separately builds the
Native applications through the assembled SDK, constructs the production
initramfs, and verifies that init loads the manifest, starts the session
Process, and exposes end-to-end Console input and output.
