// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One logical queue owns the FAT operation's outstanding block writes. The
//! kernel copies submissions and orders overlapping ranges; no userspace buffer
//! remains DMA-visible after the submitting call returns.

use hyper_fs_fat::block::{self, BlockDevice, Error, WriteRequest};
use hyper_os::block::{BlockQueue, NativeBlock, QueueConfig, Ticket};

const REQUESTS: usize = 16;
const TRANSFER_BYTES: usize = block::WRITE_BATCH_BYTES;

pub(super) struct Device {
    block: NativeBlock,
    queue: Option<BlockQueue>,
    pending: [Option<Ticket>; REQUESTS],
    sectors: u64,
    readonly: bool,
}

impl Device {
    pub(super) fn new(block: NativeBlock, sectors: u64, readonly: bool) -> Result<Self, Error> {
        let queue = block
            .create_queue(QueueConfig {
                capacity: REQUESTS as u32,
                byte_limit: 2 * 1024 * 1024,
                inflight_limit: 4,
            })
            .map_err(block_error)?;
        Ok(Self {
            block,
            queue: Some(queue),
            pending: [None; REQUESTS],
            sectors,
            readonly,
        })
    }

    /// End the derived queue before handing the original volume capability to
    /// another worker. No accepted write may cross that ownership boundary.
    pub(super) fn into_block(mut self) -> Result<NativeBlock, Error> {
        self.complete_writes()?;
        drop(self.queue.take());
        Ok(self.block)
    }

    fn queue(&self) -> Result<&BlockQueue, Error> {
        self.queue.as_ref().ok_or(Error::Disconnected)
    }

    fn abandon(&mut self, error: hyper_os::Error) -> Error {
        // A failed wait/reap syscall cannot prove physical completion. Closing
        // the queue cancels undispatched work; the kernel retains DMA ownership
        // for submissions that may still be in use by the backend.
        drop(self.queue.take());
        self.pending.fill(None);
        block_error(error)
    }

    fn admit(
        &mut self,
        bytes: usize,
        submit: impl Fn(&BlockQueue) -> hyper_os::Result<Ticket>,
    ) -> Result<Ticket, Error> {
        loop {
            match submit(self.queue()?) {
                Ok(ticket) => return Ok(ticket),
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {
                    // Return our own completed credits first. With no local
                    // tickets, contention belongs to another logical queue:
                    // wait for this request's exact device/queue space instead
                    // of poisoning the filesystem for temporary backpressure.
                    if self.pending.iter().all(Option::is_none) {
                        if let Err(error) = self.queue()?.wait_space(bytes, u64::MAX) {
                            return Err(self.abandon(error));
                        }
                    } else {
                        self.complete_one()?;
                    }
                }
                Err(error) => return Err(block_error(error)),
            }
        }
    }

    fn complete_one(&mut self) -> Result<(), Error> {
        let completion = match self.queue()?.wait_any(u64::MAX) {
            Ok(completion) => completion,
            Err(error) => return Err(self.abandon(error)),
        };
        let Some(index) = self
            .pending
            .iter()
            .position(|ticket| *ticket == Some(completion.ticket))
        else {
            return Err(self.abandon(hyper_os::Error::InvalidResponse));
        };
        let completion = match self.queue()?.reap(completion.ticket, &mut []) {
            Ok(completion) => completion,
            Err(error) => return Err(self.abandon(error)),
        };
        self.pending[index] = None;
        completion.result().map_err(block_error)
    }

    fn finish(&mut self, ticket: Ticket, output: &mut [u8]) -> Result<(), Error> {
        let expected = output.len();
        let completion = match self.queue()?.wait(ticket, u64::MAX) {
            Ok(completion) => completion,
            Err(error) => return Err(self.abandon(error)),
        };
        let bytes = if completion.status == hyper_os::Status::OK {
            output
        } else {
            &mut []
        };
        let completion = match self.queue()?.reap(ticket, bytes) {
            Ok(completion) => completion,
            Err(error) => return Err(self.abandon(error)),
        };
        completion.result().map_err(block_error)?;
        if completion.bytes != expected {
            return Err(self.abandon(hyper_os::Error::InvalidResponse));
        }
        Ok(())
    }
}

impl BlockDevice for Device {
    fn is_read_only(&self) -> bool {
        self.readonly
    }

    fn sector_count(&self) -> u64 {
        self.sectors
    }

    fn read_sectors(&mut self, first: u64, output: &mut [u8]) -> Result<(), Error> {
        block::validate_range(self.sectors, first, output.len())?;
        for (index, bytes) in output.chunks_mut(TRANSFER_BYTES).enumerate() {
            let sector = first + (index * TRANSFER_BYTES / block::SECTOR_SIZE) as u64;
            let ticket = self.admit(bytes.len(), |queue| queue.submit_read(sector, bytes.len()))?;
            self.finish(ticket, bytes)?;
        }
        Ok(())
    }

    fn write_sectors(&mut self, first: u64, input: &[u8]) -> Result<(), Error> {
        block::validate_range(self.sectors, first, input.len())?;
        let submitted = (|| {
            for (index, bytes) in input.chunks(TRANSFER_BYTES).enumerate() {
                let sector = first + (index * TRANSFER_BYTES / block::SECTOR_SIZE) as u64;
                self.submit_writes(&[WriteRequest {
                    first: sector,
                    bytes,
                }])?;
            }
            Ok(())
        })();
        let completed = self.complete_writes();
        submitted.and(completed)
    }

    fn submit_writes(&mut self, requests: &[WriteRequest<'_>]) -> Result<(), Error> {
        block::validate_writes(self.sectors, requests)?;
        if requests.is_empty() {
            return Ok(());
        }
        if self.readonly {
            return Err(Error::ReadOnly);
        }
        if requests.len() > block::WRITE_BATCH_REQUESTS {
            return Err(Error::InvalidRange);
        }
        let payload = requests.iter().try_fold(0usize, |length, request| {
            length
                .checked_add(request.bytes.len())
                .ok_or(Error::InvalidRange)
        })?;
        if payload > block::WRITE_BATCH_BYTES {
            return Err(Error::InvalidRange);
        }
        let mut batch = [hyper_os::block::WriteRequest {
            first: 0,
            bytes: &[],
        }; block::WRITE_BATCH_REQUESTS];
        for (target, source) in batch.iter_mut().zip(requests) {
            *target = hyper_os::block::WriteRequest {
                first: source.first,
                bytes: source.bytes,
            };
        }
        if self.pending.iter().all(Option::is_some) {
            self.complete_one()?;
        }
        let index = self
            .pending
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Exhausted)?;
        let ticket = self.admit(payload, |queue| {
            queue.submit_writes(&batch[..requests.len()])
        })?;
        self.pending[index] = Some(ticket);
        Ok(())
    }

    fn complete_writes(&mut self) -> Result<(), Error> {
        let mut failure = None;
        while self.pending.iter().any(Option::is_some) {
            if let Err(error) = self.complete_one() {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.complete_writes()?;
        let ticket = self.admit(0, BlockQueue::submit_flush)?;
        self.finish(ticket, &mut [])
    }
}

fn block_error(error: hyper_os::Error) -> Error {
    match error {
        hyper_os::Error::Status(hyper_os::Status::READ_ONLY) => Error::ReadOnly,
        hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED) => Error::Disconnected,
        hyper_os::Error::Status(hyper_os::Status::INVALID_ARGUMENT) => Error::InvalidRange,
        hyper_os::Error::Status(hyper_os::Status::RESOURCE_LIMIT | hyper_os::Status::NO_MEMORY) => {
            Error::Exhausted
        }
        _ => Error::Io,
    }
}
