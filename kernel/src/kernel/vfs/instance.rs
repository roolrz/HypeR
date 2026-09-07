// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem, mount, and immutable namespace ownership.

use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use hyper::fs::ramfs::{DirectoryCookie, Error as RamFsError, RamFs};
use hyper::fs::{Name, NodeAttributes, NodeId, NodeKind};
use hyper::mm::{AllocationError, FallibleArc};

use crate::kernel::accounting::ResourceDomain;

use super::ExecutableSnapshot;

static NEXT_FILESYSTEM_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_MOUNT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FilesystemId(u64);

impl FilesystemId {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct MountId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    IdentifierExhausted,
    InvalidDirectoryCookie,
    InvalidBackendResult,
    NotDirectory,
    NotRegularFile,
    NotSymlink,
    RamFs(RamFsError),
}

impl From<AllocationError> for Error {
    fn from(_: AllocationError) -> Self {
        Self::Allocation
    }
}

impl From<RamFsError> for Error {
    fn from(error: RamFsError) -> Self {
        Self::RamFs(error)
    }
}

/// Closed adapter set for filesystems mounted by this kernel revision.
///
/// Keeping the adapter closed avoids freezing a broad object-safe trait before
/// a second backend establishes the genuinely shared contract. VFS policy
/// calls only these narrow methods, so a direct or IPC-backed adapter can be
/// added without changing namespace or capability objects.
enum Backend {
    RamFs(RamFs<'static>),
}

pub(crate) struct FilesystemInstance {
    id: FilesystemId,
    backend: Backend,
}

/// One borrowed backend directory entry and its continuation cookie.
pub(super) struct DirectoryEntry<'entry> {
    pub(super) name: &'entry str,
    pub(super) attributes: NodeAttributes,
    pub(super) next_cookie: u64,
}

impl FilesystemInstance {
    pub(crate) fn try_from_ramfs(ramfs: RamFs<'static>) -> Result<FallibleArc<Self>, Error> {
        FallibleArc::try_new(Self {
            id: FilesystemId(allocate_identifier(&NEXT_FILESYSTEM_ID)?),
            backend: Backend::RamFs(ramfs),
        })
        .map_err(Error::from)
    }

    pub(crate) const fn id(&self) -> FilesystemId {
        self.id
    }

    pub(crate) fn cache_generation(&self) -> crate::kernel::io_cache::FilesystemGeneration {
        let Some(generation) = NonZeroU64::new(self.id().get()) else {
            instance_invariant_violation();
        };
        crate::kernel::io_cache::FilesystemGeneration::new(generation)
    }

    pub(crate) fn root(&self) -> NodeId {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.root().id(),
        }
    }

    pub(crate) fn attributes(&self, node: NodeId) -> Result<NodeAttributes, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.attributes(node).map_err(map_ramfs_error),
        }
    }

    pub(crate) fn lookup_child(
        &self,
        directory: NodeId,
        name: Name<'_>,
    ) -> Result<Option<NodeId>, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs
                .lookup_child(directory, name)
                .map(|node| node.map(|node| node.id()))
                .map_err(map_ramfs_error),
        }
    }

    pub(crate) fn read_at(
        &self,
        node: NodeId,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        let actual = match &self.backend {
            Backend::RamFs(ramfs) => ramfs
                .read_at(node, offset, destination)
                .map_err(map_ramfs_error),
        }?;
        validate_buffer_result(actual, destination.len())
    }

    pub(crate) fn read_link(&self, node: NodeId, destination: &mut [u8]) -> Result<usize, Error> {
        let actual = match &self.backend {
            Backend::RamFs(ramfs) => ramfs.read_link(node, destination).map_err(map_ramfs_error),
        }?;
        validate_buffer_result(actual, destination.len())
    }

    pub(super) fn read_directory_entry(
        &self,
        node: NodeId,
        cookie: u64,
    ) -> Result<Option<DirectoryEntry<'_>>, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => {
                let mut entries = ramfs
                    .enumerate(node, DirectoryCookie::new(cookie))
                    .map_err(map_ramfs_error)?;
                Ok(entries.next().map(|entry| {
                    let node = entry.node();
                    DirectoryEntry {
                        name: node.name(),
                        attributes: node.attributes(),
                        next_cookie: entry.next_cookie().get(),
                    }
                }))
            }
        }
    }

    /// Returns immutable executable storage for the bootstrap loader.
    ///
    /// This is intentionally narrower than general file I/O. A future mutable
    /// or remote backend must first create an owned immutable executable
    /// snapshot so validation and mapping cannot race later file changes. The
    /// operation is fallible, and any owned snapshot must charge its storage
    /// to `sponsor` before allocating it. `RamFs` borrows immutable archive bytes
    /// and therefore needs neither allocation nor an additional charge.
    pub(crate) fn executable_snapshot(
        &self,
        node: NodeId,
        _sponsor: &ResourceDomain,
    ) -> Result<Option<ExecutableSnapshot>, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => Ok(ramfs.node(node).and_then(|node| {
                (node.kind() == NodeKind::File && node.is_executable())
                    .then(|| ExecutableSnapshot::borrowed(node.data()))
            })),
        }
    }
}

pub(crate) struct Mount {
    id: MountId,
    filesystem: FallibleArc<FilesystemInstance>,
    root: NodeId,
}

impl Mount {
    fn try_new(filesystem: FallibleArc<FilesystemInstance>) -> Result<FallibleArc<Self>, Error> {
        let root = filesystem.root();
        FallibleArc::try_new(Self {
            id: MountId(allocate_identifier(&NEXT_MOUNT_ID)?),
            filesystem,
            root,
        })
        .map_err(Error::from)
    }

    pub(crate) const fn id(&self) -> MountId {
        self.id
    }

    pub(crate) fn filesystem(&self) -> &FilesystemInstance {
        &self.filesystem
    }

    pub(crate) const fn root(&self) -> NodeId {
        self.root
    }
}

fn map_ramfs_error(error: RamFsError) -> Error {
    match error {
        RamFsError::InvalidDirectoryCookie => Error::InvalidDirectoryCookie,
        RamFsError::InvalidNode => Error::InvalidBackendResult,
        RamFsError::NotDirectory => Error::NotDirectory,
        RamFsError::NotRegularFile => Error::NotRegularFile,
        RamFsError::NotSymlink => Error::NotSymlink,
        other => Error::RamFs(other),
    }
}

const fn validate_buffer_result(actual: usize, capacity: usize) -> Result<usize, Error> {
    if actual <= capacity {
        Ok(actual)
    } else {
        Err(Error::InvalidBackendResult)
    }
}

#[derive(Clone)]
pub(crate) struct Location {
    mount: FallibleArc<Mount>,
    node: NodeId,
}

impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.mount.id() == other.mount.id() && self.node == other.node
    }
}

impl Eq for Location {}

impl Location {
    pub(crate) fn new(mount: FallibleArc<Mount>, node: NodeId) -> Self {
        Self { mount, node }
    }

    pub(crate) fn mount(&self) -> &FallibleArc<Mount> {
        &self.mount
    }

    pub(crate) const fn node(&self) -> NodeId {
        self.node
    }
}

/// Immutable view of the system mount topology.
///
/// The first milestone has one root mount. Directory capabilities still carry
/// this owner so a later immutable mount-table snapshot can be added without
/// changing their authority or lifetime shape.
pub(crate) struct MountNamespace {
    root: Location,
    cache: FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>>,
}

impl MountNamespace {
    pub(super) fn try_new(
        filesystem: FallibleArc<FilesystemInstance>,
        cache: FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>>,
    ) -> Result<FallibleArc<Self>, Error> {
        let mount = Mount::try_new(filesystem)?;
        let root = Location::new(mount.clone(), mount.root());
        FallibleArc::try_new(Self { root, cache }).map_err(Error::from)
    }

    pub(crate) fn root(&self) -> Location {
        self.root.clone()
    }

    pub(super) fn cache(
        &self,
    ) -> FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>> {
        self.cache.clone()
    }

    pub(super) fn read_directory_entry<'entry>(
        &self,
        directory: &'entry Location,
        cookie: u64,
    ) -> Result<Option<DirectoryEntry<'entry>>, Error> {
        directory
            .mount()
            .filesystem()
            .read_directory_entry(directory.node(), cookie)
    }
}

fn allocate_identifier(source: &AtomicU64) -> Result<u64, Error> {
    let mut current = source.load(Ordering::Relaxed);
    loop {
        if current == 0 {
            return Err(Error::IdentifierExhausted);
        }
        let Some(next) = current.checked_add(1) else {
            return Err(Error::IdentifierExhausted);
        };
        match source.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Ok(current),
            Err(observed) => current = observed,
        }
    }
}

#[cold]
fn instance_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
