//! Runtime binding for the linker-provided kernel symbol table.

use core::ptr::addr_of;

#[cfg(not(hyper_embed_kallsyms))]
use hyper::debug::kallsyms::COMPACT_MAGIC;
use hyper::debug::kallsyms::{CompactSymbolTable, SymbolTable};
pub use hyper::debug::kallsyms::{Error, Symbol};

#[used]
#[unsafe(link_section = ".kallsyms")]
#[cfg(hyper_embed_kallsyms)]
static KALLSYMS_STORAGE: [u8; include_bytes!(env!("HYPER_KALLSYMS_BLOB")).len()] =
    *include_bytes!(env!("HYPER_KALLSYMS_BLOB"));

#[used]
#[unsafe(link_section = ".kallsyms")]
#[cfg(not(hyper_embed_kallsyms))]
static KALLSYMS_STORAGE: [u8; COMPACT_MAGIC.len()] = COMPACT_MAGIC;

unsafe extern "C" {
    static __image_start: u8;
    static __image_end: u8;
    static __kallsyms_start: u8;
    static __kallsyms_end: u8;
    static __kallsyms_symbols_start: u8;
    static __kallsyms_symbols_end: u8;
    static __kallsyms_strings_start: u8;
    static __kallsyms_strings_end: u8;
}

/// Resolves a runtime kernel address without allocation or locking.
#[unsafe(export_name = "hyper_kallsyms_lookup")]
pub fn lookup(address: usize) -> Result<Option<Symbol<'static>>, Error> {
    let image_start = addr_of!(__image_start) as usize;
    let image_end = addr_of!(__image_end) as usize;
    let image_size = image_end
        .checked_sub(image_start)
        .ok_or(Error::InvalidSymbolTable)?;
    if let Ok(table) = CompactSymbolTable::new(compact_storage()?, image_start, image_size) {
        return table.lookup_containing(address);
    }
    dynamic_table(image_start, image_size)?.lookup_containing(address)
}

fn compact_storage() -> Result<&'static [u8], Error> {
    let start = addr_of!(__kallsyms_start) as usize;
    let end = addr_of!(__kallsyms_end) as usize;
    let size = end.checked_sub(start).ok_or(Error::InvalidSymbolTable)?;
    // SAFETY: The linker retains this complete immutable range in the
    // permanent read-only kernel image mapping.
    Ok(unsafe { core::slice::from_raw_parts(start as *const u8, size) })
}

fn dynamic_table(image_start: usize, image_size: usize) -> Result<SymbolTable<'static>, Error> {
    let symbols_start = addr_of!(__kallsyms_symbols_start) as usize;
    let symbols_end = addr_of!(__kallsyms_symbols_end) as usize;
    let strings_start = addr_of!(__kallsyms_strings_start) as usize;
    let strings_end = addr_of!(__kallsyms_strings_end) as usize;
    let symbol_size = symbols_end
        .checked_sub(symbols_start)
        .ok_or(Error::InvalidSymbolTable)?;
    let string_size = strings_end
        .checked_sub(strings_start)
        .ok_or(Error::InvalidStringTable)?;

    // SAFETY: These linker ranges are retained in the mapped kernel image and
    // are immutable after the relocation trampoline completes.
    let symbols = unsafe { core::slice::from_raw_parts(symbols_start as *const u8, symbol_size) };
    // SAFETY: The linker emits `.dynstr` beside `.dynsym` in the same permanent
    // read-only image mapping.
    let strings = unsafe { core::slice::from_raw_parts(strings_start as *const u8, string_size) };
    SymbolTable::new(symbols, strings, image_start, image_size)
}
