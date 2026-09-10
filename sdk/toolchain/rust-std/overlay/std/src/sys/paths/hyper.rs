// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::ffi::{OsStr, OsString};
use crate::path::{Path, PathBuf};
use crate::sys::pal::{cvt, ffi, unsupported};
use crate::{fmt, io};

pub fn getcwd() -> io::Result<PathBuf> {
    crate::sys::fs::canonicalize(Path::new("."))
}
pub fn chdir(path: &Path) -> io::Result<()> {
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?.as_bytes();
    if path.is_empty() || path.contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    cvt(unsafe { ffi::__hyper_std_fs_chdir(path.as_ptr(), path.len()) })
}
pub struct SplitPaths<'a>(crate::slice::Split<'a, u8, fn(&u8) -> bool>);
pub fn split_paths(value: &OsStr) -> SplitPaths<'_> {
    SplitPaths(
        value
            .as_encoded_bytes()
            .split((|byte| *byte == b':') as fn(&u8) -> bool),
    )
}
impl Iterator for SplitPaths<'_> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        // SAFETY: ':' is an ASCII separator, so splitting encoded OS strings
        // preserves the encoding boundary guaranteed by OsStr.
        self.0
            .next()
            .map(|part| PathBuf::from(unsafe { OsStr::from_encoded_bytes_unchecked(part) }))
    }
}
#[derive(Debug)]
pub struct JoinPathsError;
pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut output = crate::vec::Vec::new();
    for (index, path) in paths.enumerate() {
        let bytes = path.as_ref().as_encoded_bytes();
        if bytes.contains(&b':') {
            return Err(JoinPathsError);
        }
        if index != 0 {
            output.push(b':');
        }
        output.extend_from_slice(bytes);
    }
    // SAFETY: concatenation with ASCII separators preserves encoded OS strings.
    Ok(unsafe { OsString::from_encoded_bytes_unchecked(output) })
}
impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("path contains ':' separator")
    }
}
impl crate::error::Error for JoinPathsError {}
pub fn current_exe() -> io::Result<PathBuf> {
    unsupported()
}
pub fn temp_dir() -> PathBuf {
    crate::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
}
pub fn home_dir() -> Option<PathBuf> {
    crate::env::var_os("HOME").map(PathBuf::from)
}
