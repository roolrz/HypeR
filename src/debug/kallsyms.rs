//! Allocation-free lookup over an in-image ELF dynamic symbol table.

const ELF64_SYMBOL_SIZE: usize = 24;
const SHN_UNDEF: u16 = 0;
const STT_FUNC: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    InvalidStringTable,
    InvalidSymbolName,
    InvalidSymbolTable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Symbol<'a> {
    pub name: &'a str,
    pub address: usize,
    pub size: usize,
    pub offset: usize,
}

impl core::fmt::Display for Symbol<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "{}+{:#x}/{:#x}",
            self.name, self.offset, self.size
        )
    }
}

/// A little-endian ELF64 symbol view with an explicit runtime load base.
pub struct SymbolTable<'a> {
    symbols: &'a [u8],
    strings: &'a [u8],
    runtime_base: usize,
    image_size: usize,
}

impl<'a> SymbolTable<'a> {
    pub fn new(
        symbols: &'a [u8],
        strings: &'a [u8],
        runtime_base: usize,
        image_size: usize,
    ) -> Result<Self, Error> {
        if symbols.is_empty() || !symbols.len().is_multiple_of(ELF64_SYMBOL_SIZE) {
            return Err(Error::InvalidSymbolTable);
        }
        if strings.first() != Some(&0) {
            return Err(Error::InvalidStringTable);
        }
        runtime_base
            .checked_add(image_size)
            .ok_or(Error::AddressOverflow)?;
        Ok(Self {
            symbols,
            strings,
            runtime_base,
            image_size,
        })
    }

    /// Resolves an address to the nearest preceding defined function symbol.
    pub fn lookup(&self, address: usize) -> Result<Option<Symbol<'a>>, Error> {
        let Some(relative_address) = address.checked_sub(self.runtime_base) else {
            return Ok(None);
        };
        if relative_address >= self.image_size {
            return Ok(None);
        }

        let mut best: Option<ElfSymbol> = None;
        for bytes in self.symbols.chunks_exact(ELF64_SYMBOL_SIZE) {
            let symbol = ElfSymbol::parse(bytes)?;
            if !symbol.is_function()
                || symbol.value > relative_address as u64
                || symbol.value >= self.image_size as u64
            {
                continue;
            }
            if best.is_none_or(|current| {
                symbol.value > current.value
                    || (symbol.value == current.value && symbol.size > current.size)
            }) {
                best = Some(symbol);
            }
        }

        let Some(best) = best else {
            return Ok(None);
        };
        let name = self.symbol_name(best.name_offset)?;
        let symbol_offset = usize::try_from(best.value).map_err(|_| Error::AddressOverflow)?;
        let symbol_address = self
            .runtime_base
            .checked_add(symbol_offset)
            .ok_or(Error::AddressOverflow)?;
        Ok(Some(Symbol {
            name,
            address: symbol_address,
            size: usize::try_from(best.size).map_err(|_| Error::AddressOverflow)?,
            offset: address - symbol_address,
        }))
    }

    fn symbol_name(&self, offset: u32) -> Result<&'a str, Error> {
        let bytes = self
            .strings
            .get(offset as usize..)
            .ok_or(Error::InvalidSymbolName)?;
        let length = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::InvalidSymbolName)?;
        core::str::from_utf8(&bytes[..length]).map_err(|_| Error::InvalidSymbolName)
    }
}

#[derive(Clone, Copy)]
struct ElfSymbol {
    name_offset: u32,
    info: u8,
    section_index: u16,
    value: u64,
    size: u64,
}

impl ElfSymbol {
    fn parse(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            name_offset: read_u32(bytes, 0)?,
            info: *bytes.get(4).ok_or(Error::InvalidSymbolTable)?,
            section_index: read_u16(bytes, 6)?,
            value: read_u64(bytes, 8)?,
            size: read_u64(bytes, 16)?,
        })
    }

    const fn is_function(self) -> bool {
        self.section_index != SHN_UNDEF && self.info & 0xf == STT_FUNC && self.name_offset != 0
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    let raw: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or(Error::InvalidSymbolTable)?
        .try_into()
        .map_err(|_| Error::InvalidSymbolTable)?;
    Ok(u16::from_le_bytes(raw))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or(Error::InvalidSymbolTable)?
        .try_into()
        .map_err(|_| Error::InvalidSymbolTable)?;
    Ok(u32::from_le_bytes(raw))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let raw: [u8; 8] = bytes
        .get(offset..offset + 8)
        .ok_or(Error::InvalidSymbolTable)?
        .try_into()
        .map_err(|_| Error::InvalidSymbolTable)?;
    Ok(u64::from_le_bytes(raw))
}
