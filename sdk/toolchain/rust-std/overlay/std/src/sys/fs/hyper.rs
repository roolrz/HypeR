// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::ffi::OsString;
use crate::fmt;
use crate::fs::TryLockError;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, SeekFrom};
use crate::path::{Path, PathBuf};
use crate::sync::{Arc, Mutex};
use crate::sys::AsInner;
pub use crate::sys::fs::common::{Dir, exists};
use crate::sys::pal::{cvt, ffi, unsupported};
use crate::sys::time::SystemTime;
use crate::vec::Vec;

#[derive(Debug)]
struct Handle(u64);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { ffi::__hyper_std_fs_close(self.0) };
    }
}

#[derive(Debug)]
struct Description {
    handle: Handle,
    offset: Mutex<u64>,
    append: bool,
}
#[derive(Clone, Debug)]
pub struct File(Arc<Description>);
#[derive(Clone, Debug)]
pub struct FileAttr(ffi::FileInfo);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilePermissions(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct FileType(u32);
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    bits: u32,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct FileTimes {
    accessed: Option<SystemTime>,
    modified: Option<SystemTime>,
}
#[derive(Debug)]
pub struct DirBuilder;

fn path_bytes(path: &Path) -> io::Result<&[u8]> {
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    if path.is_empty() || path.as_bytes().contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(path.as_bytes())
}

impl FileAttr {
    pub fn identity(&self) -> (u64, u64) {
        (self.0.filesystem_id, self.0.node_id)
    }
    pub fn mode(&self) -> u32 {
        self.0.mode
    }
    pub fn size(&self) -> u64 {
        self.0.size
    }
    pub fn perm(&self) -> FilePermissions {
        FilePermissions(self.0.mode)
    }
    pub fn file_type(&self) -> FileType {
        FileType(self.0.kind)
    }
    pub fn modified(&self) -> io::Result<SystemTime> {
        metadata_time(
            self.0.valid_times,
            2,
            self.0.modified_seconds,
            self.0.modified_nanoseconds,
        )
    }
    pub fn accessed(&self) -> io::Result<SystemTime> {
        metadata_time(
            self.0.valid_times,
            1,
            self.0.accessed_seconds,
            self.0.accessed_nanoseconds,
        )
    }
    pub fn created(&self) -> io::Result<SystemTime> {
        metadata_time(
            self.0.valid_times,
            4,
            self.0.created_seconds,
            self.0.created_nanoseconds,
        )
    }
}
impl FilePermissions {
    pub fn mode(&self) -> u32 {
        self.0
    }
    pub fn from_mode(mode: u32) -> Self {
        Self(mode)
    }
    pub fn readonly(&self) -> bool {
        self.0 & 0o222 == 0
    }
    pub fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            self.0 &= !0o222;
        } else {
            self.0 |= 0o222;
        }
    }
}
impl FileType {
    pub fn is_file(&self) -> bool {
        self.0 == 1
    }
    pub fn is_dir(&self) -> bool {
        self.0 == 2
    }
    pub fn is_symlink(&self) -> bool {
        self.0 == 3
    }
}
impl FileTimes {
    pub fn set_accessed(&mut self, time: SystemTime) {
        self.accessed = Some(time);
    }
    pub fn set_modified(&mut self, time: SystemTime) {
        self.modified = Some(time);
    }
    fn update(self) -> ffi::FileUpdate {
        let mut update = ffi::FileUpdate::default();
        if let Some(time) = self.accessed {
            update.mask |= 2;
            (update.accessed_seconds, update.accessed_nanoseconds) = time.native();
        }
        if let Some(time) = self.modified {
            update.mask |= 4;
            (update.modified_seconds, update.modified_nanoseconds) = time.native();
        }
        update
    }
}
impl OpenOptions {
    pub fn new() -> Self {
        Self::default()
    }
    fn set(&mut self, bit: u32, enabled: bool) {
        if enabled {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
    }
    pub fn read(&mut self, value: bool) {
        self.set(1, value);
    }
    pub fn write(&mut self, value: bool) {
        self.set(2, value);
    }
    pub fn append(&mut self, value: bool) {
        self.set(4, value);
    }
    pub fn truncate(&mut self, value: bool) {
        self.set(8, value);
    }
    pub fn create(&mut self, value: bool) {
        self.set(16, value);
    }
    pub fn create_new(&mut self, value: bool) {
        self.set(32, value);
    }
}
impl File {
    pub fn open(path: &Path, options: &OpenOptions) -> io::Result<Self> {
        let path = path_bytes(path)?;
        let mut handle = 0;
        cvt(unsafe {
            ffi::__hyper_std_fs_open(path.as_ptr(), path.len(), options.bits, &mut handle)
        })?;
        if handle == 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Self(Arc::new(Description {
            handle: Handle(handle),
            offset: Mutex::new(0),
            append: options.bits & 4 != 0,
        })))
    }
    pub fn file_attr(&self) -> io::Result<FileAttr> {
        let mut info = ffi::FileInfo::default();
        cvt(unsafe { ffi::__hyper_std_fs_info(self.0.handle.0, &mut info) })?;
        FileAttr::checked(info)
    }
    pub fn fsync(&self) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_fs_sync(self.0.handle.0, 1) })
    }
    pub fn datasync(&self) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_fs_sync(self.0.handle.0, 0) })
    }
    pub fn lock(&self) -> io::Result<()> {
        self.acquire_lock(1, u64::MAX)
    }
    pub fn lock_shared(&self) -> io::Result<()> {
        self.acquire_lock(0, u64::MAX)
    }
    fn acquire_lock(&self, mode: u32, deadline: u64) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_fs_lock(self.0.handle.0, mode, deadline) })
    }
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        self.try_acquire_lock(1)
    }
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        self.try_acquire_lock(0)
    }
    fn try_acquire_lock(&self, mode: u32) -> Result<(), TryLockError> {
        match self.acquire_lock(mode, 0) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Err(TryLockError::WouldBlock)
            }
            Err(error) => Err(TryLockError::Error(error)),
        }
    }
    pub fn unlock(&self) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_fs_unlock(self.0.handle.0) })
    }
    pub fn truncate(&self, size: u64) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_fs_resize(self.0.handle.0, size) })
    }
    pub fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        let mut offset = self.0.offset.lock().map_err(|_| io::ErrorKind::Other)?;
        let mut actual = 0;
        cvt(unsafe {
            ffi::__hyper_std_fs_read(
                self.0.handle.0,
                *offset,
                output.as_mut_ptr(),
                output.len(),
                &mut actual,
            )
        })?;
        if actual > output.len() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        *offset = offset
            .checked_add(actual as u64)
            .ok_or(io::ErrorKind::InvalidData)?;
        Ok(actual)
    }
    pub fn read_vectored(&self, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        io::default_read_vectored(|buffer| self.read(buffer), buffers)
    }
    pub fn is_read_vectored(&self) -> bool {
        false
    }
    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        io::default_read_buf(|buffer| self.read(buffer), cursor)
    }
    pub fn write(&self, input: &[u8]) -> io::Result<usize> {
        if input.is_empty() {
            return Ok(0);
        }
        let mut offset = self.0.offset.lock().map_err(|_| io::ErrorKind::Other)?;
        let mut actual = 0;
        let mut end = 0;
        cvt(unsafe {
            ffi::__hyper_std_fs_write(
                self.0.handle.0,
                *offset,
                self.0.append as u32,
                input.as_ptr(),
                input.len(),
                &mut actual,
                &mut end,
            )
        })?;
        if actual > input.len() || end < actual as u64 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        *offset = end;
        Ok(actual)
    }
    pub fn write_vectored(&self, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
        io::default_write_vectored(|buffer| self.write(buffer), buffers)
    }
    pub fn is_write_vectored(&self) -> bool {
        false
    }
    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }
    pub fn seek(&self, position: SeekFrom) -> io::Result<u64> {
        let mut offset = self.0.offset.lock().map_err(|_| io::ErrorKind::Other)?;
        let next = match position {
            SeekFrom::Start(value) => Some(value),
            SeekFrom::Current(delta) => offset.checked_add_signed(delta),
            SeekFrom::End(delta) => self.file_attr()?.size().checked_add_signed(delta),
        }
        .ok_or(io::ErrorKind::InvalidInput)?;
        *offset = next;
        Ok(next)
    }
    pub fn size(&self) -> Option<io::Result<u64>> {
        Some(self.file_attr().map(|info| info.size()))
    }
    pub fn tell(&self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
    pub fn duplicate(&self) -> io::Result<Self> {
        Ok(self.clone())
    }
    pub fn set_permissions(&self, permissions: FilePermissions) -> io::Result<()> {
        let update = ffi::FileUpdate {
            mask: 1,
            mode: permissions.0 & 0o7777,
            ..Default::default()
        };
        cvt(unsafe { ffi::__hyper_std_fs_set_info(self.0.handle.0, &update) })
    }
    pub fn set_times(&self, times: FileTimes) -> io::Result<()> {
        cvt(unsafe { ffi::__hyper_std_fs_set_info(self.0.handle.0, &times.update()) })
    }
}

pub struct ReadDir {
    handle: Arc<Handle>,
    path: Arc<PathBuf>,
    entries: [ffi::DirectoryEntry; 4],
    index: usize,
    count: usize,
    next: Option<u64>,
}
impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadDir").field("path", &self.path).finish()
    }
}
impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.count {
            let cookie = self.next.take()?;
            let mut next = 0;
            self.entries = [ffi::DirectoryEntry::EMPTY; 4];
            if let Err(error) = cvt(unsafe {
                ffi::__hyper_std_fs_readdir(
                    self.handle.0,
                    cookie,
                    self.entries.as_mut_ptr(),
                    &mut self.count,
                    &mut next,
                )
            }) {
                return Some(Err(error));
            }
            if self.count > 4 || (next != 0 && next <= cookie) {
                return Some(Err(io::ErrorKind::InvalidData.into()));
            }
            self.next = if next == 0 { None } else { Some(next) };
            self.index = 0;
            if self.count == 0 {
                return None;
            }
        }
        let entry = self.entries[self.index];
        self.index += 1;
        let length = entry.name_length as usize;
        if length == 0 || length > 255 || entry.reserved != 0 {
            return Some(Err(io::ErrorKind::InvalidData.into()));
        }
        let name = match crate::str::from_utf8(&entry.name[..length]) {
            Ok(name) if name != "." && name != ".." && !name.contains(['/', '\0']) => {
                OsString::from(name)
            }
            _ => return Some(Err(io::ErrorKind::InvalidData.into())),
        };
        Some(Ok(DirEntry {
            directory: self.handle.clone(),
            parent: self.path.clone(),
            name,
            attributes: FileAttr(ffi::FileInfo {
                size: entry.size,
                mode: entry.mode,
                kind: entry.kind,
                ..Default::default()
            }),
        }))
    }
}
pub struct DirEntry {
    directory: Arc<Handle>,
    parent: Arc<PathBuf>,
    name: OsString,
    attributes: FileAttr,
}
impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.parent.join(&self.name)
    }
    pub fn file_name(&self) -> OsString {
        self.name.clone()
    }
    pub fn metadata(&self) -> io::Result<FileAttr> {
        let mut info = ffi::FileInfo::default();
        let name = self.name.as_encoded_bytes();
        cvt(unsafe {
            ffi::__hyper_std_fs_stat_at(self.directory.0, name.as_ptr(), name.len(), 1, &mut info)
        })?;
        FileAttr::checked(info)
    }
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.attributes.file_type())
    }
}
impl DirBuilder {
    pub fn new() -> Self {
        Self
    }
    pub fn mkdir(&self, path: &Path) -> io::Result<()> {
        let path = path_bytes(path)?;
        cvt(unsafe { ffi::__hyper_std_fs_mkdir(path.as_ptr(), path.len()) })
    }
}
pub fn readdir(path: &Path) -> io::Result<ReadDir> {
    let bytes = path_bytes(path)?;
    let mut handle = 0;
    cvt(unsafe { ffi::__hyper_std_fs_directory(bytes.as_ptr(), bytes.len(), &mut handle) })?;
    Ok(ReadDir {
        handle: Arc::new(Handle(handle)),
        path: Arc::new(path.to_owned()),
        entries: [ffi::DirectoryEntry::EMPTY; 4],
        index: 0,
        count: 0,
        next: Some(0),
    })
}
pub fn stat(path: &Path) -> io::Result<FileAttr> {
    let path = path_bytes(path)?;
    let mut info = ffi::FileInfo::default();
    cvt(unsafe { ffi::__hyper_std_fs_stat(path.as_ptr(), path.len(), 0, &mut info) })?;
    FileAttr::checked(info)
}
pub fn unlink(path: &Path) -> io::Result<()> {
    remove(path, false)
}
pub fn rmdir(path: &Path) -> io::Result<()> {
    remove(path, true)
}
fn remove(path: &Path, directory: bool) -> io::Result<()> {
    let path = path_bytes(path)?;
    cvt(unsafe { ffi::__hyper_std_fs_remove(path.as_ptr(), path.len(), directory as u32) })
}
pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    two_paths(from, to, false)
}
fn two_paths(from: &Path, to: &Path, hardlink: bool) -> io::Result<()> {
    let from = path_bytes(from)?;
    let to = path_bytes(to)?;
    cvt(unsafe {
        ffi::__hyper_std_fs_rename(
            from.as_ptr(),
            from.len(),
            to.as_ptr(),
            to.len(),
            u32::from(hardlink),
        )
    })
}
pub fn set_perm(path: &Path, permissions: FilePermissions) -> io::Result<()> {
    let update = ffi::FileUpdate {
        mask: 1,
        mode: permissions.0 & 0o7777,
        ..Default::default()
    };
    set_path_info(path, 0, &update)
}
fn set_path_info(path: &Path, options: u32, update: &ffi::FileUpdate) -> io::Result<()> {
    let path = path_bytes(path)?;
    cvt(unsafe { ffi::__hyper_std_fs_set_path_info(path.as_ptr(), path.len(), options, update) })
}
pub fn set_times(path: &Path, times: FileTimes) -> io::Result<()> {
    set_path_info(path, 0, &times.update())
}
pub fn set_times_nofollow(path: &Path, times: FileTimes) -> io::Result<()> {
    set_path_info(path, 1, &times.update())
}
pub fn readlink(path: &Path) -> io::Result<PathBuf> {
    path_result(path, false)
}
pub fn symlink(target: &Path, path: &Path) -> io::Result<()> {
    let target = path_bytes(target)?;
    let path = path_bytes(path)?;
    cvt(unsafe {
        ffi::__hyper_std_fs_symlink(target.as_ptr(), target.len(), path.as_ptr(), path.len())
    })
}
pub fn link(from: &Path, to: &Path) -> io::Result<()> {
    two_paths(from, to, true)
}
pub fn lstat(path: &Path) -> io::Result<FileAttr> {
    let path = path_bytes(path)?;
    let mut info = ffi::FileInfo::default();
    cvt(unsafe { ffi::__hyper_std_fs_stat(path.as_ptr(), path.len(), 1, &mut info) })?;
    FileAttr::checked(info)
}
pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    path_result(path, true)
}
fn path_result(path: &Path, canonical: bool) -> io::Result<PathBuf> {
    let path = path_bytes(path)?;
    let mut bytes = vec![0; 4096];
    let mut actual = 0;
    cvt(unsafe {
        ffi::__hyper_std_fs_path(
            path.as_ptr(),
            path.len(),
            u32::from(canonical),
            bytes.as_mut_ptr(),
            bytes.len(),
            &mut actual,
        )
    })?;
    if actual == 0 || actual > bytes.len() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    bytes.truncate(actual);
    let value = crate::string::String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData)?;
    if value.as_bytes().contains(&0) || (canonical && !value.starts_with('/')) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(PathBuf::from(value))
}
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    let mut source = crate::fs::File::open(from)?;
    let metadata = source.metadata()?;
    if !metadata.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut target = crate::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(to)?;
    // Check opened objects before truncation, including hard links and aliases.
    if metadata.as_inner().identity() == target.metadata()?.as_inner().identity() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source and destination are the same file",
        ));
    }
    target.set_len(0)?;
    let copied = io::copy(&mut source, &mut target)?;
    target.set_permissions(metadata.permissions())?;
    Ok(copied)
}

fn metadata_time(mask: u32, bit: u32, seconds: i64, nanos: u32) -> io::Result<SystemTime> {
    if mask & bit == 0 {
        return unsupported();
    }
    SystemTime::from_native(seconds, nanos).ok_or_else(|| io::ErrorKind::InvalidData.into())
}
impl FileAttr {
    fn checked(info: ffi::FileInfo) -> io::Result<Self> {
        if info.filesystem_id == 0
            || info.mount_id == 0
            || info.reserved != 0
            || info.valid_times & !15 != 0
            || !(1..=4).contains(&info.kind)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        for (bit, seconds, nanos, reserved) in [
            (
                1,
                info.accessed_seconds,
                info.accessed_nanoseconds,
                info.accessed_reserved,
            ),
            (
                2,
                info.modified_seconds,
                info.modified_nanoseconds,
                info.modified_reserved,
            ),
            (
                4,
                info.created_seconds,
                info.created_nanoseconds,
                info.created_reserved,
            ),
            (
                8,
                info.changed_seconds,
                info.changed_nanoseconds,
                info.changed_reserved,
            ),
        ] {
            if reserved != 0
                || nanos >= 1_000_000_000
                || (info.valid_times & bit == 0 && (seconds != 0 || nanos != 0))
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
        }
        Ok(Self(info))
    }
}

// Traversal owns each opened directory. It never resolves a descendant through
// a pathname that another thread can replace with a symlink. Conditional final
// removal refuses a name that stopped naming the directory we traversed.
pub fn remove_dir_all(path: &Path) -> io::Result<()> {
    let bytes = path_bytes(path)?;
    let trailing_slash = bytes.last() == Some(&b'/');
    let end = bytes
        .iter()
        .rposition(|byte| *byte != b'/')
        .map_or(0, |index| index + 1);
    let trimmed = &bytes[..end];
    let (parent_path, name) = match trimmed.iter().rposition(|byte| *byte == b'/') {
        Some(0) => (b"/".as_slice(), &trimmed[1..]),
        Some(index) => (&trimmed[..index], &trimmed[index + 1..]),
        None => (b".".as_slice(), trimmed),
    };
    // Mutation names cannot denote the traversal root or dot components.
    // Reject them before descending, so failure cannot leave a partially
    // emptied root or current directory.
    if name.is_empty() || name == b"." || name == b".." {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut scope = 0;
    cvt(unsafe { ffi::__hyper_std_fs_acquire(bytes.as_ptr(), bytes.len(), &mut scope) })?;
    let scope = Handle(scope);
    // The parent's final symlink is an intermediate component of the original
    // path. Resolve it once, then retain this exact directory for final removal.
    let parent = open_directory_at(&scope, parent_path, false)?;
    let info = stat_at(&parent, name)?;
    if trailing_slash && !info.file_type().is_dir() {
        return Err(io::ErrorKind::NotADirectory.into());
    }
    if info.file_type().is_symlink() {
        return remove_observed(&parent, name, &info);
    }
    if !info.file_type().is_dir() {
        return Err(io::ErrorKind::NotADirectory.into());
    }
    let directory = open_directory_at(&parent, name, true)?;
    let opened = directory_info(&directory)?;
    if !same_location(&opened, &info) {
        return Err(io::ErrorKind::ResourceBusy.into());
    }
    let mut stack = Vec::new();
    stack.push(RemovalFrame {
        directory,
        parent,
        name: name.to_vec(),
        identity: opened,
        cookie: Some(0),
    });
    while let Some(frame) = stack.last_mut() {
        let Some(cookie) = frame.cookie.take() else {
            let frame = match stack.pop() {
                Some(frame) => frame,
                None => break,
            };
            remove_observed(&frame.parent, &frame.name, &frame.identity)?;
            continue;
        };
        let mut entries = [ffi::DirectoryEntry::EMPTY; 4];
        let mut count = 0;
        let mut next = 0;
        cvt(unsafe {
            ffi::__hyper_std_fs_readdir(
                frame.directory.0,
                cookie,
                entries.as_mut_ptr(),
                &mut count,
                &mut next,
            )
        })?;
        if count > 4 || (next != 0 && next <= cookie) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        // Process just the first remaining child, then restart enumeration.
        // This keeps directory handles proportional to depth, not entry count.
        if count == 0 {
            continue;
        }
        frame.cookie = Some(0);
        let entry = &entries[0];
        let length = entry.name_length as usize;
        if length == 0 || length > 255 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let name = &entry.name[..length];
        if name == b"." || name == b".." || name.iter().any(|byte| *byte == b'/' || *byte == 0) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let info = stat_at(&frame.directory, name)?;
        if !info.file_type().is_dir() {
            remove_observed(&frame.directory, name, &info)?;
            continue;
        }
        let child = open_directory_at(&frame.directory, name, true)?;
        let opened = directory_info(&child)?;
        if !same_location(&opened, &info) {
            return Err(io::ErrorKind::ResourceBusy.into());
        }
        // A parent owner for the child frame is opened as '.', avoiding a
        // global descriptor registry or a borrow invalidated by stack growth.
        let parent = open_directory_at(&frame.directory, b".", true)?;
        stack.push(RemovalFrame {
            directory: child,
            parent,
            name: name.to_vec(),
            identity: opened,
            cookie: Some(0),
        });
    }
    Ok(())
}
struct RemovalFrame {
    directory: Handle,
    parent: Handle,
    name: Vec<u8>,
    identity: FileAttr,
    cookie: Option<u64>,
}
fn stat_at(parent: &Handle, name: &[u8]) -> io::Result<FileAttr> {
    let mut info = ffi::FileInfo::default();
    cvt(unsafe { ffi::__hyper_std_fs_stat_at(parent.0, name.as_ptr(), name.len(), 1, &mut info) })?;
    FileAttr::checked(info)
}
fn directory_info(directory: &Handle) -> io::Result<FileAttr> {
    let mut info = ffi::FileInfo::default();
    cvt(unsafe { ffi::__hyper_std_fs_self_info(directory.0, &mut info) })?;
    FileAttr::checked(info)
}
fn open_directory_at(parent: &Handle, name: &[u8], nofollow: bool) -> io::Result<Handle> {
    let mut handle = 0;
    cvt(unsafe {
        ffi::__hyper_std_fs_open_directory_at(
            parent.0,
            name.as_ptr(),
            name.len(),
            u32::from(nofollow),
            &mut handle,
        )
    })?;
    if handle == 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Handle(handle))
}
fn remove_observed(parent: &Handle, name: &[u8], info: &FileAttr) -> io::Result<()> {
    cvt(unsafe {
        ffi::__hyper_std_fs_remove_if(
            parent.0,
            name.as_ptr(),
            name.len(),
            u32::from(info.file_type().is_dir()),
            info.0.node_id,
        )
    })
}

fn same_location(left: &FileAttr, right: &FileAttr) -> bool {
    (left.0.filesystem_id, left.0.mount_id, left.0.node_id)
        == (right.0.filesystem_id, right.0.mount_id, right.0.node_id)
}
