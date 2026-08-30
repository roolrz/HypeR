// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-local authority and kernel-object identity.
//!
//! This module owns object references, rights, generation handles, and the
//! unpublished-slot transaction. It does not own object payload policy, IPC,
//! process locking, user copies, or architecture entry. A Process will
//! serialize its `HandleTable`; resolved object references remain valid after
//! that lock is released, while authority is checked only during resolution.

mod handle;
mod object;
mod rights;

pub(crate) use handle::{
    ClosedHandle, HandleError, HandleFlags, HandleInfo, HandleReservation, HandleTable,
    HandleValue, PreparedHandle, ResolvedObject, RetiredHandleStorage, TeardownCursor,
};
pub(crate) use object::{KernelObject, Koid, ObjectCreationError, ObjectKind, ObjectRef, private};
pub(crate) use rights::Rights;

/// Stops after a private capability invariant is violated.
///
/// This leaf path must remain allocation-, lock-, and diagnostic-free so it is
/// safe from refcount, process-table, and teardown contexts.
#[cold]
fn invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
