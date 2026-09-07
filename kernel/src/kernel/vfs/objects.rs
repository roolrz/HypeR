// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability objects for namespace traversal and immutable file access.

use hyper::fs::{NodeAttributes, NodeKind};
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
    InvalidPath,
    Missing,
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
        Self::Backend(error)
    }
}

/// Authority to resolve paths below one namespace location.
pub(crate) struct DirectoryObject {
    namespace: FallibleArc<MountNamespace>,
    root: Location,
    _object_charge: CommittedCharge,
}

impl DirectoryObject {
    pub(crate) fn try_root(
        namespace: FallibleArc<MountNamespace>,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let root = namespace.root();
        Ok(Self {
            namespace,
            root,
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
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

    fn try_new(
        namespace: FallibleArc<MountNamespace>,
        root: Location,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let attributes = root
            .mount()
            .filesystem()
            .attributes(root.node())
            .map_err(Error::Backend)?;
        if attributes.kind() != NodeKind::Directory {
            return Err(Error::NotDirectory);
        }
        Ok(Self {
            namespace,
            root,
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
            .map_err(Error::Backend)?;
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

    pub(crate) fn len(&self) -> u64 {
        self.attributes.size()
    }

    pub(crate) fn read(&self, offset: u64, destination: &mut [u8]) -> Result<usize, Error> {
        super::read::cached(self, offset, destination)
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
            .map_err(Error::Backend)
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
            .map_err(Error::Backend)?
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
        .union(Rights::EXECUTE);
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;

    fn supported_rights(&self) -> Rights {
        let common = Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::READ);
        if self.attributes.is_executable() {
            common.union(Rights::EXECUTE)
        } else {
            common
        }
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
