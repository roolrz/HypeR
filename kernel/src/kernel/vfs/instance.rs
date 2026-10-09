// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem, mount, and immutable namespace ownership.

use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use hyper::fs::ramfs::{Error as RamFsError, RamFs};
use hyper::fs::{MAX_NAME_BYTES, Name, NodeAttributes};
use hyper::mm::{AllocationError, FallibleArc, WeakFallibleArc};
use hyper::time::Timestamp;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceDomainId, ResourceKind,
};
use crate::kernel::io_cache::{self, FileDataCache, FileIdentity, NodeIdentity, ReadError};

use super::ExecutableSnapshot;
use super::file_data::FileContent;
use super::file_record::{CachePage, FileRecord};
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
    Remote(hyper_filesystem::protocol::Error),
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
/// Ramfs owns resident boot/runtime data; remote mounts use the format-neutral
/// service contract. Adding a userspace filesystem does not add a kernel
/// adapter variant or change namespace/capability objects.
enum Backend {
    RamFs(super::ramfs::Ramfs),
    Remote(FallibleArc<super::remote::RemoteFs>),
}

/// Whether reads benefit from copying backend data into clean page cache.
///
/// A memory-resident filesystem already owns stable bytes, so caching it would
/// create a second physical copy without avoiding I/O. Cached backends must
/// route every content mutation through the node's shared content gate and
/// must not reuse node identities within one filesystem generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadCachePolicy {
    Direct,
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

/// Speculation retains content identity, never an active namespace lease.
pub(super) struct ReadAheadFile {
    node: WeakFallibleArc<super::remote::Node>,
    content: WeakFallibleArc<FileRecord>,
}

impl ReadAheadFile {
    pub(super) fn new(node: &FallibleArc<super::remote::Node>) -> Self {
        Self {
            node: node.downgrade(),
            content: node.record.downgrade(),
        }
    }

    pub(super) fn content_if_open(&self) -> Option<FallibleArc<FileRecord>> {
        // Liveness is only an admission hint. Closing the last file while a
        // prediction runs must still permit unlink; resolving the incarnation
        // under the backend mutex excludes stale-path I/O after removal.
        self.node
            .is_alive()
            .then(|| self.content.upgrade())
            .flatten()
    }
}

pub(super) struct ReadObservation {
    pub(super) identity: FileIdentity,
    pub(super) length: u64,
    pub(super) had_miss: bool,
}

#[derive(Clone)]
enum NodeBackend {
    RamFs(FallibleArc<super::ramfs::Node>),
    Remote(FallibleArc<super::remote::Node>),
}
impl NodeLease {
    pub(super) fn downgrade_cacheable(&self) -> Option<ReadAheadFile> {
        match &self.0 {
            NodeBackend::RamFs(_) => None,
            NodeBackend::Remote(node) => Some(ReadAheadFile::new(node)),
        }
    }

    fn from_ramfs(node: FallibleArc<super::ramfs::Node>) -> Self {
        Self(NodeBackend::RamFs(node))
    }
    fn from_remote(node: FallibleArc<super::remote::Node>) -> Self {
        Self(NodeBackend::Remote(node))
    }
    pub(crate) fn get(&self) -> u64 {
        match &self.0 {
            NodeBackend::RamFs(node) => node.id(),
            NodeBackend::Remote(node) => node.id(),
        }
    }
    pub(super) fn locks(&self) -> &super::locks::FileLocks {
        match &self.0 {
            NodeBackend::RamFs(node) => &node.locks,
            NodeBackend::Remote(node) => &node.locks,
        }
    }
    fn content(&self) -> &FileContent {
        match &self.0 {
            NodeBackend::RamFs(node) => &node.content,
            NodeBackend::Remote(node) => &node.record.content,
        }
    }
    fn ramfs(&self) -> Result<&FallibleArc<super::ramfs::Node>, Error> {
        match &self.0 {
            NodeBackend::RamFs(node) => Ok(node),
            _ => Err(Error::InvalidBackendResult),
        }
    }
    fn remote(&self) -> Result<&FallibleArc<super::remote::Node>, Error> {
        match &self.0 {
            NodeBackend::Remote(node) => Ok(node),
            _ => Err(Error::InvalidBackendResult),
        }
    }
}

pub(super) enum MountPin {
    RamFs { _pin: super::ramfs::MountPin },
    Remote { _pin: super::remote::MountPin },
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

    pub(super) fn try_from_remote(
        device: super::remote::Transport,
        sponsor: &ResourceDomain,
    ) -> Result<FallibleArc<Self>, Error> {
        let charge = allocation_charge::<Self>(sponsor)?;
        let filesystem = super::remote::RemoteFs::mount(device, sponsor.clone())?;
        FallibleArc::try_new(Self {
            _charge: Some(charge),
            id: FilesystemId(allocate_identifier(&NEXT_FILESYSTEM_ID)?),
            backend: Backend::Remote(FallibleArc::try_new(filesystem)?),
        })
        .map_err(Error::from)
    }
    pub(super) fn pin_mount(&self, node: &NodeLease) -> Result<MountPin, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs
                .pin_mount(node.ramfs()?)
                .map(|pin| MountPin::RamFs { _pin: pin }),
            Backend::Remote(fs) => fs
                .pin_mount(node.remote()?)
                .map(|pin| MountPin::Remote { _pin: pin }),
        }
    }

    pub(crate) const fn id(&self) -> FilesystemId {
        self.id
    }

    pub(super) fn reclaim_file_records(&self, limit: usize) -> (usize, usize) {
        match &self.backend {
            Backend::RamFs(_) => (0, 0),
            Backend::Remote(fs) => fs.reclaim_records(limit),
        }
    }

    pub(super) fn reclaim_idle_records(&self, domain: Option<ResourceDomainId>) {
        if let Backend::Remote(fs) = &self.backend {
            fs.reclaim_idle_records(domain);
        }
    }

    pub(crate) fn cache_generation(&self) -> crate::kernel::io_cache::FilesystemGeneration {
        let Some(generation) = NonZeroU64::new(self.id().get()) else {
            instance_invariant_violation();
        };
        crate::kernel::io_cache::FilesystemGeneration::new(generation)
    }

    const fn read_cache_policy(&self) -> ReadCachePolicy {
        match &self.backend {
            Backend::RamFs(_) => ReadCachePolicy::Direct,
            Backend::Remote(_) => ReadCachePolicy::PageCache,
        }
    }

    pub(crate) fn root(&self) -> NodeLease {
        match &self.backend {
            Backend::RamFs(ramfs) => NodeLease::from_ramfs(ramfs.root()),
            Backend::Remote(fs) => NodeLease::from_remote(fs.root()),
        }
    }

    pub(crate) fn attributes(&self, node: &NodeLease) -> Result<NodeAttributes, Error> {
        match &self.backend {
            Backend::RamFs(_) => node.ramfs()?.attributes(),
            Backend::Remote(fs) => fs.attributes(node.remote()?),
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
            Backend::Remote(fs) => fs
                .lookup(directory.remote()?, name)
                .map(|node| node.map(NodeLease::from_remote)),
        }
    }

    /// Check backend health without reading metadata or file contents. Cached
    /// bytes and lengths must not hide a latched volume failure.
    fn read_status(&self) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(_) => Ok(()),
            Backend::Remote(fs) => fs.read_status(),
        }
    }

    pub(super) fn file_len(&self, node: &NodeLease) -> Result<u64, Error> {
        let mut content = node.content().lock()?;
        self.read_status()?;
        content.length(|| self.attributes(node).map(|attributes| attributes.size()))
    }

    /// Reads into kernel scratch under the content gate shared by all opens.
    ///
    /// Check backend health even on a cache hit, and retain the same content
    /// revision through lookup, backend reads and copying. The caller must copy
    /// to userspace only after this call releases the gate. Cache admission is
    /// opportunistic; inability to retain a page does not fail a backend read.
    pub(super) fn read_file_observed(
        &self,
        node: &NodeLease,
        cache: &FileDataCache<CachePage>,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(usize, Option<ReadObservation>), Error> {
        let mut content = node.content().lock()?;
        self.read_status()?;
        match self.read_cache_policy() {
            ReadCachePolicy::Direct => self
                .read_backend_at(node, offset, destination)
                .map(|bytes| (bytes, None)),
            ReadCachePolicy::PageCache => {
                let length =
                    content.length(|| self.attributes(node).map(|attributes| attributes.size()))?;
                let identity = FileIdentity::new(
                    self.cache_generation(),
                    NodeIdentity::new(NonZeroU64::new(node.get()).ok_or(Error::NotRegularFile)?),
                    content.revision(),
                );
                // The guard covers both cache publication and the copy into
                // kernel scratch. Callers release it before user-page access.
                let (bytes, had_miss) = io_cache::read_observed(
                    cache,
                    identity,
                    &node.remote()?.record,
                    length,
                    offset,
                    destination,
                    |offset, output| self.read_backend_at(node, offset, output),
                )
                .map_err(read_error)?;
                Ok((
                    bytes,
                    Some(ReadObservation {
                        identity,
                        length,
                        had_miss,
                    }),
                ))
            }
        }
    }

    /// Weak queue owners may retain allocation headers after the last strong
    /// owner drops its ordinary charge. Keep a conservative independent charge
    /// until the prediction and all of its weak references are gone.
    pub(super) fn reserve_read_ahead_metadata(&self) -> Result<CommittedCharge, Error> {
        let Backend::Remote(fs) = &self.backend else {
            return Err(Error::Unsupported);
        };
        let bytes = FallibleArc::<Self>::allocation_size()
            .checked_add(FallibleArc::<super::remote::Node>::allocation_size())
            .and_then(|bytes| bytes.checked_add(FallibleArc::<FileRecord>::allocation_size()))
            .and_then(|bytes| {
                bytes.checked_add(FallibleArc::<FileDataCache<CachePage>>::allocation_size())
            })
            .ok_or(Error::Allocation)?;
        fs.reserve_read_ahead_metadata(bytes)
    }

    /// Opportunistic fill for a previously observed content incarnation. It
    /// never waits for a busy content/volume gate and never predicts more work.
    pub(super) fn prefetch_file(
        &self,
        record: &FallibleArc<FileRecord>,
        cache: &FileDataCache<CachePage>,
        expected: FileIdentity,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), Error> {
        let Backend::Remote(fs) = &self.backend else {
            return Ok(());
        };
        let Some(content) = record.content.try_lock()? else {
            return Ok(());
        };
        let identity = FileIdentity::new(
            self.cache_generation(),
            NodeIdentity::new(NonZeroU64::new(record.id()).ok_or(Error::NotRegularFile)?),
            content.revision(),
        );
        if identity != expected || !cache.prefetch_allowed() {
            return Ok(());
        }
        let Some(length) = content.known_length() else {
            return Ok(());
        };
        self.read_status()?;
        io_cache::read(
            cache,
            identity,
            record,
            length,
            offset,
            destination,
            |at, bytes| fs.try_read(record, at, bytes)?.ok_or(Error::Busy),
        )
        .map_err(read_error)?;
        Ok(())
    }

    /// Raw regular-file reads are private to the content coordinator. A
    /// backend must never call back through a guarded `FileObject` operation.
    fn read_backend_at(
        &self,
        node: &NodeLease,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.read(node.ramfs()?, offset, destination, false),
            Backend::Remote(fs) => fs.read(node.remote()?, offset, destination, false),
        }
    }

    pub(crate) fn read_link(
        &self,
        node: &NodeLease,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.read(node.ramfs()?, 0, destination, true),
            Backend::Remote(fs) => fs.read(node.remote()?, 0, destination, true),
        }
    }

    pub(super) fn read_directory_entry(
        &self,
        node: &NodeLease,
        cookie: u64,
    ) -> Result<Option<DirectoryEntry>, Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.entry(node.ramfs()?, cookie),
            Backend::Remote(fs) => fs.entry(node.remote()?, cookie),
        }
    }

    /// Obtains an executable snapshot while excluding concurrent file mutation.
    ///
    /// The returned snapshot owns its bytes independently of this open node.
    /// `None` means the backend cannot supply an executable file at this node;
    /// backend and allocation failures remain errors.
    pub(crate) fn executable_snapshot(
        &self,
        node: &NodeLease,
        sponsor: &ResourceDomain,
    ) -> Result<Option<ExecutableSnapshot>, Error> {
        let _content = node.content().lock()?;
        self.read_status()?;
        match &self.backend {
            Backend::RamFs(fs) => fs.executable(node.ramfs()?, sponsor),
            Backend::Remote(fs) => fs.executable(node.remote()?, sponsor),
        }
    }

    /// Writes at an explicit offset, or appends when `offset` is `None`.
    ///
    /// Returns the byte count and resulting offset. Advance the content
    /// revision before entering the backend: even an error may follow a partial
    /// mutation. The shared content gate also serializes append positioning
    /// against other opens of the file.
    pub(super) fn write_at(
        &self,
        node: &NodeLease,
        offset: Option<u64>,
        input: &[u8],
    ) -> Result<(usize, u64), Error> {
        let mut content = node.content().lock()?;
        content.begin_mutation()?;
        match &self.backend {
            Backend::RamFs(fs) => fs.write(node.ramfs()?, offset, input),
            Backend::Remote(fs) => fs.write(node.remote()?, offset, input),
        }
    }

    pub(super) fn resize(&self, node: &NodeLease, length: u64) -> Result<(), Error> {
        let mut content = node.content().lock()?;
        content.begin_mutation()?;
        match &self.backend {
            Backend::RamFs(fs) => fs.resize(node.ramfs()?, length),
            Backend::Remote(fs) => fs.resize(node.remote()?, length),
        }?;
        content.set_length(length);
        Ok(())
    }

    pub(super) fn epoch(&self) -> u64 {
        match &self.backend {
            Backend::RamFs(fs) => fs.epoch(),
            Backend::Remote(fs) => fs.epoch(),
        }
    }
    pub(super) fn wait_for_namespace(&self) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(fs) => fs.wait_for_namespace(),
            Backend::Remote(fs) => fs.wait_for_namespace(),
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
            Backend::Remote(fs) => {
                for (node, name) in fs.ancestry(root.remote()?, start.remote()?, budget)? {
                    result.push((NodeLease::from_remote(node), name))?;
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
            Backend::Remote(fs) => fs.metadata(node.remote()?),
        }
    }
    pub(super) fn set_metadata(
        &self,
        node: &NodeLease,
        update: super::MetadataUpdate,
    ) -> Result<(), Error> {
        match &self.backend {
            Backend::RamFs(_) => node.ramfs()?.set_metadata(update),
            Backend::Remote(fs) => fs.set_metadata(node.remote()?, update),
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
            Backend::Remote(fs) => {
                fs.create(directory.remote()?, name, creation, Some(epoch), |node| {
                    publish(
                        NodeLease::from_remote(node),
                        NodeAttributes::new(kind, 0o777, 0),
                    )
                })
            }
        }
    }
    pub(super) fn open_existing<R, E: From<Error>>(
        &self,
        node: &NodeLease,
        truncate: bool,
        publish: impl FnOnce(NodeAttributes) -> Result<R, E>,
    ) -> Result<R, E> {
        let mut content = node.content().lock()?;
        if truncate {
            content.begin_mutation()?;
        }
        // The callback prepares an object from the supplied attributes; it
        // must not reenter content operations while backend locks are held.
        // Actual handle publication follows a successful backend operation.
        let prepared = match &self.backend {
            Backend::RamFs(fs) => fs.open_existing(node.ramfs()?, truncate, publish),
            Backend::Remote(fs) => fs.open_existing(node.remote()?, truncate, publish),
        }?;
        if truncate {
            content.set_length(0);
        }
        Ok(prepared)
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
            Backend::Remote(fs) => {
                fs.remove(directory.remote()?, name, kind, expected, Some(epoch))
            }
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
            Backend::Remote(fs) => fs.link(node.remote()?, directory.remote()?, name, epoch),
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
            Backend::Remote(fs) => fs.rename(
                source.remote()?,
                name,
                destination.remote()?,
                new_name,
                epoch,
            ),
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
            Backend::Remote(fs) => fs.sync(node.remote()?, scope),
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

    pub(super) fn filesystem_owner(&self) -> &FallibleArc<FilesystemInstance> {
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
    cache: FallibleArc<FileDataCache<CachePage>>,
}

impl MountNamespace {
    pub(super) fn try_new(
        filesystem: FallibleArc<FilesystemInstance>,
        cache: FallibleArc<FileDataCache<CachePage>>,
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

    pub(super) fn cache(&self) -> FallibleArc<FileDataCache<CachePage>> {
        self.cache.clone()
    }

    pub(super) fn cache_ref(&self) -> &FileDataCache<CachePage> {
        &self.cache
    }

    pub(super) fn reclaim_file_records(&self, limit: usize) -> usize {
        self.mounts
            .reclaim_file_records(self.root.mount().filesystem(), limit)
    }

    pub(super) fn reclaim_idle_records(&self, domain: Option<ResourceDomainId>) {
        self.mounts
            .reclaim_idle_records(self.root.mount().filesystem(), domain);
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

fn read_error(error: ReadError<Error>) -> Error {
    match error {
        ReadError::Backend(error) => error,
        ReadError::InvalidBackendResult => Error::InvalidBackendResult,
        ReadError::ArithmeticOverflow => Error::InvalidInput,
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
