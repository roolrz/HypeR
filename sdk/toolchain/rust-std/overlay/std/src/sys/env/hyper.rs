// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::ffi::{CStr, OsStr, OsString};
use crate::sys::pal::ffi;
use crate::{fmt, io, vec};

pub struct Env(vec::IntoIter<(OsString, OsString)>);
impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.as_slice().fmt(f)
    }
}
impl Iterator for Env {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
pub fn env() -> Env {
    let mut values = Vec::new();
    for index in 0.. {
        let pointer = unsafe { ffi::__hyper_std_environment(index) };
        if pointer.is_null() {
            break;
        }
        let bytes = unsafe { CStr::from_ptr(pointer.cast()).to_bytes() };
        if let Some(separator) = bytes.iter().position(|byte| *byte == b'=') {
            let key =
                unsafe { OsString::from_encoded_bytes_unchecked(bytes[..separator].to_vec()) };
            let value =
                unsafe { OsString::from_encoded_bytes_unchecked(bytes[separator + 1..].to_vec()) };
            values.push((key, value));
        }
    }
    Env(values.into_iter())
}
pub fn getenv(key: &OsStr) -> Option<OsString> {
    env().find_map(|(name, value)| if name == key { Some(value) } else { None })
}
pub unsafe fn setenv(_: &OsStr, _: &OsStr) -> io::Result<()> {
    Err(io::Error::UNSUPPORTED_PLATFORM)
}
pub unsafe fn unsetenv(_: &OsStr) -> io::Result<()> {
    Err(io::Error::UNSUPPORTED_PLATFORM)
}
