// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::io::{self, Read, Write};
use crate::sys::pal::{cvt, ffi};

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;
pub const STDIN_BUF_SIZE: usize = 8192;
impl Stdin {
    pub const fn new() -> Self {
        Self
    }
}
impl Stdout {
    pub const fn new() -> Self {
        Self
    }
}
impl Stderr {
    pub const fn new() -> Self {
        Self
    }
}
impl Read for Stdin {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut actual = 0;
        cvt(unsafe { ffi::__hyper_std_read(buffer.as_mut_ptr(), buffer.len(), &mut actual) })?;
        Ok(actual)
    }
}
macro_rules! output {
    ($ty:ty, $stream:expr) => {
        impl Write for $ty {
            fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
                let mut actual = 0;
                cvt(unsafe {
                    ffi::__hyper_std_write($stream, buffer.as_ptr(), buffer.len(), &mut actual)
                })?;
                Ok(actual)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
    };
}
output!(Stdout, 1);
output!(Stderr, 2);
pub fn is_ebadf(_: &io::Error) -> bool {
    false
}
pub fn panic_output() -> Option<Stderr> {
    Some(Stderr)
}
