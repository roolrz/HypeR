// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ordered filesystem RPC. Shared bytes are never interpreted as Rust objects.
//! A timed-out or malformed transaction permanently retires this
//! generation: late server writes cannot become a subsequent request's reply.

use crate::kernel::{
    accounting::ResourceDomain,
    authority::Rights,
    capability::HandleValue,
    ipc::{ByteChannel, ByteChannelError, PreparedByteMessage},
    mm::user_space::{
        DomainAccount, KernelPageBackend, VmoObject, WritableMappingLease, WritableVmo,
    },
    object::{KernelObject, KernelRef, KernelService, SignalWaitOutcome, wait_one},
    process::Process,
};
use core::sync::atomic::{AtomicBool, Ordering};
use hyper::mm::FallibleArc;
use hyper_filesystem::{
    Timestamp,
    protocol::{self, Entry, Error, Operation, Request, Response},
};

pub(super) struct Health {
    channel: KernelRef<ByteChannel, KernelService>,
    failed: AtomicBool,
}
impl Health {
    pub(super) fn check(&self) -> Result<(), Error> {
        if self.failed.load(Ordering::Acquire) || self.channel.object().is_closed() {
            Err(Error::Closed)
        } else {
            Ok(())
        }
    }
    fn fail(&self) {
        self.failed.store(true, Ordering::Release);
    }
}
pub(in crate::kernel::vfs) struct Transport {
    pub(super) health: FallibleArc<Health>,
    buffer: WritableVmo<KernelPageBackend, DomainAccount>,
    _mapping: WritableMappingLease<KernelPageBackend, DomainAccount>,
    domain: ResourceDomain,
    sequence: u64,
}
impl Transport {
    pub(crate) fn prepare(
        process: &Process,
        channel: HandleValue,
        buffer: HandleValue,
    ) -> Result<Self, super::Error> {
        let channel = process
            .resolve_handle::<ByteChannel>(
                channel,
                Rights::READ.union(Rights::WRITE).union(Rights::WAIT),
            )
            .map_err(|_| super::Error::InvalidInput)?;
        let object = process
            .resolve_handle::<VmoObject>(
                buffer,
                Rights::READ.union(Rights::WRITE).union(Rights::MAP),
            )
            .map_err(|_| super::Error::InvalidInput)?;
        let buffer = object
            .object()
            .writable_clone()
            .ok_or(super::Error::InvalidInput)?;
        if buffer.size() != protocol::DATA_BYTES as u64 {
            return Err(super::Error::InvalidInput);
        }
        let mapping = buffer
            .try_mapping_write_lease()
            .map_err(|_| super::Error::InvalidInput)?;
        buffer
            .populate(0, buffer.size())
            .map_err(|_| super::Error::Allocation)?;
        let channel = channel.into_operation_pin().into_filesystem_transport();
        let health = FallibleArc::try_new(Health {
            channel,
            failed: AtomicBool::new(false),
        })?;
        Ok(Self {
            health,
            buffer,
            _mapping: mapping,
            domain: process.resource_domain(),
            sequence: 0,
        })
    }
    fn call(
        &mut self,
        operation: Operation,
        path: &str,
        target: &str,
        offset: u64,
        length: u64,
        times: [Option<Timestamp>; 2],
    ) -> Result<Response, Error> {
        self.health.check()?;
        let result = self.exchange(operation, path, target, offset, length, times);
        if result.as_ref().err().is_some_and(|error| error.terminal()) {
            self.health.fail();
        }
        result
    }
    fn exchange(
        &mut self,
        operation: Operation,
        path: &str,
        target: &str,
        offset: u64,
        length: u64,
        times: [Option<Timestamp>; 2],
    ) -> Result<Response, Error> {
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Closed)?;
        let request = Request {
            sequence: self.sequence,
            operation,
            path,
            target,
            offset,
            length,
            accessed: times[0],
            modified: times[1],
        };
        let size = protocol::HEADER_BYTES
            .checked_add(path.len())
            .and_then(|n| n.checked_add(target.len()))
            .ok_or(Error::InvalidInput)?;
        let mut message =
            PreparedByteMessage::try_new(&self.domain, size).map_err(|_| Error::Allocation)?;
        request
            .encode(message.bytes_mut())
            .ok_or(Error::InvalidInput)?;
        let channel = self.health.channel.object();
        let reservation = channel.prepare_write(&message).map_err(|_| Error::Closed)?;
        reservation.publish(message);
        let deadline =
            crate::kernel::time::deadline_after(60_000_000_000).map_err(|_| Error::Closed)?;
        loop {
            match channel.peek() {
                Ok(info) => {
                    let claim = channel.claim(info).map_err(|_| Error::Closed)?;
                    let response = Response::decode(claim.bytes(), self.sequence, operation);
                    claim.commit().release();
                    let response = response.ok_or(Error::Corrupt)?;
                    response.result?;
                    return Ok(response);
                }
                Err(ByteChannelError::WouldBlock) => {}
                Err(_) => return Err(Error::Closed),
            }
            let source = channel.signal_source().ok_or(Error::Closed)?;
            match wait_one(
                source,
                &self.domain,
                ByteChannel::READABLE.union(ByteChannel::PEER_CLOSED).bits(),
                deadline,
                // Publication transfers the shared range to the worker. An
                // app's stop request cannot retire a mount used by other apps.
                // Drain this bounded transaction; the user runner handles stop
                // after the syscall unwinds. Scheduler cancellation selects one
                // current wait ticket, not every subsequent rearm.
                || false,
            )
            .map_err(|_| Error::Closed)?
            {
                SignalWaitOutcome::Observed(_) | SignalWaitOutcome::Cancelled => {}
                SignalWaitOutcome::TimedOut => return Err(Error::Closed),
            }
        }
    }
    pub(super) fn stat_into(&mut self, path: &str, entry: &mut Entry) -> Result<(), Error> {
        *entry = self.call(Operation::Stat, path, "", 0, 0, [None; 2])?.entry;
        Ok(())
    }
    pub(super) fn entry_into(
        &mut self,
        path: &str,
        index: usize,
        entry: &mut Entry,
    ) -> Result<bool, Error> {
        let response = self.call(Operation::Entry, path, "", index as u64, 0, [None; 2])?;
        let present = response.result?;
        if present > 1 {
            self.health.fail();
            return Err(Error::Corrupt);
        }
        *entry = response.entry;
        Ok(present != 0)
    }
    pub(super) fn read_at(
        &mut self,
        path: &str,
        offset: u64,
        output: &mut [u8],
    ) -> Result<usize, Error> {
        let mut done = 0;
        for chunk in output.chunks_mut(protocol::DATA_BYTES) {
            let response = self.call(
                Operation::Read,
                path,
                "",
                offset.checked_add(done as u64).ok_or(Error::InvalidInput)?,
                chunk.len() as u64,
                [None; 2],
            )?;
            let actual = usize::try_from(response.result?).map_err(|_| Error::Corrupt)?;
            if actual > chunk.len() {
                self.health.fail();
                return Err(Error::Corrupt);
            }
            self.buffer
                .read_exposed(0, &mut chunk[..actual])
                .map_err(|_| Error::Io)?;
            done += actual;
            if actual < chunk.len() {
                break;
            }
        }
        Ok(done)
    }
    pub(super) fn write_at(
        &mut self,
        path: &str,
        offset: u64,
        input: &[u8],
    ) -> Result<usize, Error> {
        let mut done = 0;
        for chunk in input.chunks(protocol::DATA_BYTES) {
            self.health.check()?;
            self.buffer.write_exposed(0, chunk).map_err(|_| Error::Io)?;
            let response = self.call(
                Operation::Write,
                path,
                "",
                offset.checked_add(done as u64).ok_or(Error::InvalidInput)?,
                chunk.len() as u64,
                [None; 2],
            )?;
            let actual = usize::try_from(response.result?).map_err(|_| Error::Corrupt)?;
            if actual > chunk.len() {
                self.health.fail();
                return Err(Error::Corrupt);
            }
            done += actual;
            if actual < chunk.len() {
                break;
            }
        }
        Ok(done)
    }
    pub(super) fn create(&mut self, path: &str, directory: bool) -> Result<(), Error> {
        self.call(
            Operation::Create,
            path,
            "",
            0,
            u64::from(directory),
            [None; 2],
        )
        .map(|_| ())
    }
    pub(super) fn remove(&mut self, path: &str) -> Result<(), Error> {
        self.call(Operation::Remove, path, "", 0, 0, [None; 2])
            .map(|_| ())
    }
    pub(super) fn rename(&mut self, path: &str, target: &str) -> Result<(), Error> {
        self.call(Operation::Rename, path, target, 0, 0, [None; 2])
            .map(|_| ())
    }
    pub(super) fn resize(&mut self, path: &str, length: u64) -> Result<(), Error> {
        self.call(Operation::Resize, path, "", 0, length, [None; 2])
            .map(|_| ())
    }
    pub(super) fn sync(&mut self) -> Result<(), Error> {
        self.call(Operation::Sync, "", "", 0, 0, [None; 2])
            .map(|_| ())
    }
    pub(super) fn set_times(
        &mut self,
        path: &str,
        accessed: Option<Timestamp>,
        modified: Option<Timestamp>,
    ) -> Result<(), Error> {
        self.call(Operation::SetMetadata, path, "", 0, 0, [accessed, modified])
            .map(|_| ())
    }
}
