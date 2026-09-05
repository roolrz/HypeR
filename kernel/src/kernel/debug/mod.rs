// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Runtime kernel diagnostics.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub mod kallsyms;
mod object_graph;

static READY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Lookup(kallsyms::Error),
    MissingSelfSymbol,
    UnexpectedSelfSymbol,
}

/// Validates the runtime symbol table before diagnostics depend on it.
pub(crate) fn initialize() -> Result<(), InitializationError> {
    let lookup_address = kallsyms::lookup as *const () as usize;
    let symbol = kallsyms::lookup(lookup_address)
        .map_err(InitializationError::Lookup)?
        .ok_or(InitializationError::MissingSelfSymbol)?;
    if symbol.name != "hyper_kallsyms_lookup" || symbol.offset != 0 {
        return Err(InitializationError::UnexpectedSelfSymbol);
    }
    READY.store(true, Ordering::Release);
    crate::pr_info!(
        "HypeR: kallsyms resolved {} at {:#x}",
        symbol.name,
        symbol.address
    );
    Ok(())
}

/// Reports the initialized runtime ownership graph from normal context.
///
/// Kallsyms initialization precedes scheduler construction, so ownership
/// reporting is a separate startup phase after task and VM publication.
pub(crate) fn report_startup_state() {
    report_object_graph();
}

/// Emits a weakly consistent object/handle graph from normal kernel context.
///
/// This must not be called from fatal or interrupt context: Process handle
/// tables are ordinary live locks, while crash output must remain lock-free.
pub(crate) fn report_object_graph() {
    object_graph::report();
}

pub(crate) fn is_ready() -> bool {
    READY.load(Ordering::Acquire)
}
