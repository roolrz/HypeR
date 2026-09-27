// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process handle resolution, transfer, object publication, and retirement.
//!
//! Table mutations retain the Process-state -> handle-table lock order.
//! Detached owners are released after both locks exit.

use super::super::user_thread::{UserThread, UserThreadObject};
use super::handle_transactions::{
    PreparedHandleConsumption, PreparedProcessHandleTransfer, ProcessHandleReservation,
};
use super::{
    HandleChargeRecord, PreparedTableStorage, Process, ProcessError, install_table_storage_charge,
    process_invariant_violation, require_handle_phase,
};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceKind};
use crate::kernel::capability::{
    ClosedHandle, HandleError, HandleFlags, HandleInfo, HandleScanCursor, HandleSnapshotPage,
    HandleTable, HandleTransferClaim, HandleTransferRequest, HandleTransferRoute,
    HandleTransferStorage, HandleValue, PreparedHandle, ResolvedObject, ResolvedWaitable, Rights,
};
use crate::kernel::object::{KernelObject, Koid, ObjectPublication, UserExportableObject};
use hyper::mm::FallibleArc;

impl Process {
    pub(crate) fn resolve_handle<T: KernelObject>(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<ResolvedObject<T>, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok(self
                .inner
                .handles
                .with(|table| table.resolve(value, rights))?)
        })
    }

    pub(crate) fn resolve_waitable(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<ResolvedWaitable, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok(self
                .inner
                .handles
                .with(|table| table.resolve_waitable(value, rights))?)
        })
    }

    /// Claims source handles without changing their process-local values.
    ///
    /// The returned transaction owns every active capability while exact
    /// lookups report `Busy`. It can either restore the same values or perform
    /// one final generation-advancing move into an in-transit batch.
    pub(crate) fn prepare_handle_transfer(
        &self,
        requests: &[HandleTransferRequest],
        forbidden_object: Option<Koid>,
        forbidden_kind: Option<crate::kernel::object::ObjectKind>,
        route: HandleTransferRoute,
    ) -> Result<PreparedProcessHandleTransfer, ProcessError> {
        HandleTransferStorage::validate_count(requests.len())?;
        let entry_bytes = HandleTransferClaim::entry_allocation_size(requests.len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let handle_bytes = HandleTransferClaim::handle_allocation_size(requests.len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let scratch_bytes = requests
            .len()
            .checked_mul(
                core::mem::size_of::<CommittedCharge>()
                    .saturating_add(core::mem::size_of::<FallibleArc<HandleChargeRecord>>())
                    .saturating_add(core::mem::size_of::<HandleValue>()),
            )
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let entry_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, entry_bytes))?
            .commit();
        let handle_charge = self
            .inner
            .domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, handle_bytes)
                    .with(
                        ResourceKind::IpcHandles,
                        u64::try_from(requests.len()).map_err(|_| ProcessError::Allocation)?,
                    ),
            )?
            .commit();
        let scratch_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, scratch_bytes))?
            .commit();
        let mut storage = Some(HandleTransferStorage::try_new(requests.len())?);
        let mut released_charges = alloc::vec::Vec::new();
        released_charges
            .try_reserve_exact(requests.len())
            .map_err(|_| ProcessError::Allocation)?;
        let mut retired_records = alloc::vec::Vec::new();
        retired_records
            .try_reserve_exact(requests.len())
            .map_err(|_| ProcessError::Allocation)?;
        let mut moved_values = alloc::vec::Vec::new();
        moved_values
            .try_reserve_exact(requests.len())
            .map_err(|_| ProcessError::Allocation)?;
        let claim = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok::<_, ProcessError>(self.inner.handles.with(|table| {
                table.prepare_transfer_with_storage(
                    requests,
                    forbidden_object,
                    forbidden_kind,
                    route,
                    &mut storage,
                )
            })?)
        })?;
        moved_values.extend(claim.values());
        Ok(PreparedProcessHandleTransfer {
            process: self.clone(),
            claim: Some(claim),
            entry_charge: Some(entry_charge),
            handle_charge: Some(handle_charge),
            scratch_charge: Some(scratch_charge),
            released_charges,
            retired_records,
            moved_values,
        })
    }

    /// Reversibly claims one typed handle for consume-on-success lifecycle use.
    ///
    /// Unlike capability transfer, this path requires an object operation
    /// right and no propagation right. `expected_koid` binds the claim to the
    /// object resolved by the syscall adapter before entering the coordinator.
    pub(crate) fn prepare_handle_consumption(
        &self,
        value: HandleValue,
        required: Rights,
        expected_kind: crate::kernel::object::ObjectKind,
        expected_koid: Koid,
    ) -> Result<PreparedHandleConsumption, ProcessError> {
        let entry_bytes = HandleTransferClaim::entry_allocation_size(1)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let handle_bytes = HandleTransferClaim::handle_allocation_size(1)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ProcessError::Allocation)?;
        let scratch_bytes = core::mem::size_of::<CommittedCharge>()
            .saturating_add(core::mem::size_of::<FallibleArc<HandleChargeRecord>>())
            .saturating_add(core::mem::size_of::<HandleValue>());
        let scratch_bytes = u64::try_from(scratch_bytes).map_err(|_| ProcessError::Allocation)?;
        let entry_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, entry_bytes))?
            .commit();
        let handle_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, handle_bytes))?
            .commit();
        let scratch_charge = self
            .inner
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, scratch_bytes))?
            .commit();
        let mut storage = Some(HandleTransferStorage::try_new(1)?);
        let mut released_charges = alloc::vec::Vec::new();
        released_charges
            .try_reserve_exact(1)
            .map_err(|_| ProcessError::Allocation)?;
        let mut retired_records = alloc::vec::Vec::new();
        retired_records
            .try_reserve_exact(1)
            .map_err(|_| ProcessError::Allocation)?;
        let mut moved_values = alloc::vec::Vec::new();
        moved_values
            .try_reserve_exact(1)
            .map_err(|_| ProcessError::Allocation)?;
        let claim = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok::<_, ProcessError>(self.inner.handles.with(|table| {
                table.prepare_consumption_with_storage(
                    value,
                    required,
                    expected_kind,
                    expected_koid,
                    &mut storage,
                )
            })?)
        })?;
        moved_values.push(value);
        Ok(PreparedHandleConsumption {
            transfer: Some(PreparedProcessHandleTransfer {
                process: self.clone(),
                claim: Some(claim),
                entry_charge: Some(entry_charge),
                handle_charge: Some(handle_charge),
                scratch_charge: Some(scratch_charge),
                released_charges,
                retired_records,
                moved_values,
            }),
        })
    }

    /// Publishes the first process-local handle for a new kernel object.
    ///
    /// Slot, quota, object identity, and active-handle state are prepared
    /// before the Process lock commits publication. Every failure before that
    /// point rolls back both the slot reservation and unpublished authority.
    pub(crate) fn create_object<T: UserExportableObject>(
        &self,
        payload: T,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        let reservation = self.reserve_handles::<1>()?;
        self.publish_reserved_object(reservation, payload, rights)
    }

    /// Publishes a new object into a slot reserved before fallible payload
    /// construction began.
    ///
    /// Stateful payload factories use this ordering when dropping an
    /// unpublished payload cannot itself undo an external logical mutation.
    pub(crate) fn publish_reserved_object<T: UserExportableObject>(
        &self,
        reservation: ProcessHandleReservation<1>,
        payload: T,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        let object = match ObjectPublication::try_new(payload) {
            Ok(object) => object,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        let prepared = match PreparedHandle::try_from_new_object(object, rights, HandleFlags::NONE)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        match self.publish_handles(reservation, [prepared]) {
            Ok(values) => Ok(values[0]),
            Err(failure) => Err(failure.error),
        }
    }

    /// Publishes this Process's first userspace authority to an existing thread.
    ///
    /// The thread already has a canonical object identity. This transaction
    /// mints its one initial handle without constructing a second wrapper or
    /// allowing authority resurrection after the final handle closes.
    pub(crate) fn publish_thread_handle(
        &self,
        thread: &UserThread,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        if thread.process_id() != self.id() {
            return Err(ProcessError::Handle(HandleError::AccessDenied));
        }
        let reservation = self.reserve_handles::<1>()?;
        let prepared = match PreparedHandle::try_from_new_object(
            thread.publication(),
            rights,
            HandleFlags::NONE,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        match self.publish_handles(reservation, [prepared]) {
            Ok(values) => Ok(values[0]),
            Err(failure) => Err(failure.error),
        }
    }

    /// Resolves a thread handle into the same canonical kernel object owner.
    pub(crate) fn resolve_user_thread_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<UserThread, ProcessError> {
        let resolved = self.resolve_handle::<UserThreadObject>(value, rights)?;
        Ok(UserThread::from_operation_pin(
            resolved.into_operation_pin(),
        ))
    }

    /// Publishes a same-kind object pair in one handle-table transaction.
    ///
    /// Both erased objects and both active owners exist before publication, so
    /// userspace can never observe only one endpoint of a newly created pair.
    pub(crate) fn create_object_pair<T: UserExportableObject>(
        &self,
        first: T,
        second: T,
        rights: Rights,
    ) -> Result<[HandleValue; 2], ProcessError> {
        let reservation = self.reserve_handles::<2>()?;
        let first = match ObjectPublication::try_new(first) {
            Ok(object) => object,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error.into());
            }
        };
        let second = match ObjectPublication::try_new(second) {
            Ok(object) => object,
            Err(error) => {
                self.abort_handles(reservation);
                drop(first);
                return Err(error.into());
            }
        };
        let first = match PreparedHandle::try_from_new_object(first, rights, HandleFlags::NONE) {
            Ok(handle) => handle,
            Err(error) => {
                self.abort_handles(reservation);
                drop(second);
                return Err(error.into());
            }
        };
        let second = match PreparedHandle::try_from_new_object(second, rights, HandleFlags::NONE) {
            Ok(handle) => handle,
            Err(error) => {
                self.abort_handles(reservation);
                drop(first);
                return Err(error.into());
            }
        };
        match self.publish_handles(reservation, [first, second]) {
            Ok(values) => Ok(values),
            Err(failure) => Err(failure.error),
        }
    }

    pub(crate) fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            let info = self.inner.handles.with(|table| table.get_info(value))?;
            if !info.rights.contains(required_rights) {
                return Err(ProcessError::Handle(HandleError::AccessDenied));
            }
            Ok(info)
        })
    }

    /// Returns one bounded, authority-free page of this Process handle graph.
    pub(crate) fn scan_handles(
        &self,
        cursor: HandleScanCursor,
    ) -> Result<HandleSnapshotPage, ProcessError> {
        self.inner
            .handles
            .with(|table| table.scan_handles(cursor))
            .map_err(Into::into)
    }

    pub(crate) fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        let reservation = self.reserve_handles::<1>()?;
        let prepared = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            Ok::<_, ProcessError>(
                self.inner
                    .handles
                    .with(|table| table.duplicate(value, rights))?,
            )
        });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.abort_handles(reservation);
                return Err(error);
            }
        };
        match self.publish_handles(reservation, [prepared]) {
            Ok(values) => Ok(values[0]),
            Err(failure) => Err(failure.error),
        }
    }

    pub(crate) fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        loop {
            let snapshot = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                Ok::<_, ProcessError>(
                    self.inner
                        .handles
                        .with(|table| table.replace_storage_snapshot(value, rights))?,
                )
            })?;
            let mut storage = match snapshot {
                Some(snapshot) => self.prepare_table_storage_plan(snapshot)?,
                None => PreparedTableStorage::empty(),
            };
            let attempt = self.inner.state.with(|state| {
                require_handle_phase(state.lifecycle.phase())?;
                let current = self
                    .inner
                    .handles
                    .with(|table| table.replace_storage_snapshot(value, rights))?;
                if current != snapshot {
                    return Ok::<_, ProcessError>(None);
                }
                let replacement = self
                    .inner
                    .handles
                    .with(|table| table.replace_with_plan(value, rights, &mut storage.slots))?;
                if let Some(snapshot) = snapshot {
                    install_table_storage_charge(state, snapshot, &mut storage.charge);
                    state.handle_accounting.install_storage(&mut storage.index);
                }
                state.handle_accounting.replace(value, replacement);
                Ok(Some(replacement))
            });
            match attempt {
                Ok(Some(replacement)) => {
                    self.reclaim_handle_pages();
                    return Ok(replacement);
                }
                Ok(None) => drop(storage),
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        let (closed, charge, retired_record): (
            ClosedHandle,
            CommittedCharge,
            Option<FallibleArc<HandleChargeRecord>>,
        ) = self.inner.state.with(|state| {
            require_handle_phase(state.lifecycle.phase())?;
            let closed = self.inner.handles.with(|table| table.remove(value))?;
            let (charge, retired_record) = state.handle_accounting.release(value);
            Ok::<_, ProcessError>((closed, charge, retired_record))
        })?;
        closed.complete();
        drop(charge);
        drop(retired_record);
        self.reclaim_handle_pages();
        Ok(())
    }

    /// Releases complete empty page pairs after namespace transactions leave
    /// their locks. Directory/generation metadata remains charged. A bounded
    /// batch avoids chasing concurrent producers indefinitely; each producer
    /// also drains the pages made empty by its own bounded operation.
    #[inline(never)]
    pub(super) fn reclaim_handle_pages(&self) {
        for _ in 0..64 {
            let detached = self.inner.state.with(|state| {
                let page = self.inner.handles.with(HandleTable::take_empty_page)?;
                let sidecar = state.handle_accounting.detach_empty_page(page.index());
                let amount = ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, page.backing_bytes() as u64);
                let charge = match state.handle_table_charge.as_mut() {
                    Some(total) => total.split_off(amount),
                    None => process_invariant_violation(),
                };
                Some((page, sidecar, charge))
            });
            let Some((page, sidecar, charge)) = detached else {
                break;
            };
            drop(page);
            drop(sidecar);
            // Keep quota conservative until both physical pages are returned.
            drop(charge);
        }
    }
}
