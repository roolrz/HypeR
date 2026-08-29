<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Userspace and syscall architecture

This document defines the target boundary between the HypeR kernel and
untrusted userspace. It is normative for new userspace, capability, process,
IPC, and compatibility work. No syscall number or binary layout described here
is stable yet; stability begins only when an ABI revision is deliberately
published.

The immediate goal is a native EL0 environment capable of hosting HypeR's VMM.
The same boundary must later support Linux and FreeBSD application binaries
without moving POSIX policy into architecture code or making Linux the host OS.

## Design goals

- Keep the EL2 kernel small enough to audit while exposing enough mechanism for
  a real VMM, loader, service manager, and compatibility supervisor.
- Make authority explicit and monotonically decreasing. Integer identifiers,
  object identity, and diagnostic access are never authority.
- Make every capability creation, transfer, publication, and revocation an
  all-or-nothing ownership transaction.
- Keep architecture frames and register layouts private while publishing a
  stable, fixed-width machine ABI.
- Support direct native syscalls. A vDSO is an optimization and compatibility
  surface, not a security gate.
- Preserve an efficient userspace implementation of foreign ABIs without
  preventing a future, separately reviewed in-kernel personality.
- Treat AArch64 nVHE and VHE as Tier-1 execution environments. Native userspace
  must not silently require VHE.
- Reuse the scheduler's existing wait, timeout, cancellation, affinity, and
  migration contracts.

The first implementation is not required to implement the complete target
surface. It must, however, preserve the boundaries and invariants in this
document from its first syscall.

## System model

The kernel exposes one Native ABI. A native VMM and native services use it
directly. A foreign application initially runs through an EL0 compatibility
supervisor which consumes the same architecture-neutral kernel services.

```text
native VMM and services                 Linux / FreeBSD application
          |                                      |
          | Native ABI                  restricted execution exit
          v                                      v
  +-----------------+                 +-------------------------+
  | native adapter  |                 | EL0 compatibility       |
  | and capability  |<----------------| supervisor              |
  | validation      |   Native ABI    | fd, signal, VFS, errno |
  +--------+--------+                 +------------+------------+
           |                                           |
           +-------------------+-----------------------+
                               v
          process, memory, IPC, wait, task, and VM kernel services
                               |
                               v
                 selected HAL and architecture mechanisms
```

Linux used as an I/O backend is a separate, untrusted driver-domain VM. It is
not the compatibility supervisor and receives no ambient kernel authority. The
native VMM communicates with it through bounded channels, memory grants, and
event or backend-session objects; DMA access requires an IOMMU-backed lease.

## Trust and containment

The design assumes that any EL0 caller can:

- issue every syscall number with arbitrary register values;
- race handle operations, mappings, waits, cancellation, and process exit on
  multiple CPUs;
- mutate or unmap user memory during validation;
- exhaust handles, messages, pages, timers, wait registrations, and kernel
  metadata;
- forge object identifiers, stale handle values, ABI metadata, and supervisor
  state; and
- compromise its own compatibility supervisor.

The kernel, selected HAL, architecture implementation, and boot trust chain are
inside the trusted computing base. A compromised compatibility supervisor may
compromise its compatibility domain, but must not acquire another domain's
handles, mappings, accounting budget, or device leases. A compromised Linux
driver VM remains confined by stage-2 translation and the IOMMU.

Speculative state, extended registers, debug state, TLS, and address-space
identifiers are part of the execution boundary. The initial implementation
must save and clear state eagerly when ownership is uncertain. Lazy switching
is permitted only after a separate ownership state machine and physical-
hardware validation exist.

## Layer and module boundaries

The intended production layout is:

```text
abi/native/                 compiler-checked public ABI schema
tools/abi/                  host generator and compatibility checker

src/kernel/entry/user.rs    one upward user-entry adapter
src/kernel/abi/             ABI values, native dispatch, supervised seam
src/kernel/process/         process image, handle table, user threads
src/kernel/capability/      object header, rights, handles, transactions
src/kernel/ipc/             channels, events, wait sets, backend sessions
src/kernel/mm/user_space/   VMO, VMAR, mappings, safe user-copy ownership

src/hal/selected/user.rs    selected user-world machine capability
src/arch/*/user.rs          private frames, roots, registers, entry and return
```

Dependencies flow downward. `kernel::abi` contains fixed-width values and
dispatch policy but no architecture register offsets. `kernel::capability`
does not depend on Channel or VM policy. IPC composes capability transactions;
object-specific behavior remains in its owning subsystem. HAL knows address-
space activation and user entry mechanisms but never Process, syscall numbers,
rights, ELF metadata, or compatibility policy.

The earlier `UserExecution { AddressSpaceId, UserContext }` placeholder was
removed because it could not pin mappings or authorize activation. A runnable
user Thread must retain a strong Process and prepared-address-space capability.
An identity remains useful for diagnostics and TLB tagging, but is not
authority.

## Process image and execution route

Each installed process image has immutable execution identity:

```text
ProcessImage
  machine ABI        AArch64, RV64, x86-64, future AArch32
  ABI family         HypeR Native, Linux, FreeBSD
  ABI revision       opaque family-specific revision
  execution route    NativeKernel or Supervised(session)
  execution address-space set
  initial register state
  vDSO and shared-page selection
```

The route is selected by a trusted loader from ELF class, machine, endianness,
ABI notes, interpreter, launch manifest, and policy. ELF branding is an
untrusted classification hint; it never grants authority. Unknown or ambiguous
images are rejected rather than assigned a default personality.

The route is immutable for one installed image. Initial in-place `exec` support
is limited to the same ABI family, route, and supervision session. Cross-family
or Native/Supervised replacement creates a new Process and transfers only
explicitly designated capabilities; it does not guess how to convert handles,
fds, principals, or restart state.

Exec uses a `PreparedExec` transaction. The kernel prepares mappings, initial
stack, auxiliary vector, TLS, register state, vDSO, and return context. A
supervised route additionally seals an opaque supervisor shadow containing its
close-on-exec fd, signal, and personality state. The transaction then parks and
generation-pins every sibling behind a reversible quiesce barrier; it does not
terminate them. After all fallible work is complete, one infallible commit
swaps the ProcessImage, supervision shadow generation, and current Thread
context, invalidates old resume tokens, and only then retires siblings and old
state. Abort unparks siblings against the unchanged old image. Complete
Thread/Process stop, join, cancellation, and quiescence are prerequisites, not
features to improvise inside exec.

`NativeKernel` and `Supervised` are the initial sealed routes. The internal
shape reserves a future, separately audited `KernelCompat` route if profiling
shows that an entire foreign personality belongs in the kernel. HypeR will not
route individual foreign syscalls partly through userspace and partly through
the kernel: that would split errno, restart, signal, and ordering semantics
across two owners.

## Entry and completion contract

Complete trap frames remain architecture-private. Architecture-neutral code
never borrows a raw exception frame and never learns its field offsets.

For Native calls, the architecture entry adapter:

1. proves that the current Thread owns an active user execution context;
2. copies the syscall number, six argument words, and call site into an owned
   `NativeInvocation`;
3. ends every Rust borrow of the private frame;
4. invokes the one registered `kernel::entry::user` service; and
5. consumes an architecture-private, exactly-once return capability to encode
   `NativeResult` into the same Thread's frame.

The return capability is `!Copy + !Send + !Sync`, is bound to the Thread,
ProcessImage generation, pinned stack, and frame generation, and is consumed
exactly once. Blocking, preemption, and migration may suspend the entry stack,
but no `&mut` frame is passed into code that can block. Dropping an armed return
owner enters a lock-, allocation-, and diagnostic-free fail-stop. Exit,
termination, and successful exec consume it explicitly. Architecture result
encoding is proven infallible before a syscall may publish new capabilities;
an impossible post-publication encoding failure retains the capabilities and
enters fail-stop rather than attempting rollback.

For a foreign restricted Thread, entry produces a `RestrictedExit` reason and
switches that Thread into its supervisor view. The private frame is never
exported. Each exit creates an exclusive, generation-bound
`RestrictedResumeToken` and a fixed-width, read-only, architecture-specific GPR
snapshot in pinned supervisor-only memory. The reason payload includes the
syscall PC convention or fault address and syndrome. FP/SIMD, scalable-vector,
debug, pointer-authentication, memory-tagging, and future register sets use
separate typed, feature-tagged records tied to the same stopped generation.
Resume copies the proposed record once into owned kernel memory, validates that
snapshot without rereading user memory, and consumes the token in one
publication step. A stale, duplicated, remapped, or concurrently completed
token cannot resume execution.

Architecture entry does not interpret syscall numbers, handles, errno, POSIX
signals, or process policy. Kernel services return typed internal errors; the
Native adapter maps them to `NativeStatus`, while foreign policy maps them to
Linux or FreeBSD results.

## Native machine ABI

Native syscall numbers are architecture-independent and are never reused after
publication. The initial calling conventions follow each architecture's
established syscall register convention:

| Architecture | Number | Arguments | Result |
| --- | --- | --- | --- |
| AArch64 | `x8` | `x0`-`x5` | status in `x0`, values in `x1` and `x2` |
| RISC-V 64 | `a7` | `a0`-`a5` | status in `a0`, values in `a1` and `a2` |
| x86-64 | `rax` | `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` | status in `rax`, values in documented caller-clobbered result registers |

`NativeResult { status, value0, value1 }` returns up to two new handles without
a fallible user-memory copy. Auxiliary values are zero on failure. Larger data
uses explicit output buffers.

ABI values use fixed-width integers. Public layouts never contain `usize`, Rust
references, Rust enum layout, implicit bitfields, or an unqualified `bool`.
Structures have C layout, explicit padding, reserved-zero fields, and checked
size and offset assertions. Extensible records have an explicit size only when
older and newer layouts can be interpreted safely. Unknown strict flags are
rejected; a field is flexible only when its schema says so.

The direct trap ABI and the vDSO symbol ABI are both supported. The vDSO
provides recommended C-compatible wrappers and optimized time or shared-page
operations, but the kernel never checks that a syscall instruction originated
there. Each ABI family owns its own vDSO and opaque shared-page protocol.

Native blocking calls use absolute monotonic deadlines and an explicit infinite
sentinel. They report cancellation rather than silently rewinding the PC.
Linux and FreeBSD restart rules remain foreign-personality policy.

## Declarative ABI schema

One compiler-checked Rust data schema under `abi/native/` is the source of
truth. It is dependency-free and can be included by the host generator and
`build.rs`; HypeR does not need a bespoke text parser or a kernel dependency on
a schema library.

Each syscall declaration records at least:

- permanent number, public name, introduced ABI revision, and feature gate;
- ordered argument and result shapes;
- handle object kind, required rights, consume/borrow/produce semantics, and
  resulting rights;
- user-memory direction, linked length, maximum size, and observable
  validation order;
- blocking, cancellation, restart, no-return, and audit classes; and
- strict or flexible flag behavior.

Generation produces kernel dispatch wrappers and metadata, Rust and C
bindings, architecture constants and stubs, vDSO exports, layout assertions,
an ABI reference, and number/name compatibility tests. Generated source is
checked into the SDK-facing locations where appropriate; CI regenerates it and
rejects drift. Semantic validation remains in named handlers and is not hidden
inside an unreviewable generated wrapper.

An ABI revision is explicit process-image metadata. Until revision 1 is
published, the loader rejects unknown revisions and the project may change the
experimental ABI. Publication requires a compatibility policy and frozen
generated reference, not merely a release tag.

## Objects, handles, and rights

An object has a non-reused `Koid` for diagnostics and tracing. A KOID cannot
open an object or authorize an operation.

An object reference has the semantics of `Arc<dyn KernelObject>`. The trait is
sealed, requires `Send + Sync + Any`, and exposes only a header, safe downcast
support, and an infallible zero-active-handle transition. Concrete operations
remain inherent methods on concrete types. Lookup uses `Any::downcast_ref`; an
object-kind tag is never followed by an unchecked pointer cast. One generated
declaration keeps Rust types and ABI object kinds unique and coherent.

Object construction is fallible. Phase 1 must provide an audited no-std,
Apache/MIT-compatible fallible shared owner or a comparably small reviewed
allocation boundary. Untrusted creation paths may not use an infallible shared
owner constructor or an unaudited hand-written reference count.

`HandleValue` is a nonzero, process-local, opaque `u64` containing a slot and a
large generation. Security does not depend on secrecy. Slot reuse never makes
a practical stale handle valid; a slot is retired before generation wrap. A
future 32-bit personality receives its own descriptor namespace and does not
weaken the Native table.

A handle entry contains an object reference, rights, and handle flags. Lookup
under the process handle-table lock validates generation, object type,
supported rights, and required rights, then acquires a typed object reference.
The lock is released before object code, user copy, allocation, or blocking.
A concurrent close does not cancel an already resolved operation.

Rights only decrease through duplicate, replace, or transfer. Generic rights
cover handle mechanics and genuinely common observation:

```text
DUPLICATE  TRANSFER  WAIT  INSPECT
```

Payload and mapping rights retain their literal meaning:

```text
READ  WRITE  MAP  EXECUTE  RESIZE  PIN
```

Task, VM, and hardware rights are named for the exact operation, for example
`START`, `REQUEST_STOP`, `RUN_VCPU`, `INJECT_INTERRUPT`, `GRANT_MEMORY`,
`ASSIGN_DEVICE`, `MAP_DMA`, `ACK_INTERRUPT`, and `REVOKE`. Object kinds declare
their supported mask. `WRITE` never means metadata mutation and never implies
`EXECUTE`. Mapping permission is the intersection of VMO rights, VMAR rights,
object policy, executable provenance, and requested protection.

`handle_get_info` reports handle-local kind, rights, and flags. A fixed
`object_get_basic_info` may report common identity under `INSPECT`. Lifecycle,
memory, peer, task, accounting, and hardware state use typed calls such as
`vmo_get_info` or `resource_domain_get_usage`; there is no topic selector.

The shared-reference count and active-handle count are different. Process handles
and in-transit capabilities are active; wait registrations and internal kernel
references are not. Transfer does not create a false zero transition. The last
active handle may detach state and enqueue an already-reserved teardown item,
but it may not recursively destroy contained capabilities, allocate, block,
log, or perform fallible hardware work. An iterative intrusive worklist drains
nested messages and capabilities without holding one object's lock while
decrementing another. Zero-handle resurrection is forbidden, and `Drop` never
accesses hardware.

Ordinary duplicated handles are not generically revocable. Revocable authority
uses typed lease lineages such as `MemoryGrant`, `DeviceLease`, `DmaMapping`,
and `InterruptSession`. Revocation prevents new descendants and completes only
after DMA stops, IOMMU invalidation, interrupt masking and drain, mapping
removal, and remote acknowledgement. Ambiguous teardown quarantines resources
rather than returning them to allocation.

## Publication and output handles

Capability operations follow `prepare -> validate -> publish`.

Creation of at most two handles reserves table slots and values, constructs and
validates the complete objects and `NativeResult`, proves result encoding
infallible, and publishes the slots at the sole final commit. An error returns
no usable handle. If task termination wins after commit, ordinary Process
teardown closes the handles; the syscall never attempts a second rollback.

A variable-size receive cannot use result registers. It therefore:

1. claims but does not dequeue the immutable message;
2. acquires a quota-charged linear `UserWriteReservation` over the exact output
   ranges and reserves all receiver handle slots;
3. copies bytes and the future numeric handle values while those slots remain
   unresolvable;
4. enters a short, uncancellable final section; and
5. publishes every slot and consumes the message with no remaining fallible
   operation.

The reservation is bound to the address-space identity and ProcessImage
generation, prevents unmap, remap, and protection change until consumed, and
offers checked copy operations rather than Rust references. `ReceiveClaim`
restores the same message at the queue head on cancellation or copy failure. A
fault may leave meaningless bytes or unresolved handle values in the caller's
buffer, but it publishes no authority. Concurrent access to a syscall output
buffer is caller misuse; it can observe `BadHandle`, never acquire authority
early.

## Channels and IPC

A Channel has two ordered endpoints. Each endpoint has one active handle or one
in-transit owner and does not support `DUPLICATE`. Threads in the owning process
share that handle normally. True multi-owner consumption belongs to a future
SharedQueue or dispatcher service, not weakened Channel ordering.

Each mutating operation holds a short endpoint ownership-epoch guard. A send
which transfers exclusive objects first acquires a `TransferClaim` for every
object in KOID order, prevents new mutations, and waits for existing guards to
retire. It then validates all claims, source uniqueness, rights, quota, and
queue capacity before changing any epoch or slot. Epoch updates, terminal
subscription events, source removal, and message publication form one
infallible commit. Failed preparation releases claims without changing epochs
or subscriptions; racing use of a reserved source returns `Busy`, not a false
stale-handle result. The endpoint used to send cannot be transferred in that
message.

Channel send copies user bytes and dispositions into owned memory before taking
object locks. It reserves message and accounting capacity, validates every
source generation, operation, kind, and rights attenuation, moves every source
entry into a private transaction, and publishes the complete message once. Any
pre-publication failure restores every entry to its original numeric handle.

An in-transit capability keeps the object's active-handle count but is not
usable. Send commit releases sender handle-slot charges and acquires one
in-transit charge from the sponsoring domain. Receive preparation reserves
receiver slots and charges; commit publishes them and releases in-transit
charges exactly once. Failure leaves the message and its charges unchanged.
Message destruction releases message, in-transit, and active-handle charges
exactly once. Persistent object memory remains charged to its original sponsor
unless a privileged charge-adoption transaction succeeds.

Channels have immutable, inspectable limits for queued messages, bytes, and
handles. Exhausted endpoint credits return `WouldBlock`; exhausted hierarchical
budget returns `QuotaExceeded`. Neither consumes input. `WRITABLE` means that a
minimum legal message fits; a larger write may still fail. A size-qualified
write reservation or capacity snapshot with an observation sequence prevents
retry livelock for large messages, and waiter ordering must prevent a stream of
small writes from starving an already queued legal large write. A message
retains its sponsoring resource-domain charge until consumption or destruction,
preventing quota laundering through transfer.

Queued endpoints can form active-handle cycles. ResourceDomain sponsorship is
therefore revocable across transfer. `resource_domain_revoke_all`, requiring
`REVOKE`, prevents new sponsored work, detaches endpoints and queues across
processes, and drives the iterative teardown worklist until cycles and leases
are quiescent. Ordinary handle owners do not gain sponsor-revocation authority.
Closing the final ResourceDomain control handle requests the same asynchronous
revocation; terminal observation is published only after quiescence.

There is no native `channel_call` in ABI revision 1. Request/reply libraries use
transaction IDs, Channel operations, and WaitSet. Any future kernel-assisted
call returns an explicit cancellable operation.

## Waiting, signals, and atomic waits

Object kinds expose typed, positive, level state such as `READABLE`, `WRITABLE`,
`TERMINATED`, `PEER_CLOSED`, or `INTERRUPT_PENDING`. The raw mask is namespaced
and validated by object kind; SDK bindings expose `ChannelSignals`,
`ThreadSignals`, and other distinct types. Only Event and EventPair permit
arbitrary userspace signaling. POSIX signals, scheduler wakeups, and IRQ
acknowledgement are separate concepts.

Synchronous `object_wait_one` and `object_wait_many` resolve handles to object
references before registration. `wait_many` creates one wait transaction,
canonicalizes objects, snapshots and registers in KOID order or through a
sequence-validated equivalent, and revalidates every observation before
blocking. Its level-state linearization point and the single winner among
signal, timeout, and cancellation are explicit. Source-handle close does not
cancel the wait.

Asynchronous waiting uses a WaitSet-owned, generation-qualified
`SubscriptionId`, not an arbitrary user key and not the source handle value.
Binding requires `WAIT` on the source object and a distinct `BIND_WAIT` right
on the WaitSet, retains the resolved object, and reserves exactly one
quota-charged event slot. A subscription follows
`Dormant -> Armed -> EventQueued -> Delivered -> Dormant`; publication fills or
coalesces its reserved slot without allocation or blocking, and rearm is legal
only after delivery is consumed. WaitSet size and event slots are bounded.
`WAIT` authorizes dequeue, not subscription mutation.

Subscriptions are object-lifetime operations by default, so source-handle close
or transfer does not cancel them. ChannelEndpoint and revocable-lease
subscriptions are explicitly `OwnerEpoch` operations in the schema; successful
ownership transfer then publishes one `OwnershipLost` terminal event, while
failed transfer changes nothing. This is distinct from implicit close
cancellation and prevents an old endpoint owner from retaining a readiness
side channel.

Every relevant assert and deassert advances the object's observation sequence,
even when no subscription is armed. Bind and rearm atomically compare
`{typed_level_state, sequence}` with event publication. This lets a Linux
`epoll` supervisor detect ready/not-ready/ready transitions between one-shot
deliveries without making native waits edge-triggered.

InterruptSession uses the same observation mechanism, but
`interrupt_ack(session, sequence)` is a typed operation. A stale observation
cannot acknowledge a later interrupt.

Native atomic waits resolve a shared key to memory-object identity plus byte
offset. A private key additionally contains the address-space and resolved
mapping generation, or an equivalent private namespace discriminator. Mapping
removal retains that identity until wait retirement or cancels its waiters, so
unmap/remap cannot alias an old wait. No raw pointer or VA pair alone persists
across sleep. A Linux supervisor may reproduce Linux private-futex identity
separately. The internal design supports requeue and priority inheritance, with
PI state integrated into scheduler ownership. The initial ABI publishes only
operations whose semantics are complete.

## Process, memory, and user-copy safety

Process owns its handle table, immutable current image, threads, address-space
capability, and membership in one TaskGroup and one ResourceDomain. TaskGroup
owns grouped lifecycle and stop. ResourceDomain owns hierarchical accounting.
Neither relationship grants privileged operations.

The bootstrap process receives explicit typed factories such as TaskFactory,
VmFactory, ExecutableAuthority, DeviceAuthority, IrqAuthority, and DmaAuthority,
plus a bootstrap Channel. There is no pseudo self handle or implicit root.

VMO and VMAR provide memory ownership and mapping policy. Executable mappings
require immutable executable provenance; writable authority cannot be upgraded
to executable authority. W^X is the default. A mapping operation consumes
typed user addresses and checked lengths, never Rust pointers.

The existing `UserAddressSpace: ForeignMemory` direction remains mandatory.
Syscall code represents user memory as fixed-width `UserAddress` and
`UserSlice` values. It performs checked arithmetic, bounds lengths before
allocation, copies variable input into owned kernel memory, and validates in
the schema-defined observable order. No Rust reference to user memory survives
a mapping change, lock release, block, or context switch. Ordinary kernel locks
are not held while user access can fault.

ResourceDomain quotas cover at least processes, threads, handles, objects,
kernel bytes, committed and pinned pages, guest pages, IPC messages/bytes/
handles, subscriptions, timers, VMs, vCPUs, device leases, and DMA mappings.
One `ChargeReservation` validates and reserves every affected dimension over
the complete root-to-leaf quota path under a fixed transaction/lock order.
Pending reservations count as usage, partial failure rolls back, a limit cannot
fall below current usage plus reservations, and child limits remain bounded by
ancestors. Process, TaskGroup, and ResourceDomain metadata is charged to the
parent domain. The kernel retains a separate emergency budget which untrusted
domains cannot consume.

## Foreign binary compatibility

Foreign entry is not dispatched by substituting a Native syscall number or by
nested Native dispatch. A compatibility supervisor implements the operation by
making ordinary Native calls against explicit capabilities. It owns Linux or
FreeBSD fd tables, file descriptions, credentials, namespaces, VFS policy,
signals, thread-group rules, errno mapping, restart state, `epoll` or `kqueue`,
ABI-specific auxiliary vectors, and vDSO behavior. Those structures never
alias Native handles or object signals.

One HypeR kernel Thread represents one foreign Thread and alternates between:

- a restricted view containing the foreign application mappings and no Native
  syscall authority; and
- a supervisor view containing supervisor code/state and explicitly granted
  windows into the foreign address space.

These are distinct prepared address spaces and principals, not two conventions
inside one mutually accessible mapping. The mode sidecar is kernel-owned and
absent from the restricted view. Native calls resolve only against the linked
supervisor process's handle table; restricted code has neither Native dispatch
nor a Native vDSO mapping. The foreign fd table is separate from both. A native
call from supervisor mode is valid; the same trap from restricted mode is a
foreign exit. A kick prompts restricted exit or cancels the supervisor's
explicitly interruptible Native wait, but does not itself implement POSIX
signal policy.

Every entry snapshots one immutable `ExecutionPrincipal` containing the active
handle table, prepared address space, ResourceDomain, task/audit identity, and
permitted current-task semantics. A Native Thread uses its Process principal.
Supervisor mode uses the pinned supervisor image's handle table and address
space, but remains a Thread in the compatibility session's TaskGroup and
charges CPU time and default resource use to that session's ResourceDomain.
Audit records carry both identities. Implicit `thread_exit`, `process_exit`,
private-current-address-space atomic wait, and other ambiguous current-task
operations are forbidden in borrowed supervisor mode; the supervisor uses
explicit Thread, Process, target-address-space, or supervision-session handles.

`SupervisionSession` is a revocable typed lease, not an ordinary shared
reference. It pins exact supervisor ProcessImage/address-space generations,
the restricted image set, protected state, TaskGroup, ResourceDomain, and its
lifecycle generation. Supervisor exec must first revoke or migrate its
sessions. Revocation rejects new entry, latches kicks, cancels interruptible
waits, waits for every active Thread to acknowledge exit, invalidates resume
tokens, and only then releases roots and state. Supervisor exit, unhandled
supervisor fault, or lost authority terminates the compatibility domain rather
than retaining stale supervisor state.

The session exposes a rights-limited `TargetAddressSpace` capability. Its typed
copy, atomic-access, map, unmap, protect, COW-clone, and fault-completion
operations are independent of the supervisor's VMAR and validate the target
mapping generation. Optional mapped windows are explicit bounded grants, not
the authority source. This lets the supervisor service foreign pointers,
faults, `mmap`, `clone`, signal frames, and exec without exposing supervisor
pages or publishing incomplete restricted mappings.

Compatibility implementation order is a small `TestCompat` personality first,
then Linux, then FreeBSD. `TestCompat` must use different syscall numbers,
error encoding, restart behavior, initial stack, and vDSO selection to prove
that Native assumptions have not leaked into the route. Linux validation later
uses differential tests which run identical binaries on Linux and HypeR.

## Target Native object and syscall surface

The target revision-1 surface is intentionally broad enough for a real EL0 VMM
and service runtime. Names are design identifiers; numbers and final signatures
are assigned only through the schema review.

| Family | Objects and representative calls |
| --- | --- |
| ABI | `abi_get_version`, monotonic clock read, secure random fill |
| Handles | close, close-many, duplicate, replace/attenuate, handle basic info |
| Accounting | ResourceDomain create/limit/usage/revoke-all, TaskGroup create/request-stop |
| Tasks | Process create/start/exit/terminate; Thread create/start/exit/yield/request-stop/set-affinity; termination observed by wait |
| Memory | VMO create/child/read/write/resize/executable-view; VMAR allocate/map/unmap/protect/destroy |
| IPC | Channel create/read/write; Event and EventPair create/signal; Counter create/read/add |
| Wait | wait-one/wait-many; WaitSet create/bind/rearm/cancel/wait |
| Time and atomic wait | Timer create/set/cancel, sleep-until, atomic wait/wake/requeue |
| Exceptions and supervision | exception endpoint/token, typed register sets, resume; SupervisionSession, TargetAddressSpace, restricted enter/resume/kick/revoke |
| Virtualization | VM create/map/unmap/protect; vCPU create/run/kick/inject; typed architecture state operations |
| Driver-domain resources | MemoryGrant, DeviceLease, DmaMapping, InterruptSession, and later BackendSession/SharedQueue |

Hardware operations consume typed factory or lease handles. MMIO may be exposed
as a non-resizable physical VMO derived from a DeviceLease and mapped through
VMAR. Device detach, DMA unmap, and grant revoke remain explicit asynchronous
lifecycle operations; handle close merely requests safe cleanup.

## AArch64 Tier-1 execution

VHE and nVHE require different machine mechanisms behind one `hal::user`
capability.

On VHE, native EL0 uses the host EL2&0 translation regime with `E2H=1` and
`TGE=1`. Per-process roots contain private user subtrees and share pinned,
user-inaccessible kernel-only table subtrees. Guarded-stack and other runtime
kernel mapping mutations remain serialized and coherent across every active
and future root; a root cannot retire while its kernel stack or exception frame
is active. Returning to a vCPU clears the host-user regime explicitly.
Exception code must never infer the return regime merely from a lower-EL
vector.

On nVHE, the required implementation spike is direct EL0 with `TGE=1`, `DC=1`,
stage-1 translation forced off, and a per-process stage-2 root/VMID. SVC,
faults, and physical interrupts route directly to EL2. User VA equals IPA. The
acceptance contract covers address width, tagged-address behavior, cache
defaults, and Linux ABI address semantics.

The user stage-2 implementation cannot reuse the VM registry's current
single-active-vCPU execution claim: Threads in one Process may run concurrently
on several CPUs. It needs an active-CPU residency set, mapping epoch,
targeted/global shootdown acknowledgements, and VMID generation and retirement.
Activation joins the resident set while observing the current epoch. Mutation
either blocks new admission or requires a late entrant to consume and
acknowledge the new epoch before EL0 execution. Mapping removal cannot free or
reuse a page until the snapshot and every late entrant have acknowledged the
invalidation. Hardware VMID-width rollover requires a system-wide stage-2
invalidation. Descriptor and TLBI mechanisms may be shared with VM code, but
residency ownership may not.

Before the syscall ABI is published, the nVHE spike must verify on
`cortex-a72`, VHE `max`, and physical Armv8 hardware:

- SVC, instruction/data abort, IRQ, and preemption routing;
- stage-2 read/write/execute permissions and inaccessible kernel/MMIO mappings;
- `HCR_EL2` cache defaults, shareability, instruction publication, and
  speculative-transition requirements;
- VMID allocation shared safely with guest VMIDs, TLB invalidation, SMP
  migration, and address-space reuse;
- TLS, FP/SIMD, counter access, cache-maintenance traps, WFI/WFE, debug state,
  and usable VA/IPA width; and
- switching among native EL0, supervisor/restricted views, the EL2 kernel, and
  guest vCPUs without state leakage.

If stage-2-only execution cannot supply required foreign ABI semantics, that
route may use a small immutable EL1 stage-1 relay. Kernel policy sees only
opaque prepared/active user address spaces regardless of backend.

RISC-V and x86-64 implement the same semantic contracts through their native
U/S and ring-3 mechanisms. They do not define the common abstraction by erasing
AArch64's world-regime and translation differences.

## Implementation plan and acceptance gates

### Phase 0: prove the boundary

- land this design, the threat model, and the compiler-checked schema model;
- spike AArch64 VHE and nVHE entry/address-space mechanisms before freezing ABI
  layouts; and
- specify process stop/exec and user-return ownership against scheduler
  migration.

### Phase 1: capability core

- implement object header, KOIDs, active-handle accounting, generational handle
  tables, rights/type resolution, slot reservations, transactions, revocable
  sponsorship, iterative teardown, and quotas;
- establish an audited fallible shared-owner constructor, then use only safe
  reference cloning and `Any` downcasts in the object core; and
- host-test stale handles, wrong type/rights, attenuation, close races,
  generation retirement, allocation failure, and quota rollback.

### Phase 2: first native process

- implement Process/UserThread/UserAddressSpace ownership, complete cooperative
  stop/join/cancellation and fault containment, and AArch64 entry;
- run an embedded static PIE EL0 program through direct syscalls, not a vDSO
  requirement; and
- support temporary unstable debug output, yield, and exit without calling the
  result ABI stable.

### Phase 3: usable Native runtime

- implement VMO/VMAR, Channel/Event/EventPair, WaitSet, clock/timer/sleep, and
  atomic waits;
- start an EL0 init process with multiple processes and Threads; and
- add generated Rust/C bindings and ABI conformance tests.

### Phase 4: EL0 VMM

- replace the current non-removable VM binding with safe leases, vCPU
  retirement, stage-2 teardown, and VMID retirement before exposing general
  lifecycle capabilities;
- expose installed VM/vCPU, interrupt injection, guest mapping, and lifecycle
  through typed capabilities; and
- move VM bundle selection and orchestration policy from the kernel into the
  native VMM without duplicating ownership.

### Phase 5: Linux driver domain

- introduce bounded copy-based backend transport first;
- add MemoryGrant, interrupt, IOMMU, DMA, and DeviceLease revocation; and
- move to zero-copy only after detach and cache/IOMMU ordering are proven.

### Phase 6: foreign personalities

- implement a supervised `TestCompat` route, restricted/supervisor execution,
  session revocation, target-address-space operations, and atomic exec;
- implement Linux with differential syscall, signal, futex, and ELF tests; and
- implement FreeBSD as a separate ABI family, reusing kernel mechanisms but not
  Linux personality state.

Every phase runs the quality gate and all-architecture builds. User-entry work
adds AArch64 nVHE/VHE four-CPU QEMU tests for direct syscalls, invalid pointers,
unknown calls, W^X, address-space isolation, wait cancellation, migration,
IRQ-tail preemption, TLS/SIMD preservation, and process-fault containment. The
existing Linux guest boot remains a regression contract. Cache, TLB, IOMMU,
interrupt, and speculation properties which QEMU cannot prove require physical
AArch64 validation before the corresponding feature is declared stable.

## Open implementation questions

The direction above is settled; these details require implementation proofs
before they become ABI:

- the exact Native rights bit allocation and first published ABI revision;
- the nVHE maximum user address and whether any foreign personality requires
  the EL1 relay fallback;
- the exact x86-64 secondary result registers;
- whether revision 1 includes a dedicated SharedQueue or BackendSession for
  large asynchronous workloads;
- the complete priority-inheritance atomic-wait contract; and
- how many older Native ABI revisions a stable kernel image will support
  concurrently.

These questions remain outside the published ABI until resolved.
