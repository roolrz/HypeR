// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Manually signalled, level-triggered Event objects.

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::capability::{KernelObject, ObjectKind, ObjectRef, Rights, private};
#[cfg(feature = "kernel-self-test")]
use crate::kernel::task::scheduler::WaitRegistration;

#[cfg(feature = "kernel-self-test")]
use super::signals::{PreparedSignalWait, SignalSnapshot};
use super::signals::{SignalMask, SignalState, SignalWaitError, SignalWaitOutcome};

/// Event operation failure.
///
/// Invalid input, quota failure, and sequence exhaustion are rejected before
/// publication or mutation. A scheduler failure after publication is an
/// internal invariant violation and enters coordinated fail-stop handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EventError {
    AllocationSize,
    InvalidSignals,
    Resource(ResourceError),
    SignalWait(SignalWaitError),
}

impl From<ResourceError> for EventError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<SignalWaitError> for EventError {
    fn from(error: SignalWaitError) -> Self {
        Self::SignalWait(error)
    }
}

/// A userspace-controlled level-triggered event.
///
/// Event state is independent of handle ownership. In particular, closing the
/// last active handle does not cancel an operation which already resolved and
/// retained an internal object reference.
pub(crate) struct Event {
    signals: SignalState,
    _object_charge: CommittedCharge,
}

impl Event {
    pub(crate) const SIGNALED: SignalMask =
        SignalMask::from_trusted_bits(hyper::abi::native::HYPER_NATIVE_SIGNAL_EVENT_SIGNALED);
    pub(crate) const SUPPORTED_SIGNALS: SignalMask = Self::SIGNALED;

    /// Reserves the persistent object and memory charge before publication.
    pub(crate) fn try_new(domain: &ResourceDomain) -> Result<Self, EventError> {
        let bytes = ObjectRef::allocation_size::<Self>()
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(EventError::AllocationSize)?;
        let charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, bytes)
                    .with(ResourceKind::KernelObjects, 1),
            )?
            .commit();
        Ok(Self {
            signals: SignalState::new(),
            _object_charge: charge,
        })
    }

    /// Atomically clears and asserts Event signals, then notifies every match.
    ///
    /// Overlap is rejected instead of assigning an implicit clear/set order.
    /// This keeps the public operation strict and makes every accepted update
    /// have one unambiguous value.
    pub(crate) fn signal(&self, clear: u64, set: u64) -> Result<(), EventError> {
        let clear = Self::validate_signals(clear, true).ok_or(EventError::InvalidSignals)?;
        let set = Self::validate_signals(set, true).ok_or(EventError::InvalidSignals)?;
        if clear.intersects(set) {
            return Err(EventError::InvalidSignals);
        }
        self.signals.update(clear, set)?;
        Ok(())
    }

    /// Observes an already-satisfied wait without allocation or scheduler use.
    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn observe(&self, requested: u64) -> Result<Option<SignalSnapshot>, EventError> {
        let requested =
            Self::validate_signals(requested, false).ok_or(EventError::InvalidSignals)?;
        Ok(self.signals.observe(requested))
    }

    /// Fallibly prepares storage before a scheduler wait is armed.
    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn prepare_wait(
        &self,
        requested: u64,
        charge: CommittedCharge,
    ) -> Result<PreparedSignalWait, EventError> {
        let requested =
            Self::validate_signals(requested, false).ok_or(EventError::InvalidSignals)?;
        Ok(PreparedSignalWait::try_new(requested, charge)?)
    }

    /// Commits one already-armed scheduler wait to this Event.
    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn wait_registered(
        &self,
        prepared: PreparedSignalWait,
        registration: WaitRegistration,
    ) -> Result<SignalWaitOutcome, EventError> {
        Ok(self.signals.wait_registered(prepared, registration)?)
    }

    /// Waits for one Event signal, deadline, or Process cancellation outcome.
    pub(crate) fn wait_one(
        &self,
        domain: &ResourceDomain,
        requested: u64,
        deadline_nanoseconds: u64,
        cancellation_requested: impl FnOnce() -> bool,
    ) -> Result<SignalWaitOutcome, super::ObjectWaitError> {
        let requested = Self::validate_signals(requested, false)
            .ok_or(super::ObjectWaitError::InvalidSignals)?;
        super::wait::wait_one(
            &self.signals,
            domain,
            requested,
            deadline_nanoseconds,
            cancellation_requested,
        )
    }

    fn validate_signals(raw: u64, allow_empty: bool) -> Option<SignalMask> {
        let signals = SignalMask::from_bits(raw, Self::SUPPORTED_SIGNALS)?;
        if !allow_empty && signals.is_empty() {
            return None;
        }
        Some(signals)
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn waiter_count(&self) -> usize {
        self.signals.waiter_count()
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn set_sequence_for_test(&self, sequence: u64) {
        self.signals.set_sequence_for_test(sequence)
    }
}

impl private::Sealed for Event {}

impl KernelObject for Event {
    const KIND: ObjectKind = ObjectKind::EVENT;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::SIGNAL);

    // The default zero-handle callback is deliberate. A resolved operation
    // retains object lifetime but not active handle authority, and close does
    // not implicitly cancel it.
}
