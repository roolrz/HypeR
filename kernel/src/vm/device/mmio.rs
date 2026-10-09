// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One bounded, generation-qualified userspace MMIO continuation per vCPU.
//!
//! The installed owner serializes this state and publishes its readiness
//! signal only after the hardware context has been detached. No borrowed
//! exception frame, register index, or architecture completion state crosses
//! into the userspace request.

use crate::vm::exit::{MmioAccess, MmioAction, MmioOperation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub id: u64,
    pub device: u64,
    pub access: Access,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    Mmio(MmioAccess),
    FirmwareConsoleWrite(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Busy,
    Closed,
    Exhausted,
    Stale,
    WrongCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Idle,
    Staged(Request),
    Pending(Request),
    Completed(MmioAction),
    Closed,
}

pub struct PendingMmio {
    next_id: u64,
    phase: Phase,
}

impl Default for PendingMmio {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingMmio {
    pub const fn new() -> Self {
        Self {
            next_id: 1,
            phase: Phase::Idle,
        }
    }

    pub fn stage(&mut self, device: u64, access: MmioAccess) -> Result<(), Error> {
        self.stage_access(device, Access::Mmio(access))
    }
    pub fn stage_firmware_console(&mut self, device: u64, byte: u8) -> Result<(), Error> {
        self.stage_access(device, Access::FirmwareConsoleWrite(byte))
    }
    fn stage_access(&mut self, device: u64, access: Access) -> Result<(), Error> {
        match self.phase {
            Phase::Idle => {}
            Phase::Closed => return Err(Error::Closed),
            _ => return Err(Error::Busy),
        }
        let following = self.next_id.checked_add(1).ok_or(Error::Exhausted)?;
        self.phase = Phase::Staged(Request {
            id: self.next_id,
            device,
            access,
        });
        self.next_id = following;
        Ok(())
    }

    /// Called only by the owner of the detached hardware continuation.
    pub fn publish(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::Staged(request) => {
                self.phase = Phase::Pending(request);
                Ok(())
            }
            Phase::Closed => Err(Error::Closed),
            _ => Err(Error::Busy),
        }
    }

    pub const fn staged(&self) -> Option<Request> {
        match self.phase {
            Phase::Staged(request) => Some(request),
            _ => None,
        }
    }

    /// Non-consuming snapshot: service failure must not lose the continuation.
    pub const fn pending(&self) -> Option<Request> {
        match self.phase {
            Phase::Pending(request) => Some(request),
            _ => None,
        }
    }

    pub fn complete(&mut self, id: u64, action: MmioAction) -> Result<(), Error> {
        let request = match self.phase {
            Phase::Pending(request) if request.id == id => request,
            Phase::Closed => return Err(Error::Closed),
            _ => return Err(Error::Stale),
        };
        let operation = match request.access {
            Access::Mmio(access) => access.operation(),
            Access::FirmwareConsoleWrite(byte) => MmioOperation::Write(u64::from(byte)),
        };
        match (operation, action) {
            (MmioOperation::Read, MmioAction::CompleteRead(_))
            | (MmioOperation::Write(_), MmioAction::CompleteWrite)
            | (_, MmioAction::Stop) => {}
            _ => return Err(Error::WrongCompletion),
        }
        self.phase = Phase::Completed(action);
        Ok(())
    }

    pub fn take_completed(&mut self) -> Result<Option<MmioAction>, Error> {
        match self.phase {
            Phase::Completed(action) => {
                self.phase = Phase::Idle;
                Ok(Some(action))
            }
            Phase::Closed => Err(Error::Closed),
            _ => Ok(None),
        }
    }

    /// Terminal cancellation never reuses a generation or reopens this owner.
    pub fn close(&mut self) {
        self.phase = Phase::Closed;
    }
}
