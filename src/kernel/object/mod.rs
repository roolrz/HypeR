// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel-object identity, concrete payloads, and level-state observation.
//!
//! Capability handles grant authority to reach these objects. This subsystem
//! owns payload policy after a handle has been resolved and the Process handle
//! lock has been released.

use crate::kernel::authority::Rights;

mod core;
mod directory;
mod event;
mod signals;
mod wait;

#[cfg(test)]
pub(crate) use core::reap_final_objects;
pub(crate) use core::{
    ActiveHandleError, ActiveHandleOwner, Diagnostic, ErasedKernelRef, ExportPolicy, KernelObject,
    KernelRef, KernelService, Koid, ObjectCreationError, ObjectHandleState, ObjectKind,
    ObjectPublication, ObjectReferenceSnapshot, ObjectRetirement, ObjectSnapshot, OperationPin,
    PublishableRef, Scheduler, UserExportableObject, final_reap_pending, object_allocation_size,
    private, reap_one_final_object,
};
pub(crate) use directory::{ObjectScanCursor, scan};
pub(crate) use event::{Event, EventError};
#[cfg(feature = "kernel-self-test")]
pub(crate) use signals::PreparedSignalWait;
pub(crate) use signals::{
    SignalMask, SignalSource, SignalState, SignalWaitError, SignalWaitOutcome,
};
pub(crate) use wait::{ObjectWaitError, wait_one};
