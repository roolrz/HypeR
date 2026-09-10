// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem, mount, and immutable namespace ownership.

use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use hyper::fs::ramfs::{Error as RamFsError, RamFs};
use hyper::fs::{Name, NodeAttributes};
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

impl MountId {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

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
    Resource(crate::kernel::accounting::ResourceError),
    Lock(crate::kernel::sync::Error),
    AlreadyExists,
    Missing,
    NotEmpty,
    InvalidSize,
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
    RamFs(super::ramfs::Ramfs),
}

/// Whether immutable reads benefit from copying backend data into page cache.
///
/// A memory-resident filesystem already owns stable bytes, so caching it would
/// create a second physical copy without avoiding I/O. Block and remote
/// backends added later can opt into `PageCache` at their adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadCachePolicy {
    Direct,
    #[allow(dead_code, reason = "no block-backed filesystem adapter exists yet")]
    PageCache,
}

pub(crate) struct FilesystemInstance {
    id: FilesystemId,
    backend: Backend,
}

/// A backend-owned lease, independent of any directory entry lifetime.
#[derive(Clone)]
pub(crate) struct NodeLease(FallibleArc<super::ramfs::Node>);

impl NodeLease {
    pub(crate) fn get(&self) -> u64 {
        self.0.id()
    }
}

pub(super) type DirectoryEntry = super::ramfs::EntrySnapshot;

impl FilesystemInstance {
    pub(crate) fn try_from_ramfs(ramfs: RamFs<'static>) -> Result<FallibleArc<Self>, Error> {
        FallibleArc::try_new(Self {
            id: FilesystemId(allocate_identifier(&NEXT_FILESYSTEM_ID)?),
            backend: Backend::RamFs(super::ramfs::Ramfs::from_archive(ramfs)?),
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

    pub(super) const fn read_cache_policy(&self) -> ReadCachePolicy {
        match &self.backend {
            Backend::RamFs(_) => ReadCachePolicy::Direct,
        }
    }

    pub(crate) fn root(&self) -> NodeLease {
        match &self.backend {
            Backend::RamFs(ramfs) => NodeLease(ramfs.root()),
        }
    }

    pub(crate) fn attributes(&self, node: &NodeLease) -> Result<NodeAttributes, Error> {
        node.0.attributes()
    }

    pub(crate) fn lookup_child(
        &self,
        directory: &NodeLease,
        name: Name<'_>,
    ) -> Result<Option<NodeLease>, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs
                .lookup(&directory.0, name)
                .map(|node| node.map(NodeLease)),
        }
    }

    pub(crate) fn read_at(
        &self,
        node: &NodeLease,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.read(&node.0, offset, destination, false),
        }
    }

    pub(crate) fn read_link(
        &self,
        node: &NodeLease,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.read(&node.0, 0, destination, true),
        }
    }

    pub(super) fn read_directory_entry(
        &self,
        node: &NodeLease,
        cookie: u64,
    ) -> Result<Option<DirectoryEntry>, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.entry(&node.0, cookie),
        }
    }

    pub(crate) fn executable_snapshot(
        &self,
        node: &NodeLease,
        sponsor: &ResourceDomain,
    ) -> Result<Option<ExecutableSnapshot>, Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.executable(&node.0, sponsor),
        }
    }

    pub(super) fn write_at(
        &self,
        node: &NodeLease,
        offset: Option<u64>,
        input: &[u8],
    ) -> Result<(usize, u64), Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.write(&node.0, offset, input),
        }
    }

    pub(super) fn resize(&self, node: &NodeLease, length: u64) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.resize(&node.0, length),
        }
    }

    pub(super) fn create(
        &self,
        directory: &NodeLease,
        name: Name<'_>,
        kind: hyper::fs::NodeKind,
        mode: u32,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.create(&directory.0, name, kind, mode, |_| Ok(())),
        }
    }

    pub(super) fn create_file<R, E: From<Error>>(
        &self,
        directory: &NodeLease,
        name: Name<'_>,
        mode: u32,
        publish: impl FnOnce(NodeLease) -> Result<R, E>,
    ) -> Result<R, E> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.create(
                &directory.0,
                name,
                hyper::fs::NodeKind::File,
                mode,
                |node| publish(NodeLease(node)),
            ),
        }
    }

    pub(super) fn remove(
        &self,
        directory: &NodeLease,
        name: Name<'_>,
        kind: hyper::fs::NodeKind,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(ramfs) => ramfs.remove(&directory.0, name, kind),
        }
    }
}

pub(crate) struct Mount {
    id: MountId,
    filesystem: FallibleArc<FilesystemInstance>,
    root: NodeLease,
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

    pub(crate) fn root(&self) -> NodeLease {
        self.root.clone()
    }
}

#[derive(Clone)]
pub(crate) struct Location {
    mount: FallibleArc<Mount>,
    node: NodeLease,
}

impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.mount.id() == other.mount.id() && self.node.get() == other.node.get()
    }
}

impl Eq for Location {}

impl Location {
    pub(crate) fn new(mount: FallibleArc<Mount>, node: NodeLease) -> Self {
        Self { mount, node }
    }

    pub(crate) fn mount(&self) -> &FallibleArc<Mount> {
        &self.mount
    }

    pub(crate) fn node(&self) -> &NodeLease {
        &self.node
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

    pub(super) fn read_directory_entry(
        &self,
        directory: &Location,
        cookie: u64,
    ) -> Result<Option<DirectoryEntry>, Error> {
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
