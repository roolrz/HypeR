// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Level-state observation with generation-qualified scheduler arbitration.

use alloc::boxed::Box;

use hyper::mm::try_box;
use hyper::sync::InterruptSpinLock;

use crate::kernel::accounting::CommittedCharge;
use crate::kernel::task::scheduler::{self, PrepareWait, WaitRegistration};
use crate::kernel::task::{WaitOutcome, WaitQueue, WaitTicket};

type StateLock = InterruptSpinLock<State, crate::hal::irq::LocalMask>;

/// Object-specific signal bits after validation against the object's mask.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SignalMask(u64);

impl SignalMask {
    pub(crate) const EMPTY: Self = Self(0);

    pub(crate) const fn from_bits(bits: u64, supported: Self) -> Option<Self> {
        if bits & !supported.0 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// Constructs bits whose value came from the generated ABI schema.
    pub(crate) const fn from_trusted_bits(bits: u64) -> Self {
        Self(bits)
    }

    pub(crate) const fn bits(self) -> u64 {
        self.0
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Borrowed type-erased access to one object's level-state signals.
///
/// The object retains the stable `SignalState`; this value carries the exact
/// object-specific mask so generic wait code can validate untrusted bits
/// without dispatching on `ObjectKind`.
#[derive(Clone, Copy)]
pub(crate) struct SignalSource<'object> {
    state: &'object SignalState,
    supported: SignalMask,
}

impl<'object> SignalSource<'object> {
    pub(crate) const fn new(state: &'object SignalState, supported: SignalMask) -> Self {
        Self { state, supported }
    }

    pub(super) fn validate(self, raw: u64, allow_empty: bool) -> Option<SignalMask> {
        let signals = SignalMask::from_bits(raw, self.supported)?;
        if !allow_empty && signals.is_empty() {
            return None;
        }
        Some(signals)
    }

    pub(super) const fn state(self) -> &'object SignalState {
        self.state
    }

    pub(super) const fn has_empty_mask(self) -> bool {
        self.supported.is_empty()
    }
}

/// Signal value committed by the exact winning notification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SignalSnapshot {
    signals: SignalMask,
    sequence: u64,
}

impl SignalSnapshot {
    pub(crate) const fn signals(self) -> SignalMask {
        let Self {
            signals,
            sequence: _,
        } = self;
        signals
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignalWaitOutcome {
    Observed(SignalSnapshot),
    TimedOut,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignalWaitError {
    Allocation,
    SequenceExhausted,
    Scheduler(scheduler::Error),
}

impl From<scheduler::Error> for SignalWaitError {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

/// Fallibly allocated but unpublished wait storage.
///
/// Dropping this value is a local abort. Once consumed by `wait_registered`,
/// the `SignalState` owns the node until the exact wait generation completes.
#[must_use = "prepared signal wait storage must be committed or dropped"]
pub(crate) struct PreparedSignalWait {
    waiter: Box<SignalWaiter>,
}

impl PreparedSignalWait {
    pub(crate) const fn allocation_size() -> usize {
        core::mem::size_of::<SignalWaiter>()
    }

    pub(crate) fn try_new(
        requested: SignalMask,
        charge: CommittedCharge,
    ) -> Result<Self, SignalWaitError> {
        let waiter = try_box(SignalWaiter {
            ticket: None,
            requested,
            observed: None,
            next: None,
            _charge: charge,
        })
        .map_err(|_| SignalWaitError::Allocation)?;
        Ok(Self { waiter })
    }
}

struct SignalWaiter {
    // `None` exists only while the fallible preparation is unpublished.
    ticket: Option<WaitTicket>,
    requested: SignalMask,
    observed: Option<SignalSnapshot>,
    next: Option<Box<SignalWaiter>>,
    _charge: CommittedCharge,
}

struct State {
    level: SignalMask,
    sequence: u64,
    park_queue: WaitQueue,
    registrations: Option<Box<SignalWaiter>>,
}

/// Signal state embedded at a stable address in one kernel object.
pub(crate) struct SignalState {
    state: StateLock,
}

impl SignalState {
    pub(super) const fn new() -> Self {
        Self::with_initial_level(SignalMask::EMPTY)
    }

    /// Constructs unpublished signal state with one authoritative initial level.
    pub(crate) const fn with_initial_level(level: SignalMask) -> Self {
        Self {
            state: StateLock::new(State {
                level,
                sequence: 0,
                park_queue: WaitQueue::new(),
                registrations: None,
            }),
        }
    }

    pub(crate) fn update(&self, clear: SignalMask, set: SignalMask) -> Result<(), SignalWaitError> {
        let result = self.state.with(|state| {
            let previous = state.level.0;
            let next_level = SignalMask((previous & !clear.0) | set.0);
            if next_level.0 != previous {
                let next_sequence = state
                    .sequence
                    .checked_add(1)
                    .ok_or(UpdateError::SequenceExhausted)?;
                state.level = next_level;
                state.sequence = next_sequence;
            }
            notify_matching(state).map_err(UpdateError::Scheduler)
        });
        match result {
            Ok(()) => Ok(()),
            Err(UpdateError::SequenceExhausted) => Err(SignalWaitError::SequenceExhausted),
            Err(UpdateError::Scheduler(error)) => signal_scheduler_invariant(error),
        }
    }

    pub(crate) fn observe(&self, requested: SignalMask) -> Option<SignalSnapshot> {
        self.state.with(|state| {
            (!state.level.intersection(requested).is_empty()).then_some(SignalSnapshot {
                signals: state.level,
                sequence: state.sequence,
            })
        })
    }

    #[cfg(feature = "kernel-self-test")]
    pub(super) fn waiter_count(&self) -> usize {
        self.state.with(|state| {
            let mut count = 0usize;
            let mut current = state.registrations.as_deref();
            while let Some(waiter) = current {
                count = count.saturating_add(1);
                current = waiter.next.as_deref();
            }
            count
        })
    }

    #[cfg(feature = "kernel-self-test")]
    pub(super) fn set_sequence_for_test(&self, sequence: u64) {
        self.state.with(|state| {
            if state.registrations.is_some() {
                signal_invariant()
            }
            state.sequence = sequence;
        });
    }

    pub(super) fn wait_registered(
        &self,
        mut prepared: PreparedSignalWait,
        registration: WaitRegistration,
    ) -> Result<SignalWaitOutcome, SignalWaitError> {
        let ticket = registration.ticket();
        prepared.waiter.ticket = Some(ticket);

        // SAFETY: The retained local mask is transferred into a committed
        // park below or dropped before normal execution resumes. The waiter
        // node is linked and the condition is rechecked while this same object
        // lock excludes signal mutation.
        let (park, interrupt_mask) = unsafe {
            self.state.with_mask_retained(|state| {
                link_waiter(state, prepared.waiter);
                if let Err(error) = notify_ticket_if_matching(state, ticket) {
                    // Exact notification has not consumed `registration` on
                    // this error path. Finish it explicitly instead of using
                    // its fail-stop Drop as accidental cleanup.
                    return match scheduler::finish_wait(registration) {
                        Ok(_) => Err(CommitError::Notification(error)),
                        Err(cleanup) => Err(CommitError::Notification(cleanup)),
                    };
                }
                scheduler::prepare_registered_park_locked(&state.park_queue, registration)
                    .map_err(CommitError::Prepare)
            })
        };

        let park = match park {
            Ok(park) => park,
            Err(error) => {
                drop(interrupt_mask);
                let removed = self.state.with(|state| unlink_waiter(state, ticket));
                drop(removed);
                return match error {
                    CommitError::Notification(error) => signal_scheduler_invariant(error),
                    CommitError::Prepare(error) => Err(SignalWaitError::Scheduler(error)),
                };
            }
        };
        let outcome = match park {
            PrepareWait::Park(commit) => {
                scheduler::complete_park(scheduler::retain_park_mask(commit, interrupt_mask))
            }
            PrepareWait::Completed(outcome) => {
                drop(interrupt_mask);
                outcome
            }
        };
        let waiter = self.state.with(|state| unlink_waiter(state, ticket));
        classify_outcome(outcome, waiter.observed)
    }
}

enum CommitError {
    Notification(scheduler::Error),
    Prepare(scheduler::Error),
}

enum UpdateError {
    SequenceExhausted,
    Scheduler(scheduler::Error),
}

fn link_waiter(state: &mut State, mut waiter: Box<SignalWaiter>) {
    let ticket = waiter_ticket(&waiter);
    if find_waiter_mut(&mut state.registrations, ticket).is_some() {
        signal_invariant()
    }
    waiter.next = state.registrations.take();
    state.registrations = Some(waiter);
}

fn unlink_waiter(state: &mut State, ticket: WaitTicket) -> Box<SignalWaiter> {
    let mut link = &mut state.registrations;
    loop {
        let matches = match link.as_ref() {
            Some(waiter) => waiter.ticket == Some(ticket),
            None => signal_invariant(),
        };
        if matches {
            let mut removed = match link.take() {
                Some(waiter) => waiter,
                None => signal_invariant(),
            };
            *link = removed.next.take();
            return removed;
        }
        link = match link.as_mut() {
            Some(waiter) => &mut waiter.next,
            None => signal_invariant(),
        };
    }
}

fn notify_matching(state: &mut State) -> Result<(), scheduler::Error> {
    let level = state.level;
    let sequence = state.sequence;
    let mut current = state.registrations.as_deref_mut();
    while let Some(waiter) = current {
        notify_waiter_if_matching(waiter, level, sequence)?;
        current = waiter.next.as_deref_mut();
    }
    Ok(())
}

fn notify_ticket_if_matching(
    state: &mut State,
    ticket: WaitTicket,
) -> Result<(), scheduler::Error> {
    let level = state.level;
    let sequence = state.sequence;
    let waiter = match find_waiter_mut(&mut state.registrations, ticket) {
        Some(waiter) => waiter,
        None => signal_invariant(),
    };
    notify_waiter_if_matching(waiter, level, sequence)
}

fn notify_waiter_if_matching(
    waiter: &mut SignalWaiter,
    level: SignalMask,
    sequence: u64,
) -> Result<(), scheduler::Error> {
    if waiter.observed.is_some() {
        return Ok(());
    }
    let observed = level.intersection(waiter.requested);
    if observed.is_empty() {
        return Ok(());
    }
    let ticket = waiter_ticket(waiter);
    let snapshot = SignalSnapshot {
        // The requested intersection determines readiness, while the winner
        // observes the object's complete supported level at that instant.
        signals: level,
        sequence,
    };
    let resolution =
        scheduler::notify_registered_with(ticket, || waiter.observed = Some(snapshot))?;
    if resolution.won != waiter.observed.is_some() {
        signal_invariant()
    }
    Ok(())
}

fn find_waiter_mut(
    head: &mut Option<Box<SignalWaiter>>,
    ticket: WaitTicket,
) -> Option<&mut SignalWaiter> {
    let mut current = head.as_deref_mut();
    while let Some(waiter) = current {
        if waiter.ticket == Some(ticket) {
            return Some(waiter);
        }
        current = waiter.next.as_deref_mut();
    }
    None
}

fn waiter_ticket(waiter: &SignalWaiter) -> WaitTicket {
    match waiter.ticket {
        Some(ticket) => ticket,
        None => signal_invariant(),
    }
}

fn classify_outcome(
    outcome: WaitOutcome,
    observed: Option<SignalSnapshot>,
) -> Result<SignalWaitOutcome, SignalWaitError> {
    match (outcome, observed) {
        (WaitOutcome::Notified, Some(snapshot)) => Ok(SignalWaitOutcome::Observed(snapshot)),
        (WaitOutcome::TimedOut, None) => Ok(SignalWaitOutcome::TimedOut),
        (WaitOutcome::Cancelled, None) => Ok(SignalWaitOutcome::Cancelled),
        _ => signal_invariant(),
    }
}

#[cold]
fn signal_invariant() -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: object signal invariant failed"))
}

#[cold]
fn signal_scheduler_invariant(error: scheduler::Error) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: object signal scheduler invariant failed: {error:?}"
    ))
}
