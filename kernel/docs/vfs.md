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

The first implementation mounts the immutable initramfs as the system root.
It deliberately implements a small read-only subset while establishing the
ownership and concurrency contracts needed by future writable, remote, and
device-backed filesystems.

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
by the node's immutable capability ceiling and mount policy. In this initial
credential-free root filesystem, mode bits are metadata except that an
executable File can be created only from a node carrying an execute bit. Native
reads use an explicit offset. Shared offsets, file descriptors, credentials,
and POSIX path policy belong to a Linux or FreeBSD personality rather than this
Native API.

The namespace strongly owns mounted filesystems. Directory and File objects
hold the namespace view and the resolved mount/node location needed to keep an
open object valid after namespace detachment. Mounts do not strongly reference
their namespace, and node caches may retain only weak discovery references, so
the ownership graph cannot form a namespace--mount--node cycle.

## Backend contract

A backend receives validated names and opaque node identities. Its results are
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

The initramfs backend is synchronous and kernel-resident because starting
`/init` must not depend on a scheduler, driver domain, or userspace filesystem
server. Import builds explicit root, parent, name, and node-identity relations.
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

No VFS or file-data-cache operation runs in interrupt or exception context.
The conceptual lock order is namespace/mount, node state, cache shard, then
cache entry, but hot paths acquire a counted owner and release the preceding
layer before descending. No such lock spans backend execution, userspace copy,
IPC, allocation, or scheduler blocking.

Mount construction is prepare-then-publish. Publication is the only point at
which a namespace can discover the mount. Future unmount removes namespace
reachability before quiescing; existing open objects and operation pins remain
valid. Final destruction releases only already-quiescent memory. Remote close,
writeback, device teardown, and waiting belong to explicit retirement work.
