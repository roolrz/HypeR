// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Best-effort periodic balancing outside IRQ and scheduler hot paths.

use hyper::sync::PublishedOnce;

use super::{SCHEDULER, current_thread_id, publish_committed_ready, scheduler_invariant};
use crate::kernel::time::{Callout, CalloutRef, Error, TimerEvent};

const INTERVAL_NS: u64 = 20_000_000;
static CALLOUT: PublishedOnce<Callout<()>> = PublishedOnce::new();

/// Reuses the boot CPU's timer worker after the complete CPU set is admitted.
/// No balancer Thread or per-tick remote queue scanning is required.
pub(crate) fn initialize() -> Result<(), Error> {
    let count = crate::kernel::cpu::participating_cpu_count().ok_or(Error::NotInitialized)?;
    if count <= 1 {
        return Ok(());
    }
    let callout = Callout::try_new((), balance)?;
    CALLOUT
        .publish(callout)
        .map_err(|_| Error::AlreadyInitialized)?;
    CALLOUT
        .get()
        .ok_or(Error::NotInitialized)?
        .arm_after(INTERVAL_NS)
}

fn balance(_context: &(), callout: CalloutRef<'_>, _event: TimerEvent) {
    let current = match current_thread_id() {
        Ok(current) => current,
        Err(error) => scheduler_invariant("load balancing current Thread", error),
    };
    // Maintenance must not wait for a remote registry reader: its release may
    // depend on a runnable continuation on this very CPU. A contended pass is
    // skipped, and the owned callout provides the next independent attempt.
    if let Some(result) = SCHEDULER.try_with(|slot| {
        slot.as_mut()
            .ok_or(super::Error::NotInitialized)?
            .balance_ready(current)
    }) {
        let outcomes = match result {
            Ok(outcomes) => outcomes,
            Err(error) => scheduler_invariant("load balancing", error),
        };
        for outcome in outcomes.into_iter().flatten() {
            publish_committed_ready(outcome);
        }
    }
    if let Err(error) = callout.arm_after(INTERVAL_NS) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: load balancing callout rearm failed: {error:?}"
        ));
    }
}
