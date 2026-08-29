// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Opaque process-local handles and unpublished slot reservations.

use alloc::vec::Vec;
use core::array;
use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, Ordering};

use super::object::ActiveHandleError;
use super::{KernelObject, Koid, ObjectKind, ObjectRef, Rights};

const SLOT_BITS: u32 = 24;
const SLOT_MASK: u64 = (1 << SLOT_BITS) - 1;
const GENERATION_LIMIT: u64 = u64::MAX >> SLOT_BITS;
const MAX_SLOTS: usize = SLOT_MASK as usize;

static NEXT_RESERVATION_ID: AtomicU64 = AtomicU64::new(1);

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
    WrongObjectType,
    AccessDenied,
    UnsupportedRights,
    UnsupportedFlags,
    ObjectRetired,
    ObjectAlreadyActive,
    ActiveHandleLimit,
    ReservationIdExhausted,
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

/// One active but not necessarily published process handle.
///
/// Prepared handles are active authority. This keeps duplicate rollback and
/// future in-transit capability ownership from producing a false zero-active
/// transition. Dropping an unpublished value rolls that authority back and may
/// run the zero-active callback; a potentially final owner must therefore be
/// dropped only after releasing Process and object locks.
pub(crate) struct PreparedHandle {
    object: ObjectRef,
    rights: Rights,
    flags: HandleFlags,
}

impl PreparedHandle {
    /// Mints the sole first handle for a newly constructed object.
    pub(crate) fn try_from_new_object(
        object: ObjectRef,
        rights: Rights,
        flags: HandleFlags,
    ) -> Result<Self, HandleError> {
        if !object.supported_rights().contains(rights) {
            return Err(HandleError::UnsupportedRights);
        }
        if HandleFlags::from_bits(flags.bits()).is_none() {
            return Err(HandleError::UnsupportedFlags);
        }
        object
            .acquire_initial_handle()
            .map_err(|error| match error {
                ActiveHandleError::Retired => HandleError::ObjectRetired,
                ActiveHandleError::AlreadyActive => HandleError::ObjectAlreadyActive,
                ActiveHandleError::CountExhausted => HandleError::ActiveHandleLimit,
            })?;
        Ok(Self {
            object,
            rights,
            flags,
        })
    }

    fn try_duplicate(&self, rights: Rights) -> Result<Self, HandleError> {
        if !self.rights.contains(rights) {
            return Err(HandleError::AccessDenied);
        }
        self.object
            .acquire_additional_handle()
            .map_err(|error| match error {
                ActiveHandleError::Retired => HandleError::ObjectRetired,
                ActiveHandleError::AlreadyActive => super::invariant_violation(),
                ActiveHandleError::CountExhausted => HandleError::ActiveHandleLimit,
            })?;
        Ok(Self {
            object: self.object.clone(),
            rights,
            flags: self.flags,
        })
    }

    #[cfg(test)]
    pub(crate) fn duplicate_for_test(&self, rights: Rights) -> Result<Self, HandleError> {
        self.try_duplicate(rights)
    }
}

impl Drop for PreparedHandle {
    fn drop(&mut self) {
        self.object.release_active_handle();
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
    Occupied {
        generation: u64,
        handle: PreparedHandle,
    },
    Retired,
}

/// Unsynchronized table state owned and locked by one Process.
pub(crate) struct HandleTable {
    slots: Vec<Slot>,
    free_head: Option<usize>,
    free_slots: usize,
    lifecycle: TableLifecycle,
    next_teardown_generation: u64,
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
            slots: Vec::new(),
            free_head: None,
            free_slots: 0,
            lifecycle: TableLifecycle::Active,
            next_teardown_generation: 1,
        }
    }

    /// Reserves `N` unresolvable slots for one final publication transaction.
    ///
    /// Existing vacant slots are removed from an intrusive free list, so the
    /// non-allocation path costs O(N) rather than O(total table slots).
    pub(crate) fn reserve<const N: usize>(&mut self) -> Result<HandleReservation<N>, HandleError> {
        self.ensure_active()?;
        if N == 0 {
            return Err(HandleError::EmptyReservation);
        }
        let reservation = ReservationId::allocate()?;
        let additional = N.saturating_sub(self.free_slots);
        if self.slots.len().saturating_add(additional) > MAX_SLOTS {
            return Err(HandleError::TableFull);
        }
        self.slots
            .try_reserve(additional)
            .map_err(|_| HandleError::Allocation)?;
        for _ in 0..additional {
            let index = self.slots.len();
            self.slots.push(Slot::Vacant {
                generation: 1,
                next_free: self.free_head,
            });
            self.free_head = Some(index);
            self.free_slots += 1;
        }

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
        self.slots[index] = Slot::Reserved {
            generation,
            reservation,
        };
        (index, generation)
    }

    fn publish_vacant_slot(&mut self, index: usize, generation: u64) {
        if generation == 0 || generation > GENERATION_LIMIT {
            super::invariant_violation();
        }
        self.slots[index] = Slot::Vacant {
            generation,
            next_free: self.free_head,
        };
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
                self.slots[index] = Slot::Retired;
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
            self.slots[index] = Slot::Occupied { generation, handle };
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
            koid: handle.object.koid(),
            kind: handle.object.kind(),
            rights: handle.rights,
            flags: handle.flags,
        })
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
        if handle.object.kind() != T::KIND || handle.object.downcast_ref::<T>().is_none() {
            return Err(HandleError::WrongObjectType);
        }
        Ok(ResolvedObject {
            object: handle.object.clone(),
            object_type: PhantomData,
        })
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

    /// Atomically replaces a source handle with an attenuated new value.
    ///
    /// Allocation and authority validation finish while the source remains
    /// unchanged. The final source removal and destination publication only
    /// move an already-active owner and cannot fail.
    pub(crate) fn replace(
        &mut self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, HandleError> {
        self.ensure_active()?;
        let source = self.lookup(value)?;
        if !source.rights.contains(rights) {
            return Err(HandleError::AccessDenied);
        }
        let (source_index, source_generation) = value.decode();

        if source_generation != GENERATION_LIMIT {
            let source = core::mem::replace(&mut self.slots[source_index], Slot::Retired);
            let Slot::Occupied { mut handle, .. } = source else {
                unreachable_handle_value();
            };
            handle.rights = rights;
            let generation = source_generation + 1;
            self.slots[source_index] = Slot::Occupied { generation, handle };
            return Ok(HandleValue::encode(source_index, generation));
        }

        // A generation-exhausted slot cannot be reused. Reserve the replacement
        // destination before retiring it so TableFull leaves the source intact.
        let reservation = self.reserve::<1>()?;
        let source = core::mem::replace(&mut self.slots[source_index], Slot::Retired);
        let Slot::Occupied { mut handle, .. } = source else {
            unreachable_handle_value();
        };
        handle.rights = rights;
        Ok(reservation.publish(self, [handle])[0])
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
            .any(|slot| matches!(slot, Slot::Reserved { .. }))
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
        let relative = self.slots[cursor.next_slot..]
            .iter()
            .position(|slot| matches!(slot, Slot::Occupied { .. }))?;
        let index = cursor.next_slot + relative;
        cursor.next_slot = index + 1;
        let generation = match self.slots[index] {
            Slot::Occupied { generation, .. } => generation,
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
            || self
                .slots
                .iter()
                .any(|slot| matches!(slot, Slot::Reserved { .. } | Slot::Occupied { .. }))
        {
            super::invariant_violation();
        }
        self.lifecycle = TableLifecycle::Retired;
        cursor.finished = true;
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
            || self
                .slots
                .iter()
                .any(|slot| matches!(slot, Slot::Reserved { .. } | Slot::Occupied { .. }))
        {
            // Process teardown must detach and complete active owners through
            // remove_next instead of recursively dropping table contents.
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
    object: ObjectRef,
    object_type: PhantomData<T>,
}

impl<T: KernelObject> ResolvedObject<T> {
    /// Returns the compiler-checked payload reference.
    ///
    /// Type coherence was checked by `HandleTable::resolve` while cloning this
    /// immutable `ObjectRef`. Failure here is therefore a private invariant
    /// violation rather than an error an untrusted caller can cause.
    pub(crate) fn object(&self) -> &T {
        match self.object.downcast_ref::<T>() {
            Some(object) => object,
            None => super::invariant_violation(),
        }
    }

    pub(crate) fn koid(&self) -> Koid {
        self.object.koid()
    }
}
