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
const PT_PHDR: u32 = 6;
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
    interpreter: Option<&'image str>,
    program_header_address: u64,
    program_header_count: u16,
}

/// Heap-storage upper bound established without allocating parser memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationPlan {
    segment_capacity: usize,
    relocation_capacity: usize,
    dynamic_executable: bool,
}

impl AllocationPlan {
    pub const fn segment_capacity(self) -> usize {
        self.segment_capacity
    }

    /// Maximum number of decoded relocations for which parsing reserves space.
    pub const fn relocation_capacity(self) -> usize {
        self.relocation_capacity
    }

    pub fn parser_bytes(self) -> Option<usize> {
        self.segment_capacity
            .checked_mul(core::mem::size_of::<LoadSegment<'static>>())?
            .checked_add(
                self.relocation_capacity
                    .checked_mul(core::mem::size_of::<Relocation>())?,
            )
    }
}

impl<'image> Image<'image> {
    pub fn parse(bytes: &'image [u8]) -> Result<Self, Error> {
        let allocation = Self::allocation_plan(bytes)?;
        Self::parse_with_plan(bytes, allocation)
    }

    /// Inspects allocation-driving ELF metadata without allocating.
    pub fn allocation_plan(bytes: &[u8]) -> Result<AllocationPlan, Error> {
        Self::allocation_plan_for_process(bytes, false)
    }

    pub fn process_allocation_plan(bytes: &[u8]) -> Result<AllocationPlan, Error> {
        Self::allocation_plan_for_process(bytes, true)
    }

    fn allocation_plan_for_process(
        bytes: &[u8],
        permit_dynamic_executable: bool,
    ) -> Result<AllocationPlan, Error> {
        let header = bytes.get(..ELF_HEADER_SIZE).ok_or(Error::Truncated)?;
        validate_ident(header)?;
        validate_fixed_header(header)?;
        let (program_offset, program_count) = program_table(header, bytes)?;
        let mut dynamic = None;
        let mut interpreter = false;
        for index in 0..program_count {
            let offset = program_offset + index * PROGRAM_HEADER_SIZE;
            let program = &bytes[offset..offset + PROGRAM_HEADER_SIZE];
            if read_u32(program, 0)? == PT_DYNAMIC {
                if dynamic.is_some() {
                    return Err(Error::InvalidDynamicTable);
                }
                dynamic = Some(program_data(bytes, program)?);
            } else if read_u32(program, 0)? == PT_INTERP {
                if interpreter {
                    return Err(Error::UnsupportedInterpreter);
                }
                interpreter = true;
            }
        }
        if interpreter && !permit_dynamic_executable {
            return Err(Error::UnsupportedInterpreter);
        }
        let relocation_capacity = match (dynamic, interpreter) {
            (_, true) => 0,
            (Some(dynamic), false) => relocation_capacity(&read_dynamic_info(dynamic)?)?,
            (None, false) => 0,
        };
        Ok(AllocationPlan {
            segment_capacity: program_count,
            relocation_capacity,
            dynamic_executable: interpreter,
        })
    }

    /// Parses using a previously reserved allocation plan.
    pub fn parse_with_plan(bytes: &'image [u8], allocation: AllocationPlan) -> Result<Self, Error> {
        if Self::allocation_plan(bytes)? != allocation {
            return Err(Error::InvalidHeader);
        }
        Self::parse_inner(bytes, allocation)
    }

    fn parse_inner(bytes: &'image [u8], allocation: AllocationPlan) -> Result<Self, Error> {
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
        validate_fixed_header(header)?;
        let entry = read_u64(header, 24)?;
        let (program_offset, program_count) = program_table(header, bytes)?;
        // A dynamically linked process needs a mapped program-header table for
        // the runtime linker's `AT_PHDR` contract. Static images do not consume
        // that auxiliary value and remain valid when their headers are outside
        // every loadable segment.
        let program_header_address = if allocation.dynamic_executable {
            program_header_virtual_address(bytes, header)?
        } else {
            0
        };

        let mut segments = Vec::new();
        segments
            .try_reserve_exact(allocation.segment_capacity)
            .map_err(|_| Error::Allocation)?;
        let mut dynamic = None;
        let mut interpreter = None;
        let mut program_header_segment = false;
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
                PT_INTERP => {
                    if !allocation.dynamic_executable || interpreter.is_some() {
                        return Err(Error::UnsupportedInterpreter);
                    }
                    interpreter = Some(parse_interpreter(bytes, header)?);
                }
                PT_PHDR => {
                    if program_header_segment {
                        return Err(Error::InvalidHeader);
                    }
                    if allocation.dynamic_executable
                        && !valid_program_header_segment(
                            header,
                            read_u64(bytes, 32)?,
                            program_header_address,
                            program_count,
                        )?
                    {
                        return Err(Error::InvalidHeader);
                    }
                    program_header_segment = true;
                }
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
        if allocation.dynamic_executable
            && (kind != ImageKind::PositionIndependent
                || interpreter.is_none()
                || dynamic.is_none()
                || !program_header_segment)
        {
            return Err(Error::UnsupportedInterpreter);
        }
        let relocations = match (dynamic, allocation.dynamic_executable) {
            (Some(dynamic), false) => {
                parse_dynamic(dynamic, &segments, allocation.relocation_capacity)?
            }
            (Some(dynamic), true) => {
                validate_dynamic_executable(dynamic)?;
                Vec::new()
            }
            (None, _) => Vec::new(),
        };
        Ok(Self {
            kind,
            machine,
            entry,
            segments,
            relocations,
            interpreter,
            program_header_address,
            program_header_count: u16::try_from(program_count).map_err(|_| Error::InvalidHeader)?,
        })
    }

    pub fn parse_process_with_plan(
        bytes: &'image [u8],
        allocation: AllocationPlan,
    ) -> Result<Self, Error> {
        if Self::process_allocation_plan(bytes)? != allocation {
            return Err(Error::InvalidHeader);
        }
        Self::parse_inner(bytes, allocation)
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

    pub const fn interpreter(&self) -> Option<&'image str> {
        self.interpreter
    }

    pub const fn program_header_address(&self) -> u64 {
        self.program_header_address
    }

    pub const fn program_header_count(&self) -> u16 {
        self.program_header_count
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

fn valid_program_header_segment(
    segment: &[u8],
    table_offset: u64,
    table_address: u64,
    table_count: usize,
) -> Result<bool, Error> {
    let table_size = u64::try_from(table_count)
        .ok()
        .and_then(|count| count.checked_mul(PROGRAM_HEADER_SIZE as u64))
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(
        table_address.is_multiple_of(core::mem::align_of::<u64>() as u64)
            && read_u64(segment, 8)? == table_offset
            && read_u64(segment, 16)? == table_address
            && read_u64(segment, 32)? >= table_size
            && read_u64(segment, 40)? >= table_size,
    )
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

fn validate_fixed_header(header: &[u8]) -> Result<(), Error> {
    if read_u32(header, 20)? != 1
        || read_u32(header, 48)? != 0
        || usize::from(read_u16(header, 52)?) != ELF_HEADER_SIZE
        || usize::from(read_u16(header, 54)?) != PROGRAM_HEADER_SIZE
    {
        return Err(Error::InvalidHeader);
    }
    Ok(())
}

fn program_table(header: &[u8], bytes: &[u8]) -> Result<(usize, usize), Error> {
    let offset = usize::try_from(read_u64(header, 32)?).map_err(|_| Error::ArithmeticOverflow)?;
    let count = usize::from(read_u16(header, 56)?);
    if count == 0 || count > MAXIMUM_PROGRAM_HEADERS {
        return Err(Error::TooManyProgramHeaders);
    }
    let size = count
        .checked_mul(PROGRAM_HEADER_SIZE)
        .ok_or(Error::ArithmeticOverflow)?;
    let end = offset.checked_add(size).ok_or(Error::ArithmeticOverflow)?;
    bytes.get(offset..end).ok_or(Error::Truncated)?;
    Ok((offset, count))
}

fn program_header_virtual_address(bytes: &[u8], elf_header: &[u8]) -> Result<u64, Error> {
    let table_offset = read_u64(elf_header, 32)?;
    let table_count = u64::from(read_u16(elf_header, 56)?);
    let table_size = table_count
        .checked_mul(PROGRAM_HEADER_SIZE as u64)
        .ok_or(Error::ArithmeticOverflow)?;
    let table_end = table_offset
        .checked_add(table_size)
        .ok_or(Error::ArithmeticOverflow)?;
    let (program_offset, program_count) = program_table(elf_header, bytes)?;
    for index in 0..program_count {
        let offset = program_offset + index * PROGRAM_HEADER_SIZE;
        let program = &bytes[offset..offset + PROGRAM_HEADER_SIZE];
        if read_u32(program, 0)? != PT_LOAD {
            continue;
        }
        let file_offset = read_u64(program, 8)?;
        let file_size = read_u64(program, 32)?;
        let file_end = file_offset
            .checked_add(file_size)
            .ok_or(Error::ArithmeticOverflow)?;
        if file_offset <= table_offset && table_end <= file_end {
            return read_u64(program, 16)?
                .checked_add(table_offset - file_offset)
                .ok_or(Error::ArithmeticOverflow);
        }
    }
    Err(Error::InvalidHeader)
}

fn parse_interpreter<'image>(bytes: &'image [u8], header: &[u8]) -> Result<&'image str, Error> {
    const MAXIMUM_INTERPRETER_BYTES: usize = 256;
    let data = program_data(bytes, header)?;
    if data.len() < 2 || data.len() > MAXIMUM_INTERPRETER_BYTES || data.last() != Some(&0) {
        return Err(Error::UnsupportedInterpreter);
    }
    let path =
        core::str::from_utf8(&data[..data.len() - 1]).map_err(|_| Error::UnsupportedInterpreter)?;
    if !path.starts_with('/')
        || path.as_bytes().contains(&0)
        || path.split('/').any(|component| component == "..")
    {
        return Err(Error::UnsupportedInterpreter);
    }
    Ok(path)
}

fn validate_dynamic_executable(bytes: &[u8]) -> Result<(), Error> {
    if !bytes.len().is_multiple_of(DYNAMIC_ENTRY_SIZE) {
        return Err(Error::InvalidDynamicTable);
    }
    for entry in bytes.chunks_exact(DYNAMIC_ENTRY_SIZE) {
        match read_i64(entry, 0)? {
            DT_NULL => return Ok(()),
            DT_TEXTREL => return Err(Error::InvalidDynamicTable),
            _ => {}
        }
    }
    Err(Error::InvalidDynamicTable)
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
        && (alignment > PAGE_SIZE
            || !alignment.is_power_of_two()
            || virtual_address % alignment != file_offset % alignment)
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

fn parse_dynamic(
    bytes: &[u8],
    segments: &[LoadSegment<'_>],
    relocation_capacity: usize,
) -> Result<Vec<Relocation>, Error> {
    let info = read_dynamic_info(bytes)?;
    let mut relocations = Vec::new();
    relocations
        .try_reserve_exact(relocation_capacity)
        .map_err(|_| Error::Allocation)?;
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

fn read_dynamic_info(bytes: &[u8]) -> Result<DynamicInfo, Error> {
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
    Ok(info)
}

fn relocation_capacity(info: &DynamicInfo) -> Result<usize, Error> {
    let rela = optional_table_count(
        info.rela,
        info.rela_size,
        info.rela_entry_size,
        RELA_ENTRY_SIZE,
    )?;
    let relr_entries = optional_table_count(
        info.relr,
        info.relr_size,
        info.relr_entry_size,
        RELR_ENTRY_SIZE,
    )?;
    // Every RELR word expands to at most 63 relocation records.
    let relr = relr_entries
        .checked_mul(63)
        .ok_or(Error::TooManyRelocations)?;
    let total = rela.checked_add(relr).ok_or(Error::TooManyRelocations)?;
    if total > MAXIMUM_RELOCATIONS {
        return Err(Error::TooManyRelocations);
    }
    Ok(total)
}

fn optional_table_count(
    address: Option<u64>,
    size: Option<u64>,
    entry_size: Option<u64>,
    expected_entry_size: usize,
) -> Result<usize, Error> {
    let present = address.is_some() || size.is_some() || entry_size.is_some();
    if !present {
        return Ok(0);
    }
    let _ = address.ok_or(Error::InvalidDynamicTable)?;
    let size = size.ok_or(Error::InvalidDynamicTable)?;
    if entry_size != Some(expected_entry_size as u64)
        || !size.is_multiple_of(expected_entry_size as u64)
    {
        return Err(Error::InvalidDynamicTable);
    }
    usize::try_from(size / expected_entry_size as u64).map_err(|_| Error::TooManyRelocations)
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
