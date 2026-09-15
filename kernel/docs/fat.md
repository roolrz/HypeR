<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# FAT32 block volumes

The kernel mounts an exclusively owned logical block volume, not a copy of its
contents in ramfs. `fs::block::BlockDevice` describes 512-byte sector transfers
and a durable flush. Operations may sleep. The kernel VFS adapter holds a
sleepable volume mutex; no spin lock remains held during device I/O.

FAT parsing and mutations use the MIT-licensed `rafalh/rust-fatfs` revision
`2aefc2a027ce94ed0671752814dac203f0450e11`, vendored in
`third_party/rust-fatfs`. Its provenance document records the local correctness
and bounded-stack fixes; the original MIT copyright and license are retained.
Its no-std, no-alloc configuration provides long filenames. HypeR keeps the
library behind `fs::fat::FatVolume`; upstream files and directories do not
escape the adapter.

## Media admission

Before upstream parsing, HypeR validates the BPB geometry and arithmetic bounds,
FAT successor ranges, directory references, reachable chain lengths, cycles and
crosslinks. A temporary two-bit-per-cluster allocation records ownership; it is
released after mount. The mount sponsor reserves an upper bound for this
scratch before scanning. Persistent filesystem state, directory registry capacity,
node leases and retained paths are separately charged to their storage owners.
File contents are not cached or copied by this scan.
The directory graph admits at most 65,536 directories. An operation also has a
finite I/O/seek budget so a corrupt chain cannot cause an unbounded traversal.

These checks require exclusive ownership for the entire mounted lifetime. The
I/O VM must not mount or otherwise modify the filesystem. A backend that changes
sector contents behind HypeR's cache violates the block volume contract. This
adapter does not claim to defend against a malicious trusted I/O VM.

FAT has no journal. A power failure during metadata changes can require an
external filesystem checker. If mkdir runs out of parent-directory space before
publishing the short entry, its unlinked child cluster is returned to the FAT.
This rollback is not attempted after uncertain device I/O: metadata may already
have reached the device. A transport error poisons the mounted instance;
subsequent accesses fail instead of continuing with potentially inconsistent
metadata. Device errors may have committed part of an operation.

## Synchronization and lifetime

Sector writes are write-through. Four 4 KiB read-through windows batch nearby
metadata fields and keep directory and FAT lookups from displacing each other
on every read. Their fixed heap storage is charged to the mount sponsor; bulk
file data bypasses these windows. Completed writes update overlapping windows;
failed writes invalidate them and poison the volume. Ordinary writes do not
claim stable-storage durability. `sync` explicitly commits upstream FSInfo and directory metadata,
issues the block device's durable flush, and retains the mounted metadata view.

File reads retain bounded allocation maps for four paths, with at most 128
coalesced disk extents per path. Once mapped, reads locate the requested offset
directly and combine adjacent data sectors instead of reopening and walking the
FAT chain for every transfer. These maps contain metadata, not file contents;
their fixed storage is charged to the mount sponsor. Highly fragmented files
fall back to ordinary FAT reads without allocating an unbounded extent table.
All potentially mutating operations invalidate the maps before touching media,
including operations that subsequently fail. The volume mutex serializes map
construction, use and invalidation. An initial map build traverses the file's
chain; this cost is amortized across subsequent reads until mutation or eviction.
An exclusive filesystem borrow ensures all temporary file editors have been
dropped before synchronization. It does not reparse the boot sector or repeat
the admission scan. A failed synchronization closes the mounted view.

Upstream destructors attempt I/O. Device access is therefore gated to explicit
operations, with failures latched and returned to the caller. Dropping the
HypeR volume closes this gate before dropping upstream state; no destructor
can submit I/O. Upstream's frequent stream flushes only finish write-through
work; they do not turn every file read or stat into a physical cache barrier.

A read-only block volume rejects mutations before changing cached metadata.
Read, stat, synchronization and teardown issue no writes or device flushes;
a rejected write does not poison subsequent reads.

Directory traversal is iterative. A borrowed-entry visitor owns the temporary
FAT long-name buffer inside its traversal frame. Metadata lookup copies into
the volume's caller-owned output buffer; directory and file lookup return only
a small stream or cursor. The borrowed entry cannot escape the visitor. This
keeps owned long-name return values out of the outer VFS call chain without
allocating on each I/O operation or repeating sector reads. The public upstream
iterator still uses the same parser and retains its end/error behavior.
Private mutation cursors retain only the located short-entry metadata, byte
positions and filesystem owner; rename, remove and creation do not carry an
unused long-name buffer into subsequent disk operations.

Parent resolution and directory scanning use separate stack frames. Metadata
inspection also completes before mutation starts, so its filename scratch is
not retained across the later blocking operation. Compiler frame reports and
QEMU stack watermarks check the combined path; a local frame limit alone is not
a whole-call-chain bound. Watermarks measure modified bytes, not untouched stack
reservations. Audit builds therefore complement static review and are not used
for performance measurements.

## VFS semantics

Long filenames and their 8.3 aliases resolve to the same canonical stored name,
so alternate spellings cannot create separate live node or lock identities.
Exclusive creation rejects an already existing entry through either spelling.
Node leases have stable in-memory IDs distinct from reusable FAT directory
slots. Live leases prevent unlink. Successful rename updates every live
matching descendant path under the volume lock. Mount pins additionally
prevent moving a mounted subtree. A newly created same-name file therefore
cannot become visible through a previous file's handle.

FAT cannot represent hard links, symbolic links, or general Unix permission
bits. These operations report unsupported; modes are synthesized from the FAT
read-only attribute (`0777`, or `0555` when read-only). A requested creation mode
such as `0600` does not provide persistent Unix permission isolation on FAT;
Native handle rights still constrain each granted capability independently.
File creation and updates use the kernel clock, with UTC
as the FAT time convention. File access times have day precision, modification
times have two-second precision, and creation times have ten-millisecond
precision. Explicit timestamps outside 1980–2107 are rejected; invalid dates
read from media are reported as unavailable. Directory timestamp changes are
unsupported. Rename currently reports an existing destination rather than
replacing it. Ramfs retains its existing semantics.
