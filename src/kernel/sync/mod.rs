// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-aware synchronization for kernel thread context.

mod mutex;
mod semaphore;

pub use mutex::{Mutex, MutexGuard};
pub use semaphore::Semaphore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Scheduler(super::task::scheduler::Error),
    WouldDeadlock,
    NotOwner,
    PermitOverflow,
}

impl From<super::task::scheduler::Error> for Error {
    fn from(error: super::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}
