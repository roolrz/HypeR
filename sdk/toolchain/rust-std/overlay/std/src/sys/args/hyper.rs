// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub use super::common::Args;
use crate::ffi::{CStr, OsString};
use crate::sys::pal::ffi;

pub fn args() -> Args {
    let mut values = Vec::new();
    loop {
        let pointer = unsafe { ffi::__hyper_std_argument(values.len()) };
        if pointer.is_null() {
            break;
        }
        let bytes = unsafe { CStr::from_ptr(pointer.cast()).to_bytes() };
        values.push(unsafe { OsString::from_encoded_bytes_unchecked(bytes.to_vec()) });
    }
    Args::new(values)
}
