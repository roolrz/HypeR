// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One ordered request at a time over a `ByteChannel`; file bytes use a dedicated
//! resident VMO. A timed-out or malformed transaction retires the mount forever.
//! Every path is relative, UTF-8, without empty, dot or parent components.

use crate::Timestamp;
pub const DATA_BYTES: usize = 512 * 1024;
pub const PATH_BYTES: usize = 4096;
pub const NAME_BYTES: usize = 1020;
pub const HEADER_BYTES: usize = 80;
pub const REQUEST_BYTES: usize = HEADER_BYTES + PATH_BYTES * 2;
pub const RESPONSE_BYTES: usize = 1152;
pub const FILE: u32 = 1;
pub const DIRECTORY: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum Operation {
    Stat = 1,
    Entry,
    Read,
    Write,
    Create,
    Remove,
    Rename,
    Resize,
    Sync,
    SetMetadata,
}
impl Operation {
    pub fn decode(value: u64) -> Option<Self> {
        Some(match value {
            1 => Self::Stat,
            2 => Self::Entry,
            3 => Self::Read,
            4 => Self::Write,
            5 => Self::Create,
            6 => Self::Remove,
            7 => Self::Rename,
            8 => Self::Resize,
            9 => Self::Sync,
            10 => Self::SetMetadata,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum Error {
    Io = 1,
    Allocation,
    Missing,
    Exists,
    NotEmpty,
    InvalidInput,
    NoSpace,
    Unsupported,
    Closed,
    ReadOnly,
    Corrupt,
    ResourceLimit,
}
impl Error {
    pub fn decode(value: u64) -> Option<Self> {
        Some(match value {
            1 => Self::Io,
            2 => Self::Allocation,
            3 => Self::Missing,
            4 => Self::Exists,
            5 => Self::NotEmpty,
            6 => Self::InvalidInput,
            7 => Self::NoSpace,
            8 => Self::Unsupported,
            9 => Self::Closed,
            10 => Self::ReadOnly,
            11 => Self::Corrupt,
            12 => Self::ResourceLimit,
            _ => return None,
        })
    }
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Io | Self::Closed | Self::Corrupt)
    }
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: [u8; NAME_BYTES],
    pub name_len: usize,
    pub kind: u32,
    pub mode: u32,
    pub size: u64,
    pub created: Option<Timestamp>,
    pub accessed: Option<Timestamp>,
    pub modified: Option<Timestamp>,
}
impl Entry {
    pub const fn empty() -> Self {
        Self {
            name: [0; NAME_BYTES],
            name_len: 0,
            kind: DIRECTORY,
            mode: 0o777,
            size: 0,
            created: None,
            accessed: None,
            modified: None,
        }
    }
    pub fn name(&self) -> &str {
        self.name
            .get(..self.name_len)
            .and_then(|name| core::str::from_utf8(name).ok())
            .unwrap_or("")
    }
    pub const fn directory(&self) -> bool {
        self.kind == DIRECTORY
    }
    pub const fn read_only(&self) -> bool {
        self.mode & 0o222 == 0
    }
}
#[derive(Clone, Copy)]
pub struct Request<'a> {
    pub sequence: u64,
    pub operation: Operation,
    pub offset: u64,
    pub length: u64,
    pub path: &'a str,
    pub target: &'a str,
    pub accessed: Option<Timestamp>,
    pub modified: Option<Timestamp>,
}
impl<'a> Request<'a> {
    pub fn encode(&self, output: &mut [u8]) -> Option<usize> {
        if self.sequence == 0 || !valid_path(self.path) || !valid_path(self.target) {
            return None;
        }
        let length = HEADER_BYTES
            .checked_add(self.path.len())?
            .checked_add(self.target.len())?;
        let output = output.get_mut(..length)?;
        output.fill(0);
        put(output, 0, self.sequence);
        put(output, 8, self.operation as u64);
        put(output, 16, self.offset);
        put(output, 24, self.length);
        put(output, 32, self.path.len() as u64);
        put(output, 40, self.target.len() as u64);
        put_time(output, 48, self.accessed);
        put_time(output, 64, self.modified);
        output[HEADER_BYTES..HEADER_BYTES + self.path.len()].copy_from_slice(self.path.as_bytes());
        output[HEADER_BYTES + self.path.len()..].copy_from_slice(self.target.as_bytes());
        Some(length)
    }
    pub fn decode(input: &'a [u8]) -> Option<Self> {
        if input.len() < HEADER_BYTES {
            return None;
        }
        let first = usize::try_from(get(input, 32)?).ok()?;
        let second = usize::try_from(get(input, 40)?).ok()?;
        if first > PATH_BYTES || second > PATH_BYTES || input.len() != HEADER_BYTES + first + second
        {
            return None;
        }
        let request = Self {
            sequence: get(input, 0)?,
            operation: Operation::decode(get(input, 8)?)?,
            offset: get(input, 16)?,
            length: get(input, 24)?,
            path: core::str::from_utf8(&input[HEADER_BYTES..HEADER_BYTES + first]).ok()?,
            target: core::str::from_utf8(&input[HEADER_BYTES + first..]).ok()?,
            accessed: get_time(input, 48)?,
            modified: get_time(input, 64)?,
        };
        if request.sequence == 0 || !valid_path(request.path) || !valid_path(request.target) {
            return None;
        }
        Some(request)
    }
}
pub struct Response {
    pub sequence: u64,
    pub operation: Operation,
    pub result: Result<u64, Error>,
    pub entry: Entry,
}
impl Response {
    pub fn encode(&self, output: &mut [u8; RESPONSE_BYTES]) -> Option<()> {
        let name = self.entry.name.get(..self.entry.name_len)?;
        if self.sequence == 0
            || !valid_name(name)
            || !matches!(self.entry.kind, FILE | DIRECTORY)
            || self.entry.mode & !0o777 != 0
            || !valid_result(self.operation, self.result)
        {
            return None;
        }
        output.fill(0);
        put(output, 0, self.sequence);
        put(output, 8, self.operation as u64);
        match self.result {
            Ok(value) => put(output, 24, value),
            Err(error) => put(output, 16, error as u64),
        }
        put(output, 32, u64::from(self.entry.kind));
        put(output, 40, u64::from(self.entry.mode));
        put(output, 48, self.entry.size);
        put(output, 56, self.entry.name_len as u64);
        put_time(output, 64, self.entry.created);
        put_time(output, 80, self.entry.accessed);
        put_time(output, 96, self.entry.modified);
        output[112..112 + self.entry.name_len].copy_from_slice(name);
        Some(())
    }
    pub fn decode(input: &[u8], sequence: u64, operation: Operation) -> Option<Self> {
        if input.len() != RESPONSE_BYTES
            || get(input, 0)? != sequence
            || Operation::decode(get(input, 8)?)? != operation
        {
            return None;
        }
        let result = match get(input, 16)? {
            0 => Ok(get(input, 24)?),
            value if get(input, 24)? == 0 => Err(Error::decode(value)?),
            _ => return None,
        };
        if sequence == 0 || !valid_result(operation, result) {
            return None;
        }
        let length = usize::try_from(get(input, 56)?).ok()?;
        let kind = u32::try_from(get(input, 32)?).ok()?;
        let mode = u32::try_from(get(input, 40)?).ok()?;
        if length > NAME_BYTES || !matches!(kind, FILE | DIRECTORY) || mode & !0o777 != 0 {
            return None;
        }
        let mut entry = Entry::empty();
        entry.kind = kind;
        entry.mode = mode;
        entry.size = get(input, 48)?;
        entry.name_len = length;
        entry.name[..length].copy_from_slice(input.get(112..112 + length)?);
        if !valid_name(&entry.name[..length]) {
            return None;
        }
        entry.created = get_time(input, 64)?;
        entry.accessed = get_time(input, 80)?;
        entry.modified = get_time(input, 96)?;
        Some(Self {
            sequence,
            operation,
            result,
            entry,
        })
    }
}
fn valid_result(operation: Operation, result: Result<u64, Error>) -> bool {
    let Ok(value) = result else {
        return true;
    };
    match operation {
        Operation::Read | Operation::Write => value <= DATA_BYTES as u64,
        Operation::Entry => value <= 1,
        _ => value == 0,
    }
}
fn valid_name(bytes: &[u8]) -> bool {
    core::str::from_utf8(bytes)
        .is_ok_and(|name| !name.contains(['/', '\0']) && name != "." && name != "..")
}
pub fn valid_path(path: &str) -> bool {
    path.len() <= PATH_BYTES
        && !path.contains('\0')
        && (path.is_empty()
            || path
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."))
}
fn put(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn get(input: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        input.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
fn put_time(output: &mut [u8], offset: usize, time: Option<Timestamp>) {
    if let Some(time) = time {
        put(output, offset, time.seconds() as u64);
        put(output, offset + 8, u64::from(time.nanoseconds()) + 1);
    }
}
fn get_time(input: &[u8], offset: usize) -> Option<Option<Timestamp>> {
    let seconds = get(input, offset)? as i64;
    let nanos = get(input, offset + 8)?;
    if nanos == 0 {
        return (seconds == 0).then_some(None);
    }
    Some(Some(Timestamp::new(
        seconds,
        u32::try_from(nanos - 1).ok()?,
    )?))
}
