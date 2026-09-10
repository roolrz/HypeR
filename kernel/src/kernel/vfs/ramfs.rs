// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel-resident ramfs storage. Directory names and open-node leases have
//! separate lifetimes; the tree owns only downward edges.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use hyper::fs::file_data::{FileData, StorageBudget};
use hyper::fs::ramfs::RamFs;
use hyper::fs::{MAX_NAME_BYTES, Name, NodeAttributes, NodeKind};
use hyper::mm::FallibleArc;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind, ResourceLimits,
};
use crate::kernel::sync::Mutex;

use super::ExecutableSnapshot;
use super::instance::Error;

const STORAGE_LIMIT: u64 = 256 * 1024 * 1024;

pub(super) struct Budget(ResourceDomain);

impl StorageBudget for Budget {
    type Charge = CommittedCharge;
    type Error = ResourceError;

    fn reserve(&self, bytes: usize) -> Result<CommittedCharge, ResourceError> {
        self.0
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, bytes as u64))
            .map(|charge| charge.commit())
    }
}

pub(super) struct Ramfs {
    root: FallibleArc<Node>,
    next_id: AtomicU64,
    budget: Budget,
    /// Serializes structural changes, independently of all file content I/O.
    mutation: Mutex<()>,
}

pub(super) struct Node {
    id: u64,
    linked: AtomicBool,
    attributes: NodeAttributes,
    data: Mutex<FileData<'static, CommittedCharge>>,
    children: Mutex<Children>,
    _charge: CommittedCharge,
}

struct Children {
    entries: Vec<Entry>,
    next_cookie: u64,
    _charge: Option<CommittedCharge>,
}

struct Entry {
    name: [u8; MAX_NAME_BYTES],
    length: usize,
    cookie: u64,
    node: FallibleArc<Node>,
}

pub(super) struct EntrySnapshot {
    pub(super) name: [u8; MAX_NAME_BYTES],
    pub(super) length: usize,
    pub(super) attributes: NodeAttributes,
    pub(super) next_cookie: u64,
}

impl Children {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_cookie: 1,
            _charge: None,
        }
    }

    fn prepare_insert(&mut self, budget: &Budget) -> Result<u64, Error> {
        let cookie = self.next_cookie;
        cookie.checked_add(1).ok_or(Error::IdentifierExhausted)?;
        if self.entries.len() == self.entries.capacity() {
            let capacity = self
                .entries
                .len()
                .checked_add(1)
                .and_then(usize::checked_next_power_of_two)
                .ok_or(Error::Allocation)?;
            let bytes = capacity
                .checked_mul(core::mem::size_of::<Entry>())
                .ok_or(Error::Allocation)?;
            let charge = budget.reserve(bytes).map_err(Error::Resource)?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(capacity)
                .map_err(|_| Error::Allocation)?;
            entries.append(&mut self.entries);
            self.entries = entries;
            self._charge = Some(charge);
        }
        Ok(cookie)
    }

    fn insert(&mut self, name: Name<'_>, node: FallibleArc<Node>, cookie: u64) {
        let mut bytes = [0; MAX_NAME_BYTES];
        bytes[..name.as_bytes().len()].copy_from_slice(name.as_bytes());
        self.entries.push(Entry {
            name: bytes,
            length: name.as_bytes().len(),
            cookie,
            node,
        });
        self.next_cookie = cookie + 1;
    }

    fn index(&self, name: Name<'_>) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| &entry.name[..entry.length] == name.as_bytes())
    }
}

impl Node {
    pub(super) const fn id(&self) -> u64 {
        self.id
    }

    pub(super) fn attributes(&self) -> Result<NodeAttributes, Error> {
        let size = self.data.lock().map_err(Error::Lock)?.bytes().len() as u64;
        Ok(NodeAttributes::new(
            self.attributes.kind(),
            self.attributes.mode(),
            size,
        ))
    }

    fn require(&self, kind: NodeKind) -> Result<(), Error> {
        if self.attributes.kind() == kind {
            Ok(())
        } else {
            Err(match kind {
                NodeKind::Directory => Error::NotDirectory,
                NodeKind::Symlink => Error::NotSymlink,
                _ => Error::NotRegularFile,
            })
        }
    }
}

impl Ramfs {
    pub(super) fn from_archive(archive: RamFs<'static>) -> Result<Self, Error> {
        let budget = Budget(
            ResourceDomain::try_new_root(
                ResourceLimits::UNLIMITED.with(ResourceKind::KernelMemoryBytes, STORAGE_LIMIT),
            )
            .map_err(Error::Resource)?,
        );
        // Build leaves before parents without recursion or locking before the
        // scheduler exists. Archive node identifiers are canonical path order.
        let count = archive.nodes().len();
        let mut pending: Vec<Children> = Vec::new();
        pending
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        pending.resize_with(count, Children::new);
        let mut root = None;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        sources.extend(archive.nodes());
        for source in sources.into_iter().rev() {
            let mut children =
                core::mem::replace(&mut pending[source.id().get() as usize], Children::new());
            children.entries.reverse();
            for (index, entry) in children.entries.iter_mut().enumerate() {
                entry.cookie = index as u64 + 1;
            }
            let charge = budget
                .reserve(FallibleArc::<Node>::allocation_size())
                .map_err(Error::Resource)?;
            let node = FallibleArc::try_new(Node {
                id: source.id().get(),
                linked: AtomicBool::new(true),
                attributes: source.attributes(),
                data: Mutex::new(FileData::borrowed(source.data())),
                children: Mutex::new(children),
                _charge: charge,
            })?;
            if source.id().get() == 0 {
                root = Some(node);
            } else {
                let parent = &mut pending[source.parent().get() as usize];
                let cookie = parent.prepare_insert(&budget)?;
                parent.insert(
                    Name::new(source.name()).map_err(|_| Error::InvalidBackendResult)?,
                    node,
                    cookie,
                );
            }
        }
        Ok(Self {
            root: root.ok_or(Error::InvalidBackendResult)?,
            next_id: AtomicU64::new(count as u64),
            budget,
            mutation: Mutex::new(()),
        })
    }

    pub(super) fn root(&self) -> FallibleArc<Node> {
        self.root.clone()
    }

    pub(super) fn lookup(
        &self,
        directory: &Node,
        name: Name<'_>,
    ) -> Result<Option<FallibleArc<Node>>, Error> {
        directory.require(NodeKind::Directory)?;
        let children = directory.children.lock().map_err(Error::Lock)?;
        Ok(children
            .index(name)
            .map(|index| children.entries[index].node.clone()))
    }

    pub(super) fn entry(
        &self,
        directory: &Node,
        cookie: u64,
    ) -> Result<Option<EntrySnapshot>, Error> {
        directory.require(NodeKind::Directory)?;
        let children = directory.children.lock().map_err(Error::Lock)?;
        if cookie >= children.next_cookie {
            return Err(Error::InvalidDirectoryCookie);
        }
        let Some(entry) = children
            .entries
            .iter()
            .filter(|entry| entry.cookie > cookie)
            .min_by_key(|entry| entry.cookie)
        else {
            return Ok(None);
        };
        Ok(Some(EntrySnapshot {
            name: entry.name,
            length: entry.length,
            attributes: entry.node.attributes()?,
            next_cookie: entry.cookie,
        }))
    }

    pub(super) fn read(
        &self,
        node: &Node,
        offset: u64,
        output: &mut [u8],
        link: bool,
    ) -> Result<usize, Error> {
        node.require(if link {
            NodeKind::Symlink
        } else {
            NodeKind::File
        })?;
        Ok(node.data.lock().map_err(Error::Lock)?.read(offset, output))
    }

    pub(super) fn write(
        &self,
        node: &Node,
        offset: Option<u64>,
        input: &[u8],
    ) -> Result<(usize, u64), Error> {
        node.require(NodeKind::File)?;
        let mut data = node.data.lock().map_err(Error::Lock)?;
        let offset = offset.unwrap_or(data.bytes().len() as u64);
        let actual = data
            .write(offset, input, &self.budget)
            .map_err(map_data_error)?;
        Ok((actual, offset + actual as u64))
    }

    pub(super) fn resize(&self, node: &Node, length: u64) -> Result<(), Error> {
        node.require(NodeKind::File)?;
        node.data
            .lock()
            .map_err(Error::Lock)?
            .resize(length, &self.budget)
            .map_err(map_data_error)
    }

    pub(super) fn executable(
        &self,
        node: &Node,
        sponsor: &ResourceDomain,
    ) -> Result<Option<ExecutableSnapshot>, Error> {
        if node.attributes.kind() != NodeKind::File || !node.attributes.is_executable() {
            return Ok(None);
        }
        let data = node.data.lock().map_err(Error::Lock)?;
        if let Some(archive) = data.archive() {
            return Ok(Some(ExecutableSnapshot::borrowed(archive)));
        }
        let charge = sponsor
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, data.bytes().len() as u64),
            )
            .map_err(Error::Resource)?
            .commit();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(data.bytes().len())
            .map_err(|_| Error::Allocation)?;
        bytes.extend_from_slice(data.bytes());
        Ok(Some(ExecutableSnapshot::owned(bytes, charge)))
    }

    pub(super) fn create<R, E: From<Error>>(
        &self,
        directory: &Node,
        name: Name<'_>,
        kind: NodeKind,
        mode: u32,
        publish: impl FnOnce(FallibleArc<Node>) -> Result<R, E>,
    ) -> Result<R, E> {
        directory.require(NodeKind::Directory)?;
        let _mutation = self.mutation.lock().map_err(Error::Lock)?;
        let mut children = directory.children.lock().map_err(Error::Lock)?;
        if !directory.linked.load(Ordering::Relaxed) {
            return Err(Error::Missing.into());
        }
        if children.index(name).is_some() {
            return Err(Error::AlreadyExists.into());
        }
        let id = self
            .next_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::IdentifierExhausted)?;
        let charge = self
            .budget
            .reserve(FallibleArc::<Node>::allocation_size())
            .map_err(Error::Resource)?;
        let node = FallibleArc::try_new(Node {
            id,
            linked: AtomicBool::new(true),
            attributes: NodeAttributes::new(kind, mode, 0),
            data: Mutex::new(FileData::borrowed(&[])),
            children: Mutex::new(Children::new()),
            _charge: charge,
        })
        .map_err(Error::from)?;
        let cookie = children.prepare_insert(&self.budget)?;
        // Handle publication is the last fallible step. The directory lock
        // excludes lookups until its already prepared name is committed.
        let result = publish(node.clone())?;
        children.insert(name, node, cookie);
        Ok(result)
    }

    pub(super) fn remove(
        &self,
        directory: &Node,
        name: Name<'_>,
        kind: NodeKind,
    ) -> Result<(), Error> {
        directory.require(NodeKind::Directory)?;
        let _mutation = self.mutation.lock().map_err(Error::Lock)?;
        let mut children = directory.children.lock().map_err(Error::Lock)?;
        let index = children.index(name).ok_or(Error::Missing)?;
        let node = &children.entries[index].node;
        if kind == NodeKind::Directory {
            node.require(NodeKind::Directory)?;
            if !node
                .children
                .lock()
                .map_err(Error::Lock)?
                .entries
                .is_empty()
            {
                return Err(Error::NotEmpty);
            }
        } else if node.attributes.kind() == NodeKind::Directory {
            return Err(Error::NotRegularFile);
        }
        node.linked.store(false, Ordering::Relaxed);
        let removed = children.entries.remove(index);
        drop(children);
        drop(_mutation);
        drop(removed);
        Ok(())
    }
}

fn map_data_error(error: hyper::fs::file_data::Error<ResourceError>) -> Error {
    match error {
        hyper::fs::file_data::Error::Allocation => Error::Allocation,
        hyper::fs::file_data::Error::Size => Error::InvalidSize,
        hyper::fs::file_data::Error::Budget(error) => Error::Resource(error),
    }
}
