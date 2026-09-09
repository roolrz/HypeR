// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod ffi;
pub mod futex;
pub fn unsupported<T>() -> crate::io::Result<T> {
    Err(unsupported_err())
}
pub fn unsupported_err() -> crate::io::Error {
    crate::io::Error::UNSUPPORTED_PLATFORM
}

pub unsafe fn init(_argc: isize, _argv: *const *const u8, _sigpipe: u8) {
    // Native CRT initializes the process runtime before Rust's generated main.
}
pub unsafe fn cleanup() {}
pub fn abort_internal() -> ! {
    unsafe { ffi::__hyper_std_exit(1) }
}

pub fn cvt(status: i64) -> crate::io::Result<()> {
    use crate::io::{Error, ErrorKind};
    if status == 0 {
        return Ok(());
    }
    let kind = match status {
        -1 | -14 => ErrorKind::InvalidInput,
        -3 => ErrorKind::PermissionDenied,
        -9 => ErrorKind::ResourceBusy,
        -4 => ErrorKind::Unsupported,
        -5 => ErrorKind::OutOfMemory,
        -11 => ErrorKind::TimedOut,
        -13 => ErrorKind::WouldBlock,
        -15 => ErrorKind::BrokenPipe,
        -16 => ErrorKind::NotFound,
        _ => ErrorKind::Other,
    };
    // In particular, reporting allocation failure must not allocate again.
    Err(Error::from(kind))
}
