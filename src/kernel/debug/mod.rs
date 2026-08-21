//! Runtime kernel diagnostics.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub mod kallsyms;

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
    crate::println!(
        "HypeR: kallsyms resolved {} at {:#x}",
        symbol.name,
        symbol.address
    );
    Ok(())
}

pub(crate) fn is_ready() -> bool {
    READY.load(Ordering::Acquire)
}
