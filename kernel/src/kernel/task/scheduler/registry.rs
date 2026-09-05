// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Stable scheduler thread identities and fixed-capacity slot storage.
//!
//! Slot backing is allocated to its final length during scheduler startup and
//! is never resized. The transition coordinator owns slot variants and control
//! links, while locked per-CPU domains access only the schedule cells of
//! Threads whose linear residence names that CPU.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use hyper::cpu::CpuIndex;

use super::Error;
use crate::kernel::task::thread::{Thread, ThreadId, ThreadScheduleState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadRegistryStatus {
    Occupied(crate::kernel::task::thread::ExecutionKind),
    Retiring(crate::kernel::task::thread::ExecutionKind),
    Absent,
}

enum ThreadSlot {
    Vacant,
    Reserved(ThreadId),
    Occupied(Box<Thread>),
    /// Identity remains unavailable until lock-external resource teardown ends.
    Retiring {
        id: ThreadId,
        // Snapshotting preserves generation-correct diagnostics without
        // extending object lifetime. The detached Thread carries the actual
        // scheduler reference into the reaper until its resources are gone.
        object: crate::kernel::task::ThreadObjectSnapshot,
        thread: Option<Box<Thread>>,
    },
}

/// One address-stable cell in the scheduler's fixed thread table.
///
/// `UnsafeCell` is deliberately confined to this module. Slot mutation still
/// requires the transition coordinator's private authority, while per-CPU
/// capabilities expose only closure-bounded access to an occupied allocation.
/// No slot reference escapes either authority.
struct ThreadSlotCell(UnsafeCell<ThreadSlot>);

impl ThreadSlotCell {
    const fn new(slot: ThreadSlot) -> Self {
        Self(UnsafeCell::new(slot))
    }
}

/// Fixed scheduler table whose cells and occupied Thread allocations never
/// move while published.
struct ThreadTable {
    slots: Box<[ThreadSlotCell]>,
}

// SAFETY: `ThreadTable` is permanently allocated and its UnsafeCell contents
// are reachable only through private authority values. Slot/control authority
// remains under TransitionLock; each schedule cell is mutated only through
// its matching locked CPU token. These domains access disjoint UnsafeCells and
// no authority exposes a slot reference.
unsafe impl Sync for ThreadTable {}

impl ThreadTable {
    fn slot(&self, index: usize) -> Option<&ThreadSlot> {
        let cell = self.slots.get(index)?;
        // SAFETY: immutable authorities are minted from `&ThreadRegistry`.
        // Every mutable authority requires `&mut ThreadRegistry`, so Rust's
        // borrow rules prevent concurrent mutation in this Phase-A design.
        Some(unsafe { &*cell.0.get() })
    }

    #[allow(clippy::mut_from_ref)]
    fn slot_mut<'authority>(
        &'authority self,
        index: usize,
        _authority: &'authority mut ThreadTableAuthorityToken,
    ) -> Option<&'authority mut ThreadSlot> {
        let cell = self.slots.get(index)?;
        // SAFETY: callers must hold the unique private write-authority token.
        // The returned borrow is closure-bounded by that authority and cannot
        // escape into scheduler state.
        Some(unsafe { &mut *cell.0.get() })
    }

    const fn len(&self) -> usize {
        self.slots.len()
    }
}

/// Non-revocable reference to the permanently allocated thread table.
///
/// This is intentionally private and contains no raw address. It can later be
/// copied into per-CPU scheduler owners, while actual access still requires a
/// linear authority token.
#[derive(Clone, Copy)]
pub(super) struct ThreadTableCapability {
    table: &'static ThreadTable,
}

/// Linear proof that the matching per-CPU scheduler lock is held.
///
/// The token is stored inside `CpuScheduler`; only a mutable borrow obtained
/// while its lock is held can mint a CPU table authority.
pub(super) struct CpuScheduleAuthorityToken {
    _private: (),
}

impl CpuScheduleAuthorityToken {
    pub(super) const fn new() -> Self {
        Self { _private: () }
    }
}

/// Closure-bounded access to schedules owned by one locked CPU domain.
pub(super) struct CpuThreadTableAuthority<'cpu> {
    table: ThreadTableCapability,
    cpu: CpuIndex,
    _token: &'cpu mut CpuScheduleAuthorityToken,
}

impl ThreadTableCapability {
    pub(super) fn cpu_authority<'cpu>(
        self,
        cpu: CpuIndex,
        token: &'cpu mut CpuScheduleAuthorityToken,
    ) -> CpuThreadTableAuthority<'cpu> {
        CpuThreadTableAuthority {
            table: self,
            cpu,
            _token: token,
        }
    }

    pub(super) fn with_thread<R>(
        self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread) -> R,
    ) -> Result<R, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        match self.table.slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => Ok(operation(thread)),
            _ => Err(Error::ThreadNotFound),
        }
    }
}

impl CpuThreadTableAuthority<'_> {
    pub(super) const fn cpu(&self) -> CpuIndex {
        self.cpu
    }

    pub(super) fn with_thread<R>(
        &self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread, &'thread ThreadScheduleState) -> R,
    ) -> Result<R, Error> {
        self.table.with_thread(id, |thread| {
            // SAFETY: construction requires the mutable authority token held
            // inside the matching locked CpuScheduler. Owner is revalidated
            // by Thread before dereferencing its stable schedule cell.
            unsafe { thread.with_cpu_schedule(self.cpu, |schedule| operation(thread, schedule)) }
                .unwrap_or_else(|| registry_invariant())
        })
    }

    pub(super) fn with_thread_mut<R>(
        &mut self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread, &'thread mut ThreadScheduleState) -> R,
    ) -> Result<R, Error> {
        self.table.with_thread(id, |thread| {
            // SAFETY: the unique mutable CPU authority token proves exclusive
            // access to this CPU domain for the closure duration.
            unsafe {
                thread.with_cpu_schedule_mut(self.cpu, |schedule| operation(thread, schedule))
            }
            .unwrap_or_else(|| registry_invariant())
        })
    }
}

#[derive(Clone, Copy)]
enum ThreadTableAccess<'table> {
    Staged(&'table ThreadTable),
    Published(ThreadTableCapability),
}

impl<'table> ThreadTableAccess<'table> {
    const fn table(self) -> &'table ThreadTable {
        match self {
            Self::Staged(table) => table,
            Self::Published(capability) => capability.table,
        }
    }
}

enum ThreadTableStorage {
    Staged(Box<ThreadTable>),
    Published(ThreadTableCapability),
    Publishing,
}

impl ThreadTableStorage {
    fn access(&self) -> ThreadTableAccess<'_> {
        match self {
            Self::Staged(table) => ThreadTableAccess::Staged(table),
            Self::Published(capability) => ThreadTableAccess::Published(*capability),
            Self::Publishing => registry_invariant(),
        }
    }
}

struct ThreadTableAuthorityToken {
    _private: (),
}

/// Closure-bounded read access to address-stable Thread records.
pub(super) struct ThreadTableReadAuthority<'table> {
    table: ThreadTableAccess<'table>,
    _token: &'table ThreadTableAuthorityToken,
}

impl ThreadTableReadAuthority<'_> {
    pub fn with_thread<R>(
        &self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread) -> R,
    ) -> Result<R, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        match self.table.table().slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => Ok(operation(thread)),
            _ => Err(Error::ThreadNotFound),
        }
    }
}

/// Exclusive closure-bounded access to address-stable Thread records.
///
/// The invariant marker prevents accidentally weakening this authority to a
/// shared table borrow when it is later moved into per-CPU scheduler domains.
pub(super) struct ThreadTableWriteAuthority<'table> {
    table: ThreadTableAccess<'table>,
    _token: &'table mut ThreadTableAuthorityToken,
    _exclusive: PhantomData<&'table mut ThreadSlot>,
}

/// Exclusive authority over global waiting/terminated queue topology.
///
/// Control links live outside the movable scheduling domain. This authority
/// therefore needs only the TransitionLock-owned registry token and may
/// update adjacent nodes whose schedules are owned by different CPUs.
pub(super) struct ThreadControlAuthority<'table> {
    table: ThreadTableAccess<'table>,
    _token: &'table mut ThreadTableAuthorityToken,
    _exclusive: PhantomData<&'table mut ThreadSlot>,
}

impl ThreadControlAuthority<'_> {
    pub(super) fn links(
        &self,
        id: ThreadId,
    ) -> Result<crate::kernel::task::thread::QueueLinks, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        match self.table.table().slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => {
                // SAFETY: this authority borrows the registry's control token.
                Ok(unsafe { thread.control_queue_links() })
            }
            _ => Err(Error::ThreadNotFound),
        }
    }

    pub(super) fn with_links_mut<R>(
        &mut self,
        id: ThreadId,
        operation: impl FnOnce(&mut crate::kernel::task::thread::QueueLinks) -> R,
    ) -> Result<R, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        match self.table.table().slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => {
                // SAFETY: this authority exclusively borrows the registry's
                // private control token for the closure duration.
                Ok(unsafe { thread.with_control_queue_links_mut(operation) })
            }
            _ => Err(Error::ThreadNotFound),
        }
    }
}

impl ThreadTableWriteAuthority<'_> {
    pub fn with_thread<R>(
        &self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread) -> R,
    ) -> Result<R, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        match self.table.table().slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => Ok(operation(thread)),
            _ => Err(Error::ThreadNotFound),
        }
    }

    pub fn with_thread_mut<R>(
        &mut self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread mut Thread) -> R,
    ) -> Result<R, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        match self.table.table().slot_mut(slot, &mut *self._token) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => Ok(operation(thread)),
            _ => Err(Error::ThreadNotFound),
        }
    }

    pub(super) fn control_links(
        &self,
        id: ThreadId,
    ) -> Result<crate::kernel::task::thread::QueueLinks, Error> {
        self.with_thread(id, |thread| {
            // SAFETY: write authority is minted only under TransitionLock,
            // which exclusively owns the independent control-link domain.
            unsafe { thread.control_queue_links() }
        })
    }
}

/// Linear ownership of one unpublished registry slot.
#[must_use = "a thread reservation must be published or explicitly abandoned"]
pub(super) struct ThreadReservation {
    id: ThreadId,
    slot: usize,
    cpu: CpuIndex,
    armed: bool,
}

// A reservation is intentionally transferable: it owns no CPU-local machine
// state, and `cpu` is placement data rather than an execution-affinity proof.
// Every consume operation revalidates its complete ThreadId under SCHEDULER.

impl ThreadReservation {
    pub const fn id(&self) -> ThreadId {
        self.id
    }

    pub const fn cpu(&self) -> CpuIndex {
        self.cpu
    }

    pub fn disarm(&mut self) {
        if !self.armed {
            crate::hal::cpu::halt()
        }
        self.armed = false;
    }
}

impl Drop for ThreadReservation {
    fn drop(&mut self) {
        if self.armed {
            crate::hal::cpu::halt()
        }
    }
}

/// Fixed-capacity scheduler registry with never-reused public identities.
pub(super) struct ThreadRegistry {
    // Boxed slice makes the fixed-length, stable-backing contract structural:
    // no registry method can accidentally resize or relocate the slot array.
    table: ThreadTableStorage,
    table_authority: ThreadTableAuthorityToken,
    free_slots: Vec<usize>,
    high_water: usize,
    next_identity: u64,
}

impl ThreadRegistry {
    pub fn new(bootstrap: Box<Thread>, idle: Box<Thread>) -> Result<Self, Error> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(super::THREAD_CAPACITY)
            .map_err(|_| Error::Allocation)?;
        slots.resize_with(super::THREAD_CAPACITY, || {
            ThreadSlotCell::new(ThreadSlot::Vacant)
        });

        let mut free_slots = Vec::new();
        free_slots
            .try_reserve_exact(super::THREAD_CAPACITY.saturating_sub(1))
            .map_err(|_| Error::Allocation)?;

        if bootstrap.id() != ThreadId::BOOTSTRAP
            || bootstrap.id().scheduler_slot() != Some(0)
            || idle.id().scheduler_slot() != Some(1)
        {
            return Err(Error::InvalidThreadState);
        }
        *slots[0].0.get_mut() = ThreadSlot::Occupied(bootstrap);
        *slots[1].0.get_mut() = ThreadSlot::Occupied(idle);
        let table = hyper::mm::try_box(ThreadTable {
            slots: slots.into_boxed_slice(),
        })
        .map_err(|_| Error::Allocation)?;
        Ok(Self {
            table: ThreadTableStorage::Staged(table),
            table_authority: ThreadTableAuthorityToken { _private: () },
            free_slots,
            high_water: 2,
            next_identity: 2,
        })
    }

    pub fn reserve(&mut self, cpu: CpuIndex) -> Result<ThreadReservation, Error> {
        let slot = match self.free_slots.pop() {
            Some(slot) => slot,
            None if self.high_water < self.table.access().table().len() => {
                let slot = self.high_water;
                self.high_water += 1;
                slot
            }
            None => return Err(Error::ThreadLimit),
        };
        if !matches!(
            self.table.access().table().slot(slot),
            Some(ThreadSlot::Vacant)
        ) {
            registry_invariant();
        }

        let Some(id) = ThreadId::from_scheduler_parts(self.next_identity, slot) else {
            self.release_vacant(slot)?;
            return Err(Error::IdentifierExhausted);
        };
        let Some(next_identity) = self.next_identity.checked_add(1) else {
            self.release_vacant(slot)?;
            return Err(Error::IdentifierExhausted);
        };
        self.next_identity = next_identity;
        *self
            .table_slot_mut(slot)
            .unwrap_or_else(|| registry_invariant()) = ThreadSlot::Reserved(id);
        Ok(ThreadReservation {
            id,
            slot,
            cpu,
            armed: true,
        })
    }

    pub fn publish(
        &mut self,
        reservation: &ThreadReservation,
        thread: Box<Thread>,
    ) -> Result<(), (Error, Box<Thread>)> {
        if thread.id() != reservation.id
            || thread.cpu_index() != reservation.cpu
            || !matches!(
                self.table.access().table().slot(reservation.slot),
                Some(ThreadSlot::Reserved(id)) if *id == reservation.id
            )
        {
            return Err((Error::InvalidThreadState, thread));
        }
        *self
            .table_slot_mut(reservation.slot)
            .unwrap_or_else(|| registry_invariant()) = ThreadSlot::Occupied(thread);
        Ok(())
    }

    pub fn abandon(&mut self, reservation: &ThreadReservation) -> Result<(), Error> {
        if !matches!(
            self.table.access().table().slot(reservation.slot),
            Some(ThreadSlot::Reserved(id)) if *id == reservation.id
        ) {
            return Err(Error::InvalidThreadState);
        }
        self.preflight_release(reservation.slot)?;
        *self
            .table_slot_mut(reservation.slot)
            .unwrap_or_else(|| registry_invariant()) = ThreadSlot::Vacant;
        self.commit_release(reservation.slot);
        Ok(())
    }

    pub fn take(&mut self, id: ThreadId) -> Result<Box<Thread>, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        if !matches!(
            self.table.access().table().slot(slot),
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id
        ) {
            return Err(Error::ThreadNotFound);
        }
        if matches!(
            self.table.access().table().slot(slot),
            Some(ThreadSlot::Occupied(thread)) if !thread.schedule_is_coordinator_owned()
        ) {
            return Err(Error::InvalidThreadState);
        }
        // The bootstrap object is retired after kernel initialization, but
        // slot zero is its permanent identity and must never enter the reusable
        // namespace. Every other detached slot is returned to the free list.
        if slot != 0 {
            self.preflight_release(slot)?;
        }
        let slot_ref = self
            .table_slot_mut(slot)
            .unwrap_or_else(|| registry_invariant());
        let ThreadSlot::Occupied(thread) = core::mem::replace(slot_ref, ThreadSlot::Vacant) else {
            registry_invariant();
        };
        if slot != 0 {
            self.commit_release(slot);
        }
        Ok(thread)
    }

    /// Detaches one stopped Thread while retaining its slot as an ABA barrier.
    ///
    /// The allocation stays in the stable slot until the dedicated reaper
    /// takes it. The empty Retiring marker then remains until destruction and
    /// terminal publication have both completed.
    pub fn begin_retirement(&mut self, id: ThreadId) -> Result<(), Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        if !matches!(
            self.table.access().table().slot(slot),
            Some(ThreadSlot::Occupied(thread))
                if thread.id() == id && thread.schedule_is_coordinator_owned()
        ) {
            return Err(Error::InvalidThreadState);
        }
        let slot_ref = self
            .table_slot_mut(slot)
            .unwrap_or_else(|| registry_invariant());
        let ThreadSlot::Occupied(thread) = core::mem::replace(slot_ref, ThreadSlot::Vacant) else {
            registry_invariant();
        };
        let object = thread.object_snapshot();
        *slot_ref = ThreadSlot::Retiring {
            id,
            object,
            thread: Some(thread),
        };
        Ok(())
    }

    /// Transfers one detached allocation while preserving its Retiring marker.
    pub fn take_retiring(&mut self, id: ThreadId) -> Result<Box<Thread>, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        let slot_ref = self
            .table_slot_mut(slot)
            .unwrap_or_else(|| registry_invariant());
        match slot_ref {
            ThreadSlot::Retiring {
                id: retiring,
                thread,
                ..
            } if *retiring == id => thread.take().ok_or(Error::InvalidThreadState),
            _ => Err(Error::InvalidThreadState),
        }
    }

    /// Releases a slot only after every detached resource and publication ends.
    pub fn complete_retirement(&mut self, id: ThreadId) -> Result<(), Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        if !matches!(
            self.table.access().table().slot(slot),
            Some(ThreadSlot::Retiring {
                id: retiring,
                thread: None,
                ..
            }) if *retiring == id
        ) {
            return Err(Error::InvalidThreadState);
        }
        // Bootstrap may legitimately terminate after handing execution to the
        // first long-lived workload. Its reserved slot zero becomes Vacant
        // for observation, but is never admitted to the reusable free list.
        if slot != 0 {
            self.preflight_release(slot)?;
        }
        *self
            .table_slot_mut(slot)
            .unwrap_or_else(|| registry_invariant()) = ThreadSlot::Vacant;
        if slot != 0 {
            self.commit_release(slot);
        }
        Ok(())
    }

    pub fn read_authority(&self) -> ThreadTableReadAuthority<'_> {
        ThreadTableReadAuthority {
            table: self.table.access(),
            _token: &self.table_authority,
        }
    }

    pub fn write_authority(&mut self) -> ThreadTableWriteAuthority<'_> {
        let table = self.table.access();
        ThreadTableWriteAuthority {
            table,
            _token: &mut self.table_authority,
            _exclusive: PhantomData,
        }
    }

    pub(super) fn control_authority(&mut self) -> ThreadControlAuthority<'_> {
        let table = self.table.access();
        ThreadControlAuthority {
            table,
            _token: &mut self.table_authority,
            _exclusive: PhantomData,
        }
    }

    pub fn with_thread<R>(
        &self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread Thread) -> R,
    ) -> Result<R, Error> {
        self.read_authority().with_thread(id, operation)
    }

    pub fn with_thread_mut<R>(
        &mut self,
        id: ThreadId,
        operation: impl for<'thread> FnOnce(&'thread mut Thread) -> R,
    ) -> Result<R, Error> {
        self.write_authority().with_thread_mut(id, operation)
    }

    /// Returns canonical object identity for an occupied or retiring Thread.
    #[cfg(feature = "kernel-self-test")]
    pub fn object_snapshot(
        &self,
        id: ThreadId,
    ) -> Result<crate::kernel::task::ThreadObjectSnapshot, Error> {
        let slot = id.scheduler_slot().ok_or(Error::ThreadNotFound)?;
        let table = self.table.access().table();
        match table.slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => Ok(thread.object_snapshot()),
            Some(ThreadSlot::Retiring {
                id: retiring,
                object,
                ..
            }) if *retiring == id => Ok(*object),
            _ => Err(Error::ThreadNotFound),
        }
    }

    /// Captures one bounded page without allowing Thread or object references
    /// to escape the scheduler authority domain.
    pub fn scan_objects(
        &self,
        cursor: crate::kernel::task::ThreadObjectScanCursor,
    ) -> crate::kernel::task::ThreadObjectSnapshotPage {
        use crate::kernel::task::{
            ThreadObjectObservation, ThreadObjectRegistryPhase, ThreadObjectScanCursor,
            ThreadObjectSnapshotPage,
        };

        let mut entries = [None; crate::kernel::task::thread_object::THREAD_OBJECT_PAGE_CAPACITY];
        let mut len = 0usize;
        let mut index = cursor.next_slot;
        let table = self.table.access().table();
        while index < self.high_water && len < entries.len() {
            let observation = match table.slot(index) {
                Some(ThreadSlot::Occupied(thread)) => Some(ThreadObjectObservation {
                    thread: thread.id(),
                    object: thread.object_snapshot(),
                    phase: ThreadObjectRegistryPhase::Resident,
                }),
                Some(ThreadSlot::Retiring { id, object, .. }) => Some(ThreadObjectObservation {
                    thread: *id,
                    object: *object,
                    phase: ThreadObjectRegistryPhase::Retiring,
                }),
                Some(ThreadSlot::Vacant | ThreadSlot::Reserved(_)) | None => None,
            };
            if let Some(observation) = observation {
                entries[len] = Some(observation);
                len += 1;
            }
            index += 1;
        }
        let next = (index < self.high_water).then_some(ThreadObjectScanCursor { next_slot: index });
        ThreadObjectSnapshotPage { entries, len, next }
    }

    pub fn for_each_thread(&self, mut operation: impl for<'thread> FnMut(&'thread Thread)) {
        for index in 0..self.high_water {
            if let Some(ThreadSlot::Occupied(thread)) = self.table.access().table().slot(index) {
                operation(thread);
            }
        }
    }

    /// Observes one complete generation even while its allocation is detached.
    pub fn status(&self, id: ThreadId) -> ThreadRegistryStatus {
        let Some(slot) = id.scheduler_slot() else {
            return ThreadRegistryStatus::Absent;
        };
        match self.table.access().table().slot(slot) {
            Some(ThreadSlot::Occupied(thread)) if thread.id() == id => {
                ThreadRegistryStatus::Occupied(thread.execution_kind())
            }
            Some(ThreadSlot::Retiring {
                id: retiring,
                object,
                ..
            }) if *retiring == id => ThreadRegistryStatus::Retiring(object.role.execution_kind()),
            Some(
                ThreadSlot::Vacant
                | ThreadSlot::Reserved(_)
                | ThreadSlot::Occupied(_)
                | ThreadSlot::Retiring { .. },
            )
            | None => ThreadRegistryStatus::Absent,
        }
    }

    /// Number of slots ever exposed by the monotonic backing high-water mark.
    #[cfg(feature = "kernel-self-test")]
    pub const fn high_water(&self) -> usize {
        self.high_water
    }

    fn release_vacant(&mut self, slot: usize) -> Result<(), Error> {
        if !matches!(
            self.table.access().table().slot(slot),
            Some(ThreadSlot::Vacant)
        ) {
            return Err(Error::InvalidThreadState);
        }
        self.preflight_release(slot)?;
        self.commit_release(slot);
        Ok(())
    }

    fn preflight_release(&self, slot: usize) -> Result<(), Error> {
        if slot == 0
            || self.free_slots.len() >= self.table.access().table().len().saturating_sub(1)
            || self.free_slots.contains(&slot)
        {
            return Err(Error::InvalidThreadState);
        }
        Ok(())
    }

    fn commit_release(&mut self, slot: usize) {
        self.free_slots.push(slot);
    }

    fn table_slot_mut(&mut self, slot: usize) -> Option<&mut ThreadSlot> {
        let table = self.table.access();
        table.table().slot_mut(slot, &mut self.table_authority)
    }

    /// Permanently publishes the fixed table after every fallible scheduler
    /// construction step has completed.
    pub fn publish_table(&mut self) {
        let storage = core::mem::replace(&mut self.table, ThreadTableStorage::Publishing);
        let ThreadTableStorage::Staged(table) = storage else {
            registry_invariant();
        };
        let table = Box::leak(table);
        self.table = ThreadTableStorage::Published(ThreadTableCapability { table });
    }

    pub(super) fn published_table(&self) -> ThreadTableCapability {
        match self.table {
            ThreadTableStorage::Published(capability) => capability,
            ThreadTableStorage::Staged(_) | ThreadTableStorage::Publishing => registry_invariant(),
        }
    }
}

fn registry_invariant() -> ! {
    // Registry corruption is observed while the scheduler lock is held. Do
    // not enter diagnostics which could reacquire scheduler-adjacent locks.
    crate::hal::cpu::halt()
}
