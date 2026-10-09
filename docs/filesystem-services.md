<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native filesystem services

The kernel owns namespace traversal, capability checks, file identities,
`flock`, content revisions and the clean page cache. Media parsers run in Native
processes. The initial block-backed implementation is FAT32; the protocol and
worker lifecycle are independent of that format. Ramfs remains in the kernel.

## Startup and authority

Init creates an attachment channel between `io-runtime` and `/svc/fs-backend`.
After the configuration-volume client is activated, `io-runtime` moves its
exclusive `NativeBlock` capability, activated sector count/read-only policy and
readiness writer to `fs-backend`. The
manager reads a bounded boot sector, recognizes plausible filesystem geometry,
and selects a build-controlled executable. Disk bytes cannot select executable
paths. The manager starts `/svc/fs-fat` only for a FAT candidate; that worker
performs the authoritative media validation before publishing a mount.
Unsupported or corrupt media leaves storage unavailable and Native recovery
services running.

Partition discovery belongs to the Linux I/O VM's validated GPT and `dm-linear`
volume setup. The Native block capability describes one logical volume starting
at sector zero. Format probing does not scan an entire disk or interpret an MBR
as filesystem contents. FAT's informational type label is not authoritative.

The manager opens the mount directory and gives the worker a root capability
scoped to that directory, together with an explicit dynamic-library directory.
The worker receives no task factory, device authority or arbitrary writable root.
It gets a bounded child resource domain, its exclusive block capability, a
readiness writer, and a parent-lifetime channel. It links the common
`hyper-fs-service` library; FAT-specific code implements only media operations.
The manager retains the child process and task group. It is not in the data path.
Task-group retirement can cancel a currently executing worker operation; shared
and block buffers still follow their bounded transaction-drain and retirement
rules before ownership can be released.

The worker mounts `.` within its scoped directory using `filesystem_mount`.
The mount operation validates the channel, shared memory and covered directory
without calling back into the worker. Readiness is sent only after publication.
`io-runtime` continues its console and power event loop while waiting for this
readiness signal. There is no additional raw writable block alias retained by
it or the manager after the capability moves to the worker.

## Request and shared-memory contract

`hyper-filesystem` defines explicit little-endian messages for stat, directory
entry enumeration, positioned reads and writes, create, remove, rename, resize,
sync and timestamps. It contains no FAT attributes or on-disk structures. Paths
are bounded UTF-8 relative paths without dot, parent or empty components.
Directory enumeration returns ordinary children; format adapters exclude
on-disk `.` and `..` entries before indexing results. The
current protocol intentionally supports canonical path identities rather than
hard links or symbolic links; a filesystem needing these semantics must extend
the generic identity contract before claiming support.

Each mount owns one ordered `ByteChannel` and one resident 512 KiB writable VMO.
The kernel serializes requests under a sleepable mount mutex. Metadata travels
in the channel; file bytes travel in the shared range. The kernel finishes
writing an input range before publishing a write request. Receiving a request
transfers ownership of that range to the sole worker. Publishing its reply gives
ownership back. The kernel never treats shared bytes as borrowed Rust objects.
The response must match the operation and increasing request identifier, fit the
exact frame size, and contain valid lengths, names, kinds and timestamps.

A request timeout, channel closure or malformed reply permanently
retires the mount generation. Its shared range is never reused for another
request. A late server write therefore cannot corrupt a subsequent transaction;
the server's mapping remains an owner of the physical storage until teardown.
Stopping one client process drains its already published transaction within the
same deadline before unwinding its syscall; it does not invalidate the shared
mount or abandon the shared buffer. I/O errors can describe partial media changes
and are not retried as mutations.
The raw block transfer syscall checks per-operation rights, sector alignment,
range bounds and the 512 KiB payload maximum independently of worker input.
`native_block_transfer` operations are 0 (read), 1 (write), 2 (durable flush)
and 3 (disjoint write batch). For a batch, the sector argument is the record
count (1–4); the byte frame contains that many little-endian `(first_sector,
byte_length)` u64 pairs followed by their payloads in order. Total payload is
at most 512 KiB. Every complete-sector range, overlap and exact frame length is
validated before any write. The kernel's existing multi-request submission and
flight-retirement rules therefore continue to apply to the userspace engine.

The worker waits for both requests and parent-channel closure. Parent death
closes the worker service after any already executing bounded block operation
returns; worker death closes the service immediately as its handles retire.
The kernel checks local endpoint health even before satisfying a cache hit, so
cached bytes cannot hide a dead worker. Restarting a process does not reconnect
an old mount: recovery needs a fresh mount generation. A hot cache hit needs no
server round trip and does not take the mount transaction mutex.

## Identity, cache and mutation ordering

The worker returns the canonical stored leaf name for every lookup. The kernel
registry maps canonical paths to an in-memory, never-reused node incarnation.
Aliases therefore converge on one content record and advisory-lock identity.
All media access goes through the authoritative worker; independent out-of-band
writers violate this contract. Live node leases prevent unlink. Rename updates
registered descendant paths after a successful backend mutation. Unlink removes
the path binding, so recreating a name cannot see pages belonging to the old file.

Cached pages pin the content record rather than an open capability. Closing the
last handle leaves its identity available while cached pages survive; reopening
from the same or another process can hit them. After all live and cached owners
disappear, a new open receives a fresh incarnation. Quota and pressure reclaim
release pages and expired registry records without making filesystem IPC calls.

Writes, resizes and truncating opens advance the common content revision before
calling the worker. Even a failed mutation may have changed media, so old cache
entries become unreachable. The per-file gate also excludes concurrent fills.
Only clean pages are retained: ordinary writes stay synchronous, and explicit
sync is forwarded to the filesystem and durable block flush.

Sequential demand reads can enqueue bounded common readahead after a miss.
The queue contains weak filesystem, node, content and cache owners. Running
work upgrades only the content record, never an active node lease, so even an
in-flight prediction cannot keep a closed file busy. Its separate kernel worker
checks the identity and revision, tries the content and backend locks, and skips
work on contention or pressure. It resolves the incarnation's current path under
the backend mutex; unlink retires that binding, and later fills of the old cache
identity cannot become visible through a recreated file.
It cannot recursively predict more work. This worker is separate from memory
reclamation, so a backend waiting for memory cannot deadlock the reclaim path.

Implementation entry points: [manager](../app/fs-backend/src/main.rs),
[common worker](../lib/fs-service/src/lib.rs), [FAT worker](../app/fs-fat/src/main.rs),
[wire protocol](../lib/rust/hyper-filesystem/src/protocol.rs),
[kernel adapter](../kernel/src/kernel/vfs/remote.rs), and
[transport](../kernel/src/kernel/vfs/remote/transport.rs).
See [FAT semantics](fat.md) and [VFS cache](../kernel/docs/vfs.md#file-data-cache)
for format details and memory policy.

## Validation

`make test-filesystem-failure` starts real Native filesystem workers with a
controlled test volume. It checks close/reopen cache reuse, worker death after
cache fill, manager-owner channel closure, worker death during a pending read,
stopping one requesting process while another keeps using the mount, and
rejecting overflowing writes derived from worker-controlled file lengths before
publishing any write request. The
fixture is selected by an explicit Cargo feature and is never packaged in the
normal system or development profile.

`make test-board-storage` exercises the actual FAT worker and Linux block
backend: file and directory operations, aliases and shared identities, writes,
truncation, cache invalidation, synchronization and readback after a cold QEMU
restart. These are correctness tests; they do not establish hardware performance
or qualify physical power-loss durability.
