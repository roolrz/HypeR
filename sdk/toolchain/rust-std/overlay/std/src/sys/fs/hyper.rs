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
pub struct FileTimes;
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
        unsupported()
    }
    pub fn accessed(&self) -> io::Result<SystemTime> {
        unsupported()
    }
    pub fn created(&self) -> io::Result<SystemTime> {
        unsupported()
    }
}
impl FilePermissions {
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
    pub fn set_accessed(&mut self, _: SystemTime) {}
    pub fn set_modified(&mut self, _: SystemTime) {}
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
        Ok(FileAttr(info))
    }
    pub fn fsync(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn datasync(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn lock(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn lock_shared(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(io::Error::UNSUPPORTED_PLATFORM))
    }
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        self.try_lock()
    }
    pub fn unlock(&self) -> io::Result<()> {
        unsupported()
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
    pub fn set_permissions(&self, _: FilePermissions) -> io::Result<()> {
        unsupported()
    }
    pub fn set_times(&self, _: FileTimes) -> io::Result<()> {
        unsupported()
    }
}

pub struct ReadDir {
    handle: Handle,
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
            parent: self.path.clone(),
            name,
            attributes: FileAttr(ffi::FileInfo {
                size: entry.size,
                mode: entry.mode,
                kind: entry.kind,
            }),
        }))
    }
}
pub struct DirEntry {
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
        Ok(self.attributes.clone())
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
        handle: Handle(handle),
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
    cvt(unsafe { ffi::__hyper_std_fs_stat(path.as_ptr(), path.len(), &mut info) })?;
    Ok(FileAttr(info))
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
pub fn rename(_: &Path, _: &Path) -> io::Result<()> {
    unsupported()
}
pub fn set_perm(_: &Path, _: FilePermissions) -> io::Result<()> {
    unsupported()
}
pub fn set_times(_: &Path, _: FileTimes) -> io::Result<()> {
    unsupported()
}
pub fn set_times_nofollow(_: &Path, _: FileTimes) -> io::Result<()> {
    unsupported()
}
pub fn remove_dir_all(_: &Path) -> io::Result<()> {
    unsupported()
}
pub fn readlink(_: &Path) -> io::Result<PathBuf> {
    unsupported()
}
pub fn symlink(_: &Path, _: &Path) -> io::Result<()> {
    unsupported()
}
pub fn link(_: &Path, _: &Path) -> io::Result<()> {
    unsupported()
}
pub fn lstat(_: &Path) -> io::Result<FileAttr> {
    unsupported()
}
pub fn canonicalize(_: &Path) -> io::Result<PathBuf> {
    unsupported()
}
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    let mut source = crate::fs::File::open(from)?;
    // Native has no permission-mutation operation yet. Never claim a copy
    // succeeded after silently stripping executable or read-only permissions.
    let permissions = source.metadata()?.permissions();
    if permissions.as_inner().0 & 0o777 != 0o666 {
        return unsupported();
    }
    let mut target = crate::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(to)?;
    if target.metadata()?.permissions().as_inner().0 & 0o777 != 0o666 {
        return unsupported();
    }
    target.set_len(0)?;
    io::copy(&mut source, &mut target)
}
