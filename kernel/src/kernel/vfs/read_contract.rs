// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded arithmetic and backend-result contracts for file reads.

use hyper::mm::PAGE_SIZE;

const PAGE_BYTES: usize = PAGE_SIZE as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    ArithmeticOverflow,
    InvalidBackendResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadProgress {
    Complete,
    Partial,
}

/// One request fragment contained within a single file-data page and EOF.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReadPlan {
    offset: u64,
    page_index: u64,
    page_offset: u64,
    within_page: usize,
    output_len: usize,
    fill_len: usize,
}

impl ReadPlan {
    /// Plans the next bounded fragment, or returns `None` at EOF/no output.
    pub(crate) fn next(
        offset: u64,
        file_len: u64,
        remaining_output: usize,
    ) -> Result<Option<Self>, Error> {
        if remaining_output == 0 || offset >= file_len {
            return Ok(None);
        }

        let page_index = offset / PAGE_SIZE;
        let page_offset = page_index
            .checked_mul(PAGE_SIZE)
            .ok_or(Error::ArithmeticOverflow)?;
        let within_page = usize::try_from(
            offset
                .checked_sub(page_offset)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .map_err(|_| Error::ArithmeticOverflow)?;
        let page_remaining = PAGE_BYTES
            .checked_sub(within_page)
            .ok_or(Error::ArithmeticOverflow)?;
        let file_remaining = file_len
            .checked_sub(offset)
            .ok_or(Error::ArithmeticOverflow)?;
        let file_remaining = usize::try_from(file_remaining.min(PAGE_SIZE))
            .map_err(|_| Error::ArithmeticOverflow)?;
        let output_len = remaining_output.min(page_remaining).min(file_remaining);

        let fill_remaining = file_len
            .checked_sub(page_offset)
            .ok_or(Error::ArithmeticOverflow)?;
        let fill_len = usize::try_from(fill_remaining.min(PAGE_SIZE))
            .map_err(|_| Error::ArithmeticOverflow)?;

        Ok(Some(Self {
            offset,
            page_index,
            page_offset,
            within_page,
            output_len,
            fill_len,
        }))
    }

    pub(crate) const fn offset(self) -> u64 {
        self.offset
    }

    pub(crate) const fn page_index(self) -> u64 {
        self.page_index
    }

    pub(crate) const fn page_offset(self) -> u64 {
        self.page_offset
    }

    pub(crate) const fn within_page(self) -> usize {
        self.within_page
    }

    pub(crate) const fn output_len(self) -> usize {
        self.output_len
    }

    /// Validates a backend result for the bounded output fragment.
    ///
    /// Short reads are valid and terminate the outer read operation. A backend
    /// can never claim more bytes than the destination fragment it received.
    pub(crate) const fn validate_read(self, actual: usize) -> Result<ReadProgress, Error> {
        if actual > self.output_len {
            return Err(Error::InvalidBackendResult);
        }
        if actual < self.output_len {
            Ok(ReadProgress::Partial)
        } else {
            Ok(ReadProgress::Complete)
        }
    }

    /// Validates the complete page-at-a-time contract used for cache fills.
    pub(crate) const fn validate_fill(self, actual: usize, buffer_len: usize) -> Result<(), Error> {
        if self.fill_len > buffer_len || actual != self.fill_len {
            Err(Error::InvalidBackendResult)
        } else {
            Ok(())
        }
    }
}

/// Failure before any bytes have been delivered, or a violated backend contract.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BatchError<E> {
    Contract(Error),
    Transfer(E),
}

/// Drives a bounded read without turning each internal batch into a syscall.
/// The callback returns only bytes already copied to the user's destination.
/// After progress, a transfer failure returns that prefix for a later retry.
pub(crate) fn read_batches<E>(
    offset: u64,
    capacity: usize,
    batch_size: usize,
    mut transfer: impl FnMut(u64, usize, usize) -> Result<usize, E>,
) -> Result<usize, BatchError<E>> {
    if batch_size == 0 {
        return Err(BatchError::Contract(Error::InvalidBackendResult));
    }
    let length =
        u64::try_from(capacity).map_err(|_| BatchError::Contract(Error::ArithmeticOverflow))?;
    offset
        .checked_add(length)
        .ok_or(BatchError::Contract(Error::ArithmeticOverflow))?;
    let mut completed = 0usize;
    while completed < capacity {
        let length = (capacity - completed).min(batch_size);
        let actual = match transfer(offset + completed as u64, completed, length) {
            Ok(actual) => actual,
            Err(_) if completed != 0 => return Ok(completed),
            Err(error) => return Err(BatchError::Transfer(error)),
        };
        if actual > length {
            return Err(BatchError::Contract(Error::InvalidBackendResult));
        }
        completed += actual;
        if actual < length {
            break;
        }
    }
    Ok(completed)
}
