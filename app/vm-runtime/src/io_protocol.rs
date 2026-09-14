// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Versioned control records shared with the independently built I/O appliance.
//! No pointers, file descriptors or Rust layout cross this boundary.

use crate::virtio_scsi::{BackendOperation, QUEUE_MAX, QUEUES, VERSION_1};

const MAGIC: u32 = 0x314f_4948;
const VERSION: u16 = 1;
const HEADER: usize = 40;
pub const MAX_RECORD: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidIdentity,
    InvalidRecord,
    MismatchedReply,
    UnsupportedBackend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Hello,
    Device(BackendOperation),
}

impl Command {
    const fn opcode(self) -> u16 {
        match self {
            Self::Hello => 1,
            Self::Device(BackendOperation::Activate { .. }) => 2,
            Self::Device(BackendOperation::Reset) => 3,
            Self::Device(BackendOperation::StopQueue { .. }) => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub binding: u64,
    pub epoch: u32,
    pub transaction: u64,
    pub command: Command,
}

impl Request {
    pub fn encode(self, output: &mut [u8; MAX_RECORD]) -> Result<usize, Error> {
        if self.binding == 0 || self.epoch == 0 || self.transaction == 0 {
            return Err(Error::InvalidIdentity);
        }
        let length = match self.command {
            Command::Hello | Command::Device(BackendOperation::Reset) => HEADER,
            Command::Device(BackendOperation::Activate { .. }) => 144,
            Command::Device(BackendOperation::StopQueue { .. }) => 48,
        };
        output.fill(0);
        output[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        output[4..6].copy_from_slice(&VERSION.to_le_bytes());
        output[6..8].copy_from_slice(&self.command.opcode().to_le_bytes());
        output[8..12].copy_from_slice(&(length as u32).to_le_bytes());
        output[16..24].copy_from_slice(&self.binding.to_le_bytes());
        output[24..32].copy_from_slice(&u64::from(self.epoch).to_le_bytes());
        output[32..40].copy_from_slice(&self.transaction.to_le_bytes());
        match self.command {
            Command::Device(BackendOperation::Activate { features, queues }) => {
                output[40..48].copy_from_slice(&features.to_le_bytes());
                for (queue, chunk) in queues.iter().zip(output[48..144].chunks_exact_mut(32)) {
                    chunk[0..4].copy_from_slice(&queue.size.to_le_bytes());
                    chunk[8..16].copy_from_slice(&queue.descriptor.to_le_bytes());
                    chunk[16..24].copy_from_slice(&queue.available.to_le_bytes());
                    chunk[24..32].copy_from_slice(&queue.used.to_le_bytes());
                }
            }
            Command::Device(BackendOperation::StopQueue { queue }) => {
                output[40..44].copy_from_slice(&queue.to_le_bytes());
            }
            _ => {}
        }
        Ok(length)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Success,
    Unsupported,
    Invalid,
    Busy,
    /// Activation failed, but rollback has quiesced every backend producer.
    BackendFailure,
    /// No acknowledgement permitting memory/queue reuse can be inferred.
    QuiescenceFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reply {
    pub status: Status,
    pub features: Option<u64>,
}

impl Reply {
    /// Accept only the exact outstanding operation, including its epoch. A
    /// stale successful reply must never authorize queue or DMA memory reuse.
    pub fn decode(bytes: &[u8], request: Request) -> Result<Self, Error> {
        let expected = if request.command == Command::Hello {
            64
        } else {
            48
        };
        if bytes.len() != expected
            || u32_at(bytes, 0)? != MAGIC
            || u16_at(bytes, 4)? != VERSION
            || u32_at(bytes, 8)? as usize != expected
            || u32_at(bytes, 12)? != 1
            || u32_at(bytes, 44)? != 0
        {
            return Err(Error::InvalidRecord);
        }
        if u16_at(bytes, 6)? != request.command.opcode()
            || u64_at(bytes, 16)? != request.binding
            || u64_at(bytes, 24)? != u64::from(request.epoch)
            || u64_at(bytes, 32)? != request.transaction
        {
            return Err(Error::MismatchedReply);
        }
        let status = match u32_at(bytes, 40)? {
            0 => Status::Success,
            1 => Status::Unsupported,
            2 => Status::Invalid,
            3 => Status::Busy,
            4 => Status::BackendFailure,
            5 => Status::QuiescenceFailed,
            _ => return Err(Error::InvalidRecord),
        };
        let features = if request.command == Command::Hello {
            let features = u64_at(bytes, 48)?;
            if status != Status::Success
                || features & VERSION_1 == 0
                || u32_at(bytes, 56)? != QUEUES as u32
                || u32_at(bytes, 60)? != QUEUE_MAX
            {
                return Err(Error::UnsupportedBackend);
            }
            Some(features)
        } else {
            None
        };
        Ok(Self { status, features })
    }
}

fn field<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    bytes
        .get(offset..offset + N)
        .and_then(|value| value.try_into().ok())
        .ok_or(Error::InvalidRecord)
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    field(bytes, offset).map(u16::from_le_bytes)
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    field(bytes, offset).map(u32::from_le_bytes)
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    field(bytes, offset).map(u64::from_le_bytes)
}

#[cfg(test)]
#[path = "../tests/io_protocol.rs"]
mod tests;
