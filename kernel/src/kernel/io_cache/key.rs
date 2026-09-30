// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem, node-incarnation, and content identities for clean pages.

use core::num::NonZeroU64;

/// One nonzero generation of a mounted filesystem instance.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FilesystemGeneration(NonZeroU64);

impl FilesystemGeneration {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// A node incarnation that is never reused within its filesystem generation.
/// Paths and repeated opens do not identify file contents.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct NodeIdentity(NonZeroU64);

impl NodeIdentity {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// A content generation advanced before every potentially mutating operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ContentRevision(NonZeroU64);

impl ContentRevision {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// Identity of one immutable view protected by the caller's content lock.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FileIdentity {
    filesystem: FilesystemGeneration,
    node: NodeIdentity,
    revision: ContentRevision,
}

impl FileIdentity {
    pub(crate) const fn new(
        filesystem: FilesystemGeneration,
        node: NodeIdentity,
        revision: ContentRevision,
    ) -> Self {
        Self {
            filesystem,
            node,
            revision,
        }
    }
}

/// Page offset within one file-data stream.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub(crate) struct FilePageIndex(u64);

impl FilePageIndex {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CacheKey {
    file: FileIdentity,
    page: FilePageIndex,
}

impl CacheKey {
    pub(crate) const fn new(file: FileIdentity, page: FilePageIndex) -> Self {
        Self { file, page }
    }

    /// Noncryptographic placement only. The index separately caps collision
    /// chains, so adversarial placement reduces admission rather than growing
    /// an IRQ-masked lookup in proportion to cache capacity.
    pub(super) fn bucket(self, mask: usize) -> usize {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for value in [
            self.file.filesystem.0.get(),
            self.file.node.0.get(),
            self.file.revision.0.get(),
            self.page.0,
        ] {
            hash = (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= hash >> 30;
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash ^= hash >> 27;
        hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
        ((hash ^ (hash >> 31)) as usize) & mask
    }
}
