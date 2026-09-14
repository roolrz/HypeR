// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded FAT32 access. Namespace identities and open-file leases belong to VFS.
//!
//! The MIT upstream implementation is vendored with its revision and local fixes
//! recorded in third_party/rust-fatfs/PROVENANCE.md. Its destructors do
//! I/O, so no upstream object escapes this adapter. Device access is enabled
//! only inside a synchronous operation; destructor failures are latched. The
//! owner must hold a sleepable mutex across each method, never a spin lock.

use super::block::{BlockDevice, Error as BlockError, SECTOR_SIZE};
use crate::mm::FallibleArc;
use crate::sync::SpinLock;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use fatfs::{Read, Seek, SeekFrom, Write};

// Gap filling and extension only read these bytes. Keep one immutable sector
// instead of retaining a zeroed stack buffer across blocking filesystem I/O.
static ZERO_SECTOR: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Block(BlockError),
    Allocation,
    Corrupt,
    Missing,
    Exists,
    NotEmpty,
    InvalidInput,
    NoSpace,
    Unsupported,
    Closed,
}
impl From<fatfs::Error<BlockError>> for Error {
    fn from(value: fatfs::Error<BlockError>) -> Self {
        match value {
            fatfs::Error::Io(e) => Self::Block(e),
            fatfs::Error::NotFound => Self::Missing,
            fatfs::Error::AlreadyExists => Self::Exists,
            fatfs::Error::DirectoryIsNotEmpty => Self::NotEmpty,
            fatfs::Error::NotEnoughSpace => Self::NoSpace,
            fatfs::Error::InvalidInput
            | fatfs::Error::InvalidFileNameLength
            | fatfs::Error::UnsupportedFileNameCharacter => Self::InvalidInput,
            _ => Self::Corrupt,
        }
    }
}
impl fatfs::IoError for BlockError {
    fn is_interrupted(&self) -> bool {
        false
    }
    fn new_unexpected_eof_error() -> Self {
        Self::Corrupt
    }
    fn new_write_zero_error() -> Self {
        Self::Io
    }
}

struct DeviceSlot<D> {
    device: SpinLock<Option<D>>,
    enabled: AtomicBool,
    remaining: AtomicU64,
    failed: AtomicU8,
    sectors: u64,
    readonly: bool,
}
impl<D: BlockDevice> DeviceSlot<D> {
    fn access<R>(
        &self,
        operation: impl FnOnce(&mut D) -> Result<R, BlockError>,
    ) -> Result<R, BlockError> {
        self.charge()?;
        // Only transfer ownership under the lock. The callback may block.
        let mut device = self.device.with(Option::take).ok_or(BlockError::Io)?;
        let result = operation(&mut device);
        self.device.with(|slot| *slot = Some(device));
        if let Err(error) = result.as_ref() {
            self.fail(*error);
        }
        result
    }
    fn charge(&self) -> Result<(), BlockError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(BlockError::Disconnected);
        }
        if self.failed.load(Ordering::Relaxed) != 0 {
            return Err(BlockError::Io);
        }
        if self
            .remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_sub(1))
            .is_err()
        {
            self.fail(BlockError::Exhausted);
            return Err(BlockError::Exhausted);
        }
        Ok(())
    }
    fn fail(&self, error: BlockError) {
        let code = match error {
            BlockError::InvalidRange => 1,
            BlockError::ReadOnly => 2,
            BlockError::Disconnected => 3,
            BlockError::Io => 4,
            BlockError::Unsupported => 5,
            BlockError::Exhausted => 6,
            BlockError::Corrupt => 7,
        };
        let _ = self
            .failed
            .compare_exchange(0, code, Ordering::Relaxed, Ordering::Relaxed);
    }
    fn failure(&self) -> Option<BlockError> {
        match self.failed.load(Ordering::Relaxed) {
            0 => None,
            1 => Some(BlockError::InvalidRange),
            2 => Some(BlockError::ReadOnly),
            3 => Some(BlockError::Disconnected),
            5 => Some(BlockError::Unsupported),
            6 => Some(BlockError::Exhausted),
            7 => Some(BlockError::Corrupt),
            _ => Some(BlockError::Io),
        }
    }
}

struct Disk<D> {
    owner: FallibleArc<DeviceSlot<D>>,
    offset: u64,
    cached: Option<u64>,
    sector: Box<[u8; SECTOR_SIZE]>,
}
impl<D> fatfs::IoBase for Disk<D> {
    type Error = BlockError;
}
impl<D: BlockDevice> Read for Disk<D> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, BlockError> {
        self.owner.charge()?;
        let available = self.owner.sectors * SECTOR_SIZE as u64 - self.offset;
        let count = output.len().min(available.min(usize::MAX as u64) as usize);
        if count == 0 {
            return Ok(0);
        }
        let within = self.offset as usize % SECTOR_SIZE;
        let first = self.offset / SECTOR_SIZE as u64;
        let n = if within == 0 && count >= SECTOR_SIZE {
            let n = count / SECTOR_SIZE * SECTOR_SIZE;
            self.owner
                .access(|d| d.read_sectors(first, &mut output[..n]))?;
            n
        } else {
            if self.cached != Some(first) {
                self.owner
                    .access(|d| d.read_sectors(first, self.sector.as_mut()))?;
                self.cached = Some(first);
            }
            let n = count.min(SECTOR_SIZE - within);
            output[..n].copy_from_slice(&self.sector[within..within + n]);
            n
        };
        self.offset += n as u64;
        Ok(n)
    }
}
impl<D: BlockDevice> Write for Disk<D> {
    fn write(&mut self, input: &[u8]) -> Result<usize, BlockError> {
        self.owner.charge()?;
        let available = self.owner.sectors * SECTOR_SIZE as u64 - self.offset;
        let count = input.len().min(available.min(usize::MAX as u64) as usize);
        if count == 0 {
            return Ok(0);
        }
        let within = self.offset as usize % SECTOR_SIZE;
        let first = self.offset / SECTOR_SIZE as u64;
        let n = if within == 0 && count >= SECTOR_SIZE {
            let n = count / SECTOR_SIZE * SECTOR_SIZE;
            self.cached = None;
            self.owner.access(|d| d.write_sectors(first, &input[..n]))?;
            n
        } else {
            if self.cached != Some(first) {
                self.owner
                    .access(|d| d.read_sectors(first, self.sector.as_mut()))?;
                self.cached = Some(first);
            }
            let n = count.min(SECTOR_SIZE - within);
            self.sector[within..within + n].copy_from_slice(&input[..n]);
            self.owner
                .access(|d| d.write_sectors(first, self.sector.as_ref()))?;
            n
        };
        self.offset += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> Result<(), BlockError> {
        // Sector writes are write-through and already completed. Upstream
        // calls this even when dropping a read-only directory iterator; do
        // not turn every stat/read into a physical cache barrier. Volume
        // sync separately commits FSInfo and issues the durable device flush.
        self.owner.charge()
    }
}
impl<D: BlockDevice> Seek for Disk<D> {
    fn seek(&mut self, from: SeekFrom) -> Result<u64, BlockError> {
        self.owner.charge()?;
        let size = self.owner.sectors * SECTOR_SIZE as u64;
        let offset = match from {
            SeekFrom::Start(v) => Some(v),
            SeekFrom::Current(v) => self.offset.checked_add_signed(v),
            SeekFrom::End(v) => size.checked_add_signed(v),
        }
        .filter(|v| *v <= size)
        .ok_or(BlockError::InvalidRange)?;
        self.offset = offset;
        Ok(offset)
    }
}

/// Owned directory metadata; names never allocate based on media contents.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: [u8; 1020],
    pub name_len: usize,
    pub directory: bool,
    pub read_only: bool,
    pub size: u64,
    pub created: Option<crate::time::Timestamp>,
    pub accessed: Option<crate::time::Timestamp>,
    pub modified: Option<crate::time::Timestamp>,
}
impl Entry {
    pub const fn empty() -> Self {
        Self {
            name: [0; 1020],
            name_len: 0,
            directory: true,
            read_only: false,
            size: 0,
            created: None,
            accessed: None,
            modified: None,
        }
    }
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

pub struct FatVolume<D: BlockDevice> {
    fs: Option<fatfs::FileSystem<Disk<D>, super::fat_time::Clock>>,
    device: FallibleArc<DeviceSlot<D>>,
    budget: u64,
}
impl<D: BlockDevice> FatVolume<D> {
    /// Upper bound for temporary admission allocations with this geometry.
    pub fn mount_scratch_bound(sectors: u64) -> Result<usize, Error> {
        // At least one sector per cluster, two bits per cluster; directory
        // worklist grows geometrically to its explicit 65,536-entry bound.
        usize::try_from(sectors / 4 + 1)
            .ok()
            .and_then(|bitmap| bitmap.checked_add(65536 * core::mem::size_of::<u64>() + 65536))
            .ok_or(Error::InvalidInput)
    }
    pub const fn allocation_bytes() -> usize {
        FallibleArc::<DeviceSlot<D>>::allocation_size() + SECTOR_SIZE
    }
    pub fn mount(device: D) -> Result<Self, Error> {
        Self::mount_with_clock(device, || None)
    }
    pub fn mount_with_clock(
        mut device: D,
        now: fn() -> Option<crate::time::Timestamp>,
    ) -> Result<Self, Error> {
        let clock = super::fat_time::Clock(now);
        let sectors = device.sector_count();
        let readonly = device.is_read_only();
        if sectors == 0 || sectors > u64::MAX / SECTOR_SIZE as u64 {
            return Err(Error::InvalidInput);
        }
        // One persistent cache allocation replaces the VFS wrapper's former
        // boxed volume. Its buffer also serves boot validation, so no sector
        // array is retained in the mount call chain while device I/O blocks.
        let mut sector = crate::mm::try_box([0; SECTOR_SIZE]).map_err(|_| Error::Allocation)?;
        device
            .read_sectors(0, sector.as_mut())
            .map_err(Error::Block)?;
        validate_boot_sector(sector.as_ref(), sectors)?;
        super::fat_validate::validate(&mut device, sector.as_ref())?;
        let owner = FallibleArc::try_new(DeviceSlot {
            device: SpinLock::new(Some(device)),
            enabled: AtomicBool::new(true),
            remaining: AtomicU64::new(4096),
            failed: AtomicU8::new(0),
            sectors,
            readonly,
        })
        .map_err(|_| Error::Allocation)?;
        let fs = fatfs::FileSystem::new(
            Disk {
                owner: owner.clone(),
                offset: 0,
                cached: None,
                sector,
            },
            fatfs::FsOptions::new().strict(true).time_provider(clock),
        )
        .map_err(Error::from)?;
        owner.enabled.store(false, Ordering::Relaxed);
        // Bounds repeated FAT traversal on corrupt media. Count seeks as well
        // as transfers, so cache additions cannot defeat this limit.
        let budget = sectors.saturating_mul(32).max(4096);
        Ok(Self {
            fs: Some(fs),
            device: owner,
            budget,
        })
    }
    pub fn is_read_only(&self) -> bool {
        self.device.readonly
    }
    fn writable(&self) -> Result<(), Error> {
        if let Some(error) = self.device.failure() {
            return Err(Error::Block(error));
        }
        if self.fs.is_none() {
            return Err(Error::Closed);
        }
        if self.is_read_only() {
            return Err(Error::Block(BlockError::ReadOnly));
        }
        Ok(())
    }
    fn run<R>(
        &mut self,
        operation: impl FnOnce(&fatfs::FileSystem<Disk<D>, super::fat_time::Clock>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        if let Some(error) = self.device.failure() {
            return Err(Error::Block(error));
        }
        let fs = self.fs.as_ref().ok_or(Error::Closed)?;
        self.device.remaining.store(self.budget, Ordering::Relaxed);
        self.device.enabled.store(true, Ordering::Relaxed);
        let result = operation(fs);
        match &result {
            Err(Error::Corrupt) => self.device.fail(BlockError::Corrupt),
            Err(Error::Block(error)) => self.device.fail(*error),
            _ => {}
        }
        self.device.enabled.store(false, Ordering::Relaxed);
        match self.device.failure() {
            Some(e) => Err(Error::Block(e)),
            None => result,
        }
    }
    pub fn entry(&mut self, directory: &str, index: usize) -> Result<Option<Entry>, Error> {
        let mut result = Entry::empty();
        self.entry_into(directory, index, &mut result)
            .map(|found| found.then_some(result))
    }
    /// Fill caller-owned storage; large name buffers never cross return slots.
    pub fn entry_into(
        &mut self,
        directory: &str,
        index: usize,
        result: &mut Entry,
    ) -> Result<bool, Error> {
        self.find_entry(directory, Some(index), None, result)
    }
    #[inline(never)]
    fn find_entry(
        &mut self,
        directory: &str,
        index: Option<usize>,
        name: Option<&str>,
        result: &mut Entry,
    ) -> Result<bool, Error> {
        validate_path(directory)?;
        let readonly = self.is_read_only();
        self.run(|fs| {
            let dir = if directory.trim_matches('/').is_empty() {
                fs.root_dir()
            } else {
                fs.root_dir().open_dir(directory).map_err(Error::from)?
            };
            Self::scan_entry(&dir, index, name, result, readonly)
        })
    }
    // Resolving the parent and scanning it are sequential operations: keep
    // iterator/name temporaries out of the parent-resolution call chain.
    #[inline(never)]
    fn scan_entry(
        dir: &fatfs::Dir<'_, Disk<D>, super::fat_time::Clock, fatfs::LossyOemCpConverter>,
        index: Option<usize>,
        name: Option<&str>,
        result: &mut Entry,
        readonly: bool,
    ) -> Result<bool, Error> {
        let mut iterator = dir.iter();
        let mut position = 0;
        while let Some(found) = iterator
            .visit_next(|entry| -> Result<bool, Error> {
                if index.is_some_and(|wanted| position != wanted) {
                    return Ok(false);
                }
                result.name_len = 0;
                result.directory = entry.is_dir();
                result.read_only = readonly
                    || entry
                        .attributes()
                        .contains(fatfs::FileAttributes::READ_ONLY);
                result.size = entry.len();
                result.created = super::fat_time::decode(entry.created());
                result.accessed = super::fat_time::decode(fatfs::DateTime::new(
                    entry.accessed(),
                    fatfs::Time::new(0, 0, 0, 0),
                ));
                result.modified = super::fat_time::decode(entry.modified());
                if let Some(name) = entry.long_file_name_as_ucs2_units() {
                    for ch in core::char::decode_utf16(name.iter().copied()) {
                        let ch = ch.map_err(|_| Error::Corrupt)?;
                        let mut bytes = [0; 4];
                        let text = ch.encode_utf8(&mut bytes);
                        let end = result
                            .name_len
                            .checked_add(text.len())
                            .filter(|end| *end <= result.name.len())
                            .ok_or(Error::Corrupt)?;
                        result.name[result.name_len..end].copy_from_slice(text.as_bytes());
                        result.name_len = end;
                    }
                } else {
                    let name = entry.short_file_name_as_bytes();
                    if !name.is_ascii() {
                        return Err(Error::Unsupported);
                    }
                    result.name[..name.len()].copy_from_slice(name);
                    result.name_len = name.len();
                }
                if result.name_len == 0 || result.name().contains(['/', '\\', '\0']) {
                    return Err(Error::Corrupt);
                }
                if name.is_some_and(|wanted| {
                    !result
                        .name()
                        .chars()
                        .flat_map(char::to_uppercase)
                        .eq(wanted.chars().flat_map(char::to_uppercase))
                        && !entry
                            .short_file_name_as_bytes()
                            .eq_ignore_ascii_case(wanted.as_bytes())
                }) {
                    return Ok(false);
                }
                Ok(true)
            })
            .map_err(Error::from)?
        {
            if found? {
                return Ok(true);
            }
            position += 1;
        }
        Ok(false)
    }
    pub fn stat(&mut self, path: &str) -> Result<Entry, Error> {
        let mut result = Entry::empty();
        self.stat_into(path, &mut result)?;
        Ok(result)
    }
    /// Fill caller-owned storage with a single bounded name buffer.
    #[inline(never)]
    pub fn stat_into(&mut self, path: &str, result: &mut Entry) -> Result<(), Error> {
        validate_path(path)?;
        let path = path.trim_matches('/');
        if path.is_empty() {
            self.run(|_| Ok(()))?;
            *result = Entry::empty();
            result.read_only = self.is_read_only();
            return Ok(());
        }
        let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
        if self.find_entry(directory, None, Some(name), result)? {
            Ok(())
        } else {
            Err(Error::Missing)
        }
    }

    pub fn read_at(&mut self, path: &str, offset: u64, output: &mut [u8]) -> Result<usize, Error> {
        validate_path(path)?;
        if offset > u32::MAX as u64 {
            return Ok(0);
        }
        self.run(|fs| {
            let mut file = fs.root_dir().open_file(path).map_err(Error::from)?;
            let position = file.seek(SeekFrom::Start(offset)).map_err(Error::from)?;
            if position != offset {
                return Ok(0);
            }
            let mut done = 0;
            while done < output.len() {
                let n = file.read(&mut output[done..]).map_err(Error::from)?;
                if n == 0 {
                    break;
                }
                done += n;
            }
            Ok(done)
        })
    }
    pub fn write_at(&mut self, path: &str, offset: u64, input: &[u8]) -> Result<usize, Error> {
        validate_path(path)?;
        self.writable()?;
        if input.is_empty() {
            return Ok(0);
        }
        if offset
            .checked_add(input.len() as u64)
            .is_none_or(|end| end > u32::MAX as u64)
        {
            return Err(Error::InvalidInput);
        }
        self.run(|fs| {
            let mut file = fs.root_dir().open_file(path).map_err(Error::from)?;
            let position = file.seek(SeekFrom::Start(offset)).map_err(Error::from)?;
            let mut gap = offset - position;
            while gap > 0 {
                let n = gap.min(SECTOR_SIZE as u64) as usize;
                file.write_all(&ZERO_SECTOR[..n]).map_err(Error::from)?;
                gap -= n as u64;
            }
            file.write_all(input).map_err(Error::from)?;
            file.flush().map_err(Error::from)?;
            Ok(input.len())
        })
    }
    pub fn create(&mut self, path: &str, directory: bool) -> Result<(), Error> {
        validate_path(path)?;
        self.writable()?;
        match self.directory_kind(path) {
            Ok(_) => return Err(Error::Exists),
            Err(Error::Missing) => {}
            Err(e) => return Err(e),
        }
        self.run(|fs| {
            if directory {
                fs.root_dir().create_dir(path).map_err(Error::from)?;
            } else {
                fs.root_dir()
                    .create_file(path)
                    .map_err(Error::from)?
                    .flush()
                    .map_err(Error::from)?;
            }
            Ok(())
        })
    }
    // Metadata inspection and mutation are sequential. Do not retain the
    // long-name scratch in the frame that subsequently performs blocking I/O.
    #[inline(never)]
    fn directory_kind(&mut self, path: &str) -> Result<bool, Error> {
        validate_path(path)?;
        let path = path.trim_matches('/');
        self.run(|fs| {
            if path.is_empty() {
                return Ok(true);
            }
            let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
            let directory = if parent.is_empty() {
                fs.root_dir()
            } else {
                fs.root_dir().open_dir(parent).map_err(Error::from)?
            };
            Self::scan_directory_kind(&directory, name)
        })
    }
    #[inline(never)]
    fn scan_directory_kind(
        directory: &fatfs::Dir<'_, Disk<D>, super::fat_time::Clock, fatfs::LossyOemCpConverter>,
        wanted: &str,
    ) -> Result<bool, Error> {
        let mut iterator = directory.iter();
        while let Some(found) = iterator
            .visit_next(|entry| {
                Self::entry_name_matches(entry, wanted)
                    .map(|matched| matched.then(|| entry.is_dir()))
            })
            .map_err(Error::from)?
        {
            if let Some(directory) = found? {
                return Ok(directory);
            }
        }
        Err(Error::Missing)
    }

    // Preserve stat's fail-closed name validation without copying a full UTF-8
    // name into a 1020-byte metadata record merely to inspect the entry type.
    fn entry_name_matches(
        entry: &fatfs::DirEntry<'_, Disk<D>, super::fat_time::Clock, fatfs::LossyOemCpConverter>,
        wanted: &str,
    ) -> Result<bool, Error> {
        let matched = if let Some(units) = entry.long_file_name_as_ucs2_units() {
            let mut bytes = 0usize;
            for character in core::char::decode_utf16(units.iter().copied()) {
                let character = character.map_err(|_| Error::Corrupt)?;
                bytes += character.len_utf8();
                if bytes > 1020 || matches!(character, '/' | '\\' | '\0') {
                    return Err(Error::Corrupt);
                }
            }
            if bytes == 0 {
                return Err(Error::Corrupt);
            }
            // The immutable units were fully validated above. This second
            // traversal replaces stat's UTF-8 decode for case comparison.
            core::char::decode_utf16(units.iter().copied())
                .filter_map(Result::ok)
                .flat_map(char::to_uppercase)
                .eq(wanted.chars().flat_map(char::to_uppercase))
        } else {
            let short = entry.short_file_name_as_bytes();
            if !short.is_ascii() {
                return Err(Error::Unsupported);
            }
            if short.is_empty() || short.iter().any(|byte| matches!(byte, b'/' | b'\\' | 0)) {
                return Err(Error::Corrupt);
            }
            short.eq_ignore_ascii_case(wanted.as_bytes())
        };
        Ok(matched
            || entry
                .short_file_name_as_bytes()
                .eq_ignore_ascii_case(wanted.as_bytes()))
    }

    pub fn remove(&mut self, path: &str) -> Result<(), Error> {
        validate_path(path)?;
        self.writable()?;
        self.run(|fs| fs.root_dir().remove(path).map_err(Error::from))
    }
    pub fn set_times(
        &mut self,
        path: &str,
        accessed: Option<crate::time::Timestamp>,
        modified: Option<crate::time::Timestamp>,
    ) -> Result<(), Error> {
        validate_path(path)?;
        self.writable()?;
        // Validate all requested fields before changing any directory metadata.
        let accessed = accessed
            .map(|v| super::fat_time::encode(v).ok_or(Error::InvalidInput))
            .transpose()?;
        let modified = modified
            .map(|v| super::fat_time::encode(v).ok_or(Error::InvalidInput))
            .transpose()?;
        if self.directory_kind(path)? {
            return Err(Error::Unsupported);
        }
        self.run(|fs| {
            let mut file = fs.root_dir().open_file(path).map_err(Error::from)?;
            if let Some(value) = accessed {
                file.set_accessed(value.date);
            }
            if let Some(value) = modified {
                file.set_modified(value);
            }
            file.flush().map_err(Error::from)
        })
    }
    pub fn rename(&mut self, old: &str, new: &str) -> Result<(), Error> {
        validate_path(old)?;
        validate_path(new)?;
        self.writable()?;
        self.run(|fs| {
            fs.root_dir()
                .rename(old, &fs.root_dir(), new)
                .map_err(Error::from)
        })
    }
    pub fn resize(&mut self, path: &str, size: u64) -> Result<(), Error> {
        validate_path(path)?;
        self.writable()?;
        if size > u32::MAX as u64 {
            return Err(Error::InvalidInput);
        }
        self.run(|fs| {
            let mut file = fs.root_dir().open_file(path).map_err(Error::from)?;
            let position = file.seek(SeekFrom::Start(size)).map_err(Error::from)?;
            let mut remaining = size - position;
            while remaining > 0 {
                let n = remaining.min(SECTOR_SIZE as u64) as usize;
                file.write_all(&ZERO_SECTOR[..n]).map_err(Error::from)?;
                remaining -= n as u64;
            }
            file.truncate().map_err(Error::from)?;
            file.flush().map_err(Error::from)
        })
    }
    /// Commit `FSInfo` and directory metadata, issue the device durability fence,
    /// and retain the mounted metadata view. A failed sync leaves it closed.
    pub fn sync(&mut self) -> Result<(), Error> {
        // A read-only instance never dirties metadata. Avoid upstream unmount,
        // whose FSInfo cleanup can write even when no caller requested a write.
        if self.is_read_only() {
            return self.run(|_| Ok(()));
        }
        if let Some(error) = self.device.failure() {
            return Err(Error::Block(error));
        }
        let fs = self.fs.as_mut().ok_or(Error::Closed)?;
        self.device.remaining.store(self.budget, Ordering::Relaxed);
        self.device.enabled.store(true, Ordering::Relaxed);
        let result = (|| {
            // No upstream file/editor escapes a volume operation. All edits
            // have been flushed before this filesystem-wide synchronization.
            fs.sync().map_err(Error::from)?;
            // Disk::flush intentionally avoids barriers for ordinary file
            // drops. Only an explicit volume sync flushes the actual medium.
            self.device.access(BlockDevice::flush).map_err(Error::Block)
        })();
        match &result {
            Err(Error::Corrupt) => self.device.fail(BlockError::Corrupt),
            Err(Error::Block(error)) => self.device.fail(*error),
            _ => {}
        }
        self.device.enabled.store(false, Ordering::Relaxed);
        let result = match self.device.failure() {
            Some(error) => Err(Error::Block(error)),
            None => result,
        };
        if result.is_err() {
            self.close_failed_sync();
        }
        result
    }

    #[cold]
    #[inline(never)]
    fn close_failed_sync(&mut self) {
        // The caller has closed the I/O gate. Keep large FileSystem moves and its
        // best-effort destructor off the successful blocking sync stack.
        self.device.enabled.store(false, Ordering::Relaxed);
        drop(self.fs.take());
    }
}
impl<D: BlockDevice> Drop for FatVolume<D> {
    fn drop(&mut self) {
        self.device.enabled.store(false, Ordering::Relaxed);
    }
}
fn validate_path(path: &str) -> Result<(), Error> {
    if path.len() > 4096
        || path.as_bytes().contains(&0)
        || path
            .split('/')
            .any(|part| part == ".." || (part != "." && part.ends_with(['.', ' '])))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

/// Reject unsupported geometry before upstream arithmetic or table access.
pub fn validate_boot_sector(boot: &[u8; SECTOR_SIZE], volume_sectors: u64) -> Result<(), Error> {
    let u16_at = |at| u16::from_le_bytes([boot[at], boot[at + 1]]) as u64;
    let u32_at =
        |at| u32::from_le_bytes([boot[at], boot[at + 1], boot[at + 2], boot[at + 3]]) as u64;
    let cluster_sectors = boot[13] as u64;
    let reserved = u16_at(14);
    let fats = boot[16] as u64;
    let fat_sectors = u32_at(36);
    let total = u32_at(32);
    let overhead = reserved + fats * fat_sectors;
    if boot[510..512] != [0x55, 0xaa]
        || u16_at(11) != 512
        || !cluster_sectors.is_power_of_two()
        || cluster_sectors > 128
        || reserved == 0
        || !(1..=2).contains(&fats)
        || fat_sectors == 0
        || u16_at(17) != 0
        || u16_at(19) != 0
        || u16_at(22) != 0
        || u16_at(42) != 0
        || total > volume_sectors
        || total <= overhead
        || u16_at(48) >= reserved
        || fat_sectors > u32::MAX as u64 / 4096
    {
        return Err(Error::Corrupt);
    }
    let clusters = (total - overhead) / cluster_sectors;
    if !(65525..0x0fff_fff5).contains(&clusters)
        || (clusters + 2) * 4 > fat_sectors * 512
        || !(2..clusters + 2).contains(&u32_at(44))
    {
        return Err(Error::Corrupt);
    }
    Ok(())
}
