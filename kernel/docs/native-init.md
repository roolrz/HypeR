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

The kernel process loader accepts little-endian AArch64 and RISC-V ELF64 images branded
with HypeR ELF OSABI 63, ABI version 0, and either `ET_EXEC` or `ET_DYN` type.
Every load segment must be readable, use at most 4 KiB alignment, be
page-congruent with its file offset, remain nonempty in memory, and be free of
page-level overlap. Writable and executable permissions are mutually
exclusive, and the entry point must lie in executable segment memory.
The executable and interpreter must match the running architecture. RISC-V
images use LP64D, permit compressed instructions, and reject other ELF flags;
their entry point may be two-byte aligned. AArch64 entry points are four-byte aligned.

Static images reject an interpreter, dynamic dependencies, text relocations,
nonempty TLS segments, an executable stack, symbol-based relocations, and
unsupported relocation tables. Static PIE images may use
architecture-matching `R_AARCH64_RELATIVE` or `R_RISCV_RELATIVE` RELA entries,
and RELR entries. Relocation targets
must be aligned, unique, and contained in writable declared segment memory;
relocations can never modify code or a read-only segment.
The process loader also recognizes an absolute `PT_INTERP` path. It maps the
trusted interpreter at 256 MiB and transfers the main image's program-header,
entry, and interpreter-base values through standard auxiliary entries. The
userspace interpreter performs eager architecture-specific symbol relocation, seals
`PT_GNU_RELRO`, and resolves dependencies relative to a delegated `/lib`
Directory capability. A dynamic main image must expose its mapped program
header table through one consistent `PT_PHDR` entry. Main images remain below
the interpreter, and runtime libraries occupy a separate range beginning at
512 MiB.

The current process layout reserves the user range from 1 MiB through 4 GiB.
An `ET_DYN` image is biased so its lowest mapped page begins at 2 MiB. Total
segment mappings and input image size are each limited to 64 MiB. The initial
Thread receives a 256 KiB read/write stack below `0xffff0000`, separated from
the image by an unmapped guard page. Its 16-byte-aligned entry stack follows
the LP64 System V ordering for `argc`, `argv`, `envp`, and `auxv`. HypeR-private
auxiliary entries point to a bounded array of generated, fixed-width startup
handle records. TLS starts at zero. Before application entry, the SDK CRT reserves
`[0xe0000000, 0xf0000000)` under ROOT_VMAR for the process heap. This is separate
from the loader's `[0x20000000, 0xe0000000)` library range; backing pages are
mapped only on allocation. The kernel still owns the root address space and
resource accounting; allocator policy lives in `sdk/lib`.

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
`TaskFactory`, root and `/lib` `Directory` capabilities, system `TaskInspector`
and `ObjectInspector` views, the process root VMAR, and (when available)
Console. Future handle values are
encoded while unresolved and the complete batch is published before the
initial Thread can run. No self-Process handle is installed in its own table.
The root VMAR and executable File-to-VMO path are delegated with narrowly
typed rights for runtime linking; writable and executable VMO variants retain
disjoint authority ceilings.

The Kernel loads only `/init`; it neither interprets the service manifest nor
preloads system services. Init reads `/etc/hyper/services.json` through its root
`Directory`, validates the complete dependency and capability graph, and then uses the
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
physical Console capability. It routes one foreground client's input, output,
and error channels without defining a generic byte-message envelope. The
initial administrative shell receives those endpoints plus an attenuated root
`Directory`,
TaskFactory, TaskGroup, ResourceDomain, and system-inspection authorities. It
can construct child processes, but cannot widen rights or delegate the
construction authorities again. The shell keeps `DUPLICATE` and `TRANSFER`
only on the inspectors so it can stage an `INSPECT`-only task or object view
exclusively for the corresponding `ps` or `handle` command. Each command gets
fresh handle-backed standard-I/O channels. This preserves duplex, blocking I/O
without polling and leaves later foreground-session handoff to capability
rendezvous without changing physical Console ownership.

The shell also receives one persistent manager connector with exactly
`WAIT|WRITE`. It cannot duplicate that authority. Each `/bin/vmm` launch
creates private control and capability-channel pairs, transfers only their
manager endpoints through the connector, and gives the client endpoints to the
new process. Consequently an attached guest console cannot monopolize the
manager connection or prevent independent lifecycle requests.

Manifest capability purposes are symbolic service-contract names rather than
raw integers. Init resolves each name in the contract selected by the service
image, verifies its expected object kind, and rejects duplicate resolved
purposes before any builder is created. The installed Rust SDK carries these
typed contracts in `hyper-service`; numeric startup values remain an internal
wire property at the Process boundary.

The manifest schema's optional `initial-vm.image` field selects an initial
guest by canonical absolute path; the production init profile currently
requires it. The validated launch plan carries that path unchanged. Init opens
it through the root Directory capability and transfers an opaque File
capability to the unique service that declares the VM provisioning startup
contract. VM services do not assign guest identity from image paths.

Every dynamically linked service receives the loader's library Directory with
exactly `READ|EXECUTE`. Services which construct child processes receive a
second, explicitly declared `process.child-library-directory` capability with
the transport rights required to attenuate it into a child's loader slot. This
keeps ordinary services from inheriting delegation authority merely because
they use the dynamic runtime.

The manifest format reserves restart policies, but the current runtime accepts
only `never` and requires at least one critical service. The physical Console
input and output workers, the session service, and the VM manager are critical;
the interactive shell is replaceable and remains noncritical. Init observes every
service Process and its initial VM instance endpoint in one bounded
`object_wait_many` set without polling. It reports terminal Process information
before releasing each dead supervisor handle. A noncritical service exit is
recorded and removed from the set; a critical exit or failed initial VM stops
the remaining graph. A clean initial-VM shutdown is nonfatal and leaves the VM
manager resident for later provisioning. Reliable restart additionally
requires a monotonic backoff facility. Unsupported supervision graphs are
rejected during preflight before any child is started.

## Validation boundary

Host tests validate archive indexing, path rejection, ELF permissions, layout,
entry points, and supported relocation decoding. Kernel QEMU tests run
standalone mechanism tests and then retire the bootstrap execution. The `test-native` contract separately builds the
Native applications through the assembled SDK, constructs the production
initramfs, and verifies that init loads the manifest, starts the session and
shell Processes, launches `ps` and `handle` through scoped inspection handles,
executes a constructor-bearing shared-object fixture through `dlopen`, runs an
external echo command through the complete physical Console path, and attaches
`/bin/vmm` to the separately buffered guest serial stream. The VM portion must
observe repeated Linux timer wakeups, deliver guest-console input, and detach
through the local Ctrl-] menu.
