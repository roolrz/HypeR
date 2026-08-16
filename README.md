# HypeR

HypeR is an experimental, modular type-1 hypervisor kernel written in Rust.
AArch64 remains tier 1; an initial RISC-V 64-bit port also runs on QEMU's
`virt` machine and boots a Linux guest through `/init`. The kernel is
`no_std` and `no_main`; assembly is restricted to architectural entry and
low-level state transitions that Rust cannot express safely.

The long-term design leaves room for a hybrid kernel personality and Linux ABI
compatibility without coupling those policies to architecture or device code.

## Current boot path

```text
Linux AArch64 boot ABI
    -> architectural assembly entry
    -> AArch64 Rust bootstrap and flattened device tree discovery
    -> optional command-line early console
    -> permanent mappings, vectors, and kernel stack
    -> start_kernel in src/main.rs
    -> essential architecture initialization
    -> platform driver probing
    -> kernel initialization
    -> U-Boot ramdisk and nested VM bundle loader
    -> stage-2 VM address space and virtual platform
    -> Linux EL1 entry
    -> initramfs /init
```

QEMU passes the DTB address in `x0`, following the Linux AArch64 boot protocol.
The output is a position-independent raw `Image` with the standard 64-byte
AArch64 header. Early assembly establishes deterministic EL2 state and a QEMU
`virt` identity map. UART access is not required to reach Rust or initialize
memory.

## Requirements

- Rust 1.97.1 with `rust-src`, `llvm-tools`, `aarch64-unknown-none`, and
  `riscv64imac-unknown-none-elf`
- LLVM toolchain (Rust uses LLVM's integrated assembler and bundled linker)
- QEMU with `qemu-system-aarch64` and, for RISC-V, `qemu-system-riscv64`
- GNU Make (optional)

Build and run:

```sh
make defconfig
make image
make run
```

Select the RISC-V port explicitly:

```sh
make defconfig ARCH=riscv64
make image ARCH=riscv64
make run ARCH=riscv64
```

The default QEMU command line adds
`earlycon=pl011,mmio32,0x09000000` to the generated DTB `/chosen/bootargs`.
Override `QEMU_BOOTARGS` with an empty value to exercise a silent early boot.

`make image` builds HypeR without embedding a guest. `make guest-assets`
downloads a checksum-pinned Alpine AArch64 kernel and initramfs and creates the
nested CPIO boot ramdisk used by CI. `make run` passes that ramdisk separately,
matching the standard U-Boot `/chosen/linux,initrd-*` handoff. The format is
specified in `docs/vm-bundle.md`. The initial guest exposes one vCPU;
uniprocessor HypeR boot is not part of the supported or tested platform matrix.

`make image` generates the default `.config` automatically when none exists.
Use `make config` for an interactive configuration, `make olddefconfig` after
adding Kconfig symbols, or `make defconfig` to restore the tracked QEMU AArch64
defaults.

`make image` produces `target/aarch64-unknown-none/kernel/hyper.img`. The
canonical ELF, including debugger-only information, is retained beside it.
`make release` does not recompile: it creates `hyper.stripped` by removing only
debug sections from that ELF and verifies that both ELFs produce byte-identical
raw Images.

## Continuous integration

GitHub Actions runs independent required-quality stages for formatting and
Clippy on both architectures, host unit tests, canonical bare-metal builds,
image ABI validation, AArch64 QEMU tests on the `cortex-a72` and `max` CPU
models, and a four-hart RISC-V QEMU test. Runtime tests require the
ramdisk-loaded Linux guest to initialize GICv3 and the virtual Arm timer and
execute `/init`. Build artifacts contain both the ELF image used for debugging
and the raw HypeR Image.

The workflow installs the toolchain pinned by `rust-toolchain.toml`, grants only
read access to repository contents, cancels superseded runs on the same ref,
and always terminates QEMU processes before releasing a runner. The equivalent
complete local validation is:

```sh
make verify
```

Architectural constants live in `src/arch/aarch64/registers.rs`. The host-side
build script exports those values to a generated C-style header and invokes
Clang's integrated assembler for `boot.S`. The raw image is a static PIE; the
bootstrap decodes its standard ELF RELA and RELR relative relocations before
accessing Rust static data.

## Kernel address randomization

`CONFIG_RANDOMIZE_BASE=y` enables AArch64 kernel virtual-address randomization.
The allocation-free `/chosen` visitor reads the firmware-provided 64-bit
`kaslr-seed`; `nokaslr` on `/chosen/bootargs` disables randomization for a boot.
Missing or invalid entropy safely selects offset zero.

The architecture supplies a 512 GiB placement window beginning at
`0x0000_ff00_0000_0000`. The shared KASLR selector mixes the seed and chooses a
2 MiB-aligned offset that leaves the complete image inside that window. Physical
image placement is unchanged. Rust builds the final page tables around the
selected virtual base, then calls the assembly address-space trampoline.

LLVM/lld packs relative relocations into standard `.relr.dyn` metadata. Early
assembly first applies RELA and RELR using the physical load bias. Before TTBR
activation, the trampoline applies RELA for the selected virtual base and adds
the physical-to-virtual slide to every RELR location, publishes the writes, and
branches back to Rust at the randomized alias. Secondary-CPU trampoline address
recovery uses the selected base rather than a compile-time VA.

## RISC-V profile

The initial RISC-V host profile is RV64GC with the H, SSTC, and Zicbom
extensions, Sv39 for HS translation, and Sv39x4 for guest-stage translation.
Firmware must provide SBI base, TIME, IPI, RFENCE, HSM, and SRST services. The
supported board profile is QEMU `virt` with OpenSBI, the legacy PLIC binding,
ACLINT-backed supervisor timers, and an NS16550 early console. Every enabled
hart is validated independently, including a consistent DT-described CBO block
size, before the kernel enables cache maintenance or guest timers.

Rust kernel code retains the built-in `riscv64imac-unknown-none-elf` baseline;
H-extension and floating-point instructions are isolated in architecture
assembly objects compiled by Clang with the soft-float ABI. This prevents LLVM
from emitting F/D instructions in ordinary kernel code while still allowing a
guest RV64GC context to save and restore all floating-point registers. See
`docs/riscv64.md` for the execution contract and current limitations.

## Runtime symbol lookup

The kernel image retains an allocated ELF `.dynsym/.dynstr` pair through lld's
export-dynamic mode. Linker boundaries keep both tables in the permanent
read-only image mapping. The architecture-independent
`hyper::debug::kallsyms` parser validates little-endian ELF64 entries and
resolves the nearest preceding defined function without allocation, locks, a
filesystem, or DWARF.

`kernel::debug::kallsyms::lookup(address)` converts a runtime PC through the
actual KASLR base and returns the raw symbol name, runtime start, declared size,
and function offset. `Symbol` formats in `name+offset/size` form. Rust v0 names
are intentionally returned as stored; demangling is a presentation-layer
concern and must not make lookup itself allocate. Initialization verifies the
mechanism by resolving the randomized address of the exported
`hyper_kallsyms_lookup` entry.

Image construction derives the compact `.kallsyms` payload from the actual
linked function set. A bootstrap link determines the exact record and string
sizes, a second link embeds that exact-size section, and a final generation
refreshes addresses from the resulting ELF. There is no fixed kallsyms
capacity or unused reserved tail.

## Kernel configuration

The root `Kconfig` declares typed kernel options. The in-tree, dependency-free
host configurator supports boolean, integer, and string symbols, defaults,
integer ranges, and simple boolean dependencies. It writes a Linux-style
`.config`; the default fragment is `configs/qemu_aarch64_defconfig`.

During every build, `build.rs` validates the selected configuration against
`Kconfig` and exports each symbol to Rust. `CONFIG_FILE` can select a tracked
configuration without replacing `.config`, allowing automated or out-of-tree
builds to keep their configuration isolated. Enabled booleans become custom
predicates such as `cfg(CONFIG_ARCH_AARCH64)`. Integer and string symbols
become value predicates, for example `cfg(CONFIG_TIMER_HZ = "100")`. All
symbols are registered through
`rustc-check-cfg`, so misspelled or undeclared uses are rejected by Clippy.
Values are also available through `env!("CONFIG_...")` and typed constants in
`hyper::config`. The timer tick rate is already sourced from
`hyper::config::TIMER_HZ`, providing an end-to-end configuration check.

Cargo features are intentionally not generated from `.config`: Cargo resolves
features before executing `build.rs`, so dynamically deriving them there would
be too late to affect dependency or feature resolution.

`CONFIG_CRASH_CONSOLE=y` compiles an allocation-free interactive monitor into
the fatal path. After the crash owner stops the other CPUs and prints their
captured state, the monitor polls the selected emergency UART with interrupts
disabled. It can inspect CPU contexts, call traces, configured memory regions,
live stage-1 mappings, and bounded RAM contents. The default configuration
disables it, and all monitor code, command strings, UART input glue, and live
mapping inspection code are omitted from the resulting image. See
`docs/crash-console.md` for commands and safety constraints.

## Kernel logging

Kernel logging uses Linux-compatible syslog severities 0 through 7: emergency,
alert, critical, error, warning, notice, info, and debug. Call sites use
`pr_emerg!`, `pr_alert!`, `pr_crit!`, `pr_err!`, `pr_warn!`, `pr_notice!`,
`pr_info!`, or `pr_debug!`; `printk!` accepts an explicit `Level`.

Every message is formatted into a bounded record and appended to an
allocation-free byte ring before console output. Records carry a monotonic
sequence number, severity, payload length, and truncation flag. Complete oldest
records are discarded under pressure, readers receive an explicit overrun with
the number of missed records, and the ring remains readable after console
delivery for a future dmesg-style interface.

The serial console owns an independent sequence cursor and loglevel filter.
Only one flusher writes the UART at a time, and neither the ring lock nor the
console-state lock is held during MMIO output. A pending flag closes the race
between concurrent producers and the end of a drain pass. Installing the
optional early console flushes records accumulated before command-line parsing;
rebinding it to the permanent MMIO mapping preserves the cursor. `CONFIG_CONSOLE_LOGLEVEL`,
`CONFIG_LOG_BUF_SHIFT`, and `CONFIG_LOG_LINE_MAX` control the default filter,
ring size, and maximum formatted record length.

The bootstrap page-table policy is intentionally QEMU-specific: the first GiB
is Device-nGnRnE and the 1--4 GiB range is Normal-WB identity-mapped. Rust then
replaces it with a final DTB-derived address space and retires the temporary low
aliases after moving every live bootstrap reference to a permanent mapping.

Expected serial output begins with:

```text
HypeR: early console initialized
```

Early console selection is opt-in. The allocation-free FDT walk copies and
validates `/chosen/bootargs`; the console layer accepts the Linux-style explicit
forms `earlycon=pl011,<address>` and
`earlycon=pl011,mmio32,<address>`. The address must describe a DTB MMIO region
reachable through the bootstrap map. Missing, disabled, malformed, or
unsupported `earlycon` settings never prevent the kernel from booting; messages
remain in the kernel log ring for a later console or dmesg-style reader.

## Design boundaries

- `main.rs`: discoverable Rust kernel entries and top-level phase transitions.
- `arch`: architecture-specific entry assembly and CPU mechanisms.
- `hal`: narrow hardware-policy contracts consumed by architecture-independent code.
- `platform`: firmware data parsing and immutable hardware description.
- `drivers`: device implementations selected from platform description.
- `sync`: architecture-independent locking primitives composed with HAL policy.
- `kernel`: initialization orchestration and kernel policy.

Kernel policy is grouped by subsystem rather than kept as a flat collection of
files:

```text
kernel/
  boot/       boot flow, image metadata, persistent boot state
  cpu/        topology admission, SMP startup, per-CPU lifecycle
  device/     CPU power and platform-bus orchestration
  irq/        exception policy, IRQ domains, kernel timer
  log/        record production and console draining
  mm/         allocator ownership and memory initialization policy
  sync/       scheduler-aware sleeping synchronization primitives
  task/       thread objects and scheduler policy
  time/       monotonic timekeeping and per-CPU software timers
  vm/         guest lifecycle, vCPU state, devices, and memory policy
```

Reusable mechanisms follow the same rule: `mm/boot` and `mm/allocator` separate
early allocation from buddy/slab runtime allocation, while `sync/lock` contains
lock implementations and leaves portable atomics at `sync::atomic`. Public
facades preserve existing API paths where useful, but new internal code should
prefer subsystem-qualified names.

All test-only code and executables live under one top-level hierarchy:

```text
tests/
  host/       host-side unit and subsystem tests
  image/      ELF, Linux Image, PIE, and atomic-backend validation
  kernel/     feature-gated bare-metal scheduler, synchronization, and stack tests
  qemu/       four-core host boot and Linux guest-init integration tests
```

Normal images contain no test routines. QEMU test targets enable the
`kernel-self-test` Cargo feature, which includes `tests/kernel` and executes
real kernel-thread context-switch and guarded-stack tests before entering the
guest.

The kernel layer consumes only the `arch` facade. Kernel memory policy owns the
boot allocator, image/DTB reservations, and runtime-allocator handoff. The HAL
describes bootstrap reachability, permanent virtual layout, and physical-to-
virtual translation. AArch64 retains only its concrete layout values, page-table
format, mapping construction, TLB maintenance, and address-space activation.
The RISC-V port follows the same contract with private Sv39/Sv39x4 formats;
GIC/vGIC and Arm timer models are no longer compiled into that target.

No GPL-licensed code is incorporated into the Apache-2.0 HypeR source. The
generated, ignored guest payload is an external Linux/Alpine binary with its
own license and source-availability obligations; see `tools/guest/README.md`.
New Rust dependencies require a license and `no_std` review before adoption.

## CPU power management

The architecture-neutral `hal::cpu_power::CpuPower` contract describes CPU
start, local CPU off and suspend, affinity state queries, and system shutdown
or reset. Hardware IDs, physical resume addresses, opaque suspend states, and
capability reporting are typed independently from any firmware calling
convention. Kernel callers use `kernel::cpu_power`; they do not issue firmware
calls or select an architecture conduit directly.

AArch64 early-platform discovery recognizes the standard `arm,psci-0.2` and
`arm,psci-1.0` bindings and records the declared SMC or HVC conduit. The shared PSCI driver
under `drivers::power` decodes standard status values, checks the runtime
version, probes function support through `PSCI_FEATURES`, and selects SMCCC32 or
SMCCC64 function IDs from an architecture conduit contract. AArch64 owns only
the actual SMC/HVC instruction and register-clobber bridge; a future Armv7 port
can reuse the driver with a 32-bit conduit. Calls snapshot the immutable
controller before leaving the lock, which prevents a successful, non-returning
CPU_OFF operation from abandoning a global lock. Legacy PSCI 0.1 DT nodes with
platform-specific function IDs are rejected explicitly.

## Early memory initialization

The allocation-free FDT pass records RAM, the reservation map, `/reserved-memory`,
`no-map` attributes, and available device register ranges. A node with `status`
is available only for `ok` or `okay`; disabled, reserved, failed, and unknown
states are excluded. Simple-bus `ranges`
with one- or two-cell addresses are translated to the root physical domain;
three-cell PCI resources are left to a future PCI subsystem. A fixed-capacity
boot allocator reserves the kernel and DTB before allocating the final
page-table hierarchy and kernel stack. Allocations are restricted to RAM
reachable through the architecture-reported bootstrap map. Linker-derived
kernel image segments are described by `kernel::image`; `kernel::memory` owns
reservations and hands the allocator to the architecture only while page tables
are constructed.

The final non-VHE EL2 address space uses a 48-bit VA and 4 KiB granule:

| Region | Virtual address | Mapping policy |
| --- | ---: | --- |
| MMIO window | `0x0000_1000_0000_0000 + PA` | Device-nGnRnE, RW, XN |
| RAM linear map | `0x0000_4000_0000_0000 + PA` | Normal-WB, NX; kernel pages preserve RO/RW |
| Kernel image | `0x0000_ff00_0000_0000 + offset` | text RX, rodata R+XN, data/BSS RW+XN |
| Bootstrap stack | `0x0000_ff80_0000_1000` | 64 KiB transition stack; unmapped lower guard page |
| Runtime stack arena | `0x0000_ff80_0020_0000` | Guarded thread and per-CPU exception-stack slots |

After TTBR activation, execution, exception vectors, and the stack move to the
high kernel mapping. The DTB is scanned again through the linear map after the
heap is installed. The selected early PL011 is promoted to runtime ownership:
its RX, receive-timeout, and error sources are connected to the kernel IRQ
domain while the firmware baud and line settings are preserved. That UART is
reserved from ordinary platform probing; any additional PL011 instances remain
normal platform devices.
The architecture then removes transition identity leaves and invalidates the
EL2 TLB on all online CPUs. Identity aliases remain only while PSCI secondaries
execute the physical trampoline and are retired after every admitted CPU has
entered the high mapping. Memory tagged `no-map` is excluded from both temporary and linear RAM
aliases at page granularity.

## SMP startup

The allocation-free FDT pass records enabled `/cpus` children and their `reg`
hardware IDs up to `CONFIG_MAX_CPUS`. The boot CPU assigns stable logical CPU
indices, allocates an exact page-backed guarded kernel stack and initial idle
Thread for each secondary, and starts it through the architecture-neutral
CPU-power service. Each boot record is cleaned to PoC before CPU_ON because the
PSCI target begins with data caching disabled. AArch64 supplies a
position-independent PSCI physical trampoline that installs deterministic EL2
state, the final TTBR, the high runtime vectors, `TPIDR_EL2`, and the
secondary's virtual stack before entering Rust.

Each secondary matches and wakes its GICv3 Redistributor, initializes its
system-register CPU interface, enables registered PPIs, starts its private
CNTHP_EL2 deadline, and becomes its scheduler-owned idle Thread. Scheduler
current and idle identities are per CPU; ordinary ready Threads remain in the
shared scheduler pool but have an explicit CPU owner. Cross-CPU migration is
deferred until a stopped-thread hand-off protocol exists, preventing one saved
context from being selected by two CPUs simultaneously.
`make test-qemu` boots the image with four host CPUs, verifies all secondary
idle paths and recurring EL2 timer interrupts, and then requires the
ramdisk-loaded Linux guest to execute `/init` on both `cortex-a72` and `max`.

## Interrupt controller

AArch64 early-platform discovery recognizes `arm,gic-v3`, preserves the ordered
Distributor and Redistributor `reg` tuples, and records optional redistributor
region and stride properties. The driver layer owns GICD/GICR MMIO initialization, Redistributor
affinity matching, SPI/PPI configuration, routing, enable/disable, acknowledge,
RWP completion waits, and end-of-interrupt semantics. The AArch64 layer separately implements the
`ICC_*` system-register CPU interface and MPIDR affinity encoding.

The boot CPU initializes the shared Distributor and its local interface; every
secondary initializes its matching Redistributor and local interface. A 2 KiB-aligned runtime EL2 vector table preserves the complete integer
and SIMD context, dispatches physical IRQs through dynamically allocated IRQ
domains, and applies an explicit fail-stop policy to other exception classes.
A private BRK round trip validates the vector and `eret` path before IRQs are
unmasked.

IRQ domains allocate global virtual IRQ numbers independently from hardware
INTIDs. Mappings and shared handler lists grow with fallible `try_reserve`
operations outside interrupt context. Registration returns an explicit handle;
unregistering the final handler masks the hardware line, after which the empty
mapping and domain may be removed. Dispatch performs no allocation and invokes
every shared handler with its driver-owned context value. A line that remains
unhandled for eight consecutive
deliveries is masked and quarantined to prevent an interrupt storm. Handlers
run with the registry stabilized and therefore may not mutate IRQ registration
from inside their callback. Registered PPI priority and trigger state is replayed
on every Redistributor. Dynamic PPI lifecycle transitions after multiple CPUs
are online return an explicit error until cross-CPU calls can update every local
instance coherently.

AArch64 early-platform discovery also decodes the hypervisor physical timer
interrupt from the Linux `arm,armv8-timer` binding. The kernel runs CNTHP_EL2 at 100 Hz, rearms
against absolute counter deadlines to avoid cumulative drift, and routes PPI 26
through GICv3 on QEMU. HCR_EL2 routes physical IRQ, FIQ, and SError exceptions
to EL2.

The initial Linux VM owns manifest-configured contiguous guest RAM behind a 39-bit-IPA
stage-2 address space. HypeR builds a Linux-format guest DTB describing one
vCPU, PSCI over HVC, GICv3, the Arm architectural timer, and an emulated PL011
console. The guest UART is no longer a stage-2 passthrough alias of the host
UART: TX is routed to the host console backend, physical RX interrupts feed the
virtual receive FIFO, and the model injects guest SPI 33 through the vGIC.
GICD/GICR accesses are emulated, `ICC_SGI1R_EL1` self-IPIs are delivered through
the vGIC model, and the hardware-assisted virtual timer injects PPI 27. The
external Alpine kernel and deterministic initramfs are loaded from a versioned
VM bundle inside the firmware ramdisk; disk and network devices are not part of
this milestone.

The console backend is kept outside the virtual PL011 register model. This is
the boundary where a later Linux driver domain can provide a byte stream without
making the guest device model depend on a physical UART driver. Virtio-console
is intentionally deferred until there is one reusable virtio-mmio transport
with feature/status negotiation, split virtqueues, checked guest-memory DMA,
reset, notification, and interrupt handling. That transport should be shared by
console, block, and network devices; implementing a console-only subset would
create an incompatible protocol island and would not help the planned Linux
storage/network driver-domain design.

Virtual device state and virtual interrupt scheduling live under `src/vm`;
`src/drivers` contains only physical devices and firmware interfaces. The
physical NS16550 driver supports byte- and word-wide MMIO with explicit
register shift, line and baud programming, FIFOs, receive error reporting,
modem state, and interrupt control.

## Platform driver model

The generic FDT parser does not recognize concrete device compatible strings.
Before allocation, it emits raw node/property events and translated resources
to an AArch64 visitor that claims only the root interrupt controller,
architectural timer, and firmware CPU-power interface required to finish boot.

After allocator handoff, the platform bus walks the DTB a second time and builds
heap-backed device names, complete compatible tables, properties, MMIO ranges,
and interrupt cells. Early claims are excluded from normal probing. Platform
drivers register a name and compatible table; the manager performs matching and
probe, records deferred and failed devices separately, and owns bound instances
through suspend, resume, and remove. PL011 register definitions, line and baud
configuration, FIFO levels, polling I/O, modem/flow control, DMA controls, and
interrupt masks/status/acknowledgement live in the reusable physical driver.
The selected console has a dedicated runtime owner; other PL011 instances bind
through the ordinary `arm,pl011` platform entry after the allocator is
available. A serial device's presence still does not implicitly select it as
the kernel console.

## Runtime allocation

Memory ownership is transferred in two phases:

1. The fixed-capacity boot allocator owns memory while constructing page tables.
2. After the final linear map is active, all unreserved RAM is handed to an
   intrusive buddy allocator and the slab heap layered above it.

The buddy allocator supports orders 0 through 18 and stores free-list links in
free pages, avoiding a separately allocated page database during early boot.
The slab allocator provides 16 through 2048 byte power-of-two size classes;
larger or unusually aligned allocations use buddy blocks directly. Empty slab
pages are returned to the buddy allocator. The synchronized slab heap is
installed with Rust's `#[global_allocator]`, and boot validates the interface
with real `alloc::Box` and `alloc::Vec` allocations.

The global heap and kernel-global state use interrupt-safe locks parameterized
by an architecture interrupt-mask policy. The allocator implementation itself
does not import architecture code and remains reusable by future targets.

## Threads and scheduling

The kernel owns an architecture-independent `Thread` object with a stable ID,
name, lifecycle state, private kernel stack, scheduling context, and execution
payload. Payloads distinguish kernel work from EL0 user state and EL1 vCPU
state, so Linux-compatibility process policy and hypervisor guest policy can be
added without changing the scheduler's core object model.

AArch64 cooperative switching preserves the AAPCS64 callee-saved integer and
SIMD registers, FPCR/FPSR, stack pointer, frame pointer, and return address.
Each CPU owns 32 fixed-priority FIFO ready queues selected through a bitmap;
equal-priority threads use round-robin order. Queue nodes are intrusive in the
Thread object, so ready, block, wake, priority-change, and exit transitions do
not allocate. Thread IDs map directly to stable scheduler slots instead of
requiring a linear scan. `kthread_create` registers a dormant kernel thread,
and `thread_ready` enqueues it on its owning CPU. Fresh kernel threads enter
through a common trampoline; returning from an entry function terminates the
thread and transfers to another ready thread before its stack is reclaimed.
The saved `ThreadContext` is kept in the pinned Thread object rather than at the
stack bottom, so an overflow cannot corrupt the state needed by the scheduler.
Secondary CPUs abandon and refill their initialization call chain before
publishing online state and entering the idle continuation at a clean stack top.
The 64 KiB bootstrap stack exists only for CPU0's unusually deep complete
initialization call chain. Before CPU0 becomes idle, it abandons that call
chain and pivots onto the same configurable guarded stack model used by
secondary idle Threads. The
boot vCPU is a normal scheduler-owned `Thread::Vcpu`: its `VcpuExecution`
and shared VM interrupt runtime remain pinned independently of the bootstrap
call chain, and guest synchronous exceptions use that thread's guarded kernel
stack. The bootstrap Thread becomes CPU0's idle Thread after enqueueing the
vCPU.

Kernel thread stacks are dedicated buddy allocations, not heap buffers. Each is
mapped into a private virtual slot above an unmapped lower guard page and
contains a bottom canary plus a fill watermark for peak-usage diagnostics.
Every CPU also owns separate IRQ and emergency/crash stacks. AArch64 leaves the
fixed architectural exception frame on the interrupted stack, then runs IRQ
dispatch and timer callbacks on the CPU's IRQ stack; fatal reporting switches
permanently to the emergency stack before stopping peers and dumping state.
`CONFIG_KERNEL_STACK_SIZE_KB`, `CONFIG_IRQ_STACK_SIZE_KB`, and
`CONFIG_EMERGENCY_STACK_SIZE_KB` control the three stack classes, while
`CONFIG_MAX_KERNEL_STACKS` bounds the virtual arena. Thread stacks have a
tested 16 KiB default and minimum; IRQ and emergency stacks default to 32 KiB.
Bare-metal tests verify guard mappings, canaries, high-water marks, exact page
accounting and reclaim, and a real timer interrupt's stack switch.

Scheduler wait queues use the same intrusive node and atomically combine the
Blocked transition with next-thread selection. The kernel synchronization
layer builds FIFO sleeping `Mutex` and counting `Semaphore` primitives on that
operation. Mutex ownership and semaphore permits are handed directly to the
oldest waiter, avoiding lost wakeups and resource stealing. Sleeping acquire
operations reject IRQ-disabled context; IRQ-safe state locks remain the layer
used by interrupt handlers. Feature-gated bare-metal tests perform real
multi-thread context switches covering priority changes, semaphore blocking,
mutex contention, FIFO direct handoff, nonblocking operations, and wait-queue
wake-all behavior.

After initialization, `thread_become_idle` formally converts the bootstrap
execution context into the scheduler's idle Thread. The idle loop schedules
normal ready work first and executes one architectural WFE only when no work is
runnable, allowing a remote enqueue to wake it with SEV. A yielding, blocking,
or exiting normal thread falls back to this pinned idle context, so the
scheduler always has a valid running context. Every online CPU owns a distinct
idle Thread, ready set, and current-thread slot. This milestone remains
cooperative; timer preemption, load balancing and affinity migration, EL0
exception return, multi-vCPU startup and guest timeslicing remain future work.
Kernel initialization uses explicit error propagation and does not use
`unwrap` or `expect`. Since Rust's `GlobalAlloc` deallocation interface cannot
return an error, allocator invariant failures record a stable diagnostic code
and enter an explicit fail-stop path without initiating a panic.

## CPU synchronization and cache maintenance

The HAL defines explicit barrier domains and access classes plus a cache
maintenance contract. AArch64 supplies DMB, DSB, and ISB barriers; reads cache
line sizes from `CTR_EL0`; and implements data-cache clean, invalidate, and
clean-invalidate operations. The HAL describes these in terms of publishing or
discarding cached data, publishing new instructions, and synchronizing each
CPU's local execution context; AArch64-specific
PoC, PoU, shareability, and maintenance-instruction choices remain private to
the architecture implementation. Instruction publication performs the required
data clean, barriers, and instruction invalidation. Every executing CPU then
performs its own context synchronization.

`sync::atomic` exports every stable integer and pointer atomic provided by
`core` together with compiler/hardware fences and an acquire/release
`AtomicFlag`. The image retains the minimal Armv8-A ISA baseline and enables
LLVM outlined atomics. Before any outlined read-modify-write operation, the
single-threaded EL2 entry path reads `ID_AA64ISAR0_EL1.Atomic` and selects the
compiler-builtins LSE implementation when available or its LL/SC fallback.
The verifier requires both instruction paths and the runtime selector to be
present in every image. Rust 1.97.1 still labels `outline-atomics` as an
unstable code-generation feature, so the pinned toolchain and binary checks are
part of this internal compiler ABI contract.
Rust's currently unstable 128-bit integer atomic API is intentionally not
enabled and will not be replaced with an ABI-incompatible lock wrapper.

The runtime choice is system-wide. SMP admission validates that every secondary
supports the boot CPU's selected backend and rejects a processing element
that lacks an already selected extension.

The generic buddy and slab layers depend only on physical ranges and a direct
map base. AArch64 supplies the direct-map layout through `hal::memory`, while
`kernel::memory` performs the boot-memory handoff after address-space
activation.

`make verify` also validates that the canonical ELF contains only relative
relocations, carries standard RELR sections and dynamic tags, and produces a
valid Linux AArch64 Image with a page-aligned declared memory footprint. It
strips only debugger sections into a delivery copy and requires its raw Image
to be byte-identical to the canonical Image. It
checks both linked atomic backends, boots the baseline `cortex-a72` and
feature-rich `max` QEMU CPU models, validates the reported KASLR slide geometry,
and requires recurring EL2 timer interrupts on every CPU.
The QEMU checks also require Linux to select the virtual architectural timer,
finish initramfs unpacking, and print the two deterministic `/init` markers.
