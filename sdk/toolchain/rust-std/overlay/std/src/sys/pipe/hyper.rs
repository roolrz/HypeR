// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::sync::{Arc, Mutex};
use crate::sys::pal::{cvt, ffi};

#[derive(Debug)]
struct Inner {
    handle: u64,
    input: Mutex<Input>,
}
#[derive(Debug)]
struct Input {
    bytes: Vec<u8>,
    start: usize,
    end: usize,
}
impl Drop for Inner {
    fn drop(&mut self) {
        unsafe { ffi::__hyper_std_fs_close(self.handle) };
    }
}

#[derive(Clone, Debug)]
pub struct Pipe(Arc<Inner>);

pub fn pipe() -> io::Result<(Pipe, Pipe)> {
    let (mut first, mut second) = (0, 0);
    cvt(unsafe { ffi::__hyper_std_pipe_create(&mut first, &mut second) })?;
    Ok((Pipe::from_owned(first), Pipe::from_owned(second)))
}

impl Pipe {
    pub(crate) fn from_owned(handle: u64) -> Self {
        Self(Arc::new(Inner {
            handle,
            input: Mutex::new(Input {
                bytes: Vec::new(),
                start: 0,
                end: 0,
            }),
        }))
    }
    pub(crate) fn handle(&self) -> u64 {
        self.0.handle
    }
    pub(crate) fn handle_for_inheritance(&self) -> io::Result<u64> {
        let input = self.0.input.lock().map_err(|_| io::ErrorKind::Other)?;
        // A partial Native message is already consumed from the kernel queue.
        // Do not silently discard its buffered suffix during child delegation.
        if input.start != input.end {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        Ok(self.0.handle)
    }
    pub(crate) fn drain_buffered(&self, output: &mut Vec<u8>) -> io::Result<()> {
        let mut input = self.0.input.lock().map_err(|_| io::ErrorKind::Other)?;
        output.extend_from_slice(&input.bytes[input.start..input.end]);
        input.start = input.end;
        Ok(())
    }
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(self.clone())
    }
    pub(crate) fn try_read(&self, output: &mut [u8]) -> io::Result<usize> {
        self.read_inner(output, false)
    }
    pub fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        self.read_inner(output, true)
    }
    fn read_inner(&self, output: &mut [u8], blocking: bool) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let mut input = self.0.input.lock().map_err(|_| io::ErrorKind::Other)?;
        if input.start == input.end {
            input.bytes.resize(65536, 0);
            let mut actual = 0;
            cvt(unsafe {
                let read = if blocking {
                    ffi::__hyper_std_pipe_read
                } else {
                    ffi::__hyper_std_pipe_try_read
                };
                read(
                    self.0.handle,
                    input.bytes.as_mut_ptr(),
                    input.bytes.len(),
                    &mut actual,
                )
            })?;
            if actual > input.bytes.len() {
                return Err(io::ErrorKind::InvalidData.into());
            }
            input.start = 0;
            input.end = actual;
        }
        let count = output.len().min(input.end - input.start);
        output[..count].copy_from_slice(&input.bytes[input.start..input.start + count]);
        input.start += count;
        Ok(count)
    }
    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        io::default_read_buf(|buffer| self.read(buffer), cursor)
    }
    pub fn read_vectored(&self, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        io::default_read_vectored(|buffer| self.read(buffer), buffers)
    }
    pub fn is_read_vectored(&self) -> bool {
        false
    }
    pub fn read_to_end(&self, output: &mut Vec<u8>) -> io::Result<usize> {
        let start = output.len();
        let mut bytes = [0; 4096];
        loop {
            let count = self.read(&mut bytes)?;
            if count == 0 {
                return Ok(output.len() - start);
            }
            output.extend_from_slice(&bytes[..count]);
        }
    }
    pub fn write(&self, input: &[u8]) -> io::Result<usize> {
        let mut actual = 0;
        cvt(unsafe {
            ffi::__hyper_std_pipe_write(self.0.handle, input.as_ptr(), input.len(), &mut actual)
        })?;
        if actual > input.len() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(actual)
    }
    pub fn write_vectored(&self, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
        io::default_write_vectored(|buffer| self.write(buffer), buffers)
    }
    pub fn is_write_vectored(&self) -> bool {
        false
    }
}
