// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::marker::PhantomData;
use core::num::NonZeroU64;

use crate::{Error, Result, Status};

const _: () = assert!(hyper_abi::HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES <= usize::MAX as u64);
const MAX_TRANSFER_BYTES: usize = hyper_abi::HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES as usize;

/// Borrowed Console capability supplied by the Native process runtime.
#[derive(Clone, Copy)]
pub struct Console<'startup> {
    raw: NonZeroU64,
    _startup: PhantomData<&'startup ()>,
}

impl<'startup> Console<'startup> {
    pub(crate) const fn from_startup(raw: NonZeroU64) -> Self {
        Self {
            raw,
            _startup: PhantomData,
        }
    }

    /// Waits for input and returns at least one byte unless `bytes` is empty.
    pub fn read_blocking(&self, bytes: &mut [u8]) -> Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let capacity = bytes.len().min(MAX_TRANSFER_BYTES);
        loop {
            // SAFETY: this borrowed Console cannot outlive the startup handle;
            // `bytes` is uniquely borrowed and writable for `capacity` bytes.
            let result =
                unsafe { hyper_sys::console_read(self.raw.get(), bytes.as_mut_ptr(), capacity) };
            let status = Status::from_raw(result.status);
            match status {
                Status::OK | Status::WOULD_BLOCK => {
                    let transferred = decode_transfer_count(result.value0, capacity)?;
                    if transferred != 0 {
                        return Ok(transferred);
                    }
                    self.wait_readable()?;
                }
                failure => return Err(Error::Status(failure)),
            }
        }
    }

    /// Writes every byte, waiting when the Console applies backpressure.
    pub fn write_all(&self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            let count = bytes.len().min(MAX_TRANSFER_BYTES);
            // SAFETY: this borrowed Console cannot outlive the startup handle;
            // `bytes` is readable for the selected prefix during the syscall.
            let result = unsafe { hyper_sys::console_write(self.raw.get(), bytes.as_ptr(), count) };
            let status = Status::from_raw(result.status);
            match status {
                Status::OK | Status::WOULD_BLOCK => {
                    let transferred = decode_transfer_count(result.value0, count)?;
                    bytes = bytes.get(transferred..).ok_or(Error::InvalidResponse)?;
                    if status == Status::OK && transferred == 0 {
                        return Err(Error::InvalidResponse);
                    }
                    if status == Status::WOULD_BLOCK && !bytes.is_empty() {
                        self.wait_writable()?;
                    }
                }
                failure => return Err(Error::Status(failure)),
            }
        }
        Ok(())
    }

    fn wait_readable(&self) -> Result<()> {
        self.wait_for(hyper_abi::HYPER_NATIVE_SIGNAL_CONSOLE_READABLE)
    }

    fn wait_writable(&self) -> Result<()> {
        self.wait_for(hyper_abi::HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE)
    }

    fn wait_for(&self, signals: u64) -> Result<()> {
        // SAFETY: the borrowed Console handle remains live for the syscall and
        // Console objects implement the waitable contract.
        let result = unsafe {
            hyper_sys::object_wait_one(
                self.raw.get(),
                signals,
                hyper_abi::HYPER_NATIVE_DEADLINE_INFINITE,
            )
        };
        Status::from_raw(result.status).into_result()?;
        if result.value0 & signals != signals {
            return Err(Error::InvalidResponse);
        }
        Ok(())
    }
}

fn decode_transfer_count(raw: u64, limit: usize) -> Result<usize> {
    let count = usize::try_from(raw).map_err(|_| Error::InvalidResponse)?;
    (count <= limit)
        .then_some(count)
        .ok_or(Error::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::decode_transfer_count;
    use crate::Error;

    #[test]
    fn transfer_count_accepts_the_inclusive_limit() {
        assert_eq!(decode_transfer_count(32, 32), Ok(32));
    }

    #[test]
    fn transfer_count_rejects_kernel_overrun() {
        assert_eq!(decode_transfer_count(33, 32), Err(Error::InvalidResponse));
    }
}
