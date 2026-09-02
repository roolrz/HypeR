// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral fatal-crash facade.
//!
//! Crash policy coordinates a single diagnostic owner, freezes other CPUs,
//! publishes debugger-visible snapshots without locks, and renders bounded,
//! allocation-free diagnostics. Architecture entry code supplies snapshots but
//! does not own this policy. Runtime subsystems access only this facade; state
//! publication, reporting, unwinding, and the optional monitor remain private.

mod console;
mod coordination;
mod fixed_text;
#[cfg(CONFIG_CRASH_CONSOLE)]
mod monitor;
mod report;
mod state;
mod supplement;
mod unwind;

pub(crate) use coordination::is_ready;
pub(crate) use coordination::{
    EarlyInitializationError, InitializationError, early_initialize, fatal, fatal_context,
    initialize,
};
// Keep the existing crate-visible diagnostic type path even though current
// callers only propagate InitializationError as a whole.
#[allow(unused_imports)]
pub(crate) use coordination::Prerequisite;
pub use coordination::{is_stop_interrupt, panic, stop_this_cpu};

#[allow(unused_imports)]
pub(crate) use supplement::publish as publish_terminal_supplement;
