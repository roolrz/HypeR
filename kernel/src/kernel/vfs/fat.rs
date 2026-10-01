// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Disk-backed FAT namespace adapter. All media access is serialized by one
//! sleepable volume mutex. Live leases prevent unlink; rename updates every
//! retained descendant path before releasing the namespace transaction.

use super::file_record::FileRecord;
use super::instance::{Creation, EntryName, Error};
use super::instance::{EntrySnapshot, NodeMetadata};
use super::scratch::{ScratchBudget, ScratchString, ScratchVec};
use super::{ExecutableSnapshot, MetadataUpdate};
use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceDomainId, ResourceError, ResourceKind,
};
use crate::kernel::mm::user_space::{DomainAccount, KernelPageBackend, SnapshotVmo};
use crate::kernel::sync::Mutex;
use alloc::{boxed::Box, string::String, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering};
use hyper::fs::block::{BlockDevice, Error as BlockError};
use hyper::fs::fat::{Entry, Error as FatError, FatVolume};
use hyper::fs::{MAX_NAME_BYTES, MAX_PATH_BYTES, Name, NodeAttributes, NodeKind};
use hyper::mm::{FallibleArc, WeakFallibleArc};

mod records;
use records::{PreparedRecord, Records};
#[cfg(feature = "kernel-self-test")]
#[path = "../../../tests/kernel/fat_records.rs"]
mod records_test;
#[cfg(feature = "kernel-self-test")]
pub(super) use records_test::run as test_record_storage;

pub(super) struct Node {
    mount_pins: AtomicU64,
    pub(super) locks: super::locks::FileLocks,
    pub(super) record: FallibleArc<FileRecord>,
    _charge: CommittedCharge,
}
impl Node {
    pub(super) fn id(&self) -> u64 {
        self.record.id()
    }
}
pub(super) struct MountPin(FallibleArc<Node>);
impl Drop for MountPin {
    fn drop(&mut self) {
        self.0.mount_pins.fetch_sub(1, Ordering::Release);
    }
}
struct Record {
    id: u64,
    path: String,
    node: WeakFallibleArc<Node>,
    content: WeakFallibleArc<FileRecord>,
    _charge: CommittedCharge,
}
struct State<D: BlockDevice> {
    volume: FatVolume<D>,
    metadata: Box<Entry>,
    records: Records,
    next_id: u64,
}
pub(super) struct Fatfs<D: BlockDevice> {
    state: Mutex<State<D>>,
    root: FallibleArc<Node>,
    domain: ResourceDomain,
    epoch: AtomicU64,
    _charge: CommittedCharge,
}
struct Mutation<'a>(&'a AtomicU64, u64);
impl Drop for Mutation<'_> {
    fn drop(&mut self) {
        self.0.store(self.1, Ordering::Release);
    }
}
impl<D: BlockDevice> Fatfs<D> {
    #[inline(never)]
    pub(super) fn mount(device: D, domain: ResourceDomain) -> Result<Self, Error> {
        let scratch = charge(
            &domain,
            FatVolume::<D>::mount_scratch_bound(device.sector_count()).map_err(map)?,
        )?;
        let persistent = charge(
            &domain,
            FallibleArc::<Self>::allocation_size()
                .checked_add(FatVolume::<D>::allocation_bytes())
                .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Entry>()))
                .ok_or(Error::Allocation)?,
        )?;
        let metadata = metadata_scratch()?;
        let volume = mount_volume(device)?;
        drop(scratch);
        let root = node(FileRecord::try_new(0, &domain)?, &domain)?;
        let mut state = State {
            volume,
            metadata,
            records: Records::new(),
            next_id: 1,
        };
        state.records.insert(Records::prepare(Record {
            id: 0,
            path: String::new(),
            node: root.downgrade(),
            content: root.record.downgrade(),
            _charge: record_charge(&domain)?,
        })?);
        Ok(Self {
            state: Mutex::new(state),
            root,
            domain,
            epoch: AtomicU64::new(0),
            _charge: persistent,
        })
    }
    #[inline(never)]
    pub(super) fn pin_mount(&self, node: &FallibleArc<Node>) -> Result<MountPin, Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        if !state.is_directory(&path)? {
            return Err(Error::NotDirectory);
        }
        node.mount_pins
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| Error::IdentifierExhausted)?;
        Ok(MountPin(node.clone()))
    }
    pub(super) fn root(&self) -> FallibleArc<Node> {
        self.root.clone()
    }
    pub(super) fn read_status(&self) -> Result<(), Error> {
        self.state
            .lock()
            .map_err(Error::Lock)?
            .volume
            .read_status()
            .map_err(map)
    }
    pub(super) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
    pub(super) fn wait_for_namespace(&self) -> Result<(), Error> {
        drop(self.state.lock().map_err(Error::Lock)?);
        Ok(())
    }

    /// Scheduled housekeeping only: no media I/O and no waiting for a busy
    /// namespace transaction. Page reclaim has already released content pins.
    pub(super) fn reclaim_records(&self, limit: usize) -> (usize, usize) {
        let mut inspected = 0;
        let mut reclaimed = 0;
        let mut remaining = None;
        while inspected < limit {
            let retired = {
                let Ok(Some(mut state)) = self.state.try_lock() else {
                    break;
                };
                let remaining = remaining.get_or_insert(state.records.len());
                if *remaining == 0 {
                    break;
                }
                *remaining -= 1;
                let Some(retired) = state.records.inspect_next() else {
                    break;
                };
                retired
            };
            inspected += 1;
            if let Some(record) = retired {
                reclaimed += 1;
                // Release path storage, weak headers, and quota after the
                // namespace lock, independently of cache/allocator locks.
                drop(record);
            }
        }
        (inspected, reclaimed)
    }

    /// Finish one finite cleanup pass without waiting for a namespace owner.
    /// A quota requester can hold this volume's mutex; it prunes its own dead
    /// bindings after the page worker completes instead.
    pub(super) fn reclaim_idle_records(&self, domain: Option<ResourceDomainId>) {
        if domain.is_some_and(|id| !self.domain.charges_domain(id)) {
            return;
        }
        let mut remaining = match self.state.try_lock() {
            Ok(Some(state)) => state.records.cleanup_bound(),
            _ => return,
        };
        while remaining != 0 {
            let (inspected, _) = self.reclaim_records(remaining.min(64));
            if inspected == 0 {
                break;
            }
            remaining -= inspected;
            if crate::kernel::task::scheduler::cond_resched().is_err() {
                break;
            }
        }
    }
    fn mutation(&self, expected: Option<u64>) -> Result<Mutation<'_>, Error> {
        let epoch = self.epoch();
        if expected.is_some_and(|v| v != epoch) {
            return Err(Error::Busy);
        }
        let next = epoch.checked_add(2).ok_or(Error::IdentifierExhausted)?;
        self.epoch.store(epoch + 1, Ordering::Release);
        Ok(Mutation(&self.epoch, next))
    }
    #[inline(never)]
    pub(super) fn attributes(&self, node: &Node) -> Result<NodeAttributes, Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        Ok(attributes(entry))
    }
    #[inline(never)]
    pub(super) fn metadata(&self, node: &Node) -> Result<(NodeAttributes, NodeMetadata), Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        let attributes = attributes(entry);
        Ok((
            attributes,
            NodeMetadata {
                mode: attributes.mode(),
                accessed: entry.accessed,
                modified: entry.modified,
                created: entry.created,
                changed: None,
            },
        ))
    }
    #[inline(never)]
    pub(super) fn set_metadata(&self, node: &Node, update: MetadataUpdate) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        if update
            .mode
            .is_some_and(|mode| mode != attributes(entry).mode())
        {
            return Err(Error::Unsupported);
        }
        if update.accessed.is_none() && update.modified.is_none() {
            return Ok(());
        }
        state
            .volume
            .set_times(&path, update.accessed, update.modified)
            .map_err(map)
    }
    #[inline(never)]
    pub(super) fn lookup(
        &self,
        directory: &Node,
        name: Name<'_>,
    ) -> Result<Option<FallibleArc<Node>>, Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES * 3)?;
        let parent = state.path(directory)?;
        if !state.is_directory(&parent)? {
            return Err(Error::NotDirectory);
        }
        let path = child(&parent, name)?;
        let entry = match state.stat(&path) {
            Ok(entry) => entry,
            Err(Error::Missing) => return Ok(None),
            Err(e) => return Err(e),
        };
        // Use the stored spelling so differently cased opens share one lease.
        let canonical = join(&parent, entry.name())?;
        state.lease(canonical, &self.domain).map(Some)
    }
    #[inline(never)]
    pub(super) fn entry(
        &self,
        directory: &Node,
        cookie: u64,
    ) -> Result<Option<EntrySnapshot>, Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(directory)?;
        let mut index = usize::try_from(cookie).map_err(|_| Error::InvalidDirectoryCookie)?;
        loop {
            let Some(entry) = state.entry(&path, index)? else {
                return Ok(None);
            };
            index = index.checked_add(1).ok_or(Error::InvalidDirectoryCookie)?;
            if matches!(entry.name(), "." | "..") {
                continue;
            }
            if entry.name_len > MAX_NAME_BYTES {
                return Err(Error::InvalidBackendResult);
            }
            let mut name = [0; MAX_NAME_BYTES];
            name[..entry.name_len].copy_from_slice(&entry.name[..entry.name_len]);
            return Ok(Some(EntrySnapshot {
                name,
                length: entry.name_len,
                attributes: attributes(entry),
                next_cookie: index as u64,
            }));
        }
    }
    #[inline(never)]
    pub(super) fn read(
        &self,
        node: &Node,
        offset: u64,
        destination: &mut [u8],
        symlink: bool,
    ) -> Result<usize, Error> {
        if symlink {
            return Err(Error::NotSymlink);
        }
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        if state.is_directory(&path)? {
            return Err(Error::IsDirectory);
        }
        state
            .volume
            .read_at(&path, offset, destination)
            .map_err(map)
    }
    #[inline(never)]
    pub(super) fn write(
        &self,
        node: &Node,
        offset: Option<u64>,
        input: &[u8],
    ) -> Result<(usize, u64), Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        if entry.directory {
            return Err(Error::IsDirectory);
        }
        if entry.read_only {
            return Err(Error::Fat(FatError::Block(BlockError::ReadOnly)));
        }
        let offset = offset.unwrap_or(entry.size);
        let written = state.volume.write_at(&path, offset, input).map_err(map)?;
        Ok((written, offset + written as u64))
    }
    #[inline(never)]
    pub(super) fn resize(&self, node: &Node, length: u64) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        if entry.directory {
            return Err(Error::IsDirectory);
        }
        if entry.read_only {
            return Err(Error::Fat(FatError::Block(BlockError::ReadOnly)));
        }
        state.volume.resize(&path, length).map_err(map)
    }
    #[inline(never)]
    pub(super) fn sync(&self, _node: &Node, scope: u64) -> Result<(), Error> {
        if scope > 1 {
            return Err(Error::InvalidInput);
        }
        self.state
            .lock()
            .map_err(Error::Lock)?
            .volume
            .sync()
            .map_err(map)
    }
    #[inline(never)]
    pub(super) fn executable(
        &self,
        node: &Node,
        sponsor: &ResourceDomain,
    ) -> Result<Option<ExecutableSnapshot>, Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        if entry.directory {
            return Ok(None);
        }
        let size = usize::try_from(entry.size).map_err(|_| Error::InvalidSize)?;
        let charge = state.reclaim_retry(|_| charge(sponsor, size))?;
        let mut bytes = Vec::new();
        crate::kernel::mm::reclaim::reserve_exact(&mut bytes, size)
            .map_err(|_| Error::Allocation)?;
        bytes.resize(size, 0);
        if state.volume.read_at(&path, 0, &mut bytes).map_err(map)? != size {
            return Err(Error::InvalidBackendResult);
        }
        let storage = SnapshotVmo::try_from_bytes(
            &bytes,
            KernelPageBackend,
            DomainAccount::new(sponsor.clone()),
        )
        .map_err(|_| Error::Allocation)?;
        Ok(Some(ExecutableSnapshot::owned(bytes, charge, storage)))
    }
    #[inline(never)]
    pub(super) fn create<R, E: From<Error>>(
        &self,
        directory: &FallibleArc<Node>,
        name: EntryName<'_>,
        creation: Creation<'_>,
        epoch: Option<u64>,
        publish: impl FnOnce(FallibleArc<Node>) -> Result<R, E>,
    ) -> Result<R, E> {
        if creation.target.is_some()
            || !matches!(creation.kind, NodeKind::File | NodeKind::Directory)
        {
            return Err(Error::Unsupported.into());
        }
        if name.directory_required && creation.kind != NodeKind::Directory {
            return Err(Error::NotDirectory.into());
        }
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES * 3)?;
        let _mutation = self.mutation(epoch)?;
        let parent = state.path(directory)?;
        if !state.is_directory(&parent)? {
            return Err(Error::NotDirectory.into());
        }
        let path = child(&parent, name.name)?;
        // Prepare every owner and binding allocation before touching the directory.
        // The callback prepares a handle; it must not perform backend I/O.
        // A failed callback or create drops this unpublished binding, so later
        // lookup cannot inherit a ghost identity from a failed create.
        let id = state.next_id;
        state.next_id = id.checked_add(1).ok_or(Error::IdentifierExhausted)?;
        let (lease, record) = state.prepare_node(id, path, &self.domain)?;
        let prepared = publish(lease)?;
        state
            .volume
            .create(&record.value().path, creation.kind == NodeKind::Directory)
            .map_err(map)?;
        // Creation succeeded while the namespace mutex excluded competing
        // lookups. Retire any expired name binding before publishing this one.
        state
            .records
            .retain(|entry| entry.path != record.value().path);
        state.records.insert(record);
        Ok(prepared)
    }
    #[inline(never)]
    pub(super) fn open_existing<R, E: From<Error>>(
        &self,
        node: &FallibleArc<Node>,
        truncate: bool,
        publish: impl FnOnce(NodeAttributes) -> Result<R, E>,
    ) -> Result<R, E> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES)?;
        let path = state.path(node)?;
        let entry = state.stat(&path)?;
        if entry.directory {
            return Err(Error::NotRegularFile.into());
        }
        let read_only = entry.read_only;
        let attributes = attributes(entry);
        let prepared = publish(attributes)?;
        if truncate {
            if read_only {
                return Err(Error::Fat(FatError::Block(BlockError::ReadOnly)).into());
            }
            state.volume.resize(&path, 0).map_err(map)?;
        }
        Ok(prepared)
    }

    #[inline(never)]
    pub(super) fn remove(
        &self,
        directory: &Node,
        name: EntryName<'_>,
        kind: NodeKind,
        expected: Option<u64>,
        epoch: Option<u64>,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES * 3)?;
        let _mutation = self.mutation(epoch)?;
        let parent = state.path(directory)?;
        let candidate = child(&parent, name.name)?;
        let entry = state.stat(&candidate)?;
        let path = join(&parent, entry.name())?;
        if name.directory_required && !entry.directory {
            return Err(Error::NotDirectory);
        }
        if kind == NodeKind::Directory && !entry.directory {
            return Err(Error::NotDirectory);
        }
        if kind != NodeKind::Directory && entry.directory {
            return Err(Error::IsDirectory);
        }
        if state.records.iter().any(|record| {
            record.path == path
                && (record.node.upgrade().is_some() || expected.is_some_and(|id| record.id != id))
        }) {
            return Err(Error::Busy);
        }
        state.volume.remove(&path).map_err(map)?;
        // Cached content is not an active lease. Its old numeric identity may
        // remain in clean pages until eviction, but the removed name must never
        // reconnect it to a new file (or a recreated directory subtree).
        state
            .records
            .retain(|record| record.path != path && !within(&record.path, &path));
        Ok(())
    }
    #[inline(never)]
    pub(super) fn link(
        &self,
        _node: &Node,
        _directory: &Node,
        _name: EntryName<'_>,
        _epoch: u64,
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    #[inline(never)]
    pub(super) fn rename(
        &self,
        source: &Node,
        name: EntryName<'_>,
        destination: &Node,
        new_name: EntryName<'_>,
        epoch: u64,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES * 5)?;
        let _mutation = self.mutation(Some(epoch))?;
        let parent = state.path(source)?;
        let candidate = child(&parent, name.name)?;
        let entry = state.stat(&candidate)?;
        let old = join(&parent, entry.name())?;
        let directory = entry.directory;
        let same_name = entry
            .name()
            .chars()
            .flat_map(char::to_uppercase)
            .eq(new_name.name.as_str().chars().flat_map(char::to_uppercase));
        let destination_path = state.path(destination)?;
        if !state.is_directory(&destination_path)? {
            return Err(Error::NotDirectory);
        }
        let new = child(&destination_path, new_name.name)?;
        if (name.directory_required || new_name.directory_required) && !directory {
            return Err(Error::NotDirectory);
        }
        // Upstream treats a same-entry case-only rename as a no-op and keeps
        // its on-disk spelling. Preserve that spelling in the lease registry,
        // otherwise the next canonical lookup creates a second lock identity.
        if parent == destination_path && same_name {
            return Ok(());
        }
        if within(&new, &old) {
            return Err(Error::InvalidInput);
        }
        let _scratch = state.reclaim_retry(|state| {
            let bytes = state
                .records
                .len()
                .checked_mul(MAX_PATH_BYTES + core::mem::size_of::<(u64, String)>())
                .ok_or(Error::Allocation)?;
            charge(&self.domain, bytes)
        })?;
        let mut replacements = Vec::new();
        replacements
            .try_reserve_exact(state.records.len())
            .map_err(|_| Error::Allocation)?;
        for record in state.records.iter() {
            if record.path == old || within(&record.path, &old) {
                if record
                    .node
                    .upgrade()
                    .is_some_and(|node| node.mount_pins.load(Ordering::Acquire) != 0)
                {
                    return Err(Error::Busy);
                }
                let suffix = &record.path[old.len()..];
                let length = new
                    .len()
                    .checked_add(suffix.len())
                    .filter(|n| *n <= MAX_PATH_BYTES)
                    .ok_or(Error::InvalidInput)?;
                let mut path = String::new();
                path.try_reserve_exact(length)
                    .map_err(|_| Error::Allocation)?;
                path.push_str(&new);
                path.push_str(suffix);
                replacements.push((record.id, path));
            }
        }
        state.volume.rename(&old, &new).map_err(map)?;
        let mut replacements = replacements.into_iter().peekable();
        // The volume mutex preserves traversal order from preparation through
        // commit. Apply the prepared IDs in one pass, without repeated lookup.
        for record in state.records.iter_mut() {
            if replacements.peek().is_some_and(|(id, _)| *id == record.id)
                && let Some((_, path)) = replacements.next()
            {
                record.path = path;
            }
        }
        if replacements.next().is_some() {
            hyper::debug::invariant_failure("FAT rename binding order changed");
        }
        Ok(())
    }
    #[inline(never)]
    pub(super) fn ancestry(
        &self,
        root: &FallibleArc<Node>,
        start: &FallibleArc<Node>,
        budget: &ScratchBudget,
    ) -> Result<ScratchVec<(FallibleArc<Node>, ScratchString)>, Error> {
        let mut state = self.state.lock().map_err(Error::Lock)?;
        let _paths = state.scratch_charge(&self.domain, MAX_PATH_BYTES * 5)?;
        let root_path = state.path(root)?;
        let start_path = state.path(start)?;
        if root_path != start_path && !within(&start_path, &root_path) {
            return Err(Error::Missing);
        }
        let mut output = ScratchVec::new(budget.clone());
        let mut path = copy(&root_path)?;
        for component in start_path[root_path.len()..]
            .split('/')
            .filter(|s| !s.is_empty())
        {
            path = join(&path, component)?;
            let node = state.lease(copy(&path)?, &self.domain)?;
            let mut name = ScratchString::new(budget.clone());
            name.try_reserve_exact(component.len())?;
            name.push_str(component)?;
            output.try_reserve(1)?;
            output.push((node, name))?;
        }
        Ok(output)
    }
}
impl<D: BlockDevice> State<D> {
    fn reclaim_retry<T>(
        &mut self,
        mut operation: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut result = operation(self);
        for _ in 0..2 {
            let Err(Error::Resource(ResourceError::LimitExceeded {
                domain,
                resource: ResourceKind::KernelMemoryBytes,
                ..
            })) = &result
            else {
                break;
            };
            // Only unpublished preparation is retried. The worker scans the
            // denying domain (including descendant mount sponsors), never
            // waits for our volume mutex, and drops cache owners outside its
            // locks. Keep admissions paused through local pruning and retry.
            let Some(pause) = crate::kernel::mm::reclaim::reclaim(
                crate::kernel::mm::reclaim::Target::Domain(*domain),
            ) else {
                break;
            };
            self.records.retain(|record| record.content.is_alive());
            result = operation(self);
            drop(pause);
        }
        result
    }

    fn scratch_charge(
        &mut self,
        domain: &ResourceDomain,
        bytes: usize,
    ) -> Result<CommittedCharge, Error> {
        self.reclaim_retry(|_| charge(domain, bytes))
    }

    fn prepare_node(
        &mut self,
        id: u64,
        path: String,
        domain: &ResourceDomain,
    ) -> Result<(FallibleArc<Node>, PreparedRecord), Error> {
        // Keep the path across a quota retry; all fallible quota admission and
        // object allocation precedes installing it in the prepared binding.
        let (node, mut record) = self.reclaim_retry(|_| {
            let record_charge = record_charge(domain)?;
            let node = node(FileRecord::try_new(id, domain)?, domain)?;
            let record = Records::prepare(Record {
                id,
                path: String::new(),
                node: node.downgrade(),
                content: node.record.downgrade(),
                _charge: record_charge,
            })?;
            Ok((node, record))
        })?;
        record.set_path(path);
        Ok((node, record))
    }

    fn stat(&mut self, path: &str) -> Result<&Entry, Error> {
        self.volume
            .stat_into(path, &mut self.metadata)
            .map_err(map)?;
        Ok(&self.metadata)
    }
    fn entry(&mut self, path: &str, index: usize) -> Result<Option<&Entry>, Error> {
        if self
            .volume
            .entry_into(path, index, &mut self.metadata)
            .map_err(map)?
        {
            Ok(Some(&self.metadata))
        } else {
            Ok(None)
        }
    }
    fn is_directory(&mut self, path: &str) -> Result<bool, Error> {
        Ok(self.stat(path)?.directory)
    }

    fn path(&self, node: &Node) -> Result<String, Error> {
        copy(
            &self
                .records
                .iter()
                .find(|record| record.id == node.id())
                .ok_or(Error::Missing)?
                .path,
        )
    }
    fn lease(&mut self, path: String, domain: &ResourceDomain) -> Result<FallibleArc<Node>, Error> {
        self.records.retain(|record| record.content.is_alive());
        let existing = self
            .records
            .iter()
            .find(|record| record.path == path)
            .map(|record| (record.node.upgrade(), record.content.upgrade()));
        if let Some((active, content)) = existing {
            if let Some(node) = active {
                return Ok(node);
            }
            if let Some(content) = content {
                let node = self.reclaim_retry(|_| node(content.clone(), domain))?;
                let Some(record) = self
                    .records
                    .iter_mut()
                    .find(|record| record.id == node.id())
                else {
                    return Err(Error::InvalidBackendResult);
                };
                record.node = node.downgrade();
                return Ok(node);
            }
            // Reclaim may have dropped the final page pin since retain(). No
            // active node survives without its record, so a fresh identity is
            // now safe. IDs are never reused within the mounted filesystem.
            self.records.retain(|record| record.path != path);
        }
        let id = self.next_id;
        let next = id.checked_add(1).ok_or(Error::IdentifierExhausted)?;
        let (node, record) = self.prepare_node(id, path, domain)?;
        self.records.insert(record);
        self.next_id = next;
        Ok(node)
    }
}
fn node(
    record: FallibleArc<FileRecord>,
    domain: &ResourceDomain,
) -> Result<FallibleArc<Node>, Error> {
    let charge = domain
        .reserve(ResourceAmount::ZERO.with(
            ResourceKind::KernelMemoryBytes,
            FallibleArc::<Node>::allocation_size() as u64,
        ))
        .map_err(Error::Resource)?
        .commit();
    FallibleArc::try_new(Node {
        mount_pins: AtomicU64::new(0),
        locks: super::locks::FileLocks::new(),
        record,
        _charge: charge,
    })
    .map_err(Error::from)
}
fn child(parent: &str, name: Name<'_>) -> Result<String, Error> {
    join(
        parent,
        core::str::from_utf8(name.as_bytes()).map_err(|_| Error::InvalidInput)?,
    )
}
fn join(parent: &str, name: &str) -> Result<String, Error> {
    let len = parent
        .len()
        .checked_add(name.len())
        .and_then(|n| n.checked_add(usize::from(!parent.is_empty())))
        .filter(|n| *n <= MAX_PATH_BYTES)
        .ok_or(Error::InvalidInput)?;
    let mut output = String::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| Error::Allocation)?;
    output.push_str(parent);
    if !parent.is_empty() {
        output.push('/');
    }
    output.push_str(name);
    Ok(output)
}
fn copy(value: &str) -> Result<String, Error> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| Error::Allocation)?;
    output.push_str(value);
    Ok(output)
}
fn within(path: &str, parent: &str) -> bool {
    parent.is_empty()
        || path
            .strip_prefix(parent)
            .is_some_and(|tail| tail.starts_with('/'))
}
fn attributes(entry: &Entry) -> NodeAttributes {
    NodeAttributes::new(
        if entry.directory {
            NodeKind::Directory
        } else {
            NodeKind::File
        },
        if entry.read_only { 0o555 } else { 0o777 },
        entry.size,
    )
}
fn map(error: FatError) -> Error {
    match error {
        FatError::Allocation => Error::Allocation,
        FatError::Missing => Error::Missing,
        FatError::Exists => Error::AlreadyExists,
        FatError::NotEmpty => Error::NotEmpty,
        FatError::InvalidInput => Error::InvalidInput,
        FatError::Unsupported => Error::Unsupported,
        other => Error::Fat(other),
    }
}

fn record_charge(domain: &ResourceDomain) -> Result<CommittedCharge, Error> {
    // Both Weak owners retain their complete shared allocations after the
    // strong payload charges retire. Cover them here as well; while live this
    // deliberately charges conservatively rather than leaving dead headers
    // outside the mount sponsor's quota.
    let bytes = MAX_PATH_BYTES
        + Records::allocation_size()
        + FallibleArc::<Node>::allocation_size()
        + FallibleArc::<FileRecord>::allocation_size();
    domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, bytes as u64))
        .map(|charge| charge.commit())
        .map_err(Error::Resource)
}

fn charge(domain: &ResourceDomain, bytes: usize) -> Result<CommittedCharge, Error> {
    domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, bytes as u64))
        .map(|charge| charge.commit())
        .map_err(Error::Resource)
}

// Allocate one charged metadata buffer per mounted volume, under its existing
// sleepable lock. Reusing it removes per-call stack copies without per-I/O
// allocation. Keep construction out of the mount parser's live frame.
#[inline(never)]
fn metadata_scratch() -> Result<Box<Entry>, Error> {
    hyper::mm::try_box(Entry::empty()).map_err(|_| Error::Allocation)
}

// Complete volume parsing before constructing the namespace owner, keeping
// admission scratch out of the later namespace-construction call chain.
#[inline(never)]
fn mount_volume<D: BlockDevice>(device: D) -> Result<FatVolume<D>, Error> {
    FatVolume::mount_with_clock(device, crate::kernel::time::realtime).map_err(map)
}
