// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-aware synchronization for kernel thread context.

mod completion;
mod mutex;
mod semaphore;

pub use completion::Completion;
pub use mutex::{Mutex, MutexGuard};
pub use semaphore::Semaphore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Scheduler(super::task::scheduler::Error),
    WaitInterrupted(super::task::WaitOutcome),
    WouldDeadlock,
    NotOwner,
    PermitOverflow,
    CompletionOverflow,
}

fn expect_notification(outcome: super::task::WaitOutcome) -> Result<(), Error> {
    if outcome == super::task::WaitOutcome::Notified {
        Ok(())
    } else {
        Err(Error::WaitInterrupted(outcome))
    }
}

impl From<super::task::scheduler::Error> for Error {
    fn from(error: super::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}
