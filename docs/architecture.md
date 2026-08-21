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
