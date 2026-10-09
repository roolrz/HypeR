<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Virtual filesystem architecture

HypeR keeps filesystem namespace, path-resolution authority, open filesystem
objects, and the file-data cache in the kernel. A filesystem implementation is
an adapter below that policy boundary: it may execute directly in the kernel
or proxy an external service without changing the Native capability model.
Block devices form a separate backend boundary. A filesystem is not required
to be block-backed, and a block driver is not part of the VFS object model.

The system root is a writable, kernel-resident ramfs initialized from the
immutable boot archive. Namespace traversal, ramfs metadata, storage and
synchronization remain in the kernel. Other filesystem implementations and
block services may run in userspace; neither placement is required by VFS.

## Layer boundaries

`src/fs` contains reusable, policy-free filesystem mechanisms: validated names
and paths, stable backend node identities, metadata contracts, and the
hierarchical RAM filesystem importer. It has no Process, handle, scheduler, or
global-namespace dependency.

`src/kernel/vfs` owns kernel policy: filesystem and mount identity, the system
namespace, capability-relative resolution, Directory and File kernel objects,
resource accounting, and the process-facing service boundary. Syscall parsing
and user-memory validation stay in the Native ABI entry layer or its service
adapter; backend code never observes a Process.

`src/kernel/io_cache` owns bounded file-data residency. It does not resolve
paths or grant authority, and a cache miss never calls back into VFS policy.
CPU cache maintenance remains architecture/HAL work and is unrelated to the
filesystem data cache.

## Capability model

Native applications do not receive an ambient root or current working
directory. A Directory handle is explicit authority to resolve beneath one
location in one namespace view. Absolute-looking paths restart at that
capability's traversal root, not at a hidden system root; parent traversal is
clamped at the same boundary.

A File handle names one opened node and carries only the rights selected when
it was created. Opening may attenuate authority but never amplify it. `READ`
is always required on the source Directory for traversal, and every requested
File right must also be present on that Directory before the result is bounded
by the node's capability ceiling and mount policy. In the credential-free
root filesystem, a new read, write, or executable grant requires at least one
corresponding mode bit (`0444`, `0222`, or `0111`). No user/group identity or
Unix permission-selection policy is implied. Existing open capabilities retain
their rights after mode changes. Creation may grant the creator its requested
rights even when the initial mode is zero. `SET_ATTRIBUTES` and `LOCK_FILE`
are separate rights from data access.

Native reads and writes use explicit offsets; append selects the current end under
the same per-file lock that commits the write. SDK file objects may maintain their own offsets. Unix file-descriptor tables,
credentials and POSIX path semantics are not implied by this Native API.

The namespace strongly owns mounted filesystems. Directory and File objects
hold the namespace view and the resolved mount/node location needed to keep an
open object valid after namespace detachment. Mounts do not strongly reference
their namespace. File-data cache pins own only a content record, which has no
mount, namespace, backend or active-node backreference. Namespace discovery
references to that record and its active node are weak, so content retention
cannot form a namespace--mount--node cycle or acquire file authority.

## Backend contract

A backend receives validated names and opaque, counted node leases. Its results are
bounded owned metadata or caller-provided buffers; it may not retain borrowed
VFS storage. Namespace, mount, handle-table and cache-index spinlocks are
released before backend operations. The shared sleeping content gate spans
regular-file I/O. A later remote adapter may block or perform IPC under that
gate without retaining a spinlock.

Executable lookup has a separate, fallible snapshot contract. Immutable
kernel-resident storage may be borrowed, while a mutable or remote backend must
materialize immutable owned bytes before the loader validates them. The caller
supplies the resource domain that must reserve and retain the owned snapshot's
memory charge; backend errors and quota failures are never collapsed into
"not executable."

The ramfs backend is synchronous and kernel-resident. Import does not require
a userspace server; opening and loading `/init` runs on a scheduled kernel
worker because runtime file operations use sleeping mutexes. Import builds
explicit root, parent, name, and node-identity relations.
Missing ancestor directories are synthesized deterministically, while an
archive that uses a non-directory as an ancestor is rejected before mount
publication.

## File-data cache

The common VFS layer owns file-content coherence. Ramfs nodes embed one
`FileContent`; cacheable remote nodes share a `FileRecord` which owns it. Independent
opens and aliases therefore use the same content gate and identity. Its
sleeping gate protects a non-repeating content revision and optional known
length. Cache entries are keyed by filesystem identity, node incarnation,
content revision and page index. A path or raw reusable inode number is never
a cache key. A clean page also retains an opaque content-owner pin; the portable
cache has no dependency on VFS types. This preserves remote file identity across the
last close and a later open while pages remain, including opens by another
process. Page reclamation releases these pins outside the cache lock. The record
contains only its identity, content state and quota charge; advisory locks and
mount pins remain on the active node. Cached records do not keep unlink busy.
After every record owner disappears, reopening requires a fresh incarnation.
Remote bindings use individually charged storage, released when the binding is
removed. If retained content exhausts a mount sponsor or ancestor quota,
unpublished preparation can request a finite domain-scoped cache sweep and
binding cleanup. Cache admission remains paused through that retry; neither
backend mutations nor handle-publication callbacks are replayed.

Reads, writes, append, resize, open-with-truncate and executable snapshots use
this common gate. A mutation advances the revision and forgets the known length
before calling the backend, including attempts that later return an error:
part of the file may already have changed. Old cache entries remain unreachable
and are reclaimed by ordinary eviction. Open-with-truncate retains its existing
prepare-handle, change-contents, publish-handle transaction. This implementation
serializes operations on the same file for one backend batch; a multi-batch
Native read is still not an atomic file snapshot.

The clean-page residency state machine remains:

```text
Vacant -> Loading(generation) -> Clean(valid length)
                  | failure
                  v
                Vacant
```

Only one caller owns a loading reservation. Backend fill occurs outside the
cache lock, and completion must present the same generation token before it can
publish. A failed or stale completion cannot overwrite a recycled entry.
Published pages are immutable shared owners; eviction removes the index owner
but cannot invalidate an active reader. Payload reservations remain charged
until the last owner releases the page, covering loading, resident and evicted
but still referenced pages. Each production payload owns one physical 4 KiB
page, including partial EOF tails; it is accounted separately from allocator
metadata. Hash buckets index loading and clean entries, a free-slot stack finds
vacant slots, and an indexed minimum heap tracks the oldest clean entry. Hits
refresh the access sequence; fill completion preserves the reservation sequence.
Victim lookup is constant time and heap updates are logarithmic in capacity.
Hash chains contain at most 64 entries, bounding index work while interrupts
are masked. A colliding miss bypasses retention once that limit is reached.
Shrinking metadata may discard entries that would exceed a merged bucket's
limit; growth preserves resident entries.

Cache capacity grows on demand rather than imposing a fixed byte budget. The
`kio-reclaim` kernel thread grows metadata geometrically and reclaims cold clean
pages in bounded batches. It temporarily stops new loading reservations, waits
for existing loaders, and parks the table with a constant-time ownership move.
Allocation, migration and destruction happen outside the cache lock. Reads
bypass retention while the table is parked. Failed growth restores the old
table; pressure can release empty oversized metadata before attempting a small
replacement, leaving a disabled cache if even that allocation fails.

Admission uses allocator-managed RAM, excluding firmware and other unmanaged
regions. Normally the high watermark stops admission at 90% utilization and
reclamation continues to 85%. Small heaps also retain a minimum free reserve.
Checks for each physical payload and conservative metadata reservation occur
under the allocator lock, so concurrent fills cannot overbook cache headroom.
Live non-cache allocations may still consume that headroom. Loading and
reader-pinned pages remain charged until their final release, and load
completion or memory release wakes a pressure-blocked worker. Cache admission
never waits for reclamation: a miss falls back to ordinary backend I/O.

Ordinary allocation failure also permits a bounded, nonblocking attempt to
reclaim at most 64 unpinned clean pages before retrying once. This runs outside
allocator and cache locks and uses only the audited page/content-record
payload, whose destructor performs no filesystem or scheduler operation.
The generic allocator never sleeps to reclaim memory. Audited, unpublished
allocation preparation can additionally request a scheduled reclaim pass for
the actual buddy order or denying resource domain, even when usage is below
the pressure watermark. The worker scans a finite snapshot in bounded batches
and does not wait for pinned pages, in-flight loads or filesystem locks.
Requests use non-repeating generations; cancellation and delayed completion
cannot complete a later request. Request serialization ends before retrying
preparation, while a separate guard keeps cache admissions paused through the
retry. Mutating syscalls are not replayed. Competing ordinary allocations or
unreclaimable fragmentation can still cause a bounded retry to fail.

The memory observer includes cache payloads in used kernel memory and reports
unpinned indexed pages as reclaimable; pinned and loading pages are not
reclaimable.

An evicted payload can be reused only after acquiring unique ownership, with
no remaining reader or weak reference. Its allocation permit follows that
owner throughout the refill. The exact-page allocation can hold either a full
page or an EOF tail without growing. Readers holding an evicted page keep
immutable bytes. Refill, allocation and destruction occur outside the cache
lock; pressure prevents recycled pages from admitting new cache contents too.

The backend-neutral reader preserves consecutive miss ranges up to the native
512 KiB batch, using the caller's existing scratch storage. It issues no read
outside the requested range. Only fully returned pages, or a page-start-to-EOF
tail validated against the known length, are eligible for retention. Partial
cold reads need not populate the cache; partial warm reads can use complete
cached pages. Short backend reads retain their ordinary short-prefix semantics,
and backend errors are never hidden as cache misses.

Backends provide raw range I/O, metadata, stable node identity and a cheap read
admission check. Cached data and cached length still honor a failed or closed
backend once that state is latched; the check does not probe the media on a
cache hit. Remote filesystems use this common reader; it owns no file-page cache algorithm.
Writes continue through the existing synchronous backend path and explicit
`file_sync` durability contract. Dirty-page writeback, external coherence,
direct I/O and shared writable mappings require separate protocols. Destructors
perform no fallible storage work.

## Concurrency and lifecycle

Namespace resolution and file-data backend operations run in scheduled kernel
context. Short, nonblocking active-handle retirement callbacks may also execute
from masked Native handle-close entry.
Namespace traversal retains a node lease before entering file-content policy.
The file operation lock order is common content gate, then backend volume or
node-data lock. Cache spinlocks cover only short residency transitions and never
span backend execution, userspace copy, allocation, destruction or scheduler
blocking. The sleeping content gate may span backend I/O; it is released before
preparing or copying a userspace destination. Backend callbacks must not reenter
the common file operation and acquire the same content gate recursively.

Mount construction is prepare-then-publish. Publication is the only point at
which a namespace can discover the mount. Future unmount removes namespace
reachability before quiescing; existing open objects and operation pins remain
valid. Final destruction releases only already-quiescent memory. Remote close,
writeback, device teardown, and waiting belong to explicit retirement work.

## Writable ramfs

Each open node lease pins the node independently of its directory entry.
Unlink removes namespace reachability; existing File handles continue to read
and write that same node. Recreating the name produces a distinct, non-reused
node identity. Removed directories reject creation through old handles.
Directory enumeration returns owned entry snapshots with monotonically
allocated cookies, so deletion cannot turn a cursor into a reused entry index.
Enumeration is not a snapshot of the whole directory during concurrent mutation.

File bytes borrow the boot archive until mutation requires owned storage.
Growth is zero-filled and charged to a filesystem-owned 256 MiB storage domain,
including node and directory-entry allocations. Replacement buffers reserve
before allocation and retain the old charge until replacement succeeds.
Truncation to zero releases owned capacity. The namespace mutation mutex
serializes create, remove, link, and rename publication; independent file reads and writes use
per-node sleeping mutexes. User memory is copied through bounded buffers before
or after backend execution. One Native write may complete a short prefix;
append placement and that prefix's write are atomic together.

File creation prepares the node, accounting and handle publication before
committing its name. Failed preparation leaves the namespace unchanged.
Executable snapshots borrow unchanged archive bytes or own a charged immutable
copy taken under the file lock. Later writes cannot modify an admitted image.

Ramfs bypasses page retention because its owned bytes are authoritative storage,
while sharing the common file-content coordination contract. Cached writable
backends use the same revision and mutation protocol; a clean read cache does
not require dirty-page writeback. The backend seam
in `instance.rs` separates node leases, owned metadata, data operations and
executable snapshots from path policy. The generic userspace adapter owns canonical path identities and ordered
request lifetime/cancellation there; block transport stays below the Native
filesystem worker. [Filesystem services](../../docs/filesystem-services.md)
describes the bounded shared-buffer protocol, failure retirement and sequential
readahead. FAT parsing runs in `/svc/fs-fat`, selected on demand by the
`/svc/fs-backend` manager. The trusted Linux I/O VM remains the physical block
backend; see the [I/O VM contract](../../docs/io-vm.md).

The Native surface includes atomic open/create/create-new/truncate, file
write/append/resize, directory creation, rename with replacement, regular-file
hard links, symbolic links, metadata and timestamp updates, and removal of
files or empty directories. Open/create prepares handle publication before any
namespace or truncation commit. Failed publication cannot truncate an existing
file. Hard links share the same canonical node, contents, metadata, and lock
domain. Directory hard links and cross-filesystem rename/link are rejected.
Ramfs `file_sync` completes its in-memory operation; it does not promise durable
storage. Shared writable mappings and persistent writeback remain absent.

## Native file transfer batching

File transfer requests accept up to 2 MiB; this limit is independent of the
64 KiB VMO transfer limit. Reads process the request through the existing
File/backend interface in at most 512 KiB batches. Larger reads use fallible
scratch storage charged to the calling process; reads up to 1 KiB retain a
small stack buffer. Only the current batch's user destination is prepared and
pinned, after the backend has released its locks. Syscalls run in scheduled,
interruptible kernel context.

Reads stop at the observed EOF, a short backend read, or an error. An error
before any copy is reported to the caller; after completed batches, the copied
prefix is returned. Callers must still handle short reads. The operation is
not an atomic snapshot across concurrent file mutations. Scratch storage is
released on return and is not a file-data cache. Writes accept the same request
limit and accept at most 512 KiB per call, returning a short count for larger
requests. Large writes use the same charged, fallible scratch storage; small
writes retain the 1 KiB stack buffer. The accepted input is copied before taking
the backend lock, then submitted in one backend call so append offset selection
and that batch's write remain serialized. Callers must handle short writes.
This does not introduce a writeback cache or change `file_sync` semantics.

Machine-visible buffers use the HAL's non-faulting external-memory copy
contract, not Rust references that assume exclusive access. AArch64 copies
8-byte-aligned ranges with GPR pairs, then whole words and an exact byte tail;
unaligned ranges retain the byte path. No access crosses the validated range,
uses SIMD state, supplies a snapshot, or replaces the owner's publication
barriers. MMIO is excluded from this Normal-memory contract.

The actual AArch64 copy instructions run in host tests on AArch64, including
all source/destination alignment combinations and protected-page boundaries
on macOS. QEMU storage and Native user-copy acceptance cover their callers.
Physical VHE-board validation must additionally exercise simultaneous Native
and I/O-VM transfers, aligned/unaligned buffers, checksums and stop/restart;
host/QEMU tests do not establish a real device's DMA or cache-coherence behavior.

## Rooted directory scopes

`directory_scope_create` combines an explicit root location and a reachable
starting directory. Both handles must authorize every requested right. A scope
owns its root and current node independently of the input handles. Ordinary
child Directory handles remain confined to their own subtree; scopes support
relative paths from a retained cwd while absolute paths and absolute symlinks
restart at the explicit root.

Resolution reconstructs current ancestry from validated forward edges and
checks a namespace mutation epoch. A renamed cwd therefore follows the same
node, while detached contextual locations are rejected. Traversal is iterative,
with bounded path bytes, depth and symlink expansion. A writer in progress is
waited for through the namespace mutex; repeated concurrent changes can return
`Busy`. Canonical paths describe the observed spelling, not a new authority.

Final-component nofollow opens and identity-conditioned removal support safe
recursive deletion. Each traversal pins its parent Directory and uses a child
basename, so replacing a name with a symlink cannot redirect deletion outside
the pinned authority. Concurrent replacement may make deletion fail rather
than delete a different identity. An already open File remains usable after
its last name is removed.

Temporary path storage is charged to the process issuing the request, not to
the creator of its Directory capability. Buffers grow fallibly with actual
path depth and retained name bytes; there is no maximum-depth allocation or
maximum-size quota reservation for every open. Growth admits both old and new
capacity before replacing storage. Each retry releases its own buffers, and
returned paths retain their charges until userspace copyout finishes.

## Advisory file locks

Each canonical node owns one whole-file advisory lock domain. Separate opens
have distinct owners; duplicated or transferred handles retain their open
instance's owner. Locks do not prevent ordinary data I/O by uncooperative
holders. Shared and exclusive acquisition uses FIFO order, granting a shared
prefix together without allowing new readers to bypass a waiting writer.

Each owner permits one pending acquisition. A second request returns `Busy`
until the earlier continuation retires; unlock also cancels a pending request.
With no pending continuation, repeated acquisition in the same mode is idempotent. Exclusive-to-shared
conversion is atomic. Shared-to-exclusive conversion succeeds only for a sole
holder with no predecessor; otherwise it preserves the shared lock and returns
`WouldBlock` for a try operation or `Busy` for a blocking request. It never
waits while retaining a shared lock needed by another upgrading owner.

Waits park through the scheduler with monotonic deadlines and cancellation.
The final active handle releases its owner's grant immediately, even if an
operation pin keeps the object alive. Last-close callbacks do not allocate,
block, or call a filesystem backend. Pending records are quota charged before
publication and bounded to 256 per node, including records awaiting retirement.

## Filesystem time

Metadata carries signed Unix seconds and normalized nanoseconds, with validity
bits for individual timestamps. The boot archive supplies modification times.
New mutations use the kernel wall clock. Without an RTC this is an
uncalibrated Unix epoch plus uptime baseline, shared with `SystemTime::now()`.
An unavailable monotonic clock still leaves timestamps unavailable. Native callers can set
access and modification times, including times before the Unix epoch.

The [UTC clock](time.md) is independent of filesystem policy. Filesystems
choose timestamp resolution and persistence; ramfs retains supplied timestamp
precision in memory.
