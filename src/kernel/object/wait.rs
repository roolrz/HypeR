// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Blocking transaction for object-signal observation.

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::task::scheduler::{self, WaitRegistration};
use crate::kernel::task::{
    ArmedTimeout, PreparedTimeout, TimedWaitError, WaitMobility, WaitOutcome,
};

use super::signals::{
    PreparedSignalWait, SignalMask, SignalState, SignalWaitError, SignalWaitOutcome,
};

/// Failure before a signal, timeout, or cancellation outcome is selected.
#[derive(Debug)]
pub(crate) enum ObjectWaitError {
    AllocationSize,
    Deadline(crate::kernel::time::Error),
    InvalidSignals,
    Resource(ResourceError),
    Signal(SignalWaitError),
    Timer(TimedWaitError),
}

impl From<ResourceError> for ObjectWaitError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<SignalWaitError> for ObjectWaitError {
    fn from(error: SignalWaitError) -> Self {
        Self::Signal(error)
    }
}

impl From<crate::kernel::time::Error> for ObjectWaitError {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Deadline(error)
    }
}

impl From<TimedWaitError> for ObjectWaitError {
    fn from(error: TimedWaitError) -> Self {
        Self::Timer(error)
    }
}

enum WaitDeadline {
    Elapsed,
    Infinite,
    At(u64),
}

impl WaitDeadline {
    fn from_absolute_nanoseconds(nanoseconds: u64) -> Result<Self, ObjectWaitError> {
        if nanoseconds == hyper::abi::native::HYPER_NATIVE_DEADLINE_INFINITE {
            return Ok(Self::Infinite);
        }
        let deadline = crate::kernel::time::deadline_from_monotonic_nanoseconds(nanoseconds)?;
        if hyper::hal::timer::deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
            Ok(Self::Elapsed)
        } else {
            Ok(Self::At(deadline))
        }
    }
}

/// Timer resources allocated and charged before scheduler publication.
enum PreparedWaitTimer {
    Infinite,
    Finite {
        deadline: u64,
        timeout: PreparedTimeout,
        _charge: CommittedCharge,
    },
}

impl PreparedWaitTimer {
    fn try_finite(domain: &ResourceDomain, deadline: u64) -> Result<Self, ObjectWaitError> {
        let bytes = allocation_bytes(PreparedTimeout::allocation_size())?;
        let charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, bytes)
                    .with(ResourceKind::Timers, 1),
            )?
            .commit();
        let timeout = PreparedTimeout::try_new()?;
        Ok(Self::Finite {
            deadline,
            timeout,
            _charge: charge,
        })
    }

    fn arm(self, registration: &WaitRegistration) -> Result<ArmedWaitTimer, TimedWaitError> {
        match self {
            Self::Infinite => Ok(ArmedWaitTimer::Infinite),
            Self::Finite {
                deadline,
                timeout,
                _charge,
            } => Ok(ArmedWaitTimer::Finite {
                timeout: timeout.arm(registration.ticket(), deadline)?,
                _charge,
            }),
        }
    }
}

/// Exact timer ownership retained until the selected wait outcome is known.
enum ArmedWaitTimer {
    Infinite,
    Finite {
        timeout: ArmedTimeout,
        _charge: CommittedCharge,
    },
}

impl ArmedWaitTimer {
    fn retire(self, outcome: WaitOutcome) -> Result<(), TimedWaitError> {
        match self {
            Self::Infinite => Ok(()),
            Self::Finite { timeout, _charge } => timeout.retire_after(outcome),
        }
    }
}

/// Executes one generation-qualified wait from local preparation to retirement.
pub(super) fn wait_one(
    signals: &SignalState,
    domain: &ResourceDomain,
    requested: SignalMask,
    deadline_nanoseconds: u64,
    cancellation_requested: impl FnOnce() -> bool,
) -> Result<SignalWaitOutcome, ObjectWaitError> {
    if let Some(snapshot) = signals.observe(requested) {
        return Ok(SignalWaitOutcome::Observed(snapshot));
    }

    let prepared_timer = match WaitDeadline::from_absolute_nanoseconds(deadline_nanoseconds)? {
        WaitDeadline::Elapsed => return Ok(SignalWaitOutcome::TimedOut),
        WaitDeadline::Infinite => PreparedWaitTimer::Infinite,
        WaitDeadline::At(deadline) => PreparedWaitTimer::try_finite(domain, deadline)?,
    };

    let waiter_charge = reserve_waiter(domain)?;
    let prepared_wait = PreparedSignalWait::try_new(requested, waiter_charge)?;

    scheduler::ensure_sleepable().map_err(SignalWaitError::Scheduler)?;
    let registration =
        scheduler::begin_wait(WaitMobility::Migratable).map_err(SignalWaitError::Scheduler)?;
    let armed_timer = match prepared_timer.arm(&registration) {
        Ok(armed) => armed,
        Err(error) => {
            finish_unpublished_wait(registration)?;
            return Err(error.into());
        }
    };

    // A cancellation preceding scheduler publication could not resolve this
    // ticket. Any later cancellation observes its exact Armed or Queued
    // generation under the scheduler lock.
    if cancellation_requested()
        && let Err(error) = scheduler::resolve_wait(registration.ticket(), WaitOutcome::Cancelled)
    {
        // The armed timer may own a callback-visible raw context. A failed
        // exact resolution leaves its detach state ambiguous, so unwinding is
        // not a valid recovery path.
        crate::kernel::crash::fatal(format_args!(
            "HypeR: object-wait cancellation arbitration failed: {error:?}"
        ));
    }

    let outcome = match signals.wait_registered(prepared_wait, registration) {
        Ok(outcome) => outcome,
        Err(error) => {
            armed_timer.retire(WaitOutcome::Cancelled)?;
            return Err(error.into());
        }
    };
    armed_timer.retire(scheduler_outcome(outcome))?;
    Ok(outcome)
}

fn reserve_waiter(domain: &ResourceDomain) -> Result<CommittedCharge, ObjectWaitError> {
    let bytes = allocation_bytes(PreparedSignalWait::allocation_size())?;
    Ok(domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, bytes))?
        .commit())
}

fn allocation_bytes(bytes: usize) -> Result<u64, ObjectWaitError> {
    u64::try_from(bytes).map_err(|_| ObjectWaitError::AllocationSize)
}

fn finish_unpublished_wait(registration: WaitRegistration) -> Result<(), ObjectWaitError> {
    match scheduler::finish_wait(registration) {
        Ok(None | Some(WaitOutcome::Cancelled)) => Ok(()),
        Ok(Some(outcome)) => object_wait_invariant("unpublished wait resolved", Some(outcome)),
        Err(error) => Err(SignalWaitError::Scheduler(error).into()),
    }
}

const fn scheduler_outcome(outcome: SignalWaitOutcome) -> WaitOutcome {
    match outcome {
        SignalWaitOutcome::Observed(_) => WaitOutcome::Notified,
        SignalWaitOutcome::TimedOut => WaitOutcome::TimedOut,
        SignalWaitOutcome::Cancelled => WaitOutcome::Cancelled,
    }
}

#[cold]
fn object_wait_invariant(message: &str, outcome: Option<WaitOutcome>) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: object wait invariant failed: {message}; outcome={outcome:?}"
    ))
}
