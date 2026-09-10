// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability objects for namespace traversal and file access.

use hyper::fs::{MAX_NAME_BYTES, Name, NodeAttributes, NodeKind};
use hyper::mm::FallibleArc;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectCreationError, ObjectKind, TransferClass, object_allocation_size, private,
};

use super::instance::{Location, MountNamespace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    AllocationSize,
    Backend(super::instance::Error),
    Cache(crate::kernel::io_cache::CacheError),
    InvalidDirectoryCookie,
    InvalidPath,
    Missing,
    AlreadyExists,
    NotEmpty,
    InvalidSize,
    NotDirectory,
    NotExecutable,
    NotRegularFile,
    Object(ObjectCreationError),
    Resource(ResourceError),
}

impl From<ObjectCreationError> for Error {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<super::instance::Error> for Error {
    fn from(error: super::instance::Error) -> Self {
        match error {
            super::instance::Error::InvalidDirectoryCookie => Self::InvalidDirectoryCookie,
            super::instance::Error::AlreadyExists => Self::AlreadyExists,
            super::instance::Error::Missing => Self::Missing,
            super::instance::Error::NotEmpty => Self::NotEmpty,
            super::instance::Error::InvalidSize => Self::InvalidSize,
            super::instance::Error::Resource(error) => Self::Resource(error),
            super::instance::Error::Allocation => Self::Allocation,
            other => Self::Backend(other),
        }
    }
}

pub(crate) const DIRECTORY_PAGE_CAPACITY: usize =
    hyper::abi::native::HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY as usize;
const _: () = assert!(
    MAX_NAME_BYTES == hyper::abi::native::HYPER_NATIVE_DIRECTORY_ENTRY_NAME_MAX_BYTES as usize
);

#[derive(Clone, Copy)]
pub(crate) struct DirectoryEntrySnapshot {
    pub(crate) name: [u8; MAX_NAME_BYTES],
    pub(crate) name_length: u32,
    pub(crate) attributes: NodeAttributes,
}

pub(crate) struct DirectoryPage {
    entries: [Option<DirectoryEntrySnapshot>; DIRECTORY_PAGE_CAPACITY],
    len: usize,
    next_cookie: u64,
}

impl DirectoryPage {
    pub(crate) fn entries(&self) -> impl Iterator<Item = &DirectoryEntrySnapshot> {
        self.entries[..self.len].iter().filter_map(Option::as_ref)
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) const fn next_cookie(&self) -> u64 {
        self.next_cookie
    }
}

/// Authority to resolve paths below one namespace location.
pub(crate) struct DirectoryObject {
    namespace: FallibleArc<MountNamespace>,
    root: Location,
    attributes: NodeAttributes,
    _object_charge: CommittedCharge,
}

/// Observation-only identity for one node at one namespace location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NodeLocationInfo {
    pub(crate) filesystem_id: u64,
    pub(crate) mount_id: u64,
    pub(crate) node_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileInfo {
    pub(crate) location: NodeLocationInfo,
    pub(crate) size: u64,
    pub(crate) mode: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryInfo {
    pub(crate) location: NodeLocationInfo,
    pub(crate) mode: u32,
}

impl DirectoryObject {
    pub(crate) fn try_root(
        namespace: FallibleArc<MountNamespace>,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let root = namespace.root();
        Self::try_new(namespace, root, sponsor)
    }

    pub(crate) fn open_file(
        &self,
        path: &str,
        sponsor: &ResourceDomain,
    ) -> Result<FileObject, Error> {
        let location = super::resolve::file(&self.namespace, &self.root, path)?;
        FileObject::try_new(location, self.namespace.cache(), sponsor)
    }

    pub(crate) fn open_directory(
        &self,
        path: &str,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let location = super::resolve::directory(&self.namespace, &self.root, path)?;
        Self::try_new(self.namespace.clone(), location, sponsor)
    }

    pub(crate) fn create_file<R, E: From<Error> + From<super::instance::Error>>(
        &self,
        path: &str,
        mode: u32,
        sponsor: &ResourceDomain,
        publish: impl FnOnce(FileObject) -> Result<R, E>,
    ) -> Result<R, E> {
        let (parent, name) = self.parent(path)?;
        parent
            .mount()
            .filesystem()
            .create_file(parent.node(), name, mode, |node| {
                let location = Location::new(parent.mount().clone(), node);
                publish(FileObject::try_new(
                    location,
                    self.namespace.cache(),
                    sponsor,
                )?)
            })
    }

    pub(crate) fn create(&self, path: &str, kind: NodeKind, mode: u32) -> Result<(), Error> {
        let (parent, name) = self.parent(path)?;
        parent
            .mount()
            .filesystem()
            .create(parent.node(), name, kind, mode)
            .map_err(Into::into)
    }

    pub(crate) fn remove(&self, path: &str, kind: NodeKind) -> Result<(), Error> {
        let (parent, name) = self.parent(path)?;
        parent
            .mount()
            .filesystem()
            .remove(parent.node(), name, kind)
            .map_err(Into::into)
    }

    fn parent<'path>(&self, path: &'path str) -> Result<(Location, Name<'path>), Error> {
        // The final component is a name, never a followed symlink or dot entry.
        let (parent, name) = path.rsplit_once('/').unwrap_or((".", path));
        let name = Name::new(name).map_err(|_| Error::InvalidPath)?;
        let parent = if parent.is_empty() { "/" } else { parent };
        Ok((
            super::resolve::directory(&self.namespace, &self.root, parent)?,
            name,
        ))
    }

    pub(crate) fn read_page(&self, cookie: u64) -> Result<DirectoryPage, Error> {
        let mut page = DirectoryPage {
            entries: [None; DIRECTORY_PAGE_CAPACITY],
            len: 0,
            next_cookie: 0,
        };
        let mut cursor = cookie;
        while page.len < DIRECTORY_PAGE_CAPACITY {
            let Some(entry) = self.namespace.read_directory_entry(&self.root, cursor)? else {
                return Ok(page);
            };
            let entry_name = core::str::from_utf8(&entry.name[..entry.length])
                .map_err(|_| Error::Backend(super::instance::Error::InvalidBackendResult))?;
            if Name::new(entry_name).is_err()
                || entry.next_cookie == 0
                || entry.next_cookie == cursor
            {
                return Err(Error::Backend(super::instance::Error::InvalidBackendResult));
            }
            let mut name = [0_u8; MAX_NAME_BYTES];
            let Some(destination) = name.get_mut(..entry_name.len()) else {
                return Err(Error::Backend(super::instance::Error::InvalidBackendResult));
            };
            destination.copy_from_slice(entry_name.as_bytes());
            page.entries[page.len] = Some(DirectoryEntrySnapshot {
                name,
                name_length: u32::try_from(entry_name.len())
                    .map_err(|_| Error::Backend(super::instance::Error::InvalidBackendResult))?,
                attributes: entry.attributes,
            });
            page.len += 1;
            cursor = entry.next_cookie;
        }

        if self
            .namespace
            .read_directory_entry(&self.root, cursor)?
            .is_some()
        {
            page.next_cookie = cursor;
        }
        Ok(page)
    }

    pub(crate) fn info(&self) -> DirectoryInfo {
        DirectoryInfo {
            location: location_info(&self.root),
            mode: self.attributes.mode(),
        }
    }

    fn try_new(
        namespace: FallibleArc<MountNamespace>,
        root: Location,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let attributes = root
            .mount()
            .filesystem()
            .attributes(root.node())
            .map_err(Error::from)?;
        if attributes.kind() != NodeKind::Directory {
            return Err(Error::NotDirectory);
        }
        Ok(Self {
            namespace,
            root,
            attributes,
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }
}

impl private::Sealed for DirectoryObject {}
impl private::UserExportable for DirectoryObject {}

impl KernelObject for DirectoryObject {
    const KIND: ObjectKind = ObjectKind::DIRECTORY;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::EXECUTE);
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;
}

/// Immutable authority to one opened regular file.
pub(crate) struct FileObject {
    location: Location,
    attributes: NodeAttributes,
    cache: FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>>,
    _object_charge: CommittedCharge,
}

impl FileObject {
    fn try_new(
        location: Location,
        cache: FallibleArc<crate::kernel::io_cache::FileDataCache<super::read::FilePage>>,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let attributes = location
            .mount()
            .filesystem()
            .attributes(location.node())
            .map_err(Error::from)?;
        if attributes.kind() != NodeKind::File {
            return Err(Error::NotRegularFile);
        }
        Ok(Self {
            location,
            attributes,
            cache,
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn len(&self) -> Result<u64, Error> {
        Ok(self
            .location
            .mount()
            .filesystem()
            .attributes(self.location.node())?
            .size())
    }

    pub(crate) fn info(&self) -> Result<FileInfo, Error> {
        Ok(FileInfo {
            location: location_info(&self.location),
            size: self.len()?,
            mode: self.attributes.mode(),
        })
    }

    pub(crate) fn write(&self, offset: Option<u64>, input: &[u8]) -> Result<(usize, u64), Error> {
        self.location
            .mount()
            .filesystem()
            .write_at(self.location.node(), offset, input)
            .map_err(Into::into)
    }

    pub(crate) fn resize(&self, length: u64) -> Result<(), Error> {
        self.location
            .mount()
            .filesystem()
            .resize(self.location.node(), length)
            .map_err(Into::into)
    }

    pub(crate) fn read(&self, offset: u64, destination: &mut [u8]) -> Result<usize, Error> {
        match self.location.mount().filesystem().read_cache_policy() {
            super::instance::ReadCachePolicy::Direct => self.read_uncached(offset, destination),
            super::instance::ReadCachePolicy::PageCache => {
                super::read::cached(self, offset, destination)
            }
        }
    }

    pub(super) fn read_uncached(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        self.location
            .mount()
            .filesystem()
            .read_at(self.location.node(), offset, destination)
            .map_err(Error::from)
    }

    pub(super) fn location(&self) -> &Location {
        &self.location
    }

    pub(super) fn cache(&self) -> &crate::kernel::io_cache::FileDataCache<super::read::FilePage> {
        &self.cache
    }

    pub(crate) fn executable_snapshot(
        &self,
        sponsor: &ResourceDomain,
    ) -> Result<super::ExecutableSnapshot, Error> {
        self.location
            .mount()
            .filesystem()
            .executable_snapshot(self.location.node(), sponsor)
            .map_err(Error::from)?
            .ok_or(Error::NotExecutable)
    }
}

impl private::Sealed for FileObject {}
impl private::UserExportable for FileObject {}

impl KernelObject for FileObject {
    const KIND: ObjectKind = ObjectKind::FILE;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::EXECUTE);
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;

    fn supported_rights(&self) -> Rights {
        let common = Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::READ)
            .union(Rights::WRITE);
        if self.attributes.is_executable() {
            common.union(Rights::EXECUTE)
        } else {
            common
        }
    }
}

fn location_info(location: &Location) -> NodeLocationInfo {
    NodeLocationInfo {
        filesystem_id: location.mount().filesystem().id().get(),
        mount_id: location.mount().id().get(),
        node_id: location.node().get(),
    }
}

fn reserve_object_charge<T: KernelObject>(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, Error> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(Error::AllocationSize)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )?
        .commit())
}
