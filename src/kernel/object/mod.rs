// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Concrete kernel objects and their object-level observation mechanisms.
//!
//! Capability handles grant authority to reach these objects. This subsystem
//! owns payload policy after a handle has been resolved and the Process handle
//! lock has been released.

mod event;
mod signals;
mod wait;

pub(crate) use event::{Event, EventError};
#[cfg(feature = "kernel-self-test")]
pub(crate) use signals::PreparedSignalWait;
pub(crate) use signals::{SignalWaitError, SignalWaitOutcome};
pub(crate) use wait::ObjectWaitError;
