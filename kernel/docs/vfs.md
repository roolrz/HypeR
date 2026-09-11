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
the same per-file lock that commits the write. Shared offsets, file descriptors, credentials,
and POSIX path policy belong to a Linux or FreeBSD personality rather than this
Native API.

The namespace strongly owns mounted filesystems. Directory and File objects
hold the namespace view and the resolved mount/node location needed to keep an
open object valid after namespace detachment. Mounts do not strongly reference
their namespace, and node caches may retain only weak discovery references, so
the ownership graph cannot form a namespace--mount--node cycle.

## Backend contract

A backend receives validated names and opaque, counted node leases. Its results are
bounded owned metadata or caller-provided buffers; it may not retain borrowed
VFS storage. VFS locks are released before any backend operation. This permits
a later remote adapter to block or perform IPC without holding namespace,
node-cache, handle-table, or file-data-cache locks.

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

Cache entries are keyed by filesystem generation, stable node identity, and
page index. A path or raw reusable inode number is never a cache key. The first
read-only state machine is:

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
but cannot invalidate an active reader. The cache has a fixed budget for
cache-owned clean entries, and only clean entries are eviction candidates in
this milestone. In-flight load buffers and pages retained by active readers
are separately bounded by caller concurrency and lifetime rather than that
clean-entry budget.

Dirty data, writeback, truncate invalidation, direct I/O, external coherence,
and shared writable mappings are intentionally absent until their state and
cancellation protocols are implemented. Destructors never pretend to flush or
complete fallible storage work.

## Concurrency and lifecycle

Namespace resolution and file-data backend operations run in scheduled kernel
context. Short, nonblocking active-handle retirement callbacks may also execute
from masked Native handle-close entry.
The conceptual lock order is namespace/mount, node state, cache shard, then
cache entry, but hot paths acquire a counted owner and release the preceding
layer before descending. VFS policy and cache spinlocks never span backend execution, userspace copy,
IPC, allocation, or scheduler blocking. Ramfs sleeping mutexes may span its
fallible allocation, but never a userspace copy.

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

Ramfs bypasses the immutable page cache: its owned bytes are the authoritative
storage. A future cached writable backend must add content-version validation,
truncate invalidation and writeback before using that cache. The backend seam
in `instance.rs` separates node leases, owned metadata, data operations and
executable snapshots from path policy. A userspace filesystem adapter can add
its own lease variant and request lifetime/cancellation protocol there; block
transport stays below the filesystem adapter. No userspace filesystem or block
protocol is claimed by this implementation.

The Native surface includes atomic open/create/create-new/truncate, file
write/append/resize, directory creation, rename with replacement, regular-file
hard links, symbolic links, metadata and timestamp updates, and removal of
files or empty directories. Open/create prepares handle publication before any
namespace or truncation commit. Failed publication cannot truncate an existing
file. Hard links share the same canonical node, contents, metadata, and lock
domain. Directory hard links and cross-filesystem rename/link are rejected.
Ramfs `file_sync` completes its in-memory operation; it does not promise durable
storage. Shared writable mappings and persistent writeback remain absent.

## Native read batching

File transfer requests accept up to 2 MiB; this limit is independent of the
64 KiB VMO transfer limit. Reads process the request through the existing
File/backend interface in at most 64 KiB batches. Larger reads use fallible
scratch storage charged to the calling process; reads up to 1 KiB retain a
small stack buffer. Only the current batch's user destination is prepared and
pinned, after the backend has released its locks. Syscalls run in scheduled,
interruptible kernel context.

Reads stop at the observed EOF, a short backend read, or an error. An error
before any copy is reported to the caller; after completed batches, the copied
prefix is returned. Callers must still handle short reads. The operation is
not an atomic snapshot across concurrent file mutations. Scratch storage is
released on return and is not a file-data cache. Writes accept the same request
limit but retain their existing bounded short-write behavior.

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
New mutations use the optional kernel UTC clock; an unavailable timestamp is
reported as unavailable, never fabricated from uptime. Native callers can set
access and modification times, including times before the Unix epoch.

The [UTC clock](time.md) is independent of filesystem policy. Filesystems
choose timestamp resolution and persistence; ramfs retains supplied timestamp
precision in memory.
