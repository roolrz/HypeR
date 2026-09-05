// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-local authority over kernel objects.
//!
//! This module owns generation handles, rights enforcement, and the
//! unpublished-slot transaction. It does not own the shared authority
//! vocabulary, object identity, object payload policy, IPC, process locking,
//! user copies, or architecture entry. A Process will serialize its
//! `HandleTable`; resolved object references remain valid after that lock is
//! released, while authority is checked only during resolution.

mod handle;
mod transfer;

pub(crate) use super::authority::Rights;
#[cfg(test)]
pub(crate) use handle::InTransitHandleBatch;
pub(crate) use handle::{
    ClosedHandle, HANDLE_TABLE_STORAGE_SEGMENTS, HandleBatchReservation,
    HandleBatchReservationStorage, HandleError, HandleFlags, HandleInfo, HandleReservation,
    HandleScanCursor, HandleSidecar, HandleSnapshot, HandleSnapshotPage, HandleTable,
    HandleTableStoragePlan, HandleTableStorageSnapshot, HandleTransferClaim, HandleTransferRequest,
    HandleTransferStorage, HandleValue, PreparedHandle, ResolvedObject, ResolvedWaitable,
    RetiredHandleStorage, TeardownCursor,
};
pub(crate) use transfer::InTransitCapabilities;

/// Stops after a private capability invariant is violated.
///
/// This leaf path must remain allocation-, lock-, and diagnostic-free so it is
/// safe from handle-table, transfer, and teardown contexts.
#[cold]
fn invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
