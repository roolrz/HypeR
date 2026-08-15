//! Runtime binding for the linker-provided kernel symbol table.

use core::ptr::addr_of;

use hyper::debug::kallsyms::{COMPACT_MAGIC, CompactSymbolTable, SymbolTable};
pub use hyper::debug::kallsyms::{Error, Symbol};

#[cfg(debug_assertions)]
const KALLSYMS_STORAGE_SIZE: usize = 768 * 1024;
#[cfg(not(debug_assertions))]
const KALLSYMS_STORAGE_SIZE: usize = 128 * 1024;

#[repr(C, align(8))]
struct KallsymsStorage([u8; KALLSYMS_STORAGE_SIZE]);

#[used]
#[unsafe(link_section = ".kallsyms")]
static KALLSYMS_STORAGE: KallsymsStorage = KallsymsStorage(empty_storage());

unsafe extern "C" {
    static __image_start: u8;
    static __image_end: u8;
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
    if let Ok(table) = CompactSymbolTable::new(&KALLSYMS_STORAGE.0, image_start, image_size) {
        return table.lookup_containing(address);
    }
    dynamic_table(image_start, image_size)?.lookup_containing(address)
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

const fn empty_storage() -> [u8; KALLSYMS_STORAGE_SIZE] {
    let mut storage = [0; KALLSYMS_STORAGE_SIZE];
    let mut index = 0;
    while index < COMPACT_MAGIC.len() {
        storage[index] = COMPACT_MAGIC[index];
        index += 1;
    }
    storage
}
