<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# rust-fatfs

Vendored from <https://github.com/rafalh/rust-fatfs> at commit
`2aefc2a027ce94ed0671752814dac203f0450e11` (package version 0.4.0).
The upstream `src/`, `Cargo.toml`, `README.md`, and `LICENSE.txt` are retained.
This third-party implementation is MIT licensed; its original copyright and
license remain in `LICENSE.txt`. HypeR's own code remains Apache-2.0 licensed.

Most source changes are confined to `src/dir.rs`:

- Resolve directory path components iteratively for open/create/remove/rename,
  so a valid deeply nested path cannot exhaust the kernel stack. Directory
  descent and file open return their small stream/cursor directly, avoiding unused
  long-filename return buffers in callers during blocking metadata I/O.
- Find the extension without slicing at byte 1, which is not a UTF-8 boundary
  for names beginning with a multibyte character.
- Offer a borrowed-entry visitor for metadata and cursor lookup, keeping owned
  long-name return buffers out of the callers during blocking I/O. The public
  iterator remains compatible and the visitor adds no I/O allocations. The parser
  fills initialized caller-owned entry storage; its long-name builder borrows
  that entry's UTF-16 buffer and finalizes it in place, avoiding large buffer
  moves during blocking reads. Only complete entries reach callers. Long-name
  truncation respects the current logical buffer length, so a shorter restarted
  LFN chain cannot retain the suffix of an earlier incomplete chain.
- Keep only raw short-entry metadata, byte positions and the filesystem owner
  in private mutation cursors. Name matching still uses the shared borrowed
  visitor; no additional directory scan or allocation is required.
- Return an unpublished new directory cluster when extending its full parent
  fails with no space; an I/O failure still requires fail-closed recovery.
- Publish the destination entry before deleting the source, so exhaustion of
  destination directory space cannot silently unlink the source.
- For a directory moved between parents, update its on-disk `..` entry to the
  destination parent (cluster zero denotes the filesystem root).
- Flush/drop streams referring to the source entry before marking it deleted,
  since upstream file destructors can write directory metadata.

`src/dir_entry.rs` exposes the existing editor constructor within the crate for
the compact mutation cursor and removes its now-unused private entry-position
comparison helper. The public owned directory entry API remains unchanged.

`src/fs.rs` adds explicit `FileSystem::sync`, reusing the existing metadata
cleanup and then flushing the underlying storage without consuming/remounting
the filesystem. Its exclusive borrow requires open file editors to be dropped first. HypeR's
adapter enforces that lifetime boundary, issues its explicit device durability
barrier, and closes the filesystem on failure with destructor I/O disabled.

This is not a journaled transaction. A device I/O failure after destination
publication can leave an incomplete rename; HypeR latches the error and stops
further access rather than claiming rollback. On a completely full volume,
rename can report no space when it cannot publish a second directory entry;
the original entry remains intact.

Regression coverage lives in `kernel/tests/host/src/cases/fat_volume.rs`.
Keep this patch small, review it when updating the pinned upstream source,
and remove it when an upstream version supplies equivalent guarantees.
