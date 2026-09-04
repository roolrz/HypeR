// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Durable kernel-object identity for scheduler-owned execution entities.
//!
//! A `ThreadObject` owns only stable identity and observation state. Kernel
//! stacks, architecture contexts, scheduling state, and execution payloads
//! remain in `task::thread::Thread` so retaining an object reference can never
//! retain a terminated Thread's heavyweight execution resources.

use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, KernelRef, ObjectCreationError, ObjectKind, ObjectSnapshot,
    Scheduler as SchedulerReference, private,
};
use crate::kernel::process::UserThread;

pub(super) const THREAD_OBJECT_PAGE_CAPACITY: usize = 32;

/// Stable semantic role of one scheduler entity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThreadRole {
    Bootstrap,
    Idle,
    Kernel,
    User,
    Vcpu,
}

impl ThreadRole {
    pub(crate) const fn execution_kind(self) -> super::thread::ExecutionKind {
        match self {
            Self::Bootstrap | Self::Idle | Self::Kernel => super::thread::ExecutionKind::Kernel,
            Self::User => super::thread::ExecutionKind::User,
            Self::Vcpu => super::thread::ExecutionKind::Vcpu,
        }
    }
}

/// Authority-free identity returned by scheduler diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ThreadObjectSnapshot {
    pub(crate) object: ObjectSnapshot,
    pub(crate) role: ThreadRole,
}

/// Scheduler-registry phase associated with one diagnostic identity record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThreadObjectRegistryPhase {
    Resident,
    /// The identity record remains in the registry while the reaper owns the
    /// detached `Thread`. It is an authority-free generation record, not a
    /// promise that the underlying object is still live after heavy teardown.
    Retiring,
}

/// One scheduler identity to canonical-object diagnostic relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ThreadObjectObservation {
    pub(crate) thread: super::thread::ThreadId,
    pub(crate) object: ThreadObjectSnapshot,
    pub(crate) phase: ThreadObjectRegistryPhase,
}

/// Position in a bounded scheduler Thread-object scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ThreadObjectScanCursor {
    pub(super) next_slot: usize,
}

impl ThreadObjectScanCursor {
    pub(crate) const fn start() -> Self {
        Self { next_slot: 0 }
    }
}

/// One allocation-free page of scheduler-to-object diagnostic relations.
pub(crate) struct ThreadObjectSnapshotPage {
    pub(super) entries: [Option<ThreadObjectObservation>; THREAD_OBJECT_PAGE_CAPACITY],
    pub(super) len: usize,
    pub(super) next: Option<ThreadObjectScanCursor>,
}

impl ThreadObjectSnapshotPage {
    pub(crate) fn entries(&self) -> impl Iterator<Item = &ThreadObjectObservation> {
        self.entries[..self.len].iter().filter_map(Option::as_ref)
    }

    pub(crate) const fn next(&self) -> Option<ThreadObjectScanCursor> {
        self.next
    }
}

pub(super) struct SystemThreadObject {
    role: ThreadRole,
}

impl private::Sealed for SystemThreadObject {}

impl KernelObject for SystemThreadObject {
    const KIND: ObjectKind = ObjectKind::THREAD;
    // Exportability is enforced by the absence of `private::UserExportable`.
    // Rights describe supported operations, not the authority boundary.
    const SUPPORTED_RIGHTS: Rights = Rights::INSPECT;
}

/// The one canonical object owner embedded in every scheduler Thread.
///
/// User Threads retain their existing `UserThreadObject`; the enum wraps that
/// same owner instead of constructing a second identity. System variants are
/// deliberately non-cloneable here so the scheduler owns exactly one durable
/// object reference unless a focused diagnostic operation takes a snapshot.
pub(super) enum ThreadObject {
    System(KernelRef<SystemThreadObject, SchedulerReference>),
    User(UserThread),
}

impl ThreadObject {
    pub(super) fn try_system(role: ThreadRole) -> Result<Self, ObjectCreationError> {
        if role == ThreadRole::User {
            thread_object_invariant_violation();
        }
        KernelRef::try_new_scheduler(SystemThreadObject { role }).map(Self::System)
    }

    pub(super) fn user(thread: UserThread) -> Self {
        Self::User(thread.into_scheduler_owner())
    }

    pub(super) fn snapshot(&self) -> ThreadObjectSnapshot {
        match self {
            Self::System(object) => ThreadObjectSnapshot {
                object: object.snapshot(),
                role: object.object().role,
            },
            Self::User(thread) => ThreadObjectSnapshot {
                object: thread.object_snapshot(),
                role: ThreadRole::User,
            },
        }
    }

    pub(super) fn user_thread(&self) -> Option<&UserThread> {
        match self {
            Self::User(thread) => Some(thread),
            Self::System(_) => None,
        }
    }
}

#[cold]
fn thread_object_invariant_violation() -> ! {
    crate::hal::cpu::halt()
}
