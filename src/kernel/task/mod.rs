// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Thread representation and scheduling policy.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub mod scheduler;
pub mod thread;
mod wait;

pub use wait::WaitQueue;

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
