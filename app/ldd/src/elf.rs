// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded ELF64 dependency metadata reads, independent of section headers.

use std::io::{self, Read, Seek, SeekFrom};

// Match Native rtld's header, dependency and name limits. Dynamic-table work
// also has a byte bound; debug sections and the full string table are not read.
const MAX_PROGRAM_HEADERS: usize = 32;
const MAX_NEEDED: usize = 16;
const MAX_NAME_BYTES: u64 = 127;
const MAX_DYNAMIC_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    Aarch64,
    Riscv64,
    X86_64,
}

impl Architecture {
    pub fn name(self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64",
            Self::Riscv64 => "riscv64",
            Self::X86_64 => "x86_64",
        }
    }
}

#[derive(Debug)]
pub struct Image {
    pub architecture: Architecture,
    pub os_abi: u8,
    pub abi_version: u8,
    pub interpreter: Option<String>,
    pub soname: Option<String>,
    pub needed: Vec<String>,
    pub static_executable: bool,
}

#[derive(Clone, Copy)]
struct Segment {
    kind: u32,
    offset: u64,
    address: u64,
    file_size: u64,
}

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|index| bytes[offset + index]))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(std::array::from_fn(|index| bytes[offset + index]))
}

struct Reader<R> {
    input: R,
    length: u64,
}

impl<R: Read + Seek> Reader<R> {
    fn range(&self, offset: u64, length: u64) -> io::Result<()> {
        if offset > self.length || length > self.length - offset {
            return Err(invalid("ELF range exceeds file size"));
        }
        Ok(())
    }

    fn read(&mut self, offset: u64, bytes: &mut [u8]) -> io::Result<()> {
        self.range(offset, bytes.len() as u64)?;
        self.input.seek(SeekFrom::Start(offset))?;
        self.input.read_exact(bytes)
    }

    fn string(&mut self, offset: u64, available: u64, maximum: u64) -> io::Result<String> {
        let length = available.min(maximum + 1) as usize;
        let mut bytes = vec![0; length];
        self.read(offset, &mut bytes)?;
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| invalid("unterminated or overlong ELF string"))?;
        bytes.truncate(end);
        String::from_utf8(bytes).map_err(|_| invalid("ELF path is not UTF-8"))
    }
}

/// Reads only bounded program headers, dynamic entries and named strings.
/// Every disk offset is checked before seeking; virtual addresses must map
/// unambiguously to file-backed LOAD data, never to zero-filled memory.
pub fn read(input: impl Read + Seek) -> io::Result<Image> {
    let mut input = input;
    let length = input.seek(SeekFrom::End(0))?;
    let mut reader = Reader { input, length };
    let mut header = [0; 64];
    reader.read(0, &mut header)?;
    if &header[..4] != b"\x7fELF" {
        return Err(invalid("not an ELF file"));
    }
    if header[4..7] != [2, 1, 1] || u32_at(&header, 20) != 1 {
        return Err(invalid("expected little-endian ELF64 version 1"));
    }
    if !matches!(u16_at(&header, 16), 2 | 3) || u16_at(&header, 52) != 64 {
        return Err(invalid("expected an ELF executable or shared object"));
    }
    let architecture = match u16_at(&header, 18) {
        183 => Architecture::Aarch64,
        243 => Architecture::Riscv64,
        62 => Architecture::X86_64,
        machine => return Err(invalid(format!("unsupported ELF machine {machine}"))),
    };
    let flags = u32_at(&header, 48);
    if match architecture {
        Architecture::Riscv64 => flags & !1 != 4,
        _ => flags != 0,
    } {
        return Err(invalid("unsupported architecture ABI flags"));
    }
    let headers = read_segments(&mut reader, &header)?;
    let mut image = Image {
        architecture,
        os_abi: header[7],
        abi_version: header[8],
        interpreter: None,
        soname: None,
        needed: Vec::new(),
        static_executable: false,
    };
    if let Some(segment) = unique_segment(&headers, 3)? {
        if segment.file_size < 2 || segment.file_size > 4096 {
            return Err(invalid("invalid interpreter path size"));
        }
        let path = reader.string(segment.offset, segment.file_size, 4095)?;
        if !path.starts_with('/') || path.len() as u64 + 1 != segment.file_size {
            return Err(invalid(
                "interpreter must be one absolute, NUL-terminated path",
            ));
        }
        image.interpreter = Some(path);
    }
    if let Some(segment) = unique_segment(&headers, 2)? {
        read_dynamic(&mut reader, &headers, segment, &mut image)?;
    } else if image.interpreter.is_some() {
        return Err(invalid("interpreter without a dynamic table"));
    }
    image.static_executable = image.interpreter.is_none()
        && image.needed.is_empty()
        && image.soname.is_none()
        && u64_at(&header, 24) != 0;
    Ok(image)
}

fn read_segments<R: Read + Seek>(
    reader: &mut Reader<R>,
    header: &[u8; 64],
) -> io::Result<Vec<Segment>> {
    let offset = u64_at(header, 32);
    let count = usize::from(u16_at(header, 56));
    if offset < 64 || count == 0 || count > MAX_PROGRAM_HEADERS || u16_at(header, 54) != 56 {
        return Err(invalid("invalid or excessive program headers"));
    }
    reader.range(offset, count as u64 * 56)?;
    let mut segments = Vec::with_capacity(count);
    for index in 0..count {
        let mut bytes = [0; 56];
        reader.read(offset + index as u64 * 56, &mut bytes)?;
        let segment = Segment {
            kind: u32_at(&bytes, 0),
            offset: u64_at(&bytes, 8),
            address: u64_at(&bytes, 16),
            file_size: u64_at(&bytes, 32),
        };
        if matches!(segment.kind, 1..=3) {
            reader.range(segment.offset, segment.file_size)?;
        }
        if segment.kind == 1 {
            let memory_size = u64_at(&bytes, 40);
            if segment.file_size > memory_size || segment.address.checked_add(memory_size).is_none()
            {
                return Err(invalid("invalid LOAD segment size"));
            }
        }
        segments.push(segment);
    }
    if !segments.iter().any(|segment| segment.kind == 1) {
        return Err(invalid("missing LOAD segment"));
    }
    Ok(segments)
}

fn unique_segment(segments: &[Segment], kind: u32) -> io::Result<Option<Segment>> {
    let mut found = segments.iter().filter(|segment| segment.kind == kind);
    let segment = found.next().copied();
    if found.next().is_some() {
        return Err(invalid("duplicate interpreter or dynamic segment"));
    }
    Ok(segment)
}

fn file_offset(segments: &[Segment], address: u64, size: u64) -> io::Result<u64> {
    let mut found = None;
    for segment in segments.iter().filter(|segment| segment.kind == 1) {
        let Some(delta) = address.checked_sub(segment.address) else {
            continue;
        };
        if delta <= segment.file_size && size <= segment.file_size - delta {
            if found.is_some() {
                return Err(invalid("ambiguous virtual address mapping"));
            }
            found = segment.offset.checked_add(delta);
        }
    }
    found.ok_or_else(|| invalid("dynamic metadata is outside file-backed LOAD segments"))
}

fn read_dynamic<R: Read + Seek>(
    reader: &mut Reader<R>,
    segments: &[Segment],
    segment: Segment,
    image: &mut Image,
) -> io::Result<()> {
    if segment.file_size == 0
        || segment.file_size > MAX_DYNAMIC_BYTES
        || !segment.file_size.is_multiple_of(16)
    {
        return Err(invalid("invalid or excessive dynamic table size"));
    }
    if file_offset(segments, segment.address, segment.file_size)? != segment.offset {
        return Err(invalid("dynamic segment disagrees with LOAD mapping"));
    }
    let mut table = vec![0; segment.file_size as usize];
    reader.read(segment.offset, &mut table)?;
    let (mut strings, mut size, mut soname) = (None, None, None);
    let mut needed = Vec::new();
    let mut terminated = false;
    for entry in table.chunks_exact(16) {
        let value = u64_at(entry, 8);
        match u64_at(entry, 0) {
            0 => {
                terminated = true;
                break;
            }
            1 => {
                if needed.len() == MAX_NEEDED {
                    return Err(invalid("too many DT_NEEDED entries"));
                }
                needed.push(value);
            }
            5 => set_once(&mut strings, value)?,
            10 => set_once(&mut size, value)?,
            14 => set_once(&mut soname, value)?,
            _ => {}
        }
    }
    if !terminated {
        return Err(invalid("dynamic table has no DT_NULL terminator"));
    }
    if needed.is_empty() && soname.is_none() {
        return Ok(());
    }
    let address = strings.ok_or_else(|| invalid("missing DT_STRTAB"))?;
    let size = size
        .filter(|size| *size != 0)
        .ok_or_else(|| invalid("missing DT_STRSZ"))?;
    let offset = file_offset(segments, address, size)?;
    let mut read_name = |index| {
        if index >= size {
            return Err(invalid("dynamic string offset exceeds DT_STRSZ"));
        }
        reader.string(offset + index, size - index, MAX_NAME_BYTES)
    };
    for index in needed {
        let name = read_name(index)?;
        if name.is_empty() || matches!(name.as_str(), "." | "..") || name.contains('/') {
            return Err(invalid(
                "DT_NEEDED must name a file within the library directory",
            ));
        }
        image.needed.push(name);
    }
    image.soname = soname.map(read_name).transpose()?;
    Ok(())
}

fn set_once(slot: &mut Option<u64>, value: u64) -> io::Result<()> {
    if slot.replace(value).is_some() {
        return Err(invalid("duplicate dynamic string-table metadata"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/elf.rs"]
mod tests;
