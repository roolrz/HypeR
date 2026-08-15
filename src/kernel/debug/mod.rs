//! Runtime kernel diagnostics.

pub mod kallsyms;

/// Validates the runtime symbol table before diagnostics depend on it.
pub(crate) fn initialize() {
    let lookup_address = kallsyms::lookup as *const () as usize;
    let symbol = match kallsyms::lookup(lookup_address) {
        Ok(Some(symbol)) => symbol,
        Ok(None) => super::boot::fail("kallsyms self lookup", "symbol not found"),
        Err(error) => super::boot::fail("kallsyms self lookup", error),
    };
    if symbol.name != "hyper_kallsyms_lookup" || symbol.offset != 0 {
        super::boot::fail("kallsyms self lookup", symbol);
    }
    crate::println!(
        "HypeR: kallsyms resolved {} at {:#x}",
        symbol.name,
        symbol.address
    );
}
