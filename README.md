# HypeR

HypeR is an experimental, modular type-1 hypervisor kernel written in Rust. The
current tier-1 target is AArch64, running on QEMU's `virt` machine. The kernel is
`no_std` and `no_main`; assembly is restricted to architectural entry and
low-level state transitions that Rust cannot express safely.

The long-term design leaves room for a hybrid kernel personality and Linux ABI
compatibility without coupling those policies to architecture or device code.

## Current boot path

```text
Linux AArch64 boot ABI
    -> architectural assembly entry
    -> Rust entry
    -> flattened device tree discovery
    -> early platform driver initialization
    -> kernel initialization
    -> idle
```

QEMU passes the DTB address in `x0`, following the Linux AArch64 boot protocol.
The output is a position-independent raw `Image` with the standard 64-byte
AArch64 header. Early assembly establishes deterministic EL2 state and a QEMU
`virt` identity map before Rust discovers the PL011 UART from the DTB.

## Requirements

- Rust 1.97.1 with `rust-src`, `llvm-tools`, and `aarch64-unknown-none`
- LLVM toolchain (Rust uses LLVM's integrated assembler and bundled linker)
- QEMU with `qemu-system-aarch64`
- GNU Make (optional)

Build and run:

```sh
make defconfig
make image
make run
```

`make run` starts the verified four-CPU configuration. Uniprocessor QEMU is not
part of the supported or tested platform matrix.

`make image` generates the default `.config` automatically when none exists.
Use `make config` for an interactive configuration, `make olddefconfig` after
adding Kconfig symbols, or `make defconfig` to restore the tracked QEMU AArch64
defaults.

`make image` produces `target/aarch64-unknown-none/debug/hyper.img`. The ELF is
retained beside it for symbolic debugging.

## Continuous integration

GitHub Actions runs independent required-quality stages for formatting and
Clippy, host unit tests, debug/release bare-metal builds, image ABI validation,
and four-core QEMU boot tests on the `cortex-a72` and `max` CPU models. Build
artifacts contain both the ELF image used for debugging and the raw Linux Image.

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
bootstrap applies its `R_AARCH64_RELATIVE` records before accessing Rust static
data.

## Kernel configuration

The root `Kconfig` declares typed kernel options. The in-tree, dependency-free
host configurator supports boolean, integer, and string symbols, defaults,
integer ranges, and simple boolean dependencies. It writes a Linux-style
`.config`; the default fragment is `configs/qemu_aarch64_defconfig`.

During every build, `build.rs` validates `.config` against `Kconfig` and exports
each symbol to Rust. Enabled booleans become custom predicates such as
`cfg(CONFIG_ARCH_AARCH64)`. Integer and string symbols become value predicates,
for example `cfg(CONFIG_TIMER_HZ = "100")`. All symbols are registered through
`rustc-check-cfg`, so misspelled or undeclared uses are rejected by Clippy.
Values are also available through `env!("CONFIG_...")` and typed constants in
`hyper::config`. The timer tick rate is already sourced from
`hyper::config::TIMER_HZ`, providing an end-to-end configuration check.

Cargo features are intentionally not generated from `.config`: Cargo resolves
features before executing `build.rs`, so dynamically deriving them there would
be too late to affect dependency or feature resolution.

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
between concurrent producers and the end of a drain pass. Installing the early
console flushes records accumulated before UART discovery; rebinding PL011 to
the permanent MMIO mapping preserves the cursor. `CONFIG_CONSOLE_LOGLEVEL`,
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

## Design boundaries

- `arch`: architecture-specific entry and CPU mechanisms.
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
  task/       thread objects and scheduler policy
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
  qemu/       four-core boot, SMP, idle-thread, and timer integration tests
```

The kernel layer consumes only the `arch` facade. Kernel memory policy owns the
boot allocator, image/DTB reservations, and runtime-allocator handoff. The HAL
describes bootstrap reachability, permanent virtual layout, and physical-to-
virtual translation. AArch64 retains only its concrete layout values, page-table
format, mapping construction, TLB maintenance, and address-space activation.

No GPL-licensed runtime code or dependency is used. New dependencies require a
license and `no_std` review before adoption.

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
| Kernel stack | `0x0000_ff00_0100_0000` | Normal-WB, RW, XN |

After TTBR activation, execution, exception vectors, and the stack move to the
high kernel mapping. The DTB is scanned again through the linear map after the
heap is installed, and the PL011 driver is rebound to the MMIO window as a
runtime mapping check.
The architecture then removes transition identity leaves and invalidates the
EL2 TLB on all online CPUs. Identity aliases remain only while PSCI secondaries
execute the physical trampoline and are retired after every admitted CPU has
entered the high mapping. Memory tagged `no-map` is excluded from both temporary and linear RAM
aliases at page granularity.

## SMP startup

The allocation-free FDT pass records enabled `/cpus` children and their `reg`
hardware IDs up to `CONFIG_MAX_CPUS`. The boot CPU assigns stable logical CPU
indices, allocates a private 64 KiB kernel stack and initial idle Thread for
each secondary, and starts it through the architecture-neutral CPU-power
service. Each boot record is cleaned to PoC before CPU_ON because the PSCI
target begins with data caching disabled. AArch64 supplies a position-independent PSCI physical trampoline that
installs deterministic EL2 state, the final TTBR, the high runtime vectors,
`TPIDR_EL2`, and the secondary's virtual stack before entering Rust.

Each secondary matches and wakes its GICv3 Redistributor, initializes its
system-register CPU interface, enables registered PPIs, starts its private
CNTHP_EL2 deadline, and becomes its scheduler-owned idle Thread. Scheduler
current and idle identities are per CPU; ordinary ready Threads remain in the
shared scheduler pool but have an explicit CPU owner. Cross-CPU migration is
deferred until a stopped-thread hand-off protocol exists, preventing one saved
context from being selected by two CPUs simultaneously.
`make test-qemu` boots an existing image with four CPUs and verifies all
secondary idle paths and
recurring timer interrupts on both `cortex-a72` and `max`.

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
to EL2. GIC virtualization state and guest virtual interrupts remain a separate
future hypervisor subsystem.

## Platform driver model

The generic FDT parser does not recognize concrete device compatible strings.
Before allocation, it emits raw node/property events and translated resources
to an AArch64 visitor that claims only the console, root interrupt controller,
architectural timer, and firmware CPU-power interface required to finish boot.

After allocator handoff, the platform bus walks the DTB a second time and builds
heap-backed device names, complete compatible tables, properties, MMIO ranges,
and interrupt cells. Early claims are excluded from normal probing. Platform
drivers register a name and compatible table; the manager performs matching and
probe, records deferred and failed devices separately, and owns bound instances
through suspend, resume, and remove. No ordinary built-in platform drivers are
registered yet.

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
`kthread_create` registers a dormant kernel thread, while the generic
`thread_ready` scheduler transition makes either a dormant or blocked Thread
runnable. This keeps run-queue policy independent of the kernel-thread, user,
and vCPU execution kinds. Fresh kernel threads enter through a common
trampoline; returning from an entry function terminates the thread and
transfers to another ready thread before its stack is reclaimed. Heap
allocation pins each registered Thread independently of scheduler-container
growth.

After initialization, `thread_become_idle` formally converts the bootstrap
execution context into the scheduler's idle Thread. The idle loop schedules
normal ready work first and executes one architectural WFI only when no work is
runnable. A yielding or exiting normal thread falls back to this pinned idle
context, so the scheduler always has a valid running context. Every online CPU
owns a distinct idle Thread and current-thread slot. This milestone remains
cooperative; timer preemption, load balancing and affinity policy, EL0 exception
return, vCPU entry, stack guard pages, and address-space activation remain
future work.
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

`make verify` also validates that debug and release ELF files contain only
`R_AARCH64_RELATIVE` dynamic relocations, that each raw image has a valid Linux
AArch64 header and page-aligned declared memory footprint, and that the linked
atomic helpers contain both LSE and LL/SC implementations. It boots both the
baseline `cortex-a72` and feature-rich `max` QEMU CPU models and requires three
recurring EL2 timer ticks from each.
