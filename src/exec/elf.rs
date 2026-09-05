// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Strict ELF64 load planning for `HypeR` Native processes.

use alloc::vec::Vec;

use crate::mm::PAGE_SIZE;

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const DYNAMIC_ENTRY_SIZE: usize = 16;
const RELA_ENTRY_SIZE: usize = 24;
const RELR_ENTRY_SIZE: usize = 8;
const MAXIMUM_PROGRAM_HEADERS: usize = 128;
const MAXIMUM_RELOCATIONS: usize = 1_048_576;

const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_AARCH64: u16 = 183;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_TLS: u32 = 7;
const PT_GNU_STACK: u32 = 0x6474_e551;

const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ: u32 = 4;
const PF_MASK: u32 = PF_EXECUTE | PF_WRITE | PF_READ;

const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_PLTRELSZ: i64 = 2;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_RELAENT: i64 = 9;
const DT_REL: i64 = 17;
const DT_RELSZ: i64 = 18;
const DT_RELENT: i64 = 19;
const DT_TEXTREL: i64 = 22;
const DT_JMPREL: i64 = 23;
const DT_RELRSZ: i64 = 35;
const DT_RELR: i64 = 36;
const DT_RELRENT: i64 = 37;

const R_AARCH64_RELATIVE: u32 = 1027;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    ArithmeticOverflow,
    DuplicateDynamicEntry,
    DuplicateRelocation,
    ExecutableStack,
    InvalidAlignment,
    InvalidDynamicTable,
    InvalidEntry,
    InvalidHeader,
    InvalidLoadSegment,
    InvalidMagic,
    InvalidRelocation,
    OverlappingLoadSegments,
    TooManyProgramHeaders,
    TooManyRelocations,
    Truncated,
    UnsupportedClass,
    UnsupportedDataEncoding,
    UnsupportedFileType,
    UnsupportedInterpreter,
    UnsupportedMachine,
    UnsupportedOperatingSystemAbi,
    UnsupportedAbiVersion,
    UnsupportedRelocation,
    UnsupportedTls,
    WritableExecutableSegment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageKind {
    Executable,
    PositionIndependent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Machine {
    Aarch64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentPermissions {
    read: bool,
    write: bool,
    execute: bool,
}

impl SegmentPermissions {
    pub const fn readable(self) -> bool {
        self.read
    }

    pub const fn writable(self) -> bool {
        self.write
    }

    pub const fn executable(self) -> bool {
        self.execute
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadSegment<'image> {
    mapping_address: u64,
    mapping_size: u64,
    data_offset: u64,
    memory_address: u64,
    memory_size: u64,
    data: &'image [u8],
    permissions: SegmentPermissions,
}

impl<'image> LoadSegment<'image> {
    pub const fn mapping_address(self) -> u64 {
        self.mapping_address
    }

    pub const fn mapping_size(self) -> u64 {
        self.mapping_size
    }

    pub const fn data_offset(self) -> u64 {
        self.data_offset
    }

    pub const fn memory_address(self) -> u64 {
        self.memory_address
    }

    pub const fn memory_size(self) -> u64 {
        self.memory_size
    }

    pub const fn data(self) -> &'image [u8] {
        self.data
    }

    pub const fn permissions(self) -> SegmentPermissions {
        self.permissions
    }

    fn contains_memory(self, address: u64, length: u64) -> bool {
        let Some(end) = address.checked_add(length) else {
            return false;
        };
        let Some(segment_end) = self.memory_address.checked_add(self.memory_size) else {
            return false;
        };
        self.memory_address <= address && end <= segment_end
    }

    fn file_slice(self, address: u64, length: usize) -> Option<&'image [u8]> {
        let offset = address.checked_sub(self.memory_address)?;
        let offset = usize::try_from(offset).ok()?;
        let end = offset.checked_add(length)?;
        self.data.get(offset..end)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Relocation {
    Relative { target: u64, addend: i64 },
    RelativeInPlace { target: u64 },
}

impl Relocation {
    pub const fn target(self) -> u64 {
        match self {
            Self::Relative { target, .. } | Self::RelativeInPlace { target } => target,
        }
    }
}

pub struct Image<'image> {
    kind: ImageKind,
    machine: Machine,
    entry: u64,
    segments: Vec<LoadSegment<'image>>,
    relocations: Vec<Relocation>,
}

impl<'image> Image<'image> {
    pub fn parse(bytes: &'image [u8]) -> Result<Self, Error> {
        let header = bytes.get(..ELF_HEADER_SIZE).ok_or(Error::Truncated)?;
        validate_ident(header)?;
        let kind = match read_u16(header, 16)? {
            ET_EXEC => ImageKind::Executable,
            ET_DYN => ImageKind::PositionIndependent,
            _ => return Err(Error::UnsupportedFileType),
        };
        let machine = match read_u16(header, 18)? {
            EM_AARCH64 => Machine::Aarch64,
            _ => return Err(Error::UnsupportedMachine),
        };
        if read_u32(header, 20)? != 1
            || read_u32(header, 48)? != 0
            || usize::from(read_u16(header, 52)?) != ELF_HEADER_SIZE
            || usize::from(read_u16(header, 54)?) != PROGRAM_HEADER_SIZE
        {
            return Err(Error::InvalidHeader);
        }
        let entry = read_u64(header, 24)?;
        let program_offset =
            usize::try_from(read_u64(header, 32)?).map_err(|_| Error::ArithmeticOverflow)?;
        let program_count = usize::from(read_u16(header, 56)?);
        if program_count == 0 || program_count > MAXIMUM_PROGRAM_HEADERS {
            return Err(Error::TooManyProgramHeaders);
        }
        let program_size = program_count
            .checked_mul(PROGRAM_HEADER_SIZE)
            .ok_or(Error::ArithmeticOverflow)?;
        let program_end = program_offset
            .checked_add(program_size)
            .ok_or(Error::ArithmeticOverflow)?;
        bytes
            .get(program_offset..program_end)
            .ok_or(Error::Truncated)?;

        let mut segments = Vec::new();
        segments
            .try_reserve_exact(program_count)
            .map_err(|_| Error::Allocation)?;
        let mut dynamic = None;
        for index in 0..program_count {
            let offset = program_offset + index * PROGRAM_HEADER_SIZE;
            let header = &bytes[offset..offset + PROGRAM_HEADER_SIZE];
            let header_type = read_u32(header, 0)?;
            let flags = read_u32(header, 4)?;
            match header_type {
                PT_LOAD => {
                    if let Some(segment) = parse_load_segment(bytes, header, flags)? {
                        segments.push(segment);
                    }
                }
                PT_DYNAMIC => {
                    if dynamic.is_some() {
                        return Err(Error::InvalidDynamicTable);
                    }
                    dynamic = Some(program_data(bytes, header)?);
                }
                PT_INTERP => return Err(Error::UnsupportedInterpreter),
                PT_TLS if read_u64(header, 40)? != 0 => return Err(Error::UnsupportedTls),
                PT_GNU_STACK if flags & PF_EXECUTE != 0 => return Err(Error::ExecutableStack),
                _ => {}
            }
        }
        if segments.is_empty() {
            return Err(Error::InvalidLoadSegment);
        }
        segments.sort_unstable_by_key(|segment| segment.mapping_address);
        validate_segment_layout(&segments, entry)?;
        let relocations = match dynamic {
            Some(dynamic) => parse_dynamic(dynamic, &segments)?,
            None => Vec::new(),
        };
        Ok(Self {
            kind,
            machine,
            entry,
            segments,
            relocations,
        })
    }

    pub const fn kind(&self) -> ImageKind {
        self.kind
    }

    pub const fn machine(&self) -> Machine {
        self.machine
    }

    pub const fn entry(&self) -> u64 {
        self.entry
    }

    pub fn segments(&self) -> impl ExactSizeIterator<Item = LoadSegment<'image>> + '_ {
        self.segments.iter().copied()
    }

    pub fn relocations(&self) -> impl ExactSizeIterator<Item = Relocation> + '_ {
        self.relocations.iter().copied()
    }

    pub fn minimum_mapping_address(&self) -> u64 {
        self.segments[0].mapping_address
    }

    pub fn maximum_mapping_address(&self) -> u64 {
        self.segments
            .last()
            .and_then(|segment| segment.mapping_address.checked_add(segment.mapping_size))
            .unwrap_or(u64::MAX)
    }
}

fn validate_ident(header: &[u8]) -> Result<(), Error> {
    if header.get(..4) != Some(b"\x7fELF") {
        return Err(Error::InvalidMagic);
    }
    if header[4] != 2 {
        return Err(Error::UnsupportedClass);
    }
    if header[5] != 1 {
        return Err(Error::UnsupportedDataEncoding);
    }
    if header[6] != 1 {
        return Err(Error::InvalidHeader);
    }
    if u64::from(header[7]) != crate::abi::native::HYPER_NATIVE_ELF_OSABI {
        return Err(Error::UnsupportedOperatingSystemAbi);
    }
    if u64::from(header[8]) != crate::abi::native::HYPER_NATIVE_ELF_ABI_VERSION {
        return Err(Error::UnsupportedAbiVersion);
    }
    Ok(())
}

fn parse_load_segment<'image>(
    bytes: &'image [u8],
    header: &[u8],
    flags: u32,
) -> Result<Option<LoadSegment<'image>>, Error> {
    if flags & !PF_MASK != 0 || flags & PF_READ == 0 {
        return Err(Error::InvalidLoadSegment);
    }
    if flags & PF_WRITE != 0 && flags & PF_EXECUTE != 0 {
        return Err(Error::WritableExecutableSegment);
    }
    let file_offset = read_u64(header, 8)?;
    let virtual_address = read_u64(header, 16)?;
    let file_size = read_u64(header, 32)?;
    let memory_size = read_u64(header, 40)?;
    let alignment = read_u64(header, 48)?;
    if memory_size == 0 {
        return Ok(None);
    }
    if file_size > memory_size {
        return Err(Error::InvalidLoadSegment);
    }
    if alignment > 1
        && (!alignment.is_power_of_two() || virtual_address % alignment != file_offset % alignment)
    {
        return Err(Error::InvalidAlignment);
    }
    if virtual_address % PAGE_SIZE != file_offset % PAGE_SIZE {
        return Err(Error::InvalidAlignment);
    }
    virtual_address
        .checked_add(memory_size)
        .ok_or(Error::ArithmeticOverflow)?;
    let file_start = usize::try_from(file_offset).map_err(|_| Error::ArithmeticOverflow)?;
    let file_length = usize::try_from(file_size).map_err(|_| Error::ArithmeticOverflow)?;
    let file_end = file_start
        .checked_add(file_length)
        .ok_or(Error::ArithmeticOverflow)?;
    let data = bytes.get(file_start..file_end).ok_or(Error::Truncated)?;
    let mapping_address = align_down(virtual_address);
    let data_offset = virtual_address - mapping_address;
    let mapping_size = align_up(
        data_offset
            .checked_add(memory_size)
            .ok_or(Error::ArithmeticOverflow)?,
    )?;
    Ok(Some(LoadSegment {
        mapping_address,
        mapping_size,
        data_offset,
        memory_address: virtual_address,
        memory_size,
        data,
        permissions: SegmentPermissions {
            read: true,
            write: flags & PF_WRITE != 0,
            execute: flags & PF_EXECUTE != 0,
        },
    }))
}

fn validate_segment_layout(segments: &[LoadSegment<'_>], entry: u64) -> Result<(), Error> {
    let mut previous_end = 0u64;
    for segment in segments {
        if segment.mapping_address < previous_end {
            return Err(Error::OverlappingLoadSegments);
        }
        previous_end = segment
            .mapping_address
            .checked_add(segment.mapping_size)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if !segments
        .iter()
        .any(|segment| segment.permissions.execute && segment.contains_memory(entry, 1))
    {
        return Err(Error::InvalidEntry);
    }
    Ok(())
}

fn program_data<'image>(bytes: &'image [u8], header: &[u8]) -> Result<&'image [u8], Error> {
    let offset = usize::try_from(read_u64(header, 8)?).map_err(|_| Error::ArithmeticOverflow)?;
    let length = usize::try_from(read_u64(header, 32)?).map_err(|_| Error::ArithmeticOverflow)?;
    let end = offset
        .checked_add(length)
        .ok_or(Error::ArithmeticOverflow)?;
    bytes.get(offset..end).ok_or(Error::Truncated)
}

#[derive(Default)]
struct DynamicInfo {
    rela: Option<u64>,
    rela_size: Option<u64>,
    rela_entry_size: Option<u64>,
    relr: Option<u64>,
    relr_size: Option<u64>,
    relr_entry_size: Option<u64>,
}

fn parse_dynamic(bytes: &[u8], segments: &[LoadSegment<'_>]) -> Result<Vec<Relocation>, Error> {
    if !bytes.len().is_multiple_of(DYNAMIC_ENTRY_SIZE) {
        return Err(Error::InvalidDynamicTable);
    }
    let mut info = DynamicInfo::default();
    let mut terminated = false;
    for entry in bytes.chunks_exact(DYNAMIC_ENTRY_SIZE) {
        let tag = read_i64(entry, 0)?;
        let value = read_u64(entry, 8)?;
        match tag {
            DT_NULL => {
                terminated = true;
                break;
            }
            DT_NEEDED | DT_TEXTREL => return Err(Error::InvalidDynamicTable),
            DT_PLTRELSZ | DT_JMPREL | DT_REL | DT_RELSZ | DT_RELENT if value != 0 => {
                return Err(Error::UnsupportedRelocation);
            }
            DT_RELA => set_once(&mut info.rela, value)?,
            DT_RELASZ => set_once(&mut info.rela_size, value)?,
            DT_RELAENT => set_once(&mut info.rela_entry_size, value)?,
            DT_RELR => set_once(&mut info.relr, value)?,
            DT_RELRSZ => set_once(&mut info.relr_size, value)?,
            DT_RELRENT => set_once(&mut info.relr_entry_size, value)?,
            _ => {}
        }
    }
    if !terminated {
        return Err(Error::InvalidDynamicTable);
    }

    let mut relocations = Vec::new();
    parse_rela(&info, segments, &mut relocations)?;
    parse_relr(&info, segments, &mut relocations)?;
    relocations.sort_unstable_by_key(|relocation| relocation.target());
    if relocations
        .windows(2)
        .any(|pair| pair[0].target() == pair[1].target())
    {
        return Err(Error::DuplicateRelocation);
    }
    Ok(relocations)
}

fn parse_rela(
    info: &DynamicInfo,
    segments: &[LoadSegment<'_>],
    output: &mut Vec<Relocation>,
) -> Result<(), Error> {
    let present = info.rela.is_some() || info.rela_size.is_some() || info.rela_entry_size.is_some();
    if !present {
        return Ok(());
    }
    let address = info.rela.ok_or(Error::InvalidDynamicTable)?;
    let size = info.rela_size.ok_or(Error::InvalidDynamicTable)?;
    if info.rela_entry_size != Some(RELA_ENTRY_SIZE as u64)
        || !size.is_multiple_of(RELA_ENTRY_SIZE as u64)
    {
        return Err(Error::InvalidDynamicTable);
    }
    let size = usize::try_from(size).map_err(|_| Error::ArithmeticOverflow)?;
    let bytes = virtual_file_slice(segments, address, size)?;
    reserve_relocations(output, bytes.len() / RELA_ENTRY_SIZE)?;
    for entry in bytes.chunks_exact(RELA_ENTRY_SIZE) {
        let target = read_u64(entry, 0)?;
        let info = read_u64(entry, 8)?;
        let symbol = info >> 32;
        let relocation_type = info as u32;
        if symbol != 0 || relocation_type != R_AARCH64_RELATIVE {
            return Err(Error::UnsupportedRelocation);
        }
        validate_relocation_target(segments, target)?;
        output.push(Relocation::Relative {
            target,
            addend: read_i64(entry, 16)?,
        });
    }
    Ok(())
}

fn parse_relr(
    info: &DynamicInfo,
    segments: &[LoadSegment<'_>],
    output: &mut Vec<Relocation>,
) -> Result<(), Error> {
    let present = info.relr.is_some() || info.relr_size.is_some() || info.relr_entry_size.is_some();
    if !present {
        return Ok(());
    }
    let address = info.relr.ok_or(Error::InvalidDynamicTable)?;
    let size = info.relr_size.ok_or(Error::InvalidDynamicTable)?;
    if info.relr_entry_size != Some(RELR_ENTRY_SIZE as u64)
        || !size.is_multiple_of(RELR_ENTRY_SIZE as u64)
    {
        return Err(Error::InvalidDynamicTable);
    }
    let size = usize::try_from(size).map_err(|_| Error::ArithmeticOverflow)?;
    let bytes = virtual_file_slice(segments, address, size)?;
    let mut cursor = None;
    for entry in bytes.chunks_exact(RELR_ENTRY_SIZE) {
        let value = read_u64(entry, 0)?;
        if value & 1 == 0 {
            append_relr(output, segments, value)?;
            cursor = Some(value.checked_add(8).ok_or(Error::ArithmeticOverflow)?);
            continue;
        }
        let base = cursor.ok_or(Error::InvalidRelocation)?;
        for bit in 1..64 {
            if value & (1u64 << bit) != 0 {
                let target = base
                    .checked_add((bit - 1) * 8)
                    .ok_or(Error::ArithmeticOverflow)?;
                append_relr(output, segments, target)?;
            }
        }
        cursor = Some(base.checked_add(63 * 8).ok_or(Error::ArithmeticOverflow)?);
    }
    Ok(())
}

fn append_relr(
    output: &mut Vec<Relocation>,
    segments: &[LoadSegment<'_>],
    target: u64,
) -> Result<(), Error> {
    reserve_relocations(output, 1)?;
    validate_relocation_target(segments, target)?;
    output.push(Relocation::RelativeInPlace { target });
    Ok(())
}

fn reserve_relocations(output: &mut Vec<Relocation>, additional: usize) -> Result<(), Error> {
    let total = output
        .len()
        .checked_add(additional)
        .ok_or(Error::TooManyRelocations)?;
    if total > MAXIMUM_RELOCATIONS {
        return Err(Error::TooManyRelocations);
    }
    // RELR bitmaps append one target at a time. Geometric growth bounds
    // total copying even when the allocator cannot extend a block in place.
    output
        .try_reserve(additional)
        .map_err(|_| Error::Allocation)
}

fn validate_relocation_target(segments: &[LoadSegment<'_>], target: u64) -> Result<(), Error> {
    if !target.is_multiple_of(8)
        || !segments
            .iter()
            .any(|segment| segment.permissions.writable() && segment.contains_memory(target, 8))
    {
        return Err(Error::InvalidRelocation);
    }
    Ok(())
}

fn virtual_file_slice<'image>(
    segments: &[LoadSegment<'image>],
    address: u64,
    length: usize,
) -> Result<&'image [u8], Error> {
    segments
        .iter()
        .find_map(|segment| segment.file_slice(address, length))
        .ok_or(Error::InvalidDynamicTable)
}

fn set_once(slot: &mut Option<u64>, value: u64) -> Result<(), Error> {
    if slot.replace(value).is_some() {
        return Err(Error::DuplicateDynamicEntry);
    }
    Ok(())
}

const fn align_down(value: u64) -> u64 {
    value & !(PAGE_SIZE - 1)
}

fn align_up(value: u64) -> Result<u64, Error> {
    value
        .checked_add(PAGE_SIZE - 1)
        .map(|rounded| rounded & !(PAGE_SIZE - 1))
        .ok_or(Error::ArithmeticOverflow)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    let value = bytes
        .get(offset..)
        .and_then(|tail| tail.get(..2))
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let value = bytes
        .get(offset..)
        .and_then(|tail| tail.get(..4))
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let value = bytes
        .get(offset..)
        .and_then(|tail| tail.get(..8))
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u64::from_le_bytes(value))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, Error> {
    let value = bytes
        .get(offset..)
        .and_then(|tail| tail.get(..8))
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(i64::from_le_bytes(value))
}
