// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(
    feature = "kernel-self-test",
    allow(
        dead_code,
        reason = "Native VFS consumers are replaced by the bare-metal self-test"
    )
)]

//! Kernel-owned VFS namespace, capability objects, and file-data policy.

mod executable;
mod instance;
mod objects;
mod ramfs;
mod read;
mod read_contract;
mod resolve;
mod resolve_state;
mod rights_contract;
mod service;

pub(crate) use executable::ExecutableSnapshot;
pub(crate) use objects::{
    DirectoryEntrySnapshot, DirectoryInfo, DirectoryObject, DirectoryPage, Error,
    Error as VfsError, FileInfo, FileObject,
};
pub(crate) use service::{
    ServiceError as VfsServiceError, create_directory, create_file, directory_info, file_info,
    open_directory, open_file, read_directory, read_file_at, remove_entry, resize_file,
    write_file_at,
};

use hyper::fs::ramfs::RamFs;
use hyper::fs::{NodeAttributes, NodeKind};
use hyper::mm::{AllocationError, FallibleArc};
use hyper::sync::PublishedOnce;

use instance::{FilesystemInstance, MountNamespace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Allocation,
    AlreadyInitialized,
    Cache(crate::kernel::io_cache::CacheError),
    FileSystem(instance::Error),
    RamFs(hyper::fs::ramfs::Error),
}

impl From<AllocationError> for InitializationError {
    fn from(_: AllocationError) -> Self {
        Self::Allocation
    }
}

impl From<instance::Error> for InitializationError {
    fn from(error: instance::Error) -> Self {
        Self::FileSystem(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LookupError {
    Backend(instance::Error),
    InvalidPath,
    NotInitialized,
}

/// Backend-neutral immutable executable view used before the first Process exists.
pub(crate) struct BootstrapFile {
    attributes: NodeAttributes,
    executable: Option<ExecutableSnapshot>,
}

impl BootstrapFile {
    pub(crate) const fn kind(&self) -> NodeKind {
        self.attributes.kind()
    }

    pub(crate) const fn is_executable(&self) -> bool {
        self.attributes.is_executable()
    }

    pub(crate) fn into_executable(self) -> Option<ExecutableSnapshot> {
        self.executable
    }
}

static SYSTEM_NAMESPACE: PublishedOnce<FallibleArc<MountNamespace>> = PublishedOnce::new();

pub(crate) fn initialize(boot: &super::boot::Initialization) -> Result<(), InitializationError> {
    let ramfs = RamFs::from_newc(boot.initial_ramdisk()).map_err(InitializationError::RamFs)?;
    let node_count = ramfs.nodes().len();
    let filesystem = FilesystemInstance::try_from_ramfs(ramfs)?;
    // The system cache is a VFS-service resource, not namespace-local state.
    // Injecting its owner prevents future namespace creation from silently
    // multiplying the global clean-page budget.
    let cache = crate::kernel::io_cache::FileDataCache::try_new_system()
        .map_err(InitializationError::Cache)?;
    let cache = FallibleArc::try_new(cache)?;
    let namespace = MountNamespace::try_new(filesystem, cache)?;
    SYSTEM_NAMESPACE
        .publish(namespace)
        .map_err(|_| InitializationError::AlreadyInitialized)?;
    crate::pr_info!("HypeR: mounted initramfs VFS with {node_count} node(s)");
    Ok(())
}

pub(crate) fn root_directory(
    sponsor: &super::accounting::ResourceDomain,
) -> Result<DirectoryObject, Error> {
    let namespace = SYSTEM_NAMESPACE.get().ok_or(Error::Missing)?;
    DirectoryObject::try_root(namespace.clone(), sponsor)
}

/// Transitional trusted lookup used only to obtain the first executable.
///
/// The initial Process does not exist yet and therefore cannot carry a
/// Directory handle. Every subsequent userspace lookup is capability-relative.
pub(crate) fn lookup(
    path: &str,
    sponsor: &super::accounting::ResourceDomain,
) -> Result<Option<BootstrapFile>, LookupError> {
    let namespace = SYSTEM_NAMESPACE.get().ok_or(LookupError::NotInitialized)?;
    let root = namespace.root();
    let location = match resolve::file(namespace, &root, path) {
        Ok(location) => location,
        Err(Error::Missing) => return Ok(None),
        Err(Error::Backend(error)) => return Err(LookupError::Backend(error)),
        Err(_) => return Err(LookupError::InvalidPath),
    };
    let filesystem = location.mount().filesystem();
    let attributes = filesystem
        .attributes(location.node())
        .map_err(LookupError::Backend)?;
    Ok(Some(BootstrapFile {
        attributes,
        executable: filesystem
            .executable_snapshot(location.node(), sponsor)
            .map_err(LookupError::Backend)?,
    }))
}
