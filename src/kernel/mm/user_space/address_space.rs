// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Concurrent VMAR authority and transactional native address-space mappings.

use alloc::vec::Vec;
use core::mem::{ManuallyDrop, size_of};
use core::sync::atomic::{AtomicU64, Ordering};

use hyper::mm::{FallibleArc, ForeignMemory, PAGE_SIZE, PhysicalAddress};
use hyper::sync::InterruptSpinLock;

use super::contract::{
    Access, MemoryAccount, MemoryCharge, PageBackend, Permissions, UserAddress, UserAddressWindow,
    UserSlice,
};
use super::vmo::{
    ExecutableVmo, MappingObject, VmoError, WritableMappingLease, WritableVmo, resident_pages,
};

#[cfg(not(test))]
type AddressSpaceLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

#[cfg(test)]
struct TestInterruptMask;

#[cfg(test)]
impl hyper::hal::interrupt::InterruptMask for TestInterruptMask {
    type State = ();

    fn save_and_disable() -> Self::State {}
    fn restore(_: Self::State) {}
    fn wait_for_lock_owner() {
        std::thread::yield_now();
    }
}

#[cfg(test)]
type AddressSpaceLock<T> = InterruptSpinLock<T, TestInterruptMask>;

static NEXT_ADDRESS_SPACE_ID: AtomicU64 = AtomicU64::new(1);

type SpaceResult<Backend, Account, Value> = Result<
    Value,
    AddressSpaceError<<Backend as PageBackend>::Error, <Account as MemoryAccount>::Error>,
>;
type MappingOwner<Account> = FallibleArc<MappingOwnership<<Account as MemoryAccount>::Charge>>;
type VmarOwner<Account> = FallibleArc<VmarOwnership<<Account as MemoryAccount>::Charge>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub(crate) struct AddressSpaceId(u64);

impl AddressSpaceId {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Vmar {
    address_space: AddressSpaceId,
    id: u64,
    generation: u64,
    range: UserSlice,
}

impl Vmar {
    pub(crate) const fn range(self) -> UserSlice {
        self.range
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MappingToken {
    address_space: AddressSpaceId,
    id: u64,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MappingSnapshot {
    pub(crate) token: MappingToken,
    pub(crate) range: UserSlice,
    pub(crate) object_offset: u64,
    pub(crate) permissions: Permissions,
    pub(crate) executable_backing: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MappingChange {
    pub(crate) address_space: AddressSpaceId,
    pub(crate) previous_epoch: u64,
    pub(crate) epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AddressSpaceError<BackendError, AccountError> {
    Account(AccountError),
    Allocation,
    BackingNotResident,
    Backend(BackendError),
    Busy,
    EmptyRange,
    IdentityExhausted,
    InvalidAddressSpace,
    InvalidPermissions,
    InvalidRange,
    NotMapped,
    Overlap,
    ReadDenied,
    SizeMismatch,
    SizeOverflow,
    StaleMapping,
    StaleTransaction,
    StaleVmar,
    WriteDenied,
    WritableExecutableBacking,
}

impl<BackendError, AccountError> From<VmoError<BackendError, AccountError>>
    for AddressSpaceError<BackendError, AccountError>
{
    fn from(error: VmoError<BackendError, AccountError>) -> Self {
        match error {
            VmoError::Account(error) => Self::Account(error),
            VmoError::Allocation => Self::Allocation,
            VmoError::Backend(error) => Self::Backend(error),
            VmoError::Busy => Self::Busy,
            VmoError::InvalidRange => Self::InvalidRange,
            VmoError::SizeOverflow => Self::SizeOverflow,
        }
    }
}

struct MappingOwnership<Charge> {
    _charge: Charge,
}

struct Mapping<Backend: PageBackend, Account: MemoryAccount> {
    owner_vmar: u64,
    snapshot: MappingSnapshot,
    maximum_permissions: Permissions,
    object: MappingObject<Backend, Account>,
    ownership: FallibleArc<MappingOwnership<Account::Charge>>,
    write_lease: Option<WritableMappingLease<Backend, Account>>,
}

impl<Backend: PageBackend, Account: MemoryAccount> Clone for Mapping<Backend, Account> {
    fn clone(&self) -> Self {
        Self {
            owner_vmar: self.owner_vmar,
            snapshot: self.snapshot,
            maximum_permissions: self.maximum_permissions,
            object: self.object.clone(),
            ownership: self.ownership.clone(),
            write_lease: self.write_lease.clone(),
        }
    }
}

struct VmarOwnership<Charge> {
    _charge: Charge,
}

struct VmarRecord<Charge> {
    token: Vmar,
    parent_id: u64,
    ownership: FallibleArc<VmarOwnership<Charge>>,
}

impl<Charge> Clone for VmarRecord<Charge> {
    fn clone(&self) -> Self {
        Self {
            token: self.token,
            parent_id: self.parent_id,
            ownership: self.ownership.clone(),
        }
    }
}

struct MappingSet<Backend: PageBackend, Account: MemoryAccount> {
    records: Vec<Mapping<Backend, Account>>,
    storage_charge: Option<Account::Charge>,
}

struct VmarSet<Account: MemoryAccount> {
    records: Vec<VmarRecord<Account::Charge>>,
    _storage_charge: Option<Account::Charge>,
}

struct AddressSpaceState<Backend: PageBackend, Account: MemoryAccount> {
    mappings: FallibleArc<MappingSet<Backend, Account>>,
    vmars: FallibleArc<VmarSet<Account>>,
    next_mapping_id: u64,
    next_vmar_id: u64,
    authority_epoch: u64,
    mapping_epoch: u64,
}

struct AddressSpaceSnapshot<Backend: PageBackend, Account: MemoryAccount> {
    mappings: FallibleArc<MappingSet<Backend, Account>>,
    vmars: FallibleArc<VmarSet<Account>>,
    authority_epoch: u64,
    mapping_epoch: u64,
}

pub(crate) struct UserAddressSpace<Backend: PageBackend, Account: MemoryAccount> {
    id: AddressSpaceId,
    root: Vmar,
    backend: Backend,
    account: Account,
    state: AddressSpaceLock<AddressSpaceState<Backend, Account>>,
    _metadata_charge: Account::Charge,
}

impl<Backend: PageBackend, Account: MemoryAccount> UserAddressSpace<Backend, Account> {
    pub(crate) fn try_new(
        window: UserAddressWindow,
        range: UserSlice,
        backend: Backend,
        account: Account,
    ) -> Result<Self, AddressSpaceError<Backend::Error, Account::Error>> {
        require_nonempty_aligned(range)?;
        if !window.range().contains(range) {
            return Err(AddressSpaceError::InvalidRange);
        }
        let id = allocate_address_space_id()?;
        let bytes =
            u64::try_from(size_of::<Self>()).map_err(|_| AddressSpaceError::SizeOverflow)?;
        let metadata_charge = account
            .try_charge(MemoryCharge {
                kernel_bytes: bytes,
                kernel_objects: 1,
                address_spaces: 1,
                ..MemoryCharge::default()
            })
            .map_err(AddressSpaceError::Account)?;
        let mapping_set_charge = account
            .try_charge(MemoryCharge {
                kernel_bytes: FallibleArc::<MappingSet<Backend, Account>>::allocation_size() as u64,
                ..MemoryCharge::default()
            })
            .map_err(AddressSpaceError::Account)?;
        let mappings = FallibleArc::try_new(MappingSet {
            records: Vec::new(),
            storage_charge: Some(mapping_set_charge),
        })
        .map_err(|_| AddressSpaceError::Allocation)?;
        let vmar_set_charge = account
            .try_charge(MemoryCharge {
                kernel_bytes: FallibleArc::<VmarSet<Account>>::allocation_size() as u64,
                ..MemoryCharge::default()
            })
            .map_err(AddressSpaceError::Account)?;
        let vmars = FallibleArc::try_new(VmarSet {
            records: Vec::new(),
            _storage_charge: Some(vmar_set_charge),
        })
        .map_err(|_| AddressSpaceError::Allocation)?;
        let root = Vmar {
            address_space: id,
            id: 0,
            generation: 1,
            range,
        };
        Ok(Self {
            id,
            root,
            backend,
            account,
            state: AddressSpaceLock::new(AddressSpaceState {
                mappings,
                vmars,
                next_mapping_id: 1,
                next_vmar_id: 1,
                authority_epoch: 1,
                mapping_epoch: 1,
            }),
            _metadata_charge: metadata_charge,
        })
    }

    pub(crate) const fn id(&self) -> AddressSpaceId {
        self.id
    }

    pub(crate) const fn root_vmar(&self) -> Vmar {
        self.root
    }

    pub(crate) fn mapping_epoch(&self) -> u64 {
        self.state.with(|state| state.mapping_epoch)
    }

    fn snapshot(&self) -> AddressSpaceSnapshot<Backend, Account> {
        self.state.with(|state| AddressSpaceSnapshot {
            mappings: state.mappings.clone(),
            vmars: state.vmars.clone(),
            authority_epoch: state.authority_epoch,
            mapping_epoch: state.mapping_epoch,
        })
    }

    pub(crate) fn try_create_vmar(
        &self,
        parent: Vmar,
        range: UserSlice,
    ) -> Result<Vmar, AddressSpaceError<Backend::Error, Account::Error>> {
        require_nonempty_aligned(range)?;
        let snapshot = self.snapshot();
        let authority = validate_vmar(self.id, self.root, &snapshot, parent)?;
        validate_child_range(&snapshot, parent.id, authority, range)?;
        let next_epoch = snapshot
            .authority_epoch
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let id = self.state.with(|state| {
            if state.authority_epoch != snapshot.authority_epoch {
                return Err(AddressSpaceError::StaleTransaction);
            }
            let id = state.next_vmar_id;
            let next = id
                .checked_add(1)
                .ok_or(AddressSpaceError::IdentityExhausted)?;
            state.next_vmar_id = next;
            Ok(id)
        })?;
        let capacity = snapshot
            .vmars
            .records
            .len()
            .checked_add(1)
            .ok_or(AddressSpaceError::SizeOverflow)?;
        let ownership = self.try_charge_vmar_record()?;
        let storage_charge = self.try_vmar_set_charge(capacity)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| AddressSpaceError::Allocation)?;
        let token = Vmar {
            address_space: self.id,
            id,
            generation: next_epoch,
            range,
        };
        replacement.extend(snapshot.vmars.records.iter().cloned());
        replacement.push(VmarRecord {
            token,
            parent_id: parent.id,
            ownership,
        });
        let replacement = FallibleArc::try_new(VmarSet {
            records: replacement,
            _storage_charge: storage_charge,
        })
        .map_err(|_| AddressSpaceError::Allocation)?;
        let retired = self.state.with(|state| {
            if state.authority_epoch != snapshot.authority_epoch {
                return Err(AddressSpaceError::StaleTransaction);
            }
            // Cloning the already-owned set is infallible and ensures a stale
            // transaction destroys its prepared allocation after this IRQ-safe
            // lock has been released.
            let old = core::mem::replace(&mut state.vmars, replacement.clone());
            state.authority_epoch = next_epoch;
            Ok(old)
        })?;
        drop(retired);
        Ok(token)
    }

    pub(crate) fn destroy_vmar(
        &self,
        token: Vmar,
    ) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
        if token.id == 0 {
            return Err(AddressSpaceError::InvalidRange);
        }
        let snapshot = self.snapshot();
        let index = vmar_index(self.id, self.root, &snapshot, token)?;
        if snapshot
            .mappings
            .records
            .iter()
            .any(|mapping| mapping.owner_vmar == token.id)
            || snapshot
                .vmars
                .records
                .iter()
                .any(|record| record.parent_id == token.id)
        {
            return Err(AddressSpaceError::InvalidRange);
        }
        let next_epoch = snapshot
            .authority_epoch
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let capacity = snapshot.vmars.records.len() - 1;
        let storage_charge = self.try_vmar_set_charge(capacity)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(capacity)
            .map_err(|_| AddressSpaceError::Allocation)?;
        records.extend(
            snapshot
                .vmars
                .records
                .iter()
                .enumerate()
                .filter(|(candidate, _)| *candidate != index)
                .map(|(_, record)| record.clone()),
        );
        let replacement = FallibleArc::try_new(VmarSet {
            records,
            _storage_charge: storage_charge,
        })
        .map_err(|_| AddressSpaceError::Allocation)?;
        let retired = self.state.with(|state| {
            if state.authority_epoch != snapshot.authority_epoch {
                return Err(AddressSpaceError::StaleTransaction);
            }
            let old = core::mem::replace(&mut state.vmars, replacement.clone());
            state.authority_epoch = next_epoch;
            Ok(old)
        })?;
        drop(retired);
        Ok(())
    }

    pub(crate) fn prepare_map_writable(
        &self,
        vmar: Vmar,
        range: UserSlice,
        object: WritableVmo<Backend, Account>,
        object_offset: u64,
        permissions: Permissions,
        maximum_permissions: Permissions,
    ) -> Result<
        PreparedMappingChange<'_, Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        self.prepare_map(
            vmar,
            range,
            MappingObject::Writable(object),
            object_offset,
            permissions,
            maximum_permissions,
        )
    }

    pub(crate) fn prepare_map_executable(
        &self,
        vmar: Vmar,
        range: UserSlice,
        object: ExecutableVmo<Backend, Account>,
        object_offset: u64,
        permissions: Permissions,
        maximum_permissions: Permissions,
    ) -> Result<
        PreparedMappingChange<'_, Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        self.prepare_map(
            vmar,
            range,
            MappingObject::Executable(object),
            object_offset,
            permissions,
            maximum_permissions,
        )
    }

    fn prepare_map(
        &self,
        vmar: Vmar,
        range: UserSlice,
        object: MappingObject<Backend, Account>,
        object_offset: u64,
        permissions: Permissions,
        maximum_permissions: Permissions,
    ) -> Result<
        PreparedMappingChange<'_, Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        require_nonempty_aligned(range)?;
        require_page_aligned(object_offset)?;
        validate_mapping_permissions(&object, permissions)?;
        validate_mapping_permissions(&object, maximum_permissions)?;
        if maximum_permissions == Permissions::NONE {
            return Err(AddressSpaceError::InvalidPermissions);
        }
        if !permissions.is_subset_of(maximum_permissions) {
            return Err(AddressSpaceError::InvalidPermissions);
        }
        let object_end = object_offset
            .checked_add(range.length())
            .ok_or(AddressSpaceError::SizeOverflow)?;
        if object_end > object.size() {
            return Err(AddressSpaceError::InvalidRange);
        }
        let snapshot = self.snapshot();
        validate_mapping_range(self.id, self.root, &snapshot, vmar, range)?;
        let next_authority_epoch = snapshot
            .authority_epoch
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let next_mapping_epoch = snapshot
            .mapping_epoch
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let id = self.state.with(|state| {
            if state.authority_epoch != snapshot.authority_epoch {
                return Err(AddressSpaceError::StaleTransaction);
            }
            reserve_mapping_ids(state, 1)
        })?;
        let token = MappingToken {
            address_space: self.id,
            id,
            generation: next_authority_epoch,
        };
        let capacity = snapshot
            .mappings
            .records
            .len()
            .checked_add(1)
            .ok_or(AddressSpaceError::SizeOverflow)?;

        if !object.fully_resident(object_offset, range.length())? {
            return Err(AddressSpaceError::BackingNotResident);
        }
        let ownership = self.try_mapping_ownership()?;
        let write_lease = if permissions.contains(Access::Write) {
            Some(object.try_write_lease()?)
        } else {
            None
        };
        let storage_charge = self.try_mapping_set_charge(capacity)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| AddressSpaceError::Allocation)?;
        replacement.extend(snapshot.mappings.records.iter().cloned());
        replacement.push(Mapping {
            owner_vmar: vmar.id,
            snapshot: MappingSnapshot {
                token,
                range,
                object_offset,
                permissions,
                executable_backing: object.executable(),
            },
            maximum_permissions,
            object,
            ownership,
            write_lease,
        });
        sort_mappings(&mut replacement);
        let replacement = FallibleArc::try_new(MappingSet {
            records: replacement,
            storage_charge,
        })
        .map_err(|_| AddressSpaceError::Allocation)?;
        PreparedMappingChange::new(
            self,
            snapshot.authority_epoch,
            next_authority_epoch,
            snapshot.mapping_epoch,
            next_mapping_epoch,
            replacement,
        )
    }

    pub(crate) fn prepare_unmap(
        &self,
        vmar: Vmar,
        range: UserSlice,
    ) -> Result<
        PreparedMappingChange<'_, Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        self.prepare_rewrite(vmar, range, None)
    }

    pub(crate) fn prepare_protect(
        &self,
        vmar: Vmar,
        range: UserSlice,
        permissions: Permissions,
    ) -> Result<
        PreparedMappingChange<'_, Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        if !permissions.is_valid() {
            return Err(AddressSpaceError::InvalidPermissions);
        }
        self.prepare_rewrite(vmar, range, Some(permissions))
    }

    fn prepare_rewrite(
        &self,
        vmar: Vmar,
        range: UserSlice,
        permissions: Option<Permissions>,
    ) -> Result<
        PreparedMappingChange<'_, Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        require_nonempty_aligned(range)?;
        let snapshot = self.snapshot();
        validate_owned_mapped_range(self.id, self.root, &snapshot, vmar, range)?;
        let generation = snapshot
            .authority_epoch
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let next_mapping_epoch = snapshot
            .mapping_epoch
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let (capacity, extra_owners, rewritten) = {
            let mut capacity = 0usize;
            let mut extra = 0usize;
            let mut rewritten = 0usize;
            for mapping in &snapshot.mappings.records {
                if mapping.owner_vmar != vmar.id || !ranges_overlap(mapping.snapshot.range, range) {
                    capacity = capacity
                        .checked_add(1)
                        .ok_or(AddressSpaceError::SizeOverflow)?;
                    continue;
                }
                if let Some(new_permissions) = permissions {
                    validate_mapping_permissions(&mapping.object, new_permissions)?;
                    if !new_permissions.is_subset_of(mapping.maximum_permissions) {
                        return Err(AddressSpaceError::InvalidPermissions);
                    }
                }
                let fragments =
                    fragment_total(mapping.snapshot.range, range, permissions.is_some());
                capacity = capacity
                    .checked_add(fragments)
                    .ok_or(AddressSpaceError::SizeOverflow)?;
                rewritten = rewritten
                    .checked_add(fragments)
                    .ok_or(AddressSpaceError::SizeOverflow)?;
                extra = extra
                    .checked_add(fragments.saturating_sub(1))
                    .ok_or(AddressSpaceError::SizeOverflow)?;
            }
            (capacity, extra, rewritten)
        };
        let first_token = self.state.with(|state| {
            if state.authority_epoch != snapshot.authority_epoch {
                return Err(AddressSpaceError::StaleTransaction);
            }
            reserve_mapping_ids(state, rewritten)
        })?;

        let storage_charge = self.try_mapping_set_charge(capacity)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| AddressSpaceError::Allocation)?;
        let ownership_storage_charge =
            self.try_storage_charge::<MappingOwner<Account>>(extra_owners)?;
        let mut ownerships = Vec::new();
        ownerships
            .try_reserve_exact(extra_owners)
            .map_err(|_| AddressSpaceError::Allocation)?;
        for _ in 0..extra_owners {
            ownerships.push(self.try_mapping_ownership()?);
        }

        let mut next_token = first_token;
        let mut next_ownership = ownerships.drain(..);
        for mapping in &snapshot.mappings.records {
            if mapping.owner_vmar != vmar.id || !ranges_overlap(mapping.snapshot.range, range) {
                replacement.push(mapping.clone());
                continue;
            }
            append_fragments(
                self.id,
                &mut replacement,
                mapping,
                range,
                permissions,
                generation,
                &mut next_token,
                &mut next_ownership,
            )?;
        }
        sort_mappings(&mut replacement);
        drop(next_ownership);
        drop(ownerships);
        drop(ownership_storage_charge);
        let replacement = FallibleArc::try_new(MappingSet {
            records: replacement,
            storage_charge,
        })
        .map_err(|_| AddressSpaceError::Allocation)?;
        PreparedMappingChange::new(
            self,
            snapshot.authority_epoch,
            generation,
            snapshot.mapping_epoch,
            next_mapping_epoch,
            replacement,
        )
    }

    pub(crate) fn mapping_snapshot(
        &self,
        token: MappingToken,
    ) -> Result<MappingSnapshot, AddressSpaceError<Backend::Error, Account::Error>> {
        if token.address_space != self.id {
            return Err(AddressSpaceError::InvalidAddressSpace);
        }
        let snapshot = self.snapshot();
        snapshot
            .mappings
            .records
            .iter()
            .find(|mapping| mapping.snapshot.token == token)
            .map(|mapping| mapping.snapshot)
            .ok_or(AddressSpaceError::StaleMapping)
    }

    pub(crate) fn copy_from_user(
        &self,
        source: UserSlice,
        destination: &mut [u8],
    ) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
        let destination_length =
            u64::try_from(destination.len()).map_err(|_| AddressSpaceError::SizeOverflow)?;
        if source.length() != destination_length {
            return Err(AddressSpaceError::SizeMismatch);
        }
        if source.length() == 0 {
            return Ok(());
        }
        let plan = self.prepare_copy(source, Access::Read)?;
        let mut copied = 0usize;
        for segment in &plan.segments {
            let length =
                usize::try_from(segment.length).map_err(|_| AddressSpaceError::SizeOverflow)?;
            let end = copied
                .checked_add(length)
                .ok_or(AddressSpaceError::SizeOverflow)?;
            let target = destination
                .get_mut(copied..end)
                .ok_or(AddressSpaceError::SizeOverflow)?;
            segment.object.read_exposed(segment.object_offset, target)?;
            copied = end;
        }
        Ok(())
    }

    pub(crate) fn copy_to_user(
        &self,
        destination: UserSlice,
        source: &[u8],
    ) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
        let source_length =
            u64::try_from(source.len()).map_err(|_| AddressSpaceError::SizeOverflow)?;
        if destination.length() != source_length {
            return Err(AddressSpaceError::SizeMismatch);
        }
        if destination.length() == 0 {
            return Ok(());
        }
        let plan = self.prepare_copy(destination, Access::Write)?;
        let mut copied = 0usize;
        for segment in &plan.segments {
            let length =
                usize::try_from(segment.length).map_err(|_| AddressSpaceError::SizeOverflow)?;
            let end = copied
                .checked_add(length)
                .ok_or(AddressSpaceError::SizeOverflow)?;
            let source = source
                .get(copied..end)
                .ok_or(AddressSpaceError::SizeOverflow)?;
            segment
                .object
                .write_exposed(segment.object_offset, source)?;
            copied = end;
        }
        Ok(())
    }

    fn prepare_copy(
        &self,
        range: UserSlice,
        access: Access,
    ) -> Result<CopyPlan<Backend, Account>, AddressSpaceError<Backend::Error, Account::Error>> {
        if !self.root.range.contains(range) {
            return Err(AddressSpaceError::InvalidRange);
        }
        let snapshot = self.snapshot();
        let count = snapshot
            .mappings
            .records
            .iter()
            .filter(|mapping| ranges_overlap(mapping.snapshot.range, range))
            .count();
        let temporary_charge = self.try_storage_charge::<CopySegment<Backend, Account>>(count)?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(count)
            .map_err(|_| AddressSpaceError::Allocation)?;
        let mut cursor = range.base();
        for mapping in &snapshot.mappings.records {
            let Some(overlap) = intersection(mapping.snapshot.range, range) else {
                continue;
            };
            if overlap.base() != cursor {
                return Err(AddressSpaceError::NotMapped);
            }
            if !mapping.snapshot.permissions.contains(access) {
                return Err(match access {
                    Access::Write => AddressSpaceError::WriteDenied,
                    Access::Read | Access::Execute => AddressSpaceError::ReadDenied,
                });
            }
            let delta = overlap
                .base()
                .get()
                .checked_sub(mapping.snapshot.range.base().get())
                .ok_or(AddressSpaceError::SizeOverflow)?;
            segments.push(CopySegment {
                object: mapping.object.clone(),
                _write_lease: mapping.write_lease.clone(),
                object_offset: mapping
                    .snapshot
                    .object_offset
                    .checked_add(delta)
                    .ok_or(AddressSpaceError::SizeOverflow)?,
                length: overlap.length(),
            });
            cursor = overlap.end();
        }
        if cursor != range.end() {
            return Err(AddressSpaceError::NotMapped);
        }
        Ok(CopyPlan {
            segments,
            _temporary_charge: temporary_charge,
        })
    }

    fn try_mapping_ownership(&self) -> SpaceResult<Backend, Account, MappingOwner<Account>> {
        let charge = self
            .account
            .try_charge(MemoryCharge {
                kernel_bytes: FallibleArc::<MappingOwnership<Account::Charge>>::allocation_size()
                    as u64,
                mappings: 1,
                ..MemoryCharge::default()
            })
            .map_err(AddressSpaceError::Account)?;
        FallibleArc::try_new(MappingOwnership { _charge: charge })
            .map_err(|_| AddressSpaceError::Allocation)
    }

    fn try_charge_vmar_record(&self) -> SpaceResult<Backend, Account, VmarOwner<Account>> {
        let charge = self
            .account
            .try_charge(MemoryCharge {
                kernel_bytes: FallibleArc::<VmarOwnership<Account::Charge>>::allocation_size()
                    as u64,
                kernel_objects: 1,
                ..MemoryCharge::default()
            })
            .map_err(AddressSpaceError::Account)?;
        FallibleArc::try_new(VmarOwnership { _charge: charge })
            .map_err(|_| AddressSpaceError::Allocation)
    }

    fn try_storage_charge<Item>(
        &self,
        capacity: usize,
    ) -> SpaceResult<Backend, Account, Option<Account::Charge>> {
        if capacity == 0 {
            return Ok(None);
        }
        let bytes = capacity
            .checked_mul(size_of::<Item>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(AddressSpaceError::SizeOverflow)?;
        self.account
            .try_charge(MemoryCharge {
                kernel_bytes: bytes,
                ..MemoryCharge::default()
            })
            .map(Some)
            .map_err(AddressSpaceError::Account)
    }

    fn try_mapping_set_charge(
        &self,
        capacity: usize,
    ) -> SpaceResult<Backend, Account, Option<Account::Charge>> {
        self.try_owner_storage_charge::<Mapping<Backend, Account>>(
            capacity,
            FallibleArc::<MappingSet<Backend, Account>>::allocation_size(),
        )
    }

    fn try_vmar_set_charge(
        &self,
        capacity: usize,
    ) -> SpaceResult<Backend, Account, Option<Account::Charge>> {
        self.try_owner_storage_charge::<VmarRecord<Account::Charge>>(
            capacity,
            FallibleArc::<VmarSet<Account>>::allocation_size(),
        )
    }

    fn try_owner_storage_charge<Item>(
        &self,
        capacity: usize,
        owner_bytes: usize,
    ) -> SpaceResult<Backend, Account, Option<Account::Charge>> {
        let bytes = capacity
            .checked_mul(size_of::<Item>())
            .and_then(|bytes| bytes.checked_add(owner_bytes))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(AddressSpaceError::SizeOverflow)?;
        self.account
            .try_charge(MemoryCharge {
                kernel_bytes: bytes,
                ..MemoryCharge::default()
            })
            .map(Some)
            .map_err(AddressSpaceError::Account)
    }
}

#[must_use = "prepared mapping state must be committed or explicitly abandoned"]
pub(crate) struct PreparedMappingChange<'a, Backend: PageBackend, Account: MemoryAccount> {
    address_space: &'a UserAddressSpace<Backend, Account>,
    base_authority_epoch: u64,
    next_authority_epoch: u64,
    base_mapping_epoch: u64,
    next_mapping_epoch: u64,
    replacement: FallibleArc<MappingSet<Backend, Account>>,
}

impl<'a, Backend: PageBackend, Account: MemoryAccount> PreparedMappingChange<'a, Backend, Account> {
    fn new(
        address_space: &'a UserAddressSpace<Backend, Account>,
        base_authority_epoch: u64,
        next_authority_epoch: u64,
        base_mapping_epoch: u64,
        next_mapping_epoch: u64,
        replacement: FallibleArc<MappingSet<Backend, Account>>,
    ) -> Result<Self, AddressSpaceError<Backend::Error, Account::Error>> {
        Ok(Self {
            address_space,
            base_authority_epoch,
            next_authority_epoch,
            base_mapping_epoch,
            next_mapping_epoch,
            replacement,
        })
    }

    pub(crate) fn snapshots(&self) -> impl Iterator<Item = MappingSnapshot> + '_ {
        self.replacement
            .records
            .iter()
            .map(|mapping| mapping.snapshot)
    }

    pub(super) const fn base_epoch(&self) -> u64 {
        self.base_mapping_epoch
    }

    pub(super) const fn next_epoch(&self) -> u64 {
        self.next_mapping_epoch
    }

    pub(crate) fn resident_pages(
        &self,
        token: MappingToken,
    ) -> Result<
        PreparedPageSnapshot<Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        let mapping = self
            .replacement
            .records
            .iter()
            .find(|mapping| mapping.snapshot.token == token)
            .ok_or(AddressSpaceError::StaleMapping)?;
        let count = usize::try_from(mapping.snapshot.range.length() / PAGE_SIZE)
            .map_err(|_| AddressSpaceError::SizeOverflow)?;
        let charge = self
            .address_space
            .try_storage_charge::<PhysicalAddress>(count)?;
        let pages = resident_pages(
            &mapping.object,
            mapping.snapshot.object_offset,
            mapping.snapshot.range.length(),
        )?;
        Ok(PreparedPageSnapshot {
            pages,
            _charge: charge,
            _pins: self.replacement.clone(),
        })
    }

    /// Publishes the logical set only if no intervening mapping/VMAR commit won.
    pub(super) fn commit_machine(
        self,
    ) -> Result<
        CommittedMappingChange<Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        let retired = self.address_space.state.with(|state| {
            if state.authority_epoch != self.base_authority_epoch
                || state.mapping_epoch != self.base_mapping_epoch
            {
                return Err(AddressSpaceError::StaleTransaction);
            }
            // Keep the prepared owner outside the lock so a stale commit never
            // runs mapping/page/account destructors with local IRQs masked.
            let old = core::mem::replace(&mut state.mappings, self.replacement.clone());
            state.authority_epoch = self.next_authority_epoch;
            state.mapping_epoch = self.next_mapping_epoch;
            Ok(old)
        })?;
        Ok(CommittedMappingChange {
            change: MappingChange {
                address_space: self.address_space.id,
                previous_epoch: self.base_mapping_epoch,
                epoch: self.next_mapping_epoch,
            },
            retired: ManuallyDrop::new(retired),
        })
    }

    #[cfg(any(test, feature = "kernel-self-test"))]
    pub(crate) fn commit_for_test(
        self,
    ) -> Result<
        CommittedMappingChange<Backend, Account>,
        AddressSpaceError<Backend::Error, Account::Error>,
    > {
        self.commit_machine()
    }
}

pub(crate) struct PreparedPageSnapshot<Backend: PageBackend, Account: MemoryAccount> {
    pages: Vec<PhysicalAddress>,
    _charge: Option<Account::Charge>,
    _pins: FallibleArc<MappingSet<Backend, Account>>,
}

impl<Backend: PageBackend, Account: MemoryAccount> PreparedPageSnapshot<Backend, Account> {
    pub(crate) fn pages(&self) -> &[PhysicalAddress] {
        &self.pages
    }
}

/// Published logical state whose prior physical owners await invalidation.
#[must_use = "retired mappings remain pinned until invalidation is acknowledged"]
pub(crate) struct CommittedMappingChange<Backend: PageBackend, Account: MemoryAccount> {
    change: MappingChange,
    retired: ManuallyDrop<FallibleArc<MappingSet<Backend, Account>>>,
}

impl<Backend: PageBackend, Account: MemoryAccount> CommittedMappingChange<Backend, Account> {
    pub(crate) const fn change(&self) -> MappingChange {
        self.change
    }

    /// Releases old backing after every machine translation is quiescent.
    ///
    /// # Safety
    ///
    /// No CPU may retain an active or cached translation derived from the
    /// previous epoch. Stage3 must establish that through its opaque
    /// invalidation acknowledgement before calling this method.
    pub(super) unsafe fn complete_machine_retirement(mut self) {
        // SAFETY: The caller proves the exact translation-quiescence condition
        // required before old mapping owners and their pages may be reclaimed.
        unsafe { ManuallyDrop::drop(&mut self.retired) };
    }

    #[cfg(any(test, feature = "kernel-self-test"))]
    pub(crate) unsafe fn complete_retirement_for_test(mut self) {
        // SAFETY: Test callers model the required acknowledged quiescence.
        unsafe { ManuallyDrop::drop(&mut self.retired) };
    }
}

struct CopySegment<Backend: PageBackend, Account: MemoryAccount> {
    object: MappingObject<Backend, Account>,
    _write_lease: Option<WritableMappingLease<Backend, Account>>,
    object_offset: u64,
    length: u64,
}

struct CopyPlan<Backend: PageBackend, Account: MemoryAccount> {
    segments: Vec<CopySegment<Backend, Account>>,
    _temporary_charge: Option<Account::Charge>,
}

impl<Backend: PageBackend, Account: MemoryAccount> ForeignMemory
    for UserAddressSpace<Backend, Account>
{
    type Error = AddressSpaceError<Backend::Error, Account::Error>;

    fn address_base(&self) -> u64 {
        self.root.range.base().get()
    }

    fn address_size(&self) -> u64 {
        self.root.range.length()
    }

    fn page_size(&self) -> usize {
        PAGE_SIZE as usize
    }

    fn read_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let range = self.page_chunk(page_index, page_offset, destination.len())?;
        self.copy_from_user(range, destination)
    }

    fn write_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        let range = self.page_chunk(page_index, page_offset, source.len())?;
        self.copy_to_user(range, source)
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> UserAddressSpace<Backend, Account> {
    fn page_chunk(
        &self,
        page_index: usize,
        page_offset: usize,
        length: usize,
    ) -> Result<UserSlice, AddressSpaceError<Backend::Error, Account::Error>> {
        let page_index = u64::try_from(page_index).map_err(|_| AddressSpaceError::SizeOverflow)?;
        let page_offset =
            u64::try_from(page_offset).map_err(|_| AddressSpaceError::SizeOverflow)?;
        let length = u64::try_from(length).map_err(|_| AddressSpaceError::SizeOverflow)?;
        let offset = page_index
            .checked_mul(PAGE_SIZE)
            .and_then(|base| base.checked_add(page_offset))
            .ok_or(AddressSpaceError::SizeOverflow)?;
        let address = self
            .root
            .range
            .base()
            .checked_add(offset)
            .ok_or(AddressSpaceError::SizeOverflow)?;
        UserSlice::new(address, length).map_err(|_| AddressSpaceError::SizeOverflow)
    }
}

fn allocate_address_space_id<BackendError, AccountError>()
-> Result<AddressSpaceId, AddressSpaceError<BackendError, AccountError>> {
    let mut current = NEXT_ADDRESS_SPACE_ID.load(Ordering::Relaxed);
    loop {
        if current == 0 {
            return Err(AddressSpaceError::IdentityExhausted);
        }
        let next = current
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        match NEXT_ADDRESS_SPACE_ID.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(AddressSpaceId(current)),
            Err(observed) => current = observed,
        }
    }
}

fn validate_vmar<Backend: PageBackend, Account: MemoryAccount>(
    address_space: AddressSpaceId,
    root: Vmar,
    state: &AddressSpaceSnapshot<Backend, Account>,
    token: Vmar,
) -> Result<UserSlice, AddressSpaceError<Backend::Error, Account::Error>> {
    if token.address_space != address_space {
        return Err(AddressSpaceError::InvalidAddressSpace);
    }
    if token.id == 0 {
        return if token == root {
            Ok(root.range)
        } else {
            Err(AddressSpaceError::StaleVmar)
        };
    }
    state
        .vmars
        .records
        .iter()
        .find(|record| record.token == token)
        .map(|record| record.token.range)
        .ok_or(AddressSpaceError::StaleVmar)
}

fn vmar_index<Backend: PageBackend, Account: MemoryAccount>(
    address_space: AddressSpaceId,
    root: Vmar,
    state: &AddressSpaceSnapshot<Backend, Account>,
    token: Vmar,
) -> Result<usize, AddressSpaceError<Backend::Error, Account::Error>> {
    validate_vmar(address_space, root, state, token)?;
    state
        .vmars
        .records
        .iter()
        .position(|record| record.token == token)
        .ok_or(AddressSpaceError::StaleVmar)
}

fn validate_child_range<Backend: PageBackend, Account: MemoryAccount>(
    state: &AddressSpaceSnapshot<Backend, Account>,
    parent_id: u64,
    authority: UserSlice,
    range: UserSlice,
) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
    if !authority.contains(range) {
        return Err(AddressSpaceError::InvalidRange);
    }
    if state
        .vmars
        .records
        .iter()
        .any(|record| record.parent_id == parent_id && ranges_overlap(record.token.range, range))
        || state.mappings.records.iter().any(|mapping| {
            mapping.owner_vmar == parent_id && ranges_overlap(mapping.snapshot.range, range)
        })
    {
        return Err(AddressSpaceError::Overlap);
    }
    Ok(())
}

fn validate_mapping_range<Backend: PageBackend, Account: MemoryAccount>(
    address_space: AddressSpaceId,
    root: Vmar,
    state: &AddressSpaceSnapshot<Backend, Account>,
    vmar: Vmar,
    range: UserSlice,
) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
    let authority = validate_vmar(address_space, root, state, vmar)?;
    if !authority.contains(range) {
        return Err(AddressSpaceError::InvalidRange);
    }
    if state
        .vmars
        .records
        .iter()
        .any(|record| record.parent_id == vmar.id && ranges_overlap(record.token.range, range))
        || state
            .mappings
            .records
            .iter()
            .any(|mapping| ranges_overlap(mapping.snapshot.range, range))
    {
        return Err(AddressSpaceError::Overlap);
    }
    Ok(())
}

fn validate_owned_mapped_range<Backend: PageBackend, Account: MemoryAccount>(
    address_space: AddressSpaceId,
    root: Vmar,
    state: &AddressSpaceSnapshot<Backend, Account>,
    vmar: Vmar,
    range: UserSlice,
) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
    let authority = validate_vmar(address_space, root, state, vmar)?;
    if !authority.contains(range)
        || state
            .vmars
            .records
            .iter()
            .any(|record| record.parent_id == vmar.id && ranges_overlap(record.token.range, range))
    {
        return Err(AddressSpaceError::NotMapped);
    }
    let mut cursor = range.base();
    for mapping in &state.mappings.records {
        if mapping.owner_vmar != vmar.id {
            continue;
        }
        let Some(overlap) = intersection(mapping.snapshot.range, range) else {
            continue;
        };
        if overlap.base() != cursor {
            return Err(AddressSpaceError::NotMapped);
        }
        cursor = overlap.end();
    }
    if cursor != range.end() {
        return Err(AddressSpaceError::NotMapped);
    }
    Ok(())
}

fn reserve_mapping_ids<Backend: PageBackend, Account: MemoryAccount>(
    state: &mut AddressSpaceState<Backend, Account>,
    count: usize,
) -> Result<u64, AddressSpaceError<Backend::Error, Account::Error>> {
    let count = u64::try_from(count).map_err(|_| AddressSpaceError::SizeOverflow)?;
    let first = state.next_mapping_id;
    let next = first
        .checked_add(count)
        .ok_or(AddressSpaceError::IdentityExhausted)?;
    state.next_mapping_id = next;
    Ok(first)
}

fn validate_mapping_permissions<Backend: PageBackend, Account: MemoryAccount>(
    object: &MappingObject<Backend, Account>,
    permissions: Permissions,
) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
    if !permissions.is_valid() {
        return Err(AddressSpaceError::InvalidPermissions);
    }
    if object.executable() {
        if permissions.contains(Access::Write) {
            return Err(AddressSpaceError::WritableExecutableBacking);
        }
    } else if permissions.contains(Access::Execute) {
        return Err(AddressSpaceError::InvalidPermissions);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_fragments<'a, Backend: PageBackend, Account: MemoryAccount>(
    address_space: AddressSpaceId,
    output: &mut Vec<Mapping<Backend, Account>>,
    original: &Mapping<Backend, Account>,
    cut: UserSlice,
    permissions: Option<Permissions>,
    generation: u64,
    next_token: &mut u64,
    extra_ownerships: &mut alloc::vec::Drain<'a, FallibleArc<MappingOwnership<Account::Charge>>>,
) -> Result<(), AddressSpaceError<Backend::Error, Account::Error>> {
    let overlap =
        intersection(original.snapshot.range, cut).ok_or(AddressSpaceError::InvalidRange)?;
    let fragments = [
        (original.snapshot.range.base(), overlap.base(), None),
        (overlap.base(), overlap.end(), permissions),
        (overlap.end(), original.snapshot.range.end(), None),
    ];
    let mut first = true;
    for (base, end, changed_permissions) in fragments {
        if base >= end || (base == overlap.base() && changed_permissions.is_none()) {
            continue;
        }
        let ownership = if first {
            first = false;
            original.ownership.clone()
        } else {
            extra_ownerships
                .next()
                .ok_or(AddressSpaceError::Allocation)?
        };
        let length = end.get() - base.get();
        let range = UserSlice::new(base, length).map_err(|_| AddressSpaceError::SizeOverflow)?;
        let delta = base.get() - original.snapshot.range.base().get();
        let id = *next_token;
        *next_token = next_token
            .checked_add(1)
            .ok_or(AddressSpaceError::IdentityExhausted)?;
        let fragment_permissions = match changed_permissions {
            Some(permissions) => permissions,
            None => original.snapshot.permissions,
        };
        output.push(Mapping {
            owner_vmar: original.owner_vmar,
            snapshot: MappingSnapshot {
                token: MappingToken {
                    address_space,
                    id,
                    generation,
                },
                range,
                object_offset: original
                    .snapshot
                    .object_offset
                    .checked_add(delta)
                    .ok_or(AddressSpaceError::SizeOverflow)?,
                permissions: fragment_permissions,
                executable_backing: original.snapshot.executable_backing,
            },
            maximum_permissions: original.maximum_permissions,
            object: original.object.clone(),
            ownership,
            write_lease: if fragment_permissions.contains(Access::Write) {
                match &original.write_lease {
                    Some(lease) => Some(lease.clone()),
                    None => Some(original.object.try_write_lease()?),
                }
            } else {
                None
            },
        });
    }
    Ok(())
}

fn require_nonempty_aligned<BackendError, AccountError>(
    range: UserSlice,
) -> Result<(), AddressSpaceError<BackendError, AccountError>> {
    if range.length() == 0 {
        return Err(AddressSpaceError::EmptyRange);
    }
    require_page_aligned(range.base().get())?;
    require_page_aligned(range.length())
}

fn require_page_aligned<BackendError, AccountError>(
    value: u64,
) -> Result<(), AddressSpaceError<BackendError, AccountError>> {
    if !value.is_multiple_of(PAGE_SIZE) {
        return Err(AddressSpaceError::InvalidRange);
    }
    Ok(())
}

fn ranges_overlap(left: UserSlice, right: UserSlice) -> bool {
    left.base() < right.end() && right.base() < left.end()
}

fn intersection(left: UserSlice, right: UserSlice) -> Option<UserSlice> {
    let base = left.base().max(right.base());
    let end = left.end().min(right.end());
    if base >= end {
        return None;
    }
    UserSlice::new(base, end.get() - base.get()).ok()
}

fn fragment_total(mapping: UserSlice, cut: UserSlice, retain_middle: bool) -> usize {
    let Some(overlap) = intersection(mapping, cut) else {
        return 1;
    };
    usize::from(mapping.base() < overlap.base())
        + usize::from(retain_middle)
        + usize::from(overlap.end() < mapping.end())
}

fn sort_mappings<Backend: PageBackend, Account: MemoryAccount>(
    mappings: &mut [Mapping<Backend, Account>],
) {
    mappings.sort_unstable_by_key(|mapping| mapping.snapshot.range.base());
}
