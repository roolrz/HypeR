// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit authority for publishing immutable native executable memory.

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{KernelObject, ObjectKind, object_allocation_size, private};

/// Failure while preparing an accounted executable-authority object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutableAuthorityError {
    AllocationSize,
    Resource(ResourceError),
}

impl From<ResourceError> for ExecutableAuthorityError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

/// Capability required to derive executable provenance from writable bytes.
///
/// The payload is intentionally stateless. Possession of the object reference
/// proves only identity; the `CREATE_EXECUTABLE` right on the resolving handle
/// grants the operation. Writable VMO authority is a distinct required input.
pub(crate) struct ExecutableAuthority {
    _object_charge: CommittedCharge,
}

impl ExecutableAuthority {
    pub(crate) fn try_new(sponsor: &ResourceDomain) -> Result<Self, ExecutableAuthorityError> {
        let bytes = object_allocation_size::<Self>()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(ExecutableAuthorityError::AllocationSize)?;
        let charge = sponsor
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, bytes),
            )?
            .commit();
        Ok(Self {
            _object_charge: charge,
        })
    }

    /// Creates the private proof consumed by the memory mechanism.
    ///
    /// Callers may reach this method only after resolving an authority handle
    /// with `CREATE_EXECUTABLE`; the proof itself never crosses the subsystem.
    pub(super) const fn provenance(&self) -> super::ExecutableProvenance {
        super::ExecutableProvenance::for_capability()
    }
}

impl private::Sealed for ExecutableAuthority {}
impl private::UserExportable for ExecutableAuthority {}

impl KernelObject for ExecutableAuthority {
    const KIND: ObjectKind = ObjectKind::EXECUTABLE_AUTHORITY;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::CREATE_EXECUTABLE);
}
