<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native init contract

The production kernel boot path starts one Native userspace process from the
firmware-provided initial ramdisk. Linux guest autostart is userspace policy:
init provisions the VM manager and its isolated VM runtime through Native
capabilities. Kernel self-test images contain no Linux loader or default VM.

The AArch64 reference guest selects GICv2 or GICv3 from the creation lease's
platform metadata. The `aarch64_gic_version` field is 2 or 3 on supported Arm
hosts and zero on other architectures. vm-runtime uses this value to generate
the guest interrupt-controller node; host device-tree addresses are never
copied into guest firmware. Kernel, ABI and SDK consumers must be rebuilt
together for the extended 32-byte platform-info record. Arm guest topology is
fixed by FIT metadata (1..8 vCPUs); RISC-V currently supports one. Init does not
handle guest PSCI: vm-runtime waits on its VM handle for power requests, and
vm-manager owns creation of a fresh instance after guest reset. See
[guest power requests and SMP](vm-bundle.md#guest-power-requests-and-smp).

## Rust runtime

Init uses HypeR's Rust `std`, like the other in-tree applications. It uses
Native SDK bindings for bootstrap handles, capability delegation and service
construction; these operations are not provided by the Rust standard library.
Using `std` does not grant additional authority.

Before `main`, the Native runtime initializes the heap, attaches the initial
thread and initializes runtime capabilities from the kernel-provided startup
record. This does not require init's child services to be running. Standard
I/O uses delegated stream handles when present and otherwise falls back to a
bootstrap Console capability, allowing init to report startup errors before
the console services exist.

The default application build uses dynamic linking, including for init. Its
interpreter and runtime libraries must therefore be available in the initial
ramdisk. Rust `std` support and dynamic linking are separate choices; using
`std` does not require a shared Rust standard-library image.

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

The [Native 64-bit application ABI](syscall-abi.md#native-64-bit-application-address-space-contract)
uses architecture-specific application and system-managed user regions.
Current application limits are 128 TiB on AArch64 VA48 and 128 GiB on RISC-V
Sv39; these are provisional kernel layout policy. The current process
layout grants only the smaller range from 1 MiB through 4 GiB.
An `ET_DYN` image is biased so its lowest mapped page begins at 2 MiB. Total
segment mappings and input image size are each limited to 64 MiB. The initial
Thread receives a read/write, non-executable stack below `0xffff0000`.
The main executable's `PT_GNU_STACK.p_memsz` requests its size in bytes;
a missing header or zero selects 256 KiB. The loader rounds up to a page,
rejects duplicate or executable stack headers, and reserves a dedicated VMAR
with capacity max(initial size, 8 MiB) and an unmapped guard page at each end.
Only the initial extent is mapped. The whole reservation remains above
`0xf0000000`; executable mappings must stay below that stack arena. The interpreter's stack declaration does not override the main
executable. Startup arguments must fit in the selected extent; allocation and
committed-page charges remain subject to the process ResourceDomain limits.
Oversized or invalid requests fail before process publication. Its 16-byte-aligned entry stack follows
the LP64 System V ordering for `argc`, `argv`, `envp`, and `auxv`. HypeR-private
auxiliary entries point to a bounded array of generated, fixed-width startup
handle records. INITIAL_STACK_VMAR plus the initial-stack base/capacity/size
auxiliary entries transfer the guarded reservation to the SDK. Its application
startup view omits this runtime-owned handle. The SDK adopts main and worker
stacks into the same fixed-top, explicit downward-growth abstraction; the
worker arena occupies `0xf0000000` through the main reservation base. TLS starts
at zero. Before application entry, the SDK CRT reserves
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
physical Console capability. It establishes a virtual console before creating
its foreground shell with fresh input, output and error channels. The manager
owns that Process supervisor; shell exit or failure triggers a rate-limited new
client, without restarting the manager or physical workers. Init neither launches
nor monitors the shell as a system service.

The manager receives explicit delegatable root Directory, TaskFactory, TaskGroup,
ResourceDomain, child library and inspection capabilities. It attenuates these
into each shell's startup table. A VM connector is optional; when present, the
manager can duplicate it but each shell gets only `WAIT|WRITE`. Each console
instance keeps its own transport and client lifecycle. The current bootstrap
wires one physical console; additional UART discovery and transport wiring remain
future work. Commands continue to get fresh handle-backed standard I/O, with
blocking multi-object waits rather than polling.

The shell also receives one persistent manager connector with exactly
`WAIT|WRITE`. It cannot duplicate that authority. Each `/bin/vmm` launch
creates private control and capability-channel pairs, transfers only their
manager endpoints through the connector, and gives the client endpoints to the
new process. Consequently an attached guest console cannot monopolize the
manager connection or prevent independent lifecycle requests.

Manifest capability purposes are symbolic service-contract names rather than
raw integers. Init validates the expected object kind, explicit rights and
unique resolved purpose before creating a builder. Built-in console, session,
shell and VM-manager images retain their specialized contracts. Other Native
apps can use standard stdio and process/inspection contracts without an image
allowlist; required rights remain mandatory and optional rights are granted only
when the manifest requests them. A service can therefore receive `WAIT|WRITE`
stdout and `INSPECT` CPU statistics without authority to delegate either.
A read-only std filesystem root needs `READ|INSPECT|DUPLICATE`: the runtime
duplicates directory cursors and opens metadata-capable Files. `WRITE` and
`TRANSFER` remain optional and should be omitted when unnecessary.
Private roles such as `vm.provisioning` do not become generic app capabilities.
The installed Rust SDK carries these typed contracts in `hyper-service`.

The optional `virtual-machines.config` manifest field names the fleet config
file. Init opens it through the root Directory and transfers the File capability
to the unique service declaring the VM provisioning contract. The configuration
contains named definitions and autostart policy; a Native-only manifest can omit
it. Guest image paths are inputs to named VM definitions, not guest identities.

Native apps remain the deployment layer for system policy, including future
power-management policy services. The kernel owns privileged mechanisms and
validates their capabilities; policy decisions need not move into the kernel or
Linux I/O VM. This deployment contract does not claim that Native power-control
operations are already implemented.

Every dynamically linked service receives the loader's library Directory with
exactly `READ|EXECUTE`. Services which construct child processes receive a
second, explicitly declared `process.child-library-directory` capability with
the transport rights required to attenuate it into a child's loader slot. This
keeps ordinary services from inheriting delegation authority merely because
they use the dynamic runtime.

The manifest format reserves restart policies, but the current runtime accepts
only `never` and requires at least one critical service. The physical Console
input and output workers, the session service, and the VM manager are critical;
the interactive shell is a replaceable, noncritical client owned by the session
manager. Init observes every
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
observe Linux userspace startup, deliver guest-console input, and detach
through the local Ctrl-] menu.

The VM manager uses a std worker Thread for blocking capability rendezvous.
A bounded ownership handoff and ByteChannel notification feed its main wait set;
VM lifecycle state remains on the main Thread. Both Threads share the same
Process and authority table; this is concurrency separation, not isolation. The event loop rotates ready
sources and waits indefinitely when idle, using a finite deadline only for an
outstanding runtime exit grace period. It does not periodically poll for clients.

### Runtime loss during guest CPU power transitions

`make test-power-crash ARCH=aarch64` builds explicitly enabled test runtimes
and runs three restart/reclamation cycles at each of these boundaries:

- all four guest vCPUs installed but still dormant;
- a real Linux `CPU_ON` request pending before userspace accepts it;
- a real Linux `CPU_OFF` request accepted after the requesting CPU detached from
  hardware and entered the Off power state (scheduler parking may still race).

The fixture disables VM autostart so deliberate runtime failure cannot trip
init's initial-VM boot lease. Each abrupt process exit bypasses Rust destructors;
the test verifies the selected boundary marker, failed VM state, exact guest-page
reclamation, bounded runtime-memory retention, and successful new instances.
`test-power-crash` and `HYPER_TEST_POWER_CRASH` are test-only build selections;
ordinary runtime binaries contain neither the hooks nor a control protocol for
triggering them. This is QEMU lifecycle coverage, not physical DMA/cache proof.
