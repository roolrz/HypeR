<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Architecture boundaries

HypeR keeps policy above mechanism. A subsystem should expose the smallest
contract its caller needs and keep ownership, lifecycle, address domains, and
hardware effects explicit. This document describes the target dependency
model. It is normative for new code; the migration notes identify existing
debt rather than presenting the target as complete.

## Layer model

Dependencies normally flow downward through these layers:

1. Kernel entry and initialization policy selects boot order and failure
   policy.
2. Kernel services own scheduling, interrupts, timekeeping, memory policy,
   crash handling, physical-device integration, and installed VM lifecycles.
3. Reusable VM code describes guest packages, guest ABI data, virtual-device
   models, interrupt models, and typed exit events without owning global
   kernel state.
4. Architecture-neutral mechanisms in `mm`, `sync`, `time`, `log`, `archive`,
   and similar modules provide reusable implementation building blocks.
5. Architecture-neutral HAL contracts describe reusable capabilities. The
   binary-only selected HAL binds those contracts and narrow kernel-facing
   operations to exactly one architecture backend; it owns neither kernel
   policy nor discoverable-device lifecycle.
6. Architecture, platform, firmware, and physical-driver implementations
   execute machine operations and access registers, instructions, MMIO, or
   assembly.

`src/arch` selects one backend and exposes topical machine mechanisms only to
`src/hal/selected`. The kernel, architecture-neutral implementation modules,
and kernel self-tests consume `crate::hal`; they must not import any
`crate::arch` path. `main.rs` path-maps `hal/selected/mod.rs` into the binary as
`crate::hal`, so the binding is statically selected without making it part of
the reusable library HAL. AArch64 remains Tier 1: a common interface must not
hide behavior required for its correctness or diagnosis.

## Placement rules

- `kernel` owns host policy, resource publication, initialization order, and
  runtime VM/vCPU ownership.
- `vm` owns reusable guest-facing formats and models. It must not own a kernel
  scheduler hook, global VM registry, or selected architecture backend.
- `arch` owns context and exception ABIs, page-table and register encodings,
  instruction execution, CPU-specific behavior, and hardware virtualization
  entry/exit mechanics.
- `hyper::hal` owns reusable capability contracts. `hal::selected` is a thin,
  one-way binary adapter: it may depend on `arch`, but never on `kernel`, and
  must not acquire policy ownership or create an alternate entry path.
- `drivers` owns discoverable physical devices and firmware protocols. Virtual
  devices are VM services or reusable VM models, not physical drivers.
- Early boot code must state which runtime facilities are unavailable. It must
  not silently depend on the permanent heap, scheduler, mappings, or logging.

Architecture-defined data layouts may remain portable and host-testable. The
operation which applies such a layout to hardware belongs to the selected
backend.

## Selected binary HAL

The selected binary HAL exposes eleven enforced capability modules:

- `hal::context` owns schedulable register images, context switching, and the
  final stack-reset transition; task lifecycle and stack mapping remain kernel
  policy;
- `hal::cpu` owns typed logical/hardware CPU identity conversion, secondary
  entry mechanisms, processor lifecycle events, and firmware power binding;
- `hal::exception` owns exception-vector installation, crash-context capture,
  emergency-stack entry, and crash-stop delivery; stop/resume decisions remain
  behind the kernel entry adapters;
- `hal::guest` owns the selected Linux guest boot ABI: guest architecture
  identity, typed IPA layout, image validation/loading, boot context, and
  guest-layout diagnostics. Generic VM bundle parsing remains in `vm`, while
  hardware execution remains in `hal::vm`;
- `hal::irq` owns ordinary reversible local masking, fail-stop source masking,
  host-controller construction, platform interrupt decoding, and targeted
  reschedule notification;
- `hal::memory` owns host stage-1 construction and activation, address
  translation, local execution protection, and synchronous shared stage-1
  invalidation;
- `hal::cache` owns cache geometry and explicit data/instruction publication,
  while `hal::atomic` exposes immutable runtime atomic-backend diagnostics;
- `hal::platform` owns allocation-free essential-device discovery,
  architecture KASLR geometry, optional host port I/O, and runtime
  architecture diagnostics;
- `hal::time` owns the monotonic counter, one-shot comparator, and validated
  kernel timer description;
- `hal::vm` owns stage-2 translation, vCPU entry, virtual interrupt hardware,
  guest timer integration, and architecture-local exit completion. Reusable
  device models live in `vm`; each installed VM owns its mutable instances in
  `kernel::vm::device`. Its
  explicitly named `LegacySyncFrame` path is temporary; new exits cross the
  entry adapter as owned `hyper::vm::exit` events.

Only files below `src/hal/selected` may call the topical `crate::arch`
facades. Conversely, no selected-HAL file may call `crate::kernel`. The rule
also rejects grouped imports, relative paths, and crate-root aliases rather
than checking only familiar symbol names. `tests/ci/check-arch-facades.sh`
enforces both directions for production code and kernel self-tests.

Allocation-free FDT discovery currently keeps its bounded collector scratch on
the active boot stack. Linker bootstrap stacks and post-translation CPU0 boot
stacks therefore retain 256 KiB on every architecture, guarded by source and
linked-image CI checks. Moving that scratch into explicit caller-owned boot
storage is required before this temporary stack margin may be reduced.

## Exception and VM-exit entry adapters

Exceptions and VM exits enter from a low-level implementation but need a
kernel policy decision. This necessary upward transition belongs in one small
adapter per entry class:

1. Decode the raw architecture frame into a typed event.
2. Invoke one explicitly registered kernel service boundary.
3. Encode the returned action into machine state.

The adapter contract must define registration and publication, the fail-stop
case before registration, interrupt and preemption state, reentrancy, allowed
allocation or blocking, frame aliasing and lifetime, and hot-path costs. Other
architecture mechanism code must not call kernel logging, boot failure,
scheduling, IRQ dispatch, or VM policy directly.

## Native userspace boundary

Native user entry follows the same policy-above-mechanism rule without reusing
the vCPU world switch. Complete trap frames, translation registers, return
regimes, and user-context encoding remain architecture-private. The upward
adapter receives an owned syscall invocation and returns a fixed-width result;
it never passes a Rust frame reference into code which can allocate, block,
preempt, or migrate.

The selected `hal::user` facade currently exposes only inert machine-capability
discovery. It deliberately provides no activation or entry operation. The
completed facade will own opaque prepared user address spaces, activation and
deactivation, entry and return, architecture user-address limits, and the
selected local invalidation mechanism. Process lifetime, handles, rights,
syscall numbers, ELF policy, compatibility routing, and resource accounting
remain in the kernel. The earlier identity-only `UserExecution` scaffold was
removed; a runnable replacement must strongly own its Process and prepared
address space.

AArch64 requires two independently validated implementations behind that
facade. VHE uses the host EL2&0 translation regime. The preferred nVHE spike
uses direct EL0, `HCR_EL2.TGE`, and a per-process stage-2 address space; it must
not reuse the VM subsystem's current single-active-vCPU execution lease because
one process may execute Threads concurrently on several CPUs. User stage-2
therefore needs its own resident-CPU set, mapping epoch, shootdown
acknowledgement, VMID generation, and reuse-retirement protocol. An EL1 relay
is a compatibility fallback only if the stage-2-only proof cannot provide a
required ABI semantic.

The kernel exposes one Native ABI. Linux and FreeBSD are initially isolated
EL0 supervisor domains selected transactionally with the process image; foreign
syscalls are not translated into Native syscall calls. The route may later add
one separately audited whole-personality kernel engine, but may not split one
personality syscall-by-syscall across two semantic owners.

The complete object, capability, IPC, ABI, supervised-execution, AArch64 proof,
and implementation contracts are normative in the [userspace and syscall
architecture](syscall-abi.md).

## Construction and publication

Resources follow `prepare -> validate -> publish`. Construction happens in
local owned state, validation completes before visibility, and a failed
pre-publication step rolls back through ownership or an explicit guard. Each
lifecycle transition has one owner and one publication point.

Use `pub(crate)` by default. Add a public interface only for a demonstrated
consumer, and prefer domain types for addresses, identifiers, units, and
lifecycle states when interchange would be unsafe.

Top-level runtime startup invokes one lifecycle entry per subsystem rather
than sequencing that subsystem's internal stages. `kernel::irq::initialize`
publishes host interrupt delivery, `kernel::time::initialize` owns the
clocksource and architectural tick, and `kernel::vm::initialize` activates
hardware virtualization and guest-visible devices. Host timekeeping retains
only the firmware-derived guest-timer source description; VM initialization
owns its host IRQ mapping, handler, rollback, and final publication. Physical-
device setup has two deliberate phases: `device::early_initialize` installs
boot-critical firmware services, while `device::platform_device_initialize`
binds discoverable devices after the core runtime services exist. CPU topology
owns its immutable participation count; timer diagnostics observe that
published state instead of receiving a count forwarded by the kernel entry
path.

The concrete startup order is boot-critical CPU power, memory/allocator,
debug and scheduler, host IRQ/crash/time, one-shot SMP admission, stage-1
address-space sealing, platform drivers, complete VM initialization, and VM
bring-up. Sealing takes the same mutation lock as guarded-stack map/unmap and
retires identity aliases only after every admitted CPU entered permanent high
mappings.

SMP admission publishes a `FrozenTopology` once. `HypeR` has no CPU hotplug:
late replicated-local transactions snapshot this immutable participant set. A
future hotplug implementation must either join the in-flight snapshot or
replay every live local mapping before publishing a new CPU online.

## Kernel RPC and replicated-local IRQs

Crash-stop and scheduler reschedule retain dedicated emergency semantics. All
other cross-CPU work shares one Kernel RPC doorbell: AArch64 SGI 8, x86 vector
`0xf1`, or RISC-V SSIP. Per-CPU Release-published reason bits coalesce the
doorbell while typed users retain independent generation/acknowledgement
mailboxes. The acquire-swap dispatcher drains reasons until stable zero and
services stage-1 shootdown before IRQ lifecycle work. Polling progress hooks
drain the same dispatcher without EOI; a real exception entry performs EOI
exactly once. Thus x86 stage-1 shootdown retains its lock-free generation
protocol without consuming another vector or sharing IRQ-administration locks.

Every CPU owns an immutable typed local-controller capability in its per-CPU
slot. RPC work can touch only that CPU's Redistributor, local APIC, or synthetic
local source; it cannot access shared Distributor or IRQ-domain administration
state.

Late IRQ installation is explicitly two phase. `prepare_shared_mapping`
publishes a non-dispatchable handler record and configures the source disabled
on every frozen participant. The owner must quiesce the source and clear any
pending condition first. After publishing all handler dependencies with
Release ordering, `activate` marks the record dispatchable before enabling it
on every CPU. Final-handler removal remains dispatchable until all CPUs
acknowledge disable. A rejected activation is compensated across the complete
original target set. A rejected final disable enters fail-stop because
independently masked local sources prevent reconstructing the exact prior
per-CPU state. Route failure, timeout, generation exhaustion, or failed
compensation is likewise ambiguous and enters fail-stop while the mapping and
handler context remain pinned.

## Scheduling boundary

The scheduler owns stable pinned `Thread` allocations, lifecycle transitions,
per-CPU run queues, placement metadata, tick accounting, and reschedule
decisions. Its closed class order is `RealTime > Fair > Idle`. Real-time work
uses fixed-priority FIFO, where lower numeric priorities run first and equal-
priority threads do not rotate on a timer tick. Ordinary kernel and vCPU
threads default to Fair. Fair currently uses a replaceable round-robin backend;
`CONFIG_SCHED_FAIR_QUANTUM_MS` selects its quantum, rounded up to at least one
`CONFIG_TIMER_HZ` tick. The public Fair policy deliberately exposes no RR-
specific parameters so a CFS- or EEVDF-like backend can replace it.

Each runnable class owns a distinct intrusive queue. Explicit FIFO and Fair
policy-transition APIs move a ready thread between queues under the global
scheduler lock. A Fair slice expiry moves the running thread to its class tail
only when a Fair peer is ready. Voluntary yield replenishes the slice, while
blocking and interruption by real-time work retain its remainder. Idle threads
never enter an ordinary run queue.

Ordinary kernel threads carry movable placement policy and may retain an
explicit `CpuMask`; creation prefers the calling CPU when admitted, then the
lowest-numbered registered CPU in the mask. Empty masks and masks with no
registered CPU are rejected. Explicit migration and affinity updates move
dormant, ready, and fully stopped blocked kernel threads synchronously under the
global scheduler lock. A running or switch-in-flight thread retains the request
on its own `Thread`; the source CPU commits a switch and the incoming switch tail
publishes target membership only after assembly has saved the complete source
context. vCPU and future user threads do not yet have certified execution-state
migration hooks, while bootstrap and idle threads remain pinned. Automatic load
selection and balancing are deliberately separate future policy.
The kernel always builds the SMP-capable scheduler and per-CPU infrastructure;
the same image remains valid when firmware admits only the boot CPU. There is
no separate uniprocessor configuration or single-CPU scheduler implementation.
Run queues store stable `ThreadId` values and do not own Thread allocations, so
future per-CPU queue locks and load selection need not move object ownership.
Exited threads enter a scheduler-owned intrusive reclamation queue. Reaping
therefore scales with pending exits rather than the lifetime `ThreadId` space,
and releases an allocation only after no CPU retains it as a current or
switching-from thread.

Every `Thread` embeds one generation-tagged wait record. Its
`Idle -> Armed -> Queued -> Completed -> Idle` transaction and intrusive queue
links are updated under the global scheduler lock, so notification, timeout,
and cancellation select exactly one terminal `WaitOutcome`. A resolver which
arrives while the wait is only Armed records its result for the caller instead
of losing the event; a stale generation cannot resolve a later wait on the same
queue. Queueing and wakeup allocate no memory. Condition-based primitives hold
their own IRQ-masking state lock across the condition check and queue
publication; Mutex and Semaphore preserve direct handoff, while Completion
provides counted and permanent-complete states.

Timed waits allocate their timer and callback owner before queue publication,
then use the same wait transaction. The callback context remains heap-stable
until cancellation succeeds or a release/acquire handshake proves that an
already-detached callback has returned. A timer stays on the CPU which armed
it even if the blocked Thread migrates. Early notification or cancellation
uses the handle's queue identity to retire it from that source CPU; remote
retirement deliberately leaves an obsolete earlier comparator programmed and
therefore causes at most one harmless timer interrupt before the source
programs its next deadline. Sleep is policy over this common timed-wait path.
Waits are migratable by default. A blocker retaining genuine CPU-local state
must arm with `CpuLocal` mobility, which rejects reassignment in both Armed and
Queued phases rather than relying on the generic `Blocked` state.

Per-CPU preemption state coalesces class, quantum, and remote-wakeup requests,
tracks explicit disable guards and IRQ nesting, and is online before the local
timer can deliver interrupts. The pending bit is the durable scheduling
condition, not the IPI: Release publication follows ready-queue publication,
Acquire observation precedes the scheduling decision, and consumption occurs
only under the scheduler lock. Its `false -> true` publisher owns target
notification; later publishers coalesce until the target consumes the bit.
When a request is made inside an interrupt on its owning CPU, the active
outermost IRQ already supplies the notification and avoids a redundant
self-IPI. Remote callers never inspect target-local IRQ depth.

A targeted reschedule interrupt is only a prompt to evaluate the pending bit.
Its permanent kernel handler does not schedule or own scheduler state;
controller completion and outermost IRQ accounting precede the scheduling
decision. AArch64 qualifies this complete IRQ-tail seam and consumes requests
on the interrupted Thread's stack. An interrupted vCPU is fully deactivated
before the scheduler switch and reactivated only when that same continuation
resumes. `cond_resched` provides the corresponding cooperative safe point.
Logical CPU routing is validated before scheduler registration. An interrupt
sent during secondary bring-up may precede physical online state, but the
secondary first installs and validates its CPU-local vector register (VBAR_EL2,
STVEC, or IDTR), then initializes local IRQ and timer state. Its first
scheduler-locked idle observation publishes online only after the fallible
queue check succeeds, so the boot CPU cannot reclaim the handoff or enqueue
work into an admitted CPU which has not completed exception entry setup.
Runtime CPU offline and hotplug remain unsupported and may not assume that
bootstrap exception.

Idle entry uses one interrupt-mask ownership interval for its final run-queue
check and architecture wait. RISC-V and AArch64 execute WFI with local delivery
masked so a newly pending interrupt wakes the CPU but remains pending until the
saved mask is restored. x86 uses `STI; HLT; CLI`, whose interrupt shadow makes
the enable-and-sleep boundary atomic. This prevents a wake IPI from being
handled between an empty-queue observation and sleep.

Every scheduling transition masks interrupts before the scheduler publishes
`current = next`. Blocking primitives transfer the outer synchronization lock's
mask into their park token, so releasing that lock cannot expose a logical/
machine owner mismatch. At the final infallible boundary the transition
consumes the CPU-affine guard and supplies its exact saved state to assembly;
the guard itself never survives on a suspended stack. Assembly saves that state
in the outgoing `ThreadContext`, changes to the incoming stack, and invokes a
common completion callback while interrupts remain masked. The callback retires
source `switching_from` ownership and applies any Thread-owned migration request
before assembly restores incoming DAIF, SSTATUS.SIE, or RFLAGS.IF. Passing raw
context pointers across this boundary avoids retaining Rust references while
the callback re-enters scheduler ownership.

RISC-V and x86-64 currently retain cooperative scheduling: their exception
entry does not yet provide the private-stack continuation and complete vCPU
deactivation contract required for asynchronous IRQ-tail switching. Both can
send a targeted wake prompt—an SBI software interrupt on RISC-V and a fixed
x2APIC IPI on x86-64—while retaining the pending request for a later
cooperative point. Each architecture must qualify the IRQ-tail boundary
independently rather than inheriting the AArch64 capability through a
misleading common interface.

## Current migration debt

The present tree still contains direct `src/arch -> crate::kernel` references,
including the narrow exception/VM-exit entry adapters and some remaining boot
and hardware-virtualization integration. This raw entry path is distinct from
the selected HAL: architecture entry code must use explicitly named
`kernel::entry` adapters, while ordinary downward calls use `crate::hal`.

The architecture-boundary CI check records the remaining raw upward debt by
source file and exact kernel contract path, rejecting a new, substituted, or
increased dependency. A migration that removes references must lower the
baseline in the same change so the improvement cannot regress. Architecture
code also may not hide logging dependencies behind the crate logging macros.
This lexical ratchet cannot prove the complete Rust module graph; privacy and
review must also reject macro expansion or indirect re-exports that conceal an
upward dependency.

This ratchet is not an approved dependency direction. It is a temporary
migration guard until typed exception and VM-exit adapters replace the direct
calls, after which the baseline and compatibility allowance must be removed.
