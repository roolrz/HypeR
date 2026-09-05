// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Opaque process-local handles and unpublished slot reservations.

use alloc::vec::Vec;
use core::array;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use super::super::object::{
    ActiveHandleError, ActiveHandleOwner, ErasedKernelRef, KernelObject, Koid, ObjectKind,
    ObjectPublication, ObjectRetirement, OperationPin, SignalSource, UserExportableObject,
};
use super::Rights;

const SLOT_BITS: u32 = 24;
const SLOT_MASK: u64 = (1 << SLOT_BITS) - 1;
const GENERATION_LIMIT: u64 = u64::MAX >> SLOT_BITS;
const MAX_SLOTS: usize = SLOT_MASK as usize;
const MAX_RESERVATION_SLOTS: usize = 64;
const SLOT_SEGMENTS: usize = 19;
const FIRST_SEGMENT_SLOTS: usize = 64;
const DIAGNOSTIC_PAGE_CAPACITY: usize = 32;
const DIAGNOSTIC_SLOT_BUDGET: usize = 256;
pub(crate) const HANDLE_TABLE_STORAGE_SEGMENTS: usize = SLOT_SEGMENTS;

const _: () = assert!(
    MAX_RESERVATION_SLOTS as u64 == hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES
);

static NEXT_RESERVATION_ID: AtomicU64 = AtomicU64::new(1);

fn validate_batch_count(count: usize) -> Result<(), HandleError> {
    if count == 0 {
        Err(HandleError::EmptyReservation)
    } else if count > MAX_RESERVATION_SLOTS {
        Err(HandleError::ReservationTooLarge)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ReservationId(NonZeroU64);

impl ReservationId {
    fn allocate() -> Result<Self, HandleError> {
        let mut current = NEXT_RESERVATION_ID.load(Ordering::Relaxed);
        loop {
            let value = NonZeroU64::new(current).ok_or(HandleError::ReservationIdExhausted)?;
            let next = current
                .checked_add(1)
                .ok_or(HandleError::ReservationIdExhausted)?;
            match NEXT_RESERVATION_ID.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(Self(value)),
                Err(observed) => current = observed,
            }
        }
    }
}

/// Nonzero opaque handle value interpreted only by its owning Process.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct HandleValue(NonZeroU64);

impl HandleValue {
    /// Validates one untrusted raw ABI value before it reaches table decoding.
    pub(crate) fn try_from_raw(raw: u64) -> Result<Self, HandleError> {
        let value = NonZeroU64::new(raw).ok_or(HandleError::InvalidHandle)?;
        let slot = raw & SLOT_MASK;
        let generation = raw >> SLOT_BITS;
        if slot == 0 || generation == 0 {
            return Err(HandleError::InvalidHandle);
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }

    /// Returns the first value published by a fresh table.
    #[allow(dead_code)]
    pub(crate) fn first_for_test() -> Self {
        Self::encode(0, 1)
    }

    fn encode(slot: usize, generation: u64) -> Self {
        let raw = (generation << SLOT_BITS) | (slot as u64 + 1);
        // Slot indices and generations are private validated table state. Their
        // encoded value is nonzero because both fields begin at one.
        match NonZeroU64::new(raw) {
            Some(value) => Self(value),
            None => unreachable_handle_value(),
        }
    }

    fn decode(self) -> (usize, u64) {
        let raw = self.0.get();
        ((raw & SLOT_MASK) as usize - 1, raw >> SLOT_BITS)
    }
}

#[cold]
fn unreachable_handle_value() -> ! {
    super::invariant_violation()
}

/// Currently supported per-handle flags.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HandleFlags(u32);

impl HandleFlags {
    pub(crate) const NONE: Self = Self(0);

    pub(crate) const fn from_bits(bits: u32) -> Option<Self> {
        if bits == 0 { Some(Self(bits)) } else { None }
    }

    pub(crate) const fn bits(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HandleError {
    Allocation,
    InvalidHandle,
    Busy,
    WrongObjectType,
    AccessDenied,
    UnsupportedRights,
    UnsupportedFlags,
    UnsupportedTransfer,
    ObjectRetired,
    ObjectAlreadyActive,
    ActiveHandleLimit,
    ReservationIdExhausted,
    ReservationTooLarge,
    OutstandingReservation,
    TableFull,
    TableRetired,
    EmptyReservation,
}

/// Handle-local metadata returned without exposing the object payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandleInfo {
    pub(crate) koid: Koid,
    pub(crate) kind: ObjectKind,
    pub(crate) rights: Rights,
    pub(crate) flags: HandleFlags,
}

/// Position in a process-local handle-table diagnostic scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandleScanCursor {
    next_slot: usize,
}

impl HandleScanCursor {
    pub(crate) const fn start() -> Self {
        Self { next_slot: 0 }
    }
}

/// One handle-table edge from a process-local value to a kernel object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandleSnapshot {
    pub(crate) value: HandleValue,
    pub(crate) info: HandleInfo,
}

/// Bounded diagnostic output from one locked handle-table observation.
pub(crate) struct HandleSnapshotPage {
    entries: [Option<HandleSnapshot>; DIAGNOSTIC_PAGE_CAPACITY],
    len: usize,
    next: Option<HandleScanCursor>,
}

impl HandleSnapshotPage {
    pub(crate) fn entries(&self) -> impl Iterator<Item = &HandleSnapshot> {
        self.entries[..self.len].iter().filter_map(Option::as_ref)
    }

    pub(crate) const fn next(&self) -> Option<HandleScanCursor> {
        self.next
    }
}

/// One source handle and its attenuated rights in a move transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandleTransferRequest {
    pub(crate) value: HandleValue,
    pub(crate) rights: Rights,
    pub(crate) expected_kind: Option<ObjectKind>,
}

/// One active but not necessarily published process handle.
///
/// Prepared handles are active authority. This keeps duplicate rollback and
/// future in-transit capability ownership from producing a false zero-active
/// transition. Dropping an unpublished value rolls that authority back and may
/// run the zero-active callback; a potentially final owner must therefore be
/// dropped only after releasing Process and object locks.
pub(crate) struct PreparedHandle {
    object: Option<ActiveHandleOwner>,
    rights: Rights,
    flags: HandleFlags,
}

impl PreparedHandle {
    /// Mints the sole first handle for a newly constructed object.
    pub(crate) fn try_from_new_object<T: UserExportableObject>(
        publication: ObjectPublication<T>,
        rights: Rights,
        flags: HandleFlags,
    ) -> Result<Self, HandleError> {
        if !publication.supported_rights().contains(rights) {
            return Err(HandleError::UnsupportedRights);
        }
        if HandleFlags::from_bits(flags.bits()).is_none() {
            return Err(HandleError::UnsupportedFlags);
        }
        let object = publication.activate().map_err(|error| match error {
            ActiveHandleError::NotExportable => HandleError::UnsupportedRights,
            ActiveHandleError::Retired => HandleError::ObjectRetired,
            ActiveHandleError::AlreadyActive => HandleError::ObjectAlreadyActive,
            ActiveHandleError::CountExhausted => HandleError::ActiveHandleLimit,
        })?;
        Ok(Self {
            object: Some(object),
            rights,
            flags,
        })
    }

    fn try_duplicate(&self, rights: Rights) -> Result<Self, HandleError> {
        if !self.rights.contains(rights) {
            return Err(HandleError::AccessDenied);
        }
        let object = self.object().try_duplicate().map_err(|error| match error {
            ActiveHandleError::NotExportable => super::invariant_violation(),
            ActiveHandleError::Retired => HandleError::ObjectRetired,
            ActiveHandleError::AlreadyActive => super::invariant_violation(),
            ActiveHandleError::CountExhausted => HandleError::ActiveHandleLimit,
        })?;
        Ok(Self {
            object: Some(object),
            rights,
            flags: self.flags,
        })
    }

    #[cfg(test)]
    pub(crate) fn duplicate_for_test(&self, rights: Rights) -> Result<Self, HandleError> {
        self.try_duplicate(rights)
    }

    fn object(&self) -> &ActiveHandleOwner {
        match self.object.as_ref() {
            Some(object) => object,
            None => super::invariant_violation(),
        }
    }

    fn release_into(&mut self, retirement: &mut ObjectRetirement) {
        let object = match self.object.take() {
            Some(object) => object,
            None => super::invariant_violation(),
        };
        object.release_into(retirement);
    }
}

impl Drop for PreparedHandle {
    fn drop(&mut self) {
        if self.object.is_none() {
            return;
        }
        let mut retirement = ObjectRetirement::new();
        self.release_into(&mut retirement);
        retirement.drain();
    }
}

enum Slot {
    Vacant {
        generation: u64,
        next_free: Option<usize>,
    },
    Reserved {
        generation: u64,
        reservation: ReservationId,
    },
    TransferReserved {
        generation: u64,
        transfer: ReservationId,
    },
    Occupied {
        generation: u64,
        handle: PreparedHandle,
    },
    Retired,
}

/// Unsynchronized table state owned and locked by one Process.
pub(crate) struct HandleTable {
    slots: SlotStore,
    free_head: Option<usize>,
    free_slots: usize,
    lifecycle: TableLifecycle,
    next_teardown_generation: u64,
}

/// Detached retired table backing, destroyed only after releasing table locks.
pub(crate) struct RetiredHandleStorage {
    _segments: [Option<Vec<Slot>>; SLOT_SEGMENTS],
}

/// Structural state which one lock-free handle-table backing candidate targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandleTableStorageSnapshot {
    slot_count: usize,
    free_slots: usize,
    additional: usize,
    segment_mask: u32,
}

impl HandleTableStorageSnapshot {
    /// Bytes in complete new segments prepared for this exact snapshot.
    pub(crate) const fn growth_bytes(self) -> Option<usize> {
        let mut segment = 0usize;
        let mut bytes = 0usize;
        while segment < SLOT_SEGMENTS {
            if self.segment_mask & (1_u32 << segment) != 0 {
                let segment_bytes =
                    match segment_capacity(segment).checked_mul(core::mem::size_of::<Slot>()) {
                        Some(bytes) => bytes,
                        None => return None,
                    };
                bytes = match bytes.checked_add(segment_bytes) {
                    Some(bytes) => bytes,
                    None => return None,
                };
            }
            segment += 1;
        }
        Some(bytes)
    }
}

/// Process-owned metadata indexed by the same bounded slot geometry as handles.
/// Generations are checked by the caller's entry; no allocation occurs in lookup
/// or publication. Growth is prepared alongside the authoritative table plan.
pub(crate) struct HandleSidecar<T> {
    segments: [Option<Vec<Option<T>>>; SLOT_SEGMENTS],
}

impl<T> HandleSidecar<T> {
    pub(crate) const fn new() -> Self {
        Self {
            segments: [const { None }; SLOT_SEGMENTS],
        }
    }

    pub(crate) fn growth_bytes(snapshot: HandleTableStorageSnapshot) -> Option<usize> {
        let mut bytes = 0usize;
        for segment in 0..SLOT_SEGMENTS {
            if snapshot.segment_mask & (1 << segment) != 0 {
                bytes = bytes.checked_add(
                    segment_capacity(segment).checked_mul(core::mem::size_of::<Option<T>>())?,
                )?;
            }
        }
        Some(bytes)
    }

    pub(crate) fn prepare(snapshot: HandleTableStorageSnapshot) -> Result<Self, HandleError> {
        let mut plan = Self::new();
        for segment in 0..SLOT_SEGMENTS {
            if snapshot.segment_mask & (1 << segment) != 0 {
                let count = segment_capacity(segment);
                let mut entries = Vec::new();
                entries
                    .try_reserve_exact(count)
                    .map_err(|_| HandleError::Allocation)?;
                entries.resize_with(count, || None);
                plan.segments[segment] = Some(entries);
            }
        }
        Ok(plan)
    }

    pub(crate) fn install(&mut self, mut plan: Self) {
        for (target, prepared) in self.segments.iter_mut().zip(&mut plan.segments) {
            if let Some(entries) = prepared.take() {
                if target.is_some() {
                    super::invariant_violation();
                }
                *target = Some(entries);
            }
        }
    }

    pub(crate) fn get(&self, value: HandleValue) -> Option<&T> {
        let (segment, offset) = segment_location(value.decode().0);
        self.segments.get(segment)?.as_ref()?.get(offset)?.as_ref()
    }

    pub(crate) fn replace(&mut self, value: HandleValue, entry: Option<T>) -> Option<T> {
        let (segment, offset) = segment_location(value.decode().0);
        let target = self
            .segments
            .get_mut(segment)
            .and_then(Option::as_mut)
            .and_then(|entries| entries.get_mut(offset));
        match target {
            Some(target) => core::mem::replace(target, entry),
            None => super::invariant_violation(),
        }
    }
}

/// Fully allocated table backing prepared before Process locks are acquired.
#[must_use = "install or discard the handle-table storage plan"]
pub(crate) struct HandleTableStoragePlan {
    snapshot: HandleTableStorageSnapshot,
    segments: [Option<Vec<Slot>>; SLOT_SEGMENTS],
}

impl HandleTableStoragePlan {
    pub(crate) fn try_new(snapshot: HandleTableStorageSnapshot) -> Result<Self, HandleError> {
        let mut segments: [Option<Vec<Slot>>; SLOT_SEGMENTS] = [const { None }; SLOT_SEGMENTS];
        for (index, segment) in segments.iter_mut().enumerate() {
            if snapshot.segment_mask & (1_u32 << index) == 0 {
                continue;
            }
            *segment = Some(allocate_slot_segment(segment_capacity(index))?);
        }
        Ok(Self { snapshot, segments })
    }

    pub(crate) const fn snapshot(&self) -> HandleTableStorageSnapshot {
        self.snapshot
    }

    #[cfg(test)]
    pub(crate) fn force_allocation_failure_for_test() -> Result<(), HandleError> {
        allocate_slot_segment(usize::MAX).map(drop)
    }
}

fn allocate_slot_segment(capacity: usize) -> Result<Vec<Slot>, HandleError> {
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(capacity)
        .map_err(|_| HandleError::Allocation)?;
    for _ in 0..capacity {
        slots.push(Slot::Retired);
    }
    Ok(slots)
}

struct SlotStore {
    segments: [Option<Vec<Slot>>; SLOT_SEGMENTS],
    len: usize,
}

impl SlotStore {
    const fn new() -> Self {
        Self {
            segments: [const { None }; SLOT_SEGMENTS],
            len: 0,
        }
    }

    const fn len(&self) -> usize {
        self.len
    }

    fn get(&self, index: usize) -> Option<&Slot> {
        if index >= self.len {
            return None;
        }
        let (segment, offset) = segment_location(index);
        self.segments.get(segment)?.as_ref()?.get(offset)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut Slot> {
        if index >= self.len {
            return None;
        }
        let (segment, offset) = segment_location(index);
        self.segments.get_mut(segment)?.as_mut()?.get_mut(offset)
    }

    fn replace(&mut self, index: usize, replacement: Slot) -> Slot {
        match self.get_mut(index) {
            Some(slot) => core::mem::replace(slot, replacement),
            None => super::invariant_violation(),
        }
    }

    fn iter(&self) -> impl Iterator<Item = &Slot> {
        self.segments
            .iter()
            .filter_map(Option::as_ref)
            .flat_map(|segment| segment.iter())
            .take(self.len)
    }

    fn required_segment_mask(&self, target: usize) -> u32 {
        let mut mask = 0_u32;
        for segment in 0..SLOT_SEGMENTS {
            if segment_base(segment) >= target {
                break;
            }
            if self.segments[segment].is_none() {
                mask |= 1_u32 << segment;
            }
        }
        mask
    }

    fn install(&mut self, mut plan: HandleTableStoragePlan) {
        for (index, prepared) in plan.segments.iter().enumerate() {
            let required = plan.snapshot.segment_mask & (1_u32 << index) != 0;
            if required != prepared.is_some() || (required && self.segments[index].is_some()) {
                super::invariant_violation();
            }
        }
        for (index, prepared) in plan.segments.iter_mut().enumerate() {
            let Some(segment) = prepared.take() else {
                continue;
            };
            if self.segments[index].is_some() {
                super::invariant_violation();
            }
            self.segments[index] = Some(segment);
        }
    }

    fn push_vacant(&mut self, generation: u64, next_free: Option<usize>) -> usize {
        let index = self.len;
        let slot = match self.get_unpublished_mut(index) {
            Some(slot) => slot,
            None => super::invariant_violation(),
        };
        if !matches!(slot, Slot::Retired) {
            super::invariant_violation();
        }
        *slot = Slot::Vacant {
            generation,
            next_free,
        };
        self.len += 1;
        index
    }

    fn get_unpublished_mut(&mut self, index: usize) -> Option<&mut Slot> {
        let (segment, offset) = segment_location(index);
        self.segments.get_mut(segment)?.as_mut()?.get_mut(offset)
    }

    fn take_retired(&mut self) -> RetiredHandleStorage {
        self.len = 0;
        RetiredHandleStorage {
            _segments: core::mem::replace(&mut self.segments, [const { None }; SLOT_SEGMENTS]),
        }
    }
}

const fn segment_capacity(segment: usize) -> usize {
    if segment == 0 {
        FIRST_SEGMENT_SLOTS
    } else if segment == SLOT_SEGMENTS - 1 {
        (1usize << (segment + 5)) - 1
    } else {
        1usize << (segment + 5)
    }
}

const fn segment_base(segment: usize) -> usize {
    if segment == 0 {
        0
    } else {
        1usize << (segment + 5)
    }
}

fn segment_location(index: usize) -> (usize, usize) {
    if index < FIRST_SEGMENT_SLOTS {
        return (0, index);
    }
    let highest_bit = (usize::BITS - 1 - index.leading_zeros()) as usize;
    let segment = highest_bit - 5;
    (segment, index - (1usize << highest_bit))
}

#[derive(Clone, Copy)]
enum TableLifecycle {
    Active,
    TearingDown { generation: u64 },
    Retired,
}

impl HandleTable {
    pub(crate) const fn new() -> Self {
        Self {
            slots: SlotStore::new(),
            free_head: None,
            free_slots: 0,
            lifecycle: TableLifecycle::Active,
            next_teardown_generation: 1,
        }
    }

    /// Returns the persistent logical-slot growth required by one reservation.
    #[cfg(test)]
    pub(crate) fn reservation_growth<const N: usize>(&self) -> Result<usize, HandleError> {
        self.reservation_growth_for(N)
    }

    pub(crate) fn reservation_growth_for(&self, count: usize) -> Result<usize, HandleError> {
        self.ensure_active()?;
        validate_batch_count(count)?;
        let additional = count.saturating_sub(self.free_slots);
        if self.slots.len().saturating_add(additional) > MAX_SLOTS {
            return Err(HandleError::TableFull);
        }
        Ok(additional)
    }

    pub(crate) fn reservation_storage_snapshot_for(
        &self,
        count: usize,
    ) -> Result<HandleTableStorageSnapshot, HandleError> {
        let additional = self.reservation_growth_for(count)?;
        let target = self
            .slots
            .len()
            .checked_add(additional)
            .ok_or(HandleError::TableFull)?;
        Ok(HandleTableStorageSnapshot {
            slot_count: self.slots.len(),
            free_slots: self.free_slots,
            additional,
            segment_mask: self.slots.required_segment_mask(target),
        })
    }

    #[cfg(test)]
    pub(crate) fn reserve_batch(
        &mut self,
        count: usize,
    ) -> Result<HandleBatchReservation, HandleError> {
        let storage = HandleBatchReservationStorage::try_new(count)?;
        let snapshot = self.reservation_storage_snapshot_for(count)?;
        let plan = HandleTableStoragePlan::try_new(snapshot)?;
        let mut storage = Some(storage);
        let mut plan = Some(plan);
        self.reserve_batch_with_plan(count, &mut storage, &mut plan)
    }

    /// Reserves a runtime-sized batch from externally prepared storage.
    pub(crate) fn reserve_batch_with_plan(
        &mut self,
        count: usize,
        storage: &mut Option<HandleBatchReservationStorage>,
        plan: &mut Option<HandleTableStoragePlan>,
    ) -> Result<HandleBatchReservation, HandleError> {
        self.ensure_active()?;
        let prepared_storage = match storage.as_ref() {
            Some(storage) => storage,
            None => super::invariant_violation(),
        };
        if prepared_storage.count != count
            || !prepared_storage.slots.is_empty()
            || !prepared_storage.values.is_empty()
        {
            super::invariant_violation();
        }
        let prepared = match plan.as_ref() {
            Some(plan) => plan,
            None => super::invariant_violation(),
        };
        if self.reservation_storage_snapshot_for(count)? != prepared.snapshot() {
            super::invariant_violation();
        }
        let reservation = ReservationId::allocate()?;
        let plan = match plan.take() {
            Some(plan) => plan,
            None => super::invariant_violation(),
        };
        self.install_storage_plan(plan);
        let mut storage = match storage.take() {
            Some(storage) => storage,
            None => super::invariant_violation(),
        };
        for _ in 0..count {
            let (index, generation) = self.reserve_free_slot(reservation);
            storage.slots.push(ReservedSlot { index, generation });
            storage.values.push(HandleValue::encode(index, generation));
        }
        Ok(HandleBatchReservation {
            reservation,
            slots: storage.slots,
            values: storage.values,
            completed: false,
        })
    }

    /// Reserves `N` unresolvable slots for one final publication transaction.
    ///
    /// Existing vacant slots are removed from an intrusive free list, so the
    /// non-allocation path costs O(N) rather than O(total table slots).
    #[cfg(test)]
    pub(crate) fn reserve<const N: usize>(&mut self) -> Result<HandleReservation<N>, HandleError> {
        let snapshot = self.reservation_storage_snapshot_for(N)?;
        let plan = HandleTableStoragePlan::try_new(snapshot)?;
        let mut plan = Some(plan);
        self.reserve_with_plan(&mut plan)
    }

    /// Reserves a fixed-size batch from externally prepared table storage.
    pub(crate) fn reserve_with_plan<const N: usize>(
        &mut self,
        plan: &mut Option<HandleTableStoragePlan>,
    ) -> Result<HandleReservation<N>, HandleError> {
        self.ensure_active()?;
        let prepared = match plan.as_ref() {
            Some(plan) => plan,
            None => super::invariant_violation(),
        };
        if self.reservation_storage_snapshot_for(N)? != prepared.snapshot() {
            super::invariant_violation();
        }
        let reservation = ReservationId::allocate()?;
        let plan = match plan.take() {
            Some(plan) => plan,
            None => super::invariant_violation(),
        };
        self.install_storage_plan(plan);

        let mut selected = [0; N];
        let mut generations = [0; N];
        for position in 0..N {
            let (index, generation) = self.reserve_free_slot(reservation);
            selected[position] = index;
            generations[position] = generation;
        }
        Ok(HandleReservation {
            reservation,
            slots: selected,
            generations,
            completed: false,
        })
    }

    /// Installs a preallocated replacement and grows only within proven capacity.
    fn install_storage_plan(&mut self, plan: HandleTableStoragePlan) {
        let additional = plan.snapshot.additional;
        self.slots.install(plan);
        for _ in 0..additional {
            let index = self.slots.push_vacant(1, self.free_head);
            self.free_head = Some(index);
            self.free_slots += 1;
        }
    }

    fn reserve_free_slot(&mut self, reservation: ReservationId) -> (usize, u64) {
        let Some(index) = self.free_head else {
            super::invariant_violation();
        };
        if self.free_slots == 0 {
            super::invariant_violation();
        }
        let (generation, next_free) = match self.slots.get(index) {
            Some(Slot::Vacant {
                generation,
                next_free,
            }) => (*generation, *next_free),
            _ => super::invariant_violation(),
        };
        self.free_head = next_free;
        self.free_slots -= 1;
        self.slots.replace(
            index,
            Slot::Reserved {
                generation,
                reservation,
            },
        );
        (index, generation)
    }

    fn publish_vacant_slot(&mut self, index: usize, generation: u64) {
        if generation == 0 || generation > GENERATION_LIMIT {
            super::invariant_violation();
        }
        self.slots.replace(
            index,
            Slot::Vacant {
                generation,
                next_free: self.free_head,
            },
        );
        self.free_head = Some(index);
        self.free_slots += 1;
    }

    fn validate_reservation<const N: usize>(&self, token: &HandleReservation<N>) {
        if !matches!(self.lifecycle, TableLifecycle::Active) {
            super::invariant_violation();
        }
        for position in 0..N {
            let index = token.slots[position];
            let generation = token.generations[position];
            if !matches!(
                self.slots.get(index),
                Some(Slot::Reserved {
                    generation: found_generation,
                    reservation,
                }) if *found_generation == generation && *reservation == token.reservation
            ) {
                super::invariant_violation();
            }
        }
    }

    fn abort_reservation<const N: usize>(&mut self, mut token: HandleReservation<N>) {
        self.validate_reservation(&token);
        for position in 0..N {
            let index = token.slots[position];
            let generation = token.generations[position];
            if generation == GENERATION_LIMIT {
                self.slots.replace(index, Slot::Retired);
            } else {
                self.publish_vacant_slot(index, generation + 1);
            }
        }
        token.completed = true;
    }

    fn publish_reservation<const N: usize>(
        &mut self,
        mut token: HandleReservation<N>,
        handles: [PreparedHandle; N],
    ) -> [HandleValue; N] {
        self.validate_reservation(&token);
        let values = token.values();
        for (position, handle) in handles.into_iter().enumerate() {
            let index = token.slots[position];
            let generation = token.generations[position];
            self.slots
                .replace(index, Slot::Occupied { generation, handle });
        }
        token.completed = true;
        values
    }

    #[cfg(test)]
    pub(crate) fn free_list_is_consistent_for_test(&self) -> bool {
        let mut current = self.free_head;
        let mut linked = 0usize;
        while let Some(index) = current {
            if linked >= self.slots.len() {
                return false;
            }
            let Some(Slot::Vacant { next_free, .. }) = self.slots.get(index) else {
                return false;
            };
            current = *next_free;
            linked += 1;
        }
        linked == self.free_slots
            && self
                .slots
                .iter()
                .filter(|slot| matches!(slot, Slot::Vacant { .. }))
                .count()
                == self.free_slots
    }

    #[cfg(test)]
    pub(crate) const fn maximum_generation_for_test() -> u64 {
        GENERATION_LIMIT
    }

    #[cfg(test)]
    pub(crate) fn set_occupied_generation_for_test(
        &mut self,
        value: HandleValue,
        generation: u64,
    ) -> HandleValue {
        if generation == 0 || generation > GENERATION_LIMIT {
            super::invariant_violation();
        }
        let (index, expected) = value.decode();
        match self.slots.get_mut(index) {
            Some(Slot::Occupied {
                generation: found, ..
            }) if *found == expected => *found = generation,
            _ => super::invariant_violation(),
        }
        HandleValue::encode(index, generation)
    }

    pub(crate) fn get_info(&self, value: HandleValue) -> Result<HandleInfo, HandleError> {
        self.ensure_active()?;
        let handle = self.lookup(value)?;
        Ok(HandleInfo {
            koid: handle.object().koid(),
            kind: handle.object().kind(),
            rights: handle.rights,
            flags: handle.flags,
        })
    }

    /// Copies a bounded page of object edges without exposing payload access.
    ///
    /// Every page is coherent under the owning Process lock. A complete scan
    /// is intentionally weakly consistent with concurrent handle mutation;
    /// generation-qualified values prevent a reused slot from aliasing an old
    /// edge. The slot budget also bounds lock hold time for sparse tables.
    pub(crate) fn scan_handles(
        &self,
        cursor: HandleScanCursor,
    ) -> Result<HandleSnapshotPage, HandleError> {
        self.ensure_active()?;
        let mut entries = [None; DIAGNOSTIC_PAGE_CAPACITY];
        let mut len = 0;
        let mut slot = cursor.next_slot.min(self.slots.len());
        let end = slot
            .saturating_add(DIAGNOSTIC_SLOT_BUDGET)
            .min(self.slots.len());
        while slot < end && len < DIAGNOSTIC_PAGE_CAPACITY {
            if let Some(Slot::Occupied { generation, handle }) = self.slots.get(slot) {
                entries[len] = Some(HandleSnapshot {
                    value: HandleValue::encode(slot, *generation),
                    info: HandleInfo {
                        koid: handle.object().koid(),
                        kind: handle.object().kind(),
                        rights: handle.rights,
                        flags: handle.flags,
                    },
                });
                len += 1;
            }
            slot += 1;
        }
        let next = (slot < self.slots.len()).then_some(HandleScanCursor { next_slot: slot });
        Ok(HandleSnapshotPage { entries, len, next })
    }

    /// Resolves authority and clones only an internal object-lifetime reference.
    ///
    /// Process serialization must remain held for this call and may be released
    /// immediately after it returns. A later close does not cancel the resolved
    /// operation and cannot invalidate its object reference.
    pub(crate) fn resolve<T: KernelObject>(
        &self,
        value: HandleValue,
        required: Rights,
    ) -> Result<ResolvedObject<T>, HandleError> {
        self.ensure_active()?;
        let handle = self.lookup(value)?;
        if !handle.rights.contains(required) {
            return Err(HandleError::AccessDenied);
        }
        if handle.object().kind() != T::KIND {
            return Err(HandleError::WrongObjectType);
        }
        Ok(ResolvedObject {
            object: match handle.object().pin::<T>() {
                Some(object) => object,
                None => super::invariant_violation(),
            },
        })
    }

    /// Resolves one object exposing the common level-signal contract.
    pub(crate) fn resolve_waitable(
        &self,
        value: HandleValue,
        required: Rights,
    ) -> Result<ResolvedWaitable, HandleError> {
        self.ensure_active()?;
        let handle = self.lookup(value)?;
        if !handle.rights.contains(required) {
            return Err(HandleError::AccessDenied);
        }
        let object = handle
            .object()
            .pin_waitable()
            .ok_or(HandleError::WrongObjectType)?;
        Ok(ResolvedWaitable { object })
    }

    /// Prepares a duplicate with rights attenuated from the source handle.
    pub(crate) fn duplicate(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<PreparedHandle, HandleError> {
        self.ensure_active()?;
        let source = self.lookup(value)?;
        if !source.rights.contains(Rights::DUPLICATE) {
            return Err(HandleError::AccessDenied);
        }
        if !source.rights.contains(rights) {
            return Err(HandleError::AccessDenied);
        }
        source.try_duplicate(rights)
    }

    /// Reversibly detaches a variable-size set of source handles.
    ///
    /// Validation is complete before the first slot changes. While the claim
    /// exists, an operation on an exact claimed value reports `Busy`; rollback
    /// restores every original value, while commit alone advances generations.
    #[cfg(test)]
    pub(crate) fn prepare_transfer(
        &mut self,
        requests: &[HandleTransferRequest],
        forbidden_object: Option<Koid>,
        forbidden_kind: Option<ObjectKind>,
    ) -> Result<HandleTransferClaim, HandleError> {
        let storage = HandleTransferStorage::try_new(requests.len())?;
        let mut storage = Some(storage);
        self.prepare_transfer_with_storage(requests, forbidden_object, forbidden_kind, &mut storage)
    }

    pub(crate) fn prepare_transfer_with_storage(
        &mut self,
        requests: &[HandleTransferRequest],
        forbidden_object: Option<Koid>,
        forbidden_kind: Option<ObjectKind>,
        storage: &mut Option<HandleTransferStorage>,
    ) -> Result<HandleTransferClaim, HandleError> {
        self.ensure_active()?;
        validate_batch_count(requests.len())?;
        let prepared_storage = match storage.as_ref() {
            Some(storage) => storage,
            None => super::invariant_violation(),
        };
        if prepared_storage.count != requests.len()
            || !prepared_storage.entries.is_empty()
            || !prepared_storage.handles.is_empty()
        {
            super::invariant_violation();
        }

        let transfer = ReservationId::allocate()?;

        // This first pass performs every caller-controlled check without
        // changing the table. It also rejects aliasing source values, which
        // would otherwise make a later detach partially consume one slot.
        for (position, request) in requests.iter().enumerate() {
            if requests[..position]
                .iter()
                .any(|previous| previous.value == request.value)
            {
                return Err(HandleError::InvalidHandle);
            }
            let source = self.lookup(request.value)?;
            if !source.rights.contains(Rights::TRANSFER) || !source.rights.contains(request.rights)
            {
                return Err(HandleError::AccessDenied);
            }
            // Transfer policy is based on the resolved object, not the caller's
            // type assertion. A mismatched `expected_kind` must not disguise a
            // kind which this transport cannot own safely.
            if forbidden_kind == Some(source.object().kind()) {
                return Err(HandleError::UnsupportedTransfer);
            }
            if forbidden_object == Some(source.object().koid()) {
                return Err(HandleError::AccessDenied);
            }
            if request
                .expected_kind
                .is_some_and(|kind| kind != source.object().kind())
            {
                return Err(HandleError::WrongObjectType);
            }
        }

        // Capacity and validation are fixed above. Every exact slot below is
        // still protected by this table's exclusive caller, so detachment is
        // an infallible ownership move rather than a fallible loop.
        let mut storage = match storage.take() {
            Some(storage) => storage,
            None => super::invariant_violation(),
        };
        for request in requests {
            let (index, generation) = request.value.decode();
            let slot = self.slots.replace(index, Slot::Retired);
            let Slot::Occupied { handle, .. } = slot else {
                unreachable_handle_value();
            };
            self.slots.replace(
                index,
                Slot::TransferReserved {
                    generation,
                    transfer,
                },
            );
            storage.entries.push(TransferEntry {
                index,
                generation,
                requested_rights: request.rights,
            });
            storage.handles.push(handle);
        }

        Ok(HandleTransferClaim {
            transfer,
            entries: storage.entries,
            handles: Some(storage.handles),
            completed: false,
        })
    }

    fn validate_transfer(&self, claim: &HandleTransferClaim) {
        if !matches!(self.lifecycle, TableLifecycle::Active) {
            super::invariant_violation();
        }
        if claim.handles.as_ref().map(Vec::len) != Some(claim.entries.len()) {
            super::invariant_violation();
        }
        for entry in &claim.entries {
            if !matches!(
                self.slots.get(entry.index),
                Some(Slot::TransferReserved {
                    generation,
                    transfer,
                }) if *generation == entry.generation && *transfer == claim.transfer
            ) {
                super::invariant_violation();
            }
        }
    }

    fn rollback_transfer(
        &mut self,
        mut claim: HandleTransferClaim,
    ) -> RetiredHandleTransferStorage {
        self.validate_transfer(&claim);
        let mut handles = match claim.handles.take() {
            Some(handles) => handles,
            None => super::invariant_violation(),
        };
        for entry in claim.entries.iter().rev() {
            let handle = match handles.pop() {
                Some(handle) => handle,
                None => super::invariant_violation(),
            };
            self.slots.replace(
                entry.index,
                Slot::Occupied {
                    generation: entry.generation,
                    handle,
                },
            );
        }
        claim.completed = true;
        RetiredHandleTransferStorage {
            _entries: core::mem::take(&mut claim.entries),
            _handles: handles,
        }
    }

    fn commit_transfer(
        &mut self,
        mut claim: HandleTransferClaim,
    ) -> (InTransitHandleBatch, RetiredHandleTransferStorage) {
        self.validate_transfer(&claim);
        let mut handles = match claim.handles.take() {
            Some(handles) => handles,
            None => super::invariant_violation(),
        };
        for (entry, handle) in claim.entries.iter().zip(handles.iter_mut()) {
            handle.rights = entry.requested_rights;
            if entry.generation == GENERATION_LIMIT {
                self.slots.replace(entry.index, Slot::Retired);
            } else {
                self.publish_vacant_slot(entry.index, entry.generation + 1);
            }
        }
        claim.completed = true;
        (
            InTransitHandleBatch {
                handles: Some(handles),
            },
            RetiredHandleTransferStorage {
                _entries: core::mem::take(&mut claim.entries),
                _handles: Vec::new(),
            },
        )
    }

    /// Atomically replaces a source handle with an attenuated new value.
    ///
    /// Allocation and authority validation finish while the source remains
    /// unchanged. The final source removal and destination publication only
    /// move an already-active owner and cannot fail.
    #[cfg(test)]
    pub(crate) fn replace(
        &mut self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, HandleError> {
        let snapshot = self.replace_storage_snapshot(value, rights)?;
        let mut plan = match snapshot {
            Some(snapshot) => Some(HandleTableStoragePlan::try_new(snapshot)?),
            None => None,
        };
        self.replace_with_plan(value, rights, &mut plan)
    }

    pub(crate) fn replace_storage_snapshot(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<Option<HandleTableStorageSnapshot>, HandleError> {
        self.ensure_active()?;
        let source = self.lookup(value)?;
        if !source.rights.contains(rights) {
            return Err(HandleError::AccessDenied);
        }
        let (_, source_generation) = value.decode();
        if source_generation == GENERATION_LIMIT {
            self.reservation_storage_snapshot_for(1).map(Some)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn replace_with_plan(
        &mut self,
        value: HandleValue,
        rights: Rights,
        plan: &mut Option<HandleTableStoragePlan>,
    ) -> Result<HandleValue, HandleError> {
        let expected = self.replace_storage_snapshot(value, rights)?;
        if expected != plan.as_ref().map(HandleTableStoragePlan::snapshot) {
            super::invariant_violation();
        }
        let (source_index, source_generation) = value.decode();

        if source_generation != GENERATION_LIMIT {
            let source = self.slots.replace(source_index, Slot::Retired);
            let Slot::Occupied { mut handle, .. } = source else {
                unreachable_handle_value();
            };
            handle.rights = rights;
            let generation = source_generation + 1;
            self.slots
                .replace(source_index, Slot::Occupied { generation, handle });
            return Ok(HandleValue::encode(source_index, generation));
        }

        // A generation-exhausted slot cannot be reused. Reserve the replacement
        // destination before retiring it so TableFull leaves the source intact.
        let reservation = ReservationId::allocate()?;
        let plan = match plan.take() {
            Some(plan) => plan,
            None => super::invariant_violation(),
        };
        self.install_storage_plan(plan);
        let (destination, generation) = self.reserve_free_slot(reservation);
        let source = self.slots.replace(source_index, Slot::Retired);
        let Slot::Occupied { mut handle, .. } = source else {
            unreachable_handle_value();
        };
        handle.rights = rights;
        self.slots
            .replace(destination, Slot::Occupied { generation, handle });
        Ok(HandleValue::encode(destination, generation))
    }

    /// Detaches one published handle and advances or retires its slot.
    ///
    /// The returned owner remains active and must be completed only after the
    /// caller releases the Process handle-table lock.
    pub(crate) fn remove(&mut self, value: HandleValue) -> Result<ClosedHandle, HandleError> {
        self.ensure_active()?;
        self.remove_active(value)
    }

    fn remove_active(&mut self, value: HandleValue) -> Result<ClosedHandle, HandleError> {
        let (index, generation) = value.decode();
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(HandleError::InvalidHandle)?;
        if matches!(slot, Slot::TransferReserved { generation: found, .. } if *found == generation)
        {
            return Err(HandleError::Busy);
        }
        if !matches!(slot, Slot::Occupied { generation: found, .. } if *found == generation) {
            return Err(HandleError::InvalidHandle);
        }
        let removed = core::mem::replace(slot, Slot::Retired);
        let Slot::Occupied { handle, .. } = removed else {
            unreachable_handle_value();
        };
        if generation != GENERATION_LIMIT {
            self.publish_vacant_slot(index, generation + 1);
        }
        Ok(ClosedHandle {
            handle: Some(handle),
        })
    }

    /// Starts exclusive Process teardown and blocks every new table operation.
    ///
    /// The Process lifecycle must prevent new syscall entry before calling this
    /// method and quiesce every detached slot reservation. The table rejects a
    /// premature transition rather than stranding an armed reservation token.
    /// Once teardown begins, lookup and publication remain blocked until the
    /// cursor has detached every owner and completed retirement.
    pub(crate) fn begin_teardown(&mut self) -> Result<TeardownCursor, HandleError> {
        self.ensure_active()?;
        if self
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Reserved { .. } | Slot::TransferReserved { .. }))
        {
            return Err(HandleError::OutstandingReservation);
        }
        let generation = self.next_teardown_generation;
        self.next_teardown_generation = self.next_teardown_generation.saturating_add(1);
        self.lifecycle = TableLifecycle::TearingDown { generation };
        Ok(TeardownCursor {
            generation,
            next_slot: 0,
            finished: false,
        })
    }

    /// Detaches the next owner in O(number of slots) total teardown work.
    ///
    /// The caller holds the Process table lock only for this method, releases
    /// it before `ClosedHandle::complete`, and then resumes with the same cursor.
    pub(crate) fn remove_next(&mut self, cursor: &mut TeardownCursor) -> Option<ClosedHandle> {
        if !matches!(
            self.lifecycle,
            TableLifecycle::TearingDown { generation } if generation == cursor.generation
        ) || cursor.finished
        {
            super::invariant_violation();
        }
        let index = self
            .slots
            .iter()
            .enumerate()
            .skip(cursor.next_slot)
            .find_map(|(index, slot)| matches!(slot, Slot::Occupied { .. }).then_some(index))?;
        cursor.next_slot = index + 1;
        let generation = match self.slots.get(index) {
            Some(Slot::Occupied { generation, .. }) => *generation,
            _ => unreachable_handle_value(),
        };
        match self.remove_active(HandleValue::encode(index, generation)) {
            Ok(closed) => Some(closed),
            Err(_) => unreachable_handle_value(),
        }
    }

    /// Completes teardown after every detached owner was released out of lock.
    pub(crate) fn finish_teardown(&mut self, mut cursor: TeardownCursor) {
        if !matches!(
            self.lifecycle,
            TableLifecycle::TearingDown { generation } if generation == cursor.generation
        ) || cursor.finished
            || self.slots.iter().any(|slot| {
                matches!(
                    slot,
                    Slot::Reserved { .. } | Slot::TransferReserved { .. } | Slot::Occupied { .. }
                )
            })
        {
            super::invariant_violation();
        }
        self.lifecycle = TableLifecycle::Retired;
        cursor.finished = true;
    }

    /// Moves retired backing storage out for destruction without the table lock.
    pub(crate) fn take_retired_storage(&mut self) -> RetiredHandleStorage {
        if !matches!(self.lifecycle, TableLifecycle::Retired) {
            super::invariant_violation();
        }
        self.free_head = None;
        self.free_slots = 0;
        self.slots.take_retired()
    }

    fn ensure_active(&self) -> Result<(), HandleError> {
        if matches!(self.lifecycle, TableLifecycle::Active) {
            Ok(())
        } else {
            Err(HandleError::TableRetired)
        }
    }

    fn lookup(&self, value: HandleValue) -> Result<&PreparedHandle, HandleError> {
        let (index, generation) = value.decode();
        match self.slots.get(index) {
            Some(Slot::Occupied {
                generation: found,
                handle,
            }) if *found == generation => Ok(handle),
            Some(Slot::TransferReserved {
                generation: found, ..
            }) if *found == generation => Err(HandleError::Busy),
            _ => Err(HandleError::InvalidHandle),
        }
    }
}

impl Default for HandleTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HandleTable {
    fn drop(&mut self) {
        if matches!(self.lifecycle, TableLifecycle::TearingDown { .. })
            || self.slots.iter().any(|slot| {
                matches!(
                    slot,
                    Slot::Reserved { .. } | Slot::TransferReserved { .. } | Slot::Occupied { .. }
                )
            })
        {
            // Process teardown must detach and complete active owners through
            // remove_next instead of recursively dropping table contents.
            super::invariant_violation();
        }
    }
}

struct TransferEntry {
    index: usize,
    generation: u64,
    requested_rights: Rights,
}

/// Exact-count transfer backing prepared before Process locks are acquired.
///
/// Retaining the requested count prevents a later caller from pairing a small
/// allocation with a larger transaction and silently growing either Vec while
/// the handle table is locked.
pub(crate) struct HandleTransferStorage {
    count: usize,
    entries: Vec<TransferEntry>,
    handles: Vec<PreparedHandle>,
}

impl HandleTransferStorage {
    pub(crate) fn validate_count(count: usize) -> Result<(), HandleError> {
        validate_batch_count(count)
    }

    pub(crate) fn try_new(count: usize) -> Result<Self, HandleError> {
        Self::validate_count(count)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| HandleError::Allocation)?;
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(count)
            .map_err(|_| HandleError::Allocation)?;
        Ok(Self {
            count,
            entries,
            handles,
        })
    }
}

/// Reversible ownership of exact claimed source slots.
#[must_use = "commit or roll back the handle transfer claim"]
pub(crate) struct HandleTransferClaim {
    transfer: ReservationId,
    entries: Vec<TransferEntry>,
    handles: Option<Vec<PreparedHandle>>,
    completed: bool,
}

/// Empty transfer-token allocations returned for destruction outside locks.
pub(crate) struct RetiredHandleTransferStorage {
    _entries: Vec<TransferEntry>,
    _handles: Vec<PreparedHandle>,
}

impl HandleTransferClaim {
    pub(crate) const fn entry_allocation_size(count: usize) -> Option<usize> {
        count.checked_mul(core::mem::size_of::<TransferEntry>())
    }

    pub(crate) const fn handle_allocation_size(count: usize) -> Option<usize> {
        count.checked_mul(core::mem::size_of::<PreparedHandle>())
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = HandleValue> + '_ {
        self.entries
            .iter()
            .map(|entry| HandleValue::encode(entry.index, entry.generation))
    }

    #[cfg(test)]
    pub(crate) fn rollback(self, table: &mut HandleTable) {
        drop(table.rollback_transfer(self));
    }

    #[cfg(test)]
    pub(crate) fn commit(self, table: &mut HandleTable) -> InTransitHandleBatch {
        let (handles, retired) = table.commit_transfer(self);
        drop(retired);
        handles
    }

    pub(crate) fn rollback_with_storage(
        self,
        table: &mut HandleTable,
    ) -> RetiredHandleTransferStorage {
        table.rollback_transfer(self)
    }

    pub(crate) fn commit_with_storage(
        self,
        table: &mut HandleTable,
    ) -> (InTransitHandleBatch, RetiredHandleTransferStorage) {
        table.commit_transfer(self)
    }
}

impl Drop for HandleTransferClaim {
    fn drop(&mut self) {
        if !self.completed {
            super::invariant_violation();
        }
    }
}

/// Active capability owners detached from every process-local namespace.
#[must_use = "publish or explicitly release the in-transit handles"]
pub(crate) struct InTransitHandleBatch {
    handles: Option<Vec<PreparedHandle>>,
}

impl InTransitHandleBatch {
    pub(crate) fn from_prepared_handles(handles: Vec<PreparedHandle>) -> Self {
        if handles.is_empty() {
            super::invariant_violation();
        }
        Self {
            handles: Some(handles),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self.handles.as_ref() {
            Some(handles) => handles.len(),
            None => super::invariant_violation(),
        }
    }

    pub(crate) fn into_prepared_handles(mut self) -> Vec<PreparedHandle> {
        match self.handles.take() {
            Some(handles) => handles,
            None => super::invariant_violation(),
        }
    }

    pub(crate) fn release(self) {
        let mut retirement = ObjectRetirement::new();
        self.release_into(&mut retirement);
        retirement.drain();
    }

    pub(crate) fn release_into(mut self, retirement: &mut ObjectRetirement) {
        let handles = match self.handles.take() {
            Some(handles) => handles,
            None => super::invariant_violation(),
        };
        for mut handle in handles {
            handle.release_into(retirement);
        }
    }
}

impl Drop for InTransitHandleBatch {
    fn drop(&mut self) {
        if self.handles.is_some() {
            super::invariant_violation();
        }
    }
}

/// Monotonic progress token for allocation-free Process handle teardown.
#[must_use = "finish Process handle-table teardown"]
pub(crate) struct TeardownCursor {
    generation: u64,
    next_slot: usize,
    finished: bool,
}

impl Drop for TeardownCursor {
    fn drop(&mut self) {
        if !self.finished {
            super::invariant_violation();
        }
    }
}

/// An active handle detached from its Process table.
///
/// `complete` may invoke the object's zero-active callback and therefore must
/// execute after releasing the Process handle-table lock. Dropping an armed
/// owner is an invariant violation rather than an implicit callback site.
#[must_use = "complete removal after releasing the Process handle-table lock"]
pub(crate) struct ClosedHandle {
    handle: Option<PreparedHandle>,
}

impl ClosedHandle {
    pub(crate) fn complete(mut self) {
        let handle = self.handle.take();
        drop(handle);
    }
}

impl Drop for ClosedHandle {
    fn drop(&mut self) {
        if self.handle.is_some() {
            super::invariant_violation();
        }
    }
}

struct ReservedSlot {
    index: usize,
    generation: u64,
}

/// Exact-count runtime reservation storage allocated outside Process locks.
pub(crate) struct HandleBatchReservationStorage {
    count: usize,
    slots: Vec<ReservedSlot>,
    values: Vec<HandleValue>,
}

impl HandleBatchReservationStorage {
    pub(crate) fn validate_count(count: usize) -> Result<(), HandleError> {
        validate_batch_count(count)
    }

    pub(crate) const fn allocation_size(count: usize) -> Option<usize> {
        let slots = match count.checked_mul(core::mem::size_of::<ReservedSlot>()) {
            Some(bytes) => bytes,
            None => return None,
        };
        let values = match count.checked_mul(core::mem::size_of::<HandleValue>()) {
            Some(bytes) => bytes,
            None => return None,
        };
        slots.checked_add(values)
    }

    pub(crate) fn try_new(count: usize) -> Result<Self, HandleError> {
        Self::validate_count(count)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(count)
            .map_err(|_| HandleError::Allocation)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| HandleError::Allocation)?;
        Ok(Self {
            count,
            slots,
            values,
        })
    }
}

/// Runtime-sized unpublished destination slots for IPC receive.
#[must_use = "publish or abort the handle batch reservation"]
pub(crate) struct HandleBatchReservation {
    reservation: ReservationId,
    slots: Vec<ReservedSlot>,
    values: Vec<HandleValue>,
    completed: bool,
}

/// Empty batch-token backing returned for destruction after releasing locks.
pub(crate) struct RetiredHandleBatchReservationStorage {
    _slots: Vec<ReservedSlot>,
    _values: Vec<HandleValue>,
    _handles: Vec<PreparedHandle>,
}

#[cfg(test)]
impl RetiredHandleBatchReservationStorage {
    pub(crate) fn retained_handle_capacity_for_test(&self) -> usize {
        self._handles.capacity()
    }
}

impl HandleBatchReservation {
    pub(crate) fn values(&self) -> &[HandleValue] {
        &self.values
    }

    pub(crate) fn publish(
        mut self,
        table: &mut HandleTable,
        mut handles: Vec<PreparedHandle>,
    ) -> RetiredHandleBatchReservationStorage {
        if handles.len() != self.slots.len() || !matches!(table.lifecycle, TableLifecycle::Active) {
            super::invariant_violation();
        }
        for slot in &self.slots {
            if !matches!(
                table.slots.get(slot.index),
                Some(Slot::Reserved { generation, reservation })
                    if *generation == slot.generation && *reservation == self.reservation
            ) {
                super::invariant_violation();
            }
        }
        // Move owners out in reverse order without consuming the Vec itself.
        // Its now-empty allocation is returned to the caller for destruction
        // after releasing the Process lock.
        for slot in self.slots.iter().rev() {
            let handle = match handles.pop() {
                Some(handle) => handle,
                None => super::invariant_violation(),
            };
            table.slots.replace(
                slot.index,
                Slot::Occupied {
                    generation: slot.generation,
                    handle,
                },
            );
        }
        self.completed = true;
        RetiredHandleBatchReservationStorage {
            _slots: core::mem::take(&mut self.slots),
            _values: core::mem::take(&mut self.values),
            _handles: handles,
        }
    }

    pub(crate) fn abort(mut self, table: &mut HandleTable) -> RetiredHandleBatchReservationStorage {
        if !matches!(table.lifecycle, TableLifecycle::Active) {
            super::invariant_violation();
        }
        for slot in &self.slots {
            if !matches!(
                table.slots.get(slot.index),
                Some(Slot::Reserved { generation, reservation })
                    if *generation == slot.generation && *reservation == self.reservation
            ) {
                super::invariant_violation();
            }
            if slot.generation == GENERATION_LIMIT {
                table.slots.replace(slot.index, Slot::Retired);
            } else {
                table.publish_vacant_slot(slot.index, slot.generation + 1);
            }
        }
        self.completed = true;
        RetiredHandleBatchReservationStorage {
            _slots: core::mem::take(&mut self.slots),
            _values: core::mem::take(&mut self.values),
            _handles: Vec::new(),
        }
    }
}

impl Drop for HandleBatchReservation {
    fn drop(&mut self) {
        if !self.completed {
            super::invariant_violation();
        }
    }
}

/// Linear ownership of slots which cannot yet be resolved by lookup.
///
/// The token deliberately does not borrow the table. A Process may release its
/// table lock while a pinned user-write reservation copies the future numeric
/// values, then reacquire the same table and perform one infallible publish.
/// Every exit path must explicitly publish or abort the token.
#[must_use = "publish or abort the handle-slot reservation"]
pub(crate) struct HandleReservation<const N: usize> {
    reservation: ReservationId,
    slots: [usize; N],
    generations: [u64; N],
    completed: bool,
}

impl<const N: usize> HandleReservation<N> {
    /// Returns the future values while every corresponding slot is unresolved.
    pub(crate) fn values(&self) -> [HandleValue; N] {
        array::from_fn(|position| {
            HandleValue::encode(self.slots[position], self.generations[position])
        })
    }

    /// Publishes all active handles after reacquiring the owning table lock.
    pub(crate) fn publish(
        self,
        table: &mut HandleTable,
        handles: [PreparedHandle; N],
    ) -> [HandleValue; N] {
        table.publish_reservation(self, handles)
    }

    /// Invalidates every future value after reacquiring the owning table lock.
    pub(crate) fn abort(self, table: &mut HandleTable) {
        table.abort_reservation(self);
    }
}

impl<const N: usize> Drop for HandleReservation<N> {
    fn drop(&mut self) {
        if !self.completed {
            super::invariant_violation();
        }
    }
}

/// Type-checked internal object reference retained beyond the handle-table lock.
pub(crate) struct ResolvedObject<T: KernelObject> {
    object: super::super::object::KernelRef<T, OperationPin>,
}

/// Type-erased wait authority retained beyond the handle-table lock.
pub(crate) struct ResolvedWaitable {
    object: ErasedKernelRef<OperationPin>,
}

impl ResolvedWaitable {
    pub(crate) fn source(&self) -> SignalSource<'_> {
        match self.object.signal_source() {
            Some(source) => source,
            None => super::invariant_violation(),
        }
    }

    pub(crate) fn koid(&self) -> Koid {
        self.object.koid()
    }
}

impl<T: KernelObject> ResolvedObject<T> {
    /// Returns the compiler-checked payload reference.
    ///
    /// `HandleTable::resolve` checked type coherence before constructing this
    /// immutable typed kernel reference, so access needs no repeated downcast.
    pub(crate) fn object(&self) -> &T {
        self.object.object()
    }

    pub(crate) fn koid(&self) -> Koid {
        self.object.koid()
    }

    /// Retains the canonical erased owner after typed authority validation.
    pub(crate) fn into_operation_pin(self) -> super::super::object::KernelRef<T, OperationPin> {
        self.object
    }
}
