// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Nonblocking configuration owner for one vhost-scsi backend.
//!
//! The caller multiplexes mailbox, VM lifetime, console and vCPU observations.
//! A control request never blocks that event loop, and a guest MMIO instruction
//! is completed only after the exact backend transaction is acknowledged.

use crate::io_protocol::{self, Command, Reply, Request, Status};
use crate::virtio_scsi::{self, BackendOperation, Device, Transaction};
use hyper_os::guest_io::{Mailbox, Notification, Operation};
use hyper_os::vm::{MmioCompletion, MmioOperation, MmioRequest};
use std::num::NonZeroU64;

#[derive(Debug)]
pub enum Error {
    Native(hyper_os::Error),
    Protocol(io_protocol::Error),
    Device(virtio_scsi::Error),
    BackendRejected(Status),
    Disconnected,
    InvalidState,
    SequenceExhausted,
}

#[derive(Clone, Copy, Debug)]
pub struct Completion {
    pub vcpu: usize,
    pub id: NonZeroU64,
    pub result: MmioCompletion,
}

struct Pending {
    request: Request,
    sent: bool,
    guest: Option<(Transaction, Completion)>,
}

pub struct Backend {
    mailbox: Mailbox,
    notification: Notification,
    device: Option<Device>,
    memory_base: u64,
    memory_size: u64,
    mmio_base: u64,
    device_id: NonZeroU64,
    binding: u64,
    next_transaction: u64,
    pending: Option<Pending>,
    terminal: bool,
}

impl Backend {
    pub fn new(
        mailbox: Mailbox,
        notification: Notification,
        memory: (u64, u64),
        mmio_base: u64,
        device_id: NonZeroU64,
    ) -> Result<Self, Error> {
        let epoch = notification
            .control(Operation::Disable)
            .map_err(Error::Native)?;
        Ok(Self {
            mailbox,
            notification,
            device: None,
            memory_base: memory.0,
            memory_size: memory.1,
            mmio_base,
            device_id,
            binding: device_id.get(),
            next_transaction: 2,
            terminal: false,
            pending: Some(Pending {
                request: Request {
                    binding: device_id.get(),
                    epoch,
                    transaction: 1,
                    command: Command::Hello,
                },
                sent: false,
                guest: None,
            }),
        })
    }

    /// Preserve outstanding queue ownership and expose failure to the guest.
    /// The supervisor still owns stopping/reaping the backend and its DMA grants.
    pub fn disconnected(&mut self) -> Result<(), Error> {
        // Retain `pending`: neither a lost peer nor a late reply proves that
        // its descriptors and DMA buffers have been quiesced. Terminal state
        // independently opens diagnostic reads of DEVICE_NEEDS_RESET.
        self.terminal = true;
        if let Some(device) = self.device.as_mut() {
            device.backend_lost();
        }
        self.notification
            .control(Operation::RaiseConfigurationInterrupt)
            .map_err(Error::Native)?;
        Ok(())
    }

    pub fn mailbox(&self) -> &Mailbox {
        &self.mailbox
    }
    pub fn notification(&self) -> &Notification {
        &self.notification
    }
    pub fn ready(&self) -> bool {
        self.device.is_some() && !self.terminal
    }
    /// Whether backend progress currently blocks ordinary configuration MMIO.
    /// Terminal failure retains ownership, but must not park diagnostic reads.
    pub fn busy(&self) -> bool {
        self.pending.is_some() && !self.terminal
    }
    pub fn wants_write(&self) -> bool {
        !self.terminal && self.pending.as_ref().is_some_and(|pending| !pending.sent)
    }

    /// Run after mailbox readiness, and after enqueuing a guest operation.
    /// `WOULD_BLOCK` leaves state intact for the next readiness observation.
    pub fn progress(&mut self) -> Result<Option<Completion>, Error> {
        if self.terminal {
            return Err(Error::Disconnected);
        }
        let Some(pending) = self.pending.as_mut() else {
            return Ok(None);
        };
        if !pending.sent {
            let mut record = [0; io_protocol::MAX_RECORD];
            let length = pending
                .request
                .encode(&mut record)
                .map_err(Error::Protocol)?;
            match self.mailbox.send(&record[..length]) {
                Ok(()) => pending.sent = true,
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => return Ok(None),
                Err(error) => return Err(Error::Native(error)),
            }
        }
        let mut record = [0; io_protocol::MAX_RECORD];
        let length = match self.mailbox.receive(&mut record) {
            Ok(length) => length,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => return Ok(None),
            Err(error) => return Err(Error::Native(error)),
        };
        let reply = Reply::decode(&record[..length], pending.request).map_err(Error::Protocol)?;
        let completion = if let Some((transaction, completion)) = pending.guest {
            let success = match reply.status {
                Status::Success => true,
                Status::BackendFailure
                    if matches!(transaction.operation, BackendOperation::Activate { .. }) =>
                {
                    false
                }
                status => return Err(Error::BackendRejected(status)),
            };
            let device = self.device.as_mut().ok_or(Error::InvalidState)?;
            device
                .complete(transaction.id, success)
                .map_err(Error::Device)?;
            if matches!(transaction.operation, BackendOperation::Activate { .. }) && success {
                let epoch = self
                    .notification
                    .control(Operation::Enable)
                    .map_err(Error::Native)?;
                if epoch != pending.request.epoch {
                    return Err(Error::InvalidState);
                }
            } else if matches!(transaction.operation, BackendOperation::StopQueue { .. })
                || !success
            {
                self.notification
                    .control(Operation::RaiseConfigurationInterrupt)
                    .map_err(Error::Native)?;
            }
            Some(completion)
        } else {
            self.device = Some(
                Device::new(
                    reply.features.ok_or(Error::InvalidState)?,
                    self.memory_base,
                    self.memory_size,
                )
                .map_err(Error::Device)?,
            );
            None
        };
        self.pending = None;
        Ok(completion)
    }

    /// `None` means the instruction is deliberately parked. Other vCPUs may
    /// also park configuration requests while this single device transaction
    /// is outstanding; retry them after progress returns its completion.
    pub fn mmio(&mut self, vcpu: usize, request: MmioRequest) -> Result<Option<Completion>, Error> {
        if self.terminal && !matches!(request.operation, MmioOperation::Read) {
            return Err(Error::Disconnected);
        }
        if self.busy() {
            return Ok(None);
        }
        if request.device != self.device_id {
            return Err(Error::InvalidState);
        }
        let offset = request
            .address
            .checked_sub(self.mmio_base)
            .ok_or(Error::InvalidState)?;
        let device = self.device.as_mut().ok_or(Error::InvalidState)?;
        let width = u8::try_from(request.width).map_err(|_| Error::InvalidState)?;
        let mut completion = Completion {
            vcpu,
            id: request.id,
            result: MmioCompletion::Write,
        };
        match request.operation {
            MmioOperation::Read => {
                completion.result =
                    MmioCompletion::Read(device.read(offset, width).map_err(Error::Device)?);
            }
            MmioOperation::Write(value) => {
                if let Some(transaction) =
                    device.write(offset, width, value).map_err(Error::Device)?
                {
                    let next = self
                        .next_transaction
                        .checked_add(1)
                        .ok_or(Error::SequenceExhausted)?;
                    let epoch = self
                        .notification
                        .control(Operation::Disable)
                        .map_err(Error::Native)?;
                    self.pending = Some(Pending {
                        request: Request {
                            binding: self.binding,
                            epoch,
                            transaction: self.next_transaction,
                            command: Command::Device(transaction.operation),
                        },
                        sent: false,
                        guest: Some((transaction, completion)),
                    });
                    self.next_transaction = next;
                    return Ok(None);
                }
            }
        }
        Ok(Some(completion))
    }
}
