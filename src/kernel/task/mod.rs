// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Thread representation and scheduling policy.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub(crate) mod policy;
pub(crate) mod preempt;
mod reschedule;
pub mod scheduler;
mod sleep;
pub mod thread;
mod timeout;
mod wait;

pub use sleep::{SleepError, sleep_ms, sleep_ns, sleep_s, sleep_until, sleep_us};
pub use timeout::TimedWaitError;
pub(crate) use wait::WaitMobility;
#[cfg(feature = "kernel-self-test")]
pub(crate) use wait::WaitTicket;
pub use wait::{WaitOutcome, WaitQueue};

static READY: AtomicBool = AtomicBool::new(false);

/// Creates the bootstrap scheduling context and initial run queue.
pub(crate) fn initialize() -> Result<(), scheduler::Error> {
    let capabilities = scheduler::initialize()?;
    READY.store(true, Ordering::Release);
    crate::println!(
        "HypeR: scheduler active on bootstrap thread {}",
        capabilities.bootstrap_thread.get()
    );
    Ok(())
}

pub(crate) fn is_ready() -> bool {
    READY.load(Ordering::Acquire)
}
