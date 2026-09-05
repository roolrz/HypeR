// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Handle-visible native virtual-memory capabilities.

use hyper::mm::FallibleArc;

use super::{
    DomainAccount, ExecutableAuthority, ExecutableVmo, KernelPageBackend, KernelPageError,
    NativeAddressSpace, UserSlice, Vmar, VmoError, WritableVmo,
};
use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectCreationError, ObjectKind, ObjectPublication, object_allocation_size,
    private,
};

type NativeWritableVmo = WritableVmo<KernelPageBackend, DomainAccount>;
type NativeExecutableVmo = ExecutableVmo<KernelPageBackend, DomainAccount>;

/// Failure while preparing an accounted virtual-memory capability object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MemoryObjectError {
    AlreadyPublished,
    AllocationSize,
    Object(ObjectCreationError),
    Resource(ResourceError),
    Vmo(VmoError<KernelPageError, ResourceError>),
    WrongVariant,
}

impl From<ObjectCreationError> for MemoryObjectError {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<ResourceError> for MemoryObjectError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<VmoError<KernelPageError, ResourceError>> for MemoryObjectError {
    fn from(error: VmoError<KernelPageError, ResourceError>) -> Self {
        Self::Vmo(error)
    }
}

enum VmoStorage {
    Writable(NativeWritableVmo),
    Executable(NativeExecutableVmo),
}

/// One handle-visible VMO whose immutable variant fixes its authority ceiling.
///
/// Writable and executable backing share an ABI kind so a mapping operation
/// can accept either. Their per-instance rights differ: no executable snapshot
/// can acquire `WRITE`, and writable storage cannot acquire
/// `EXECUTE` merely because both variants use this Rust payload type.
pub(crate) struct VmoObject {
    storage: VmoStorage,
    _object_charge: CommittedCharge,
}

impl VmoObject {
    pub(crate) fn try_new_writable(
        size: u64,
        sponsor: &ResourceDomain,
    ) -> Result<Self, MemoryObjectError> {
        let storage =
            WritableVmo::try_new(size, KernelPageBackend, DomainAccount::new(sponsor.clone()))?;
        Self::from_writable(storage, sponsor)
    }

    fn from_writable(
        storage: NativeWritableVmo,
        sponsor: &ResourceDomain,
    ) -> Result<Self, MemoryObjectError> {
        Ok(Self {
            storage: VmoStorage::Writable(storage),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    fn from_executable(
        storage: NativeExecutableVmo,
        sponsor: &ResourceDomain,
    ) -> Result<Self, MemoryObjectError> {
        Ok(Self {
            storage: VmoStorage::Executable(storage),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn writable(&self) -> Option<&NativeWritableVmo> {
        match &self.storage {
            VmoStorage::Writable(storage) => Some(storage),
            VmoStorage::Executable(_) => None,
        }
    }

    pub(crate) fn executable(&self) -> Option<&NativeExecutableVmo> {
        match &self.storage {
            VmoStorage::Writable(_) => None,
            VmoStorage::Executable(storage) => Some(storage),
        }
    }

    pub(crate) fn size(&self) -> u64 {
        match &self.storage {
            VmoStorage::Writable(storage) => storage.size(),
            VmoStorage::Executable(storage) => storage.size(),
        }
    }

    pub(crate) fn read(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), MemoryObjectError> {
        match &self.storage {
            VmoStorage::Writable(storage) => storage.read(offset, destination)?,
            VmoStorage::Executable(storage) => storage.read(offset, destination)?,
        }
        Ok(())
    }

    pub(crate) fn write(&self, offset: u64, source: &[u8]) -> Result<(), MemoryObjectError> {
        let storage = self.writable().ok_or(MemoryObjectError::WrongVariant)?;
        storage.write(offset, source)?;
        Ok(())
    }

    /// Derives a physically distinct immutable executable snapshot.
    ///
    /// The caller must have independently resolved both the writable VMO with
    /// `READ` and the authority object with `CREATE_EXECUTABLE`. The pinned
    /// execution proof supplies the architecture cache-publication context.
    pub(crate) fn try_executable_snapshot<P: hyper::cpu::PinnedExecution + 'static>(
        &self,
        authority: &ExecutableAuthority,
        pin: &P,
        sponsor: &ResourceDomain,
    ) -> Result<Self, MemoryObjectError> {
        let writable = self.writable().ok_or(MemoryObjectError::WrongVariant)?;
        let executable = writable.try_executable_snapshot(&authority.provenance(), pin)?;
        Self::from_executable(executable, sponsor)
    }
}

impl private::Sealed for VmoObject {}
impl private::UserExportable for VmoObject {}

impl KernelObject for VmoObject {
    const KIND: ObjectKind = ObjectKind::VMO;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::MAP)
        .union(Rights::EXECUTE);

    fn supported_rights(&self) -> Rights {
        let common = Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::READ)
            .union(Rights::MAP);
        match &self.storage {
            VmoStorage::Writable(_) => common.union(Rights::WRITE),
            VmoStorage::Executable(_) => common.union(Rights::EXECUTE),
        }
    }
}

/// Handle-visible authority over one VMAR in a native address space.
///
/// The strong address-space owner keeps the token's generation namespace
/// alive. Mapping policy still validates the token on every operation, so a
/// destroyed child VMAR remains stale even while this object is referenced.
pub(crate) struct VmarObject {
    address_space: FallibleArc<NativeAddressSpace>,
    token: Vmar,
    _object_charge: CommittedCharge,
}

impl VmarObject {
    fn root(
        address_space: FallibleArc<NativeAddressSpace>,
        sponsor: &ResourceDomain,
    ) -> Result<Self, MemoryObjectError> {
        let token = address_space.logical().root_vmar();
        Ok(Self {
            address_space,
            token,
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    /// Constructs the single handle-visible identity for the root VMAR.
    pub(crate) fn try_root_publication(
        address_space: FallibleArc<NativeAddressSpace>,
        sponsor: &ResourceDomain,
    ) -> Result<ObjectPublication<Self>, MemoryObjectError> {
        if !address_space.claim_root_vmar_object_publication() {
            return Err(MemoryObjectError::AlreadyPublished);
        }
        let result = Self::root(address_space.clone(), sponsor)
            .and_then(|payload| ObjectPublication::try_new(payload).map_err(Into::into));
        if result.is_err() {
            address_space.abort_root_vmar_object_publication();
        }
        result
    }

    pub(crate) fn address_space(&self) -> &NativeAddressSpace {
        &self.address_space
    }

    pub(crate) const fn token(&self) -> Vmar {
        self.token
    }

    pub(crate) const fn range(&self) -> UserSlice {
        self.token.range()
    }
}

impl private::Sealed for VmarObject {}
impl private::UserExportable for VmarObject {}

impl KernelObject for VmarObject {
    const KIND: ObjectKind = ObjectKind::VMAR;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::MAP);
}

fn reserve_object_charge<T: KernelObject>(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, MemoryObjectError> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(MemoryObjectError::AllocationSize)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )?
        .commit())
}
