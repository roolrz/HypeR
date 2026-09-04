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
- `hal::time` owns the boot-entry counter origin, monotonic counter, one-shot
  comparator, and validated kernel timer description;
- `hal::user` owns user-address limits, prepared translation roots,
  CPU-affine activation tokens, user-context entry, and typed return
  completion; process policy and syscall dispatch remain in the kernel;
- `hal::vm` owns stage-2 translation, vCPU entry, virtual interrupt hardware,
  guest timer integration, and architecture-local exit completion. Reusable
  device models live in `vm`; each installed VM owns its mutable instances in
  `kernel::vm::device`. Guest exits cross registered entry services as owned,
  fixed-width events with exhaustive actions; raw frames and backend
  completion state remain architecture-private.

Linux guest architecture identity, IPA layout, image validation/loading, and
boot-register plans live in `kernel::vm::linux`. Its narrow `selected` module
is the guest-ABI build selection point; the HAL only realizes the resulting
register plan and supplies host virtualization mechanisms.

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

Fatal, physical-interrupt, and VM-exit services are immutable one-shot
registrations. Runtime vector installation and vCPU construction require the
corresponding readiness capability; entry before publication halts in the
architecture backend. Decode and completion remain allocation-free, keep local
interrupts masked, and release every raw-frame or backend-state borrow before a
policy callback. Other architecture mechanism code must not call kernel
logging, boot failure, scheduling, IRQ dispatch, or VM policy directly.

On AArch64, Rust computes the complete guest `HCR_EL2` return regime before
each world switch and passes it through the architecture-private guest-run
frame. Guest entry enables stage 2, selects AArch64 EL1, clears host-only
`TGE`/`DC`, and traps WFI/WFE. A trapped guest wait is therefore a typed vCPU
exit: VM policy snapshots pending virtual interrupts and the architectural
timer deadline, then either re-enters immediately or parks the scheduler-owned
vCPU Thread on its stable endpoint.

The guest virtual-timer PPI is a level source. Injection may mask the host
mapping while a list register owns the pending interrupt; maintenance
reconciliation unmasks it only after the virtual interrupt can no longer be
lost. Because a guest can retire that list register before its timer write has
deasserted the physical level, the existing host tick supplies a bounded
recheck for a source retained masked across that edge. The ordinary tick path
pays only one CPU-local relaxed load while no recovery is pending. The AArch64
QEMU contract requires repeated initramfs timer wakeups after `/init`, proving
that delivery survives successive list-register lifecycles rather than only
successful guest entry or the first interrupt.

## Kernel-object ownership

Kernel objects are the standard identity mechanism for service entities with
independent lifetimes or cross-subsystem ownership boundaries. Once an entity
participates in this model, it has one canonical kernel-object allocation.
Kernel code reaches that allocation through direct, counted, typed references;
it does not resolve process-local handle values through a global kernel handle
table. Userspace handle tables wrap the same allocation with process-local
generations, rights, and flags only at the ABI boundary.

Persistent references declare their ownership class. Scheduler residence,
object-to-object edges, userspace authority, and temporary resolved operations
all contribute to the object's total lifetime count while remaining separately
observable. A borrowed Rust reference is scoped to one counted owner and is not
itself an ownership edge. User-authority count is distinct from total lifetime:
closing the final userspace handle may close an endpoint or publish another
object-specific transition while an already resolved operation safely finishes.

The scheduler retains a counted scheduler-class reference to every resident
Thread object. CPU residence and scheduler authority continue to govern access
to mutable scheduling state; an object reference alone never grants that
authority. A user Thread's signals and terminal information remain in its
durable object. System Thread objects currently provide stable identity, while
the scheduler's bounded observations associate that identity with a role and
generation-qualified Thread ID. In both cases, the kernel stack, machine
context, and architecture execution payload are explicitly detached and
retired by the scheduler. A userspace handle can therefore retain a terminated
user Thread tombstone without retaining its execution resources.

The final total-reference release performs only an allocation-free, nonblocking
handoff to object reclamation. An object in the reap-pending state cannot be
upgraded from a weak reference, republished as a userspace handle, or otherwise
resurrected. Hardware and subsystem resources must already be quiescent before
this transition; their typed retirement protocols are not delegated to the
generic object reclaimer. Object destruction may release further references,
but does not recursively execute another object's finalizer on the same stack.

KOIDs, object-directory entries, and diagnostic snapshots confer no authority.
The weak global directory can report bounded metadata and aggregate reference
classes, while durable edges are enumerated by their owning Process, scheduler,
or object subsystem. Temporary operation and diagnostic pins are reported as
counts rather than globally allocated edge records. Multi-page graph scans are
weakly consistent and expose neither kernel pointers nor a lookup path from a
KOID to an operational reference.

Kernel-only objects have no generic conversion into userspace handles. Handle
publication requires an explicit typed publication capability; export-policy
flags are diagnostic metadata rather than the security boundary. Kernel
subsystem-owned edges must not form an unbroken strong-reference cycle. An
intentional subsystem cycle therefore has an explicit retirement step which
breaks it; reverse relationships otherwise use weak references or
generation-qualified identifiers.

Userspace can construct cyclic capability graphs, including handles carried by
Channel messages. Reference counting does not collect those cycles: they can
retain resources, but they cannot resurrect an object or create a use-after-free.
Resource accounting and quotas are the current containment mechanism. A future
reclamation policy may add cycle detection or a collector without changing the
typed kernel-reference and userspace-handle boundary.

Implementation records without independent identity remain ordinary subsystem
state. Run-queue links, per-CPU scheduler state, page-table entries, allocator
blocks, architecture register contexts, wait nodes, and device bookkeeping do
not acquire KOIDs merely because they participate in an object's implementation.

## Native userspace boundary

Native user entry follows the same policy-above-mechanism rule without reusing
the vCPU world switch. Complete trap frames, translation registers, return
regimes, and user-context encoding remain architecture-private. The upward
adapter receives an owned syscall invocation and returns a fixed-width result;
it never passes a Rust frame reference into code which can allocate, block,
preempt, or migrate.

The selected `hal::user` facade owns architecture user-address limits, opaque
prepared roots, CPU-affine activation/deactivation tokens, and acknowledged
local replacement and invalidation. On AArch64, synchronous exception entry
passes an owned invocation to a borrowed Native service scoped to that pinned
machine run. Explicitly classified Never-blocking calls write their fixed-width
result into the private frame and return directly; unknown calls, faults,
preemption, and deferred calls close the active translation and return owned
state to the ordinary Thread continuation. After a true scheduling point, that
continuation reacquires its scheduler pin and current execution payload rather
than retaining a CPU-affine borrow. RISC-V and x86-64 currently reject
native-user entry as unsupported. Process lifetime, handles, rights, syscall
numbers, ELF policy, compatibility routing, residency, and resource accounting
remain in the kernel. The scheduler-owned `UserExecution` strongly retains its
Process, native address space, and per-Thread machine context.

`UserThread` wraps one canonical kernel object rather than maintaining a
parallel task identity. Internal observers and eventual userspace handles share
its KOID, rights ceiling, and level-triggered termination state. Global object
and Process directories retain only weak references. Their cursor-based
snapshots combine object headers with bounded per-Process handle-table pages,
providing a pointer-free diagnostic graph without changing object lifetime or
placing reverse-reference locks on capability hot paths. Rendering is allowed
only from normal kernel context; fatal diagnostics remain lock-independent.

AArch64 provides two implementations behind that facade. VHE uses an immutable
host EL2&0 stage-1 root. nVHE uses an immutable per-process stage-2 root with a
separate resident-CPU set, mapping epoch, acknowledged shootdown, and shared
guest/native VMID allocation and retirement. Both retain old roots and tags
until every cut target acknowledges; safe abandonment leaks published owners
rather than risking reuse. A kernel self-test exercises repeated direct Native
syscall return, register-result validation, deferred-call unwind and re-entry,
contained fault unwind, join, and retirement under both QEMU host regimes. A
loader-backed runtime, blocking and migration qualification, and
physical-hardware validation remain prerequisites for general native userspace.
An EL1 relay remains a compatibility fallback only if direct stage-2-only
execution cannot provide a required ABI semantic.

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

## Runtime allocation

The central runtime heap is the sole owner of buddy free lists, slab headers,
partial-slab topology, and page-owner accounting. After SMP admission freezes
the participating CPU set, the kernel enables bounded per-CPU magazines for
the smallest slab classes. A cached object remains reserved in its central slab
and is represented by one linear ownership token; its backing page therefore
cannot return to the buddy allocator while any magazine retains an object.

Cache access requires a scheduler pin and local interrupt masking around only
the selected slot mutation. Central heap operations never run while a local
slot lock is held. Cross-CPU deallocation places the object in the freeing
CPU's magazine, and memory-pressure paths may detach all magazines before one
central allocation retry. An explicit reclaim pass supports diagnostics and
pressure recovery but is not a teardown barrier while allocations continue.
The current immutable topology has no offline transition; future CPU hotplug
must quiesce and drain a departing CPU before withdrawing its slot.

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
check and architecture wait. RISC-V executes WFI with global delivery masked;
a locally enabled pending interrupt resumes the hart and remains pending until
the saved mask is restored. AArch64 temporarily admits IRQ delivery around WFI
because a masked physical PPI is not required to retire WFI on every
implementation. If an IRQ publishes work in that enable-to-WFI window, the
qualified outermost IRQ tail switches away from the idle Thread; this
continuation can reach WFI only after idle runs again, and it reestablishes the
outer guard's masked state before returning. x86 uses `STI; HLT; CLI`, whose
interrupt shadow makes the enable-and-sleep boundary atomic. Each backend thus
closes its own queue-check-to-sleep race without assuming a cross-architecture
masked-wait behavior.

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

## Logging and diagnostic identity

The kernel log ring timestamps a record when it is produced, rather than when
the asynchronous console drain observes it. Architecture assembly captures a
boot counter before Rust entry; once the clocksource publishes its frequency,
records use boot-relative seconds with microsecond precision. Records emitted
before that publication deliberately use zero. Ring sequence numbers remain
internal cursor and loss-detection state, while console output reports an
explicit warning only when records were missed.

Thread diagnostics copy names into fixed-capacity owned snapshots while the
scheduler owns the source Thread. Logging and crash paths can therefore render
an ID/name pair without retaining a `Thread` reference or reacquiring object
ownership after the scheduler boundary.

## Bootstrap boundary

The only direct `src/arch -> crate::kernel` references are the three selected
architecture bootstrap adapters which construct typed protocol inputs and
transfer permanently into kernel boot. Ordinary exception, interrupt, VM-exit,
failure, and virtualization mechanisms use immutable registered services or
the selected HAL and have no kernel-policy dependency.

The architecture-boundary CI check records the bootstrap references by source
file and exact contract path, rejecting any new, substituted, or increased
dependency. Architecture code also may not conceal logging or policy calls
behind macros, aliases, or indirect imports. This lexical enforcement is
reinforced by facade privacy and review of every upward entry contract.
