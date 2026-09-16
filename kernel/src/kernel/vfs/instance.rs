// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem, mount, and immutable namespace ownership.

use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use hyper::fs::ramfs::{Error as RamFsError, RamFs};
use hyper::fs::{MAX_NAME_BYTES, Name, NodeAttributes};
use hyper::mm::{AllocationError, FallibleArc};
use hyper::time::Timestamp;

use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};

use super::ExecutableSnapshot;
use super::scratch::{ScratchBudget, ScratchString, ScratchVec};

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
    IsDirectory,
    NotSymlink,
    RamFs(RamFsError),
    Resource(crate::kernel::accounting::ResourceError),
    Lock(crate::kernel::sync::Error),
    AlreadyExists,
    Missing,
    NotEmpty,
    InvalidSize,
    InvalidInput,
    Busy,
    Unsupported,
    Fat(hyper::fs::fat::Error),
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

#[derive(Clone, Copy)]
pub(super) struct EntryName<'a> {
    pub(super) name: Name<'a>,
    pub(super) directory_required: bool,
}

pub(super) struct Creation<'a> {
    pub(super) kind: hyper::fs::NodeKind,
    pub(super) mode: u32,
    pub(super) target: Option<&'a [u8]>,
}

/// Closed adapter set for filesystems mounted by this kernel revision.
///
/// Keeping the adapter closed avoids freezing a broad object-safe trait before
/// a second backend establishes the genuinely shared contract. VFS policy
/// calls only these narrow methods, so a direct or IPC-backed adapter can be
/// added without changing namespace or capability objects.
enum Backend {
    RamFs(super::ramfs::Ramfs),
    Fat(FallibleArc<super::fat::Fatfs<crate::kernel::block::MountedDevice>>),
}

/// Whether immutable reads benefit from copying backend data into page cache.
///
/// A memory-resident filesystem already owns stable bytes, so caching it would
/// create a second physical copy without avoiding I/O. Block and remote
/// backends added later can opt into `PageCache` at their adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadCachePolicy {
    Direct,
    #[allow(
        dead_code,
        reason = "mutable file cache requires generation-coherent invalidation before admission"
    )]
    PageCache,
}

pub(crate) struct FilesystemInstance {
    _charge: Option<CommittedCharge>,
    id: FilesystemId,
    backend: Backend,
}

/// A backend-owned lease, independent of any directory entry lifetime.
#[derive(Clone)]
pub(crate) struct NodeLease(NodeBackend);

#[derive(Clone)]
enum NodeBackend {
    RamFs(FallibleArc<super::ramfs::Node>),
    Fat(FallibleArc<super::fat::Node>),
}
impl NodeLease {
    fn from_ramfs(node: FallibleArc<super::ramfs::Node>) -> Self {
        Self(NodeBackend::RamFs(node))
    }
    fn from_fat(node: FallibleArc<super::fat::Node>) -> Self {
        Self(NodeBackend::Fat(node))
    }
    pub(crate) fn get(&self) -> u64 {
        match &self.0 {
            NodeBackend::RamFs(node) => node.id(),
            NodeBackend::Fat(node) => node.id(),
        }
    }
    pub(super) fn locks(&self) -> &super::locks::FileLocks {
        match &self.0 {
            NodeBackend::RamFs(node) => &node.locks,
            NodeBackend::Fat(node) => &node.locks,
        }
    }
    fn ramfs(&self) -> Result<&FallibleArc<super::ramfs::Node>, Error> {
        match &self.0 {
            NodeBackend::RamFs(node) => Ok(node),
            _ => Err(Error::InvalidBackendResult),
        }
    }
    fn fat(&self) -> Result<&FallibleArc<super::fat::Node>, Error> {
        match &self.0 {
            NodeBackend::Fat(node) => Ok(node),
            _ => Err(Error::InvalidBackendResult),
        }
    }
}

pub(super) enum MountPin {
    RamFs { _pin: super::ramfs::MountPin },
    Fat { _pin: super::fat::MountPin },
}

#[derive(Clone, Copy)]
pub(super) struct NodeMetadata {
    pub(super) mode: u32,
    pub(super) accessed: Option<Timestamp>,
    pub(super) modified: Option<Timestamp>,
    pub(super) created: Option<Timestamp>,
    pub(super) changed: Option<Timestamp>,
}

pub(super) struct EntrySnapshot {
    pub(super) name: [u8; MAX_NAME_BYTES],
    pub(super) length: usize,
    pub(super) attributes: NodeAttributes,
    pub(super) next_cookie: u64,
}

pub(super) type DirectoryEntry = EntrySnapshot;

impl FilesystemInstance {
    pub(crate) fn try_from_ramfs(ramfs: RamFs<'static>) -> Result<FallibleArc<Self>, Error> {
        FallibleArc::try_new(Self {
            _charge: None,
            id: FilesystemId(allocate_identifier(&NEXT_FILESYSTEM_ID)?),
            backend: Backend::RamFs(super::ramfs::Ramfs::from_archive(ramfs)?),
        })
        .map_err(Error::from)
    }

    pub(super) fn try_from_block(
        device: crate::kernel::block::MountedDevice,
        sponsor: &ResourceDomain,
    ) -> Result<FallibleArc<Self>, Error> {
        let charge = allocation_charge::<Self>(sponsor)?;
        let filesystem = super::fat::Fatfs::mount(device, sponsor.clone())?;
        FallibleArc::try_new(Self {
            _charge: Some(charge),
            id: FilesystemId(allocate_identifier(&NEXT_FILESYSTEM_ID)?),
            backend: Backend::Fat(FallibleArc::try_new(filesystem)?),
        })
        .map_err(Error::from)
    }
    pub(super) fn pin_mount(&self, node: &NodeLease) -> Result<MountPin, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs
                .pin_mount(node.ramfs()?)
                .map(|pin| MountPin::RamFs { _pin: pin }),
            Backend::Fat(fs) => fs
                .pin_mount(node.fat()?)
                .map(|pin| MountPin::Fat { _pin: pin }),
        }
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
            Backend::RamFs(_) | Backend::Fat(_) => ReadCachePolicy::Direct,
        }
    }

    pub(crate) fn root(&self) -> NodeLease {
        match &self.backend {
            Backend::RamFs(ramfs) => NodeLease::from_ramfs(ramfs.root()),
            Backend::Fat(fs) => NodeLease::from_fat(fs.root()),
        }
    }

    pub(crate) fn attributes(&self, node: &NodeLease) -> Result<NodeAttributes, Error> {
        match &self.backend {
            Backend::RamFs(_) => node.ramfs()?.attributes(),
            Backend::Fat(fs) => fs.attributes(node.fat()?),
        }
    }

    pub(crate) fn lookup_child(
        &self,
        directory: &NodeLease,
        name: Name<'_>,
    ) -> Result<Option<NodeLease>, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs
                .lookup(directory.ramfs()?, name)
                .map(|node| node.map(NodeLease::from_ramfs)),
            Backend::Fat(fs) => fs
                .lookup(directory.fat()?, name)
                .map(|node| node.map(NodeLease::from_fat)),
        }
    }

    pub(crate) fn read_at(
        &self,
        node: &NodeLease,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.read(node.ramfs()?, offset, destination, false),
            Backend::Fat(fs) => fs.read(node.fat()?, offset, destination, false),
        }
    }

    pub(crate) fn read_link(
        &self,
        node: &NodeLease,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.read(node.ramfs()?, 0, destination, true),
            Backend::Fat(fs) => fs.read(node.fat()?, 0, destination, true),
        }
    }

    pub(super) fn read_directory_entry(
        &self,
        node: &NodeLease,
        cookie: u64,
    ) -> Result<Option<DirectoryEntry>, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.entry(node.ramfs()?, cookie),
            Backend::Fat(fs) => fs.entry(node.fat()?, cookie),
        }
    }

    pub(crate) fn executable_snapshot(
        &self,
        node: &NodeLease,
        sponsor: &ResourceDomain,
    ) -> Result<Option<ExecutableSnapshot>, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.executable(node.ramfs()?, sponsor),
            Backend::Fat(fs) => fs.executable(node.fat()?, sponsor),
        }
    }

    pub(super) fn write_at(
        &self,
        node: &NodeLease,
        offset: Option<u64>,
        input: &[u8],
    ) -> Result<(usize, u64), Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.write(node.ramfs()?, offset, input),
            Backend::Fat(fs) => fs.write(node.fat()?, offset, input),
        }
    }

    pub(super) fn resize(&self, node: &NodeLease, length: u64) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.resize(node.ramfs()?, length),
            Backend::Fat(fs) => fs.resize(node.fat()?, length),
        }
    }

    pub(super) fn epoch(&self) -> u64 {
        match &self.backend {
            Backend::RamFs(fs) => fs.epoch(),
            Backend::Fat(fs) => fs.epoch(),
        }
    }
    pub(super) fn wait_for_namespace(&self) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.wait_for_namespace(),
            Backend::Fat(fs) => fs.wait_for_namespace(),
        }
    }
    pub(super) fn ancestry(
        &self,
        root: &NodeLease,
        start: &NodeLease,
        budget: &ScratchBudget,
    ) -> Result<ScratchVec<(NodeLease, ScratchString)>, Error> {
        let mut result = ScratchVec::new(budget.clone());
        match &self.backend {
            Backend::RamFs(fs) => {
                for (node, name) in fs.ancestry(root.ramfs()?, start.ramfs()?, budget)? {
                    result.push((NodeLease::from_ramfs(node), name))?;
                }
            }
            Backend::Fat(fs) => {
                for (node, name) in fs.ancestry(root.fat()?, start.fat()?, budget)? {
                    result.push((NodeLease::from_fat(node), name))?;
                }
            }
        }
        Ok(result)
    }
    pub(super) fn metadata(
        &self,
        node: &NodeLease,
    ) -> Result<(NodeAttributes, NodeMetadata), Error> {
        match &self.backend {
            Backend::RamFs(_) => node.ramfs()?.metadata(),
            Backend::Fat(fs) => fs.metadata(node.fat()?),
        }
    }
    pub(super) fn set_metadata(
        &self,
        node: &NodeLease,
        update: super::MetadataUpdate,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(_) => node.ramfs()?.set_metadata(update),
            Backend::Fat(fs) => fs.set_metadata(node.fat()?, update),
        }
    }
    pub(super) fn create_at<R, E: From<Error>>(
        &self,
        directory: &NodeLease,
        name: EntryName<'_>,
        creation: Creation<'_>,
        epoch: u64,
        publish: impl FnOnce(NodeLease, NodeAttributes) -> Result<R, E>,
    ) -> Result<R, E> {
        let kind = creation.kind;
        match &self.backend {
            Backend::RamFs(fs) => {
                fs.create(directory.ramfs()?, name, creation, Some(epoch), |node| {
                    let attributes = node.attributes()?;
                    publish(NodeLease::from_ramfs(node), attributes)
                })
            }
            Backend::Fat(fs) => fs.create(directory.fat()?, name, creation, Some(epoch), |node| {
                publish(
                    NodeLease::from_fat(node),
                    NodeAttributes::new(kind, 0o777, 0),
                )
            }),
        }
    }
    pub(super) fn open_existing<R, E: From<Error>>(
        &self,
        node: &NodeLease,
        truncate: bool,
        publish: impl FnOnce(NodeAttributes) -> Result<R, E>,
    ) -> Result<R, E> {
        match &self.backend {
            Backend::RamFs(fs) => fs.open_existing(node.ramfs()?, truncate, publish),
            Backend::Fat(fs) => fs.open_existing(node.fat()?, truncate, publish),
        }
    }
    pub(super) fn remove_at(
        &self,
        directory: &NodeLease,
        name: EntryName<'_>,
        kind: hyper::fs::NodeKind,
        expected: Option<u64>,
        epoch: u64,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.remove(directory.ramfs()?, name, kind, expected, Some(epoch)),
            Backend::Fat(fs) => fs.remove(directory.fat()?, name, kind, expected, Some(epoch)),
        }
    }
    pub(super) fn link(
        &self,
        node: &NodeLease,
        directory: &NodeLease,
        name: EntryName<'_>,
        epoch: u64,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.link(node.ramfs()?, directory.ramfs()?, name, epoch),
            Backend::Fat(fs) => fs.link(node.fat()?, directory.fat()?, name, epoch),
        }
    }
    pub(super) fn rename(
        &self,
        source: &NodeLease,
        name: EntryName<'_>,
        destination: &NodeLease,
        new_name: EntryName<'_>,
        epoch: u64,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(fs) => {
                fs.rename(source.ramfs()?, name, destination.ramfs()?, new_name, epoch)
            }
            Backend::Fat(fs) => fs.rename(source.fat()?, name, destination.fat()?, new_name, epoch),
        }
    }
    pub(super) fn sync(&self, node: &NodeLease, scope: u64) -> Result<(), Error> {
        if scope > 1 {
            return Err(Error::InvalidInput);
        }
        // Ramfs operations already complete in memory. This acknowledges that
        // backend contract, without claiming survival across reboot.
        match &self.backend {
            Backend::RamFs(_) => Ok(()),
            Backend::Fat(fs) => fs.sync(node.fat()?, scope),
        }
    }
}

pub(crate) struct Mount {
    _charge: Option<CommittedCharge>,
    _pin: Option<MountPin>,
    id: MountId,
    filesystem: FallibleArc<FilesystemInstance>,
    root: NodeLease,
}

impl Mount {
    pub(super) fn try_new(
        filesystem: FallibleArc<FilesystemInstance>,
        pin: Option<MountPin>,
        sponsor: Option<&ResourceDomain>,
    ) -> Result<FallibleArc<Self>, Error> {
        let charge = sponsor.map(allocation_charge::<Self>).transpose()?;
        let root = filesystem.root();
        FallibleArc::try_new(Self {
            _charge: charge,
            _pin: pin,
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

/// Shared namespace owner with atomically published immutable mount snapshots.
pub(crate) struct MountNamespace {
    root: Location,
    pub(super) mounts: super::mounts::MountTable,
    cache: FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>>,
}

impl MountNamespace {
    pub(super) fn try_new(
        filesystem: FallibleArc<FilesystemInstance>,
        cache: FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>>,
    ) -> Result<FallibleArc<Self>, Error> {
        let mount = Mount::try_new(filesystem, None, None)?;
        let root = Location::new(mount.clone(), mount.root());
        FallibleArc::try_new(Self {
            root,
            cache,
            mounts: super::mounts::MountTable::new(),
        })
        .map_err(Error::from)
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
    hyper::debug::invariant_failure("VFS instance invariant")
}

pub(super) fn allocation_charge<T>(sponsor: &ResourceDomain) -> Result<CommittedCharge, Error> {
    sponsor
        .reserve(ResourceAmount::ZERO.with(
            ResourceKind::KernelMemoryBytes,
            FallibleArc::<T>::allocation_size() as u64,
        ))
        .map(|reservation| reservation.commit())
        .map_err(Error::Resource)
}
