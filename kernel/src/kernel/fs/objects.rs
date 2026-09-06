// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped access to the immutable boot filesystem.
//!
//! The mounted `RamFs` remains the authoritative namespace. These objects add
//! process-local authority and accounting without copying archive payloads or
//! exposing the global namespace through an untyped identifier.

use hyper::fs::ramfs::{Node, NodeKind};

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectCreationError, ObjectKind, TransferClass, object_allocation_size, private,
};

/// Failure while creating or resolving boot-filesystem capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BootFsError {
    AllocationSize,
    InvalidPath,
    Missing,
    NotExecutable,
    NotRegularFile,
    Object(ObjectCreationError),
    Resource(ResourceError),
}

impl From<ObjectCreationError> for BootFsError {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<ResourceError> for BootFsError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

/// Read-only authority to resolve names in the mounted initramfs.
pub(crate) struct BootFs {
    _object_charge: CommittedCharge,
}

impl BootFs {
    #[cfg_attr(
        feature = "kernel-self-test",
        expect(
            dead_code,
            reason = "Native init is replaced by the bare-metal self-test"
        )
    )]
    pub(crate) fn try_new(sponsor: &ResourceDomain) -> Result<Self, BootFsError> {
        Ok(Self {
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn open(
        &self,
        path: &str,
        sponsor: &ResourceDomain,
    ) -> Result<BootFile, BootFsError> {
        let node = super::lookup(path).map_err(|_| BootFsError::InvalidPath)?;
        let node = node.ok_or(BootFsError::Missing)?;
        BootFile::try_new(node, sponsor)
    }
}

impl private::Sealed for BootFs {}
impl private::UserExportable for BootFs {}

impl KernelObject for BootFs {
    const KIND: ObjectKind = ObjectKind::BOOT_FS;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ);
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;
}

/// Immutable authority to one regular initramfs file.
pub(crate) struct BootFile {
    node: Node<'static>,
    _object_charge: CommittedCharge,
}

impl BootFile {
    fn try_new(node: Node<'static>, sponsor: &ResourceDomain) -> Result<Self, BootFsError> {
        if node.kind() != NodeKind::File {
            return Err(BootFsError::NotRegularFile);
        }
        Ok(Self {
            node,
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn len(&self) -> u64 {
        match u64::try_from(self.node.data().len()) {
            Ok(length) => length,
            Err(_) => crate::kernel::crash::fatal(format_args!(
                "HypeR: boot file length exceeds the Native ABI range"
            )),
        }
    }

    /// Copies one bounded range and returns the number of bytes copied.
    pub(crate) fn read(&self, offset: u64, destination: &mut [u8]) -> usize {
        let Ok(offset) = usize::try_from(offset) else {
            return 0;
        };
        let Some(source) = self.node.data().get(offset..) else {
            return 0;
        };
        let count = source.len().min(destination.len());
        let Some(source) = source.get(..count) else {
            return 0;
        };
        let Some(destination) = destination.get_mut(..count) else {
            return 0;
        };
        destination.copy_from_slice(source);
        count
    }

    pub(crate) fn executable_bytes(&self) -> Result<&'static [u8], BootFsError> {
        if !self.node.is_executable() {
            return Err(BootFsError::NotExecutable);
        }
        Ok(self.node.data())
    }
}

impl private::Sealed for BootFile {}
impl private::UserExportable for BootFile {}

impl KernelObject for BootFile {
    const KIND: ObjectKind = ObjectKind::BOOT_FILE;
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
        if self.node.is_executable() {
            common.union(Rights::EXECUTE)
        } else {
            common
        }
    }
}

fn reserve_object_charge<T: KernelObject>(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, BootFsError> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(BootFsError::AllocationSize)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )?
        .commit())
}
