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
5. HAL facades describe narrow capabilities consumed by policy. They neither
   discover devices nor own selected-backend state.
6. Architecture, platform, firmware, and physical-driver implementations
   execute machine operations and access registers, instructions, MMIO, or
   assembly.

`src/arch` selects one backend and exposes topical machine mechanisms. Policy
code must use that facade rather than importing `arch::aarch64`,
`arch::riscv64`, or `arch::x86_64` directly. AArch64 remains Tier 1: a common
interface must not hide behavior required for its correctness or diagnosis.

## Placement rules

- `kernel` owns host policy, resource publication, initialization order, and
  runtime VM/vCPU ownership.
- `vm` owns reusable guest-facing formats and models. It must not own a kernel
  scheduler hook, global VM registry, or selected architecture backend.
- `arch` owns context and exception ABIs, page-table and register encodings,
  instruction execution, CPU-specific behavior, and hardware virtualization
  entry/exit mechanics.
- `hal` owns capability contracts, not discovery, policy, or global mutable
  state.
- `drivers` owns discoverable physical devices and firmware protocols. Virtual
  devices are VM services or reusable VM models, not physical drivers.
- Early boot code must state which runtime facilities are unavailable. It must
  not silently depend on the permanent heap, scheduler, mappings, or logging.

Architecture-defined data layouts may remain portable and host-testable. The
operation which applies such a layout to hardware belongs to the selected
backend.

## Selected architecture facades

The selected backend currently exposes nine enforced topical facades:

- `arch::context` owns schedulable register images, context switching, and the
  final stack-reset transition; task lifecycle and stack mapping remain kernel
  policy;
- `arch::cpu` owns typed logical/hardware CPU identity conversion, secondary
  entry mechanisms, processor lifecycle events, and firmware power binding;
- `arch::exception` owns exception-vector installation, crash-context capture,
  emergency-stack entry, and crash-stop delivery; stop/resume decisions remain
  behind the kernel entry adapters;
- `arch::guest` owns the selected Linux guest boot ABI: guest architecture
  identity, typed IPA layout, image validation/loading, boot context, and
  guest-layout diagnostics. Generic VM bundle parsing remains in `vm`, while
  hardware execution remains in `arch::vm`;
- `arch::irq` owns local interrupt masking, host-controller construction,
  and platform interrupt decoding;
- `arch::memory` owns host stage-1 construction and activation, address
  translation, local execution protection, cache maintenance, barriers, and
  atomic-operation capability reporting;
- `arch::platform` owns allocation-free essential-device discovery,
  architecture KASLR geometry, optional host port I/O, and runtime
  architecture diagnostics;
- `arch::time` owns the monotonic counter, one-shot comparator, and validated
  kernel timer description;
- `arch::vm` owns stage-2 translation, vCPU entry, virtual interrupt hardware,
  guest timer integration, and architecture-local exit completion. Reusable
  device models live in `vm`; each installed VM owns its mutable instances in
  `kernel::vm::device`. Its
  explicitly named `LegacySyncFrame` path is temporary; new exits cross the
  entry adapter as owned `hyper::vm::exit` events.

Policy code and kernel self-tests must not return to the removed flat forms of
these contracts. `tests/ci/check-arch-facades.sh` enforces both call paths and
root-facade exports while the remaining domains migrate independently.

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

## Construction and publication

Resources follow `prepare -> validate -> publish`. Construction happens in
local owned state, validation completes before visibility, and a failed
pre-publication step rolls back through ownership or an explicit guard. Each
lifecycle transition has one owner and one publication point.

Use `pub(crate)` by default. Add a public interface only for a demonstrated
consumer, and prefer domain types for addresses, identifiers, units, and
lifecycle states when interchange would be unsafe.

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
registered CPU are rejected. vCPU threads initially prefer their creating CPU,
and bootstrap/idle threads are pinned. Assignment is still fixed after
creation: migration requires a stopped-thread handoff which removes the thread
from its source queue before publishing it on the target.
The kernel always builds the SMP-capable scheduler and per-CPU infrastructure;
the same image remains valid when firmware admits only the boot CPU. There is
no separate uniprocessor configuration or single-CPU scheduler implementation.
Run queues store stable `ThreadId` values and do not own Thread allocations, so
future per-CPU queue locks and load selection need not move object ownership.
Exited threads enter a scheduler-owned intrusive reclamation queue. Reaping
therefore scales with pending exits rather than the lifetime `ThreadId` space,
and releases an allocation only after no CPU retains it as a current or
switching-from thread.

Deadline sleeps compose one local-CPU timer with a private scheduler wait
queue. An IRQ-safe record lock linearizes expiry against parking, so expiry
before the thread becomes blocked is retained instead of lost. The timer's raw
callback context has a heap-stable address and remains owned until a final
release/acquire completion handshake proves that the callback no longer
borrows it. Blocked-thread migration is not yet supported; a future migration
handoff must also move or remotely cancel the owning CPU's timer.

Per-CPU preemption state coalesces class, quantum, and remote-wakeup requests,
tracks explicit disable guards and IRQ nesting, and is online before the local
timer can deliver interrupts. IRQ handlers only account time and publish
requests. AArch64 consumes them after the outermost IRQ has completed, on the
interrupted Thread's stack. An interrupted vCPU is fully deactivated before the
scheduler switch and reactivated only when that same continuation resumes.
`cond_resched` provides the corresponding cooperative safe point.

Every scheduling transition owns a CPU-affine interrupt-mask guard from before
the scheduler publishes `current = next` until the suspended continuation
resumes. Blocking primitives transfer the outer synchronization lock's mask
into their park token, so releasing that lock cannot expose a logical/machine
owner mismatch. Architecture Thread contexts save and restore DAIF, SSTATUS.SIE,
or RFLAGS.IF at the final machine handoff. A continuation holding a transition
guard therefore cannot migrate; future migration must occur only after the
guard has restored on its owning CPU.

RISC-V and x86-64 currently retain cooperative scheduling: their exception
entry does not yet provide the private-stack continuation and complete vCPU
deactivation contract required for asynchronous IRQ-tail switching. Each
architecture must qualify this boundary independently rather than inheriting
the AArch64 capability through a misleading common interface.

## Current migration debt

The present tree still contains direct `src/arch -> crate::kernel` references,
including the narrow exception/VM-exit entry adapters and some remaining boot,
timing, and hardware-virtualization integration. The
architecture-boundary CI check records this debt by source file and explicit
kernel contract path, rejecting a new, substituted, or increased dependency. A
migration that removes references must lower the baseline in the same change so
the improvement cannot regress. This lexical ratchet cannot prove the complete
Rust module graph; privacy and review must also reject indirect re-exports that
hide an upward dependency.

This ratchet is not an approved dependency direction. It is a temporary
migration guard until typed exception and VM-exit adapters replace the direct
calls, after which the baseline and compatibility allowance must be removed.
