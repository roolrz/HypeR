// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Safe buffered virtual-serial bindings.

use crate::handle::{AnyObject, HandleRef, OwnedHandle, Rights, VirtualSerialObject};
use crate::wait::{ObjectSignals, WaitItem, wait_many};
use crate::{Error, Result, Status};

const _: () =
    assert!(hyper_abi::HYPER_NATIVE_VIRTUAL_SERIAL_MAX_TRANSFER_BYTES <= usize::MAX as u64);
/// Maximum byte prefix transferred by one virtual-serial operation.
pub const MAX_TRANSFER_BYTES: usize =
    hyper_abi::HYPER_NATIVE_VIRTUAL_SERIAL_MAX_TRANSFER_BYTES as usize;

const FULL_RIGHTS: Rights = Rights::DUPLICATE
    .union(Rights::TRANSFER)
    .union(Rights::WAIT)
    .union(Rights::INSPECT)
    .union(Rights::READ)
    .union(Rights::WRITE)
    .union(Rights::ASSIGN_DEVICE);

/// Creates one unbound buffered virtual serial port.
pub fn create() -> Result<OwnedHandle<VirtualSerialObject>> {
    // SAFETY: the safe layer adopts the sole produced handle on success.
    let result = unsafe { hyper_sys::virtual_serial_create() };
    Status::from_raw(result.status).into_result()?;
    // SAFETY: an OK result transfers one new handle to the caller.
    let owner =
        unsafe { crate::handle::adopt_produced_handle_excluding::<AnyObject>(result.value0, &[])? };
    if owner.info()?.rights != FULL_RIGHTS {
        return Err(Error::InvalidResponse);
    }
    owner
        .downcast::<VirtualSerialObject>()
        .map_err(|failure| failure.error())
}

/// Reads retained guest output, waiting while a connected port is empty.
pub fn read(serial: HandleRef<'_, VirtualSerialObject>, output: &mut [u8]) -> Result<usize> {
    let capacity = output.len().min(MAX_TRANSFER_BYTES);
    loop {
        // SAFETY: the borrowed handle and writable slice remain live for the call.
        let result = unsafe {
            hyper_sys::virtual_serial_read(serial.raw().get(), output.as_mut_ptr(), capacity)
        };
        match Status::from_raw(result.status) {
            Status::OK => return checked_count(result.value0, capacity),
            Status::BUSY => {
                let waits = [WaitItem::new(
                    serial,
                    ObjectSignals::<VirtualSerialObject>::READABLE
                        .union(ObjectSignals::<VirtualSerialObject>::DISCONNECTED),
                )];
                let observed = wait_many(&waits, crate::DEADLINE_INFINITE)?;
                if ObjectSignals::<VirtualSerialObject>::DISCONNECTED
                    .is_present_in(observed.observed)
                    && !ObjectSignals::<VirtualSerialObject>::READABLE
                        .is_present_in(observed.observed)
                {
                    return Err(Error::Status(Status::PEER_CLOSED));
                }
            }
            Status::BAD_STATE => return Err(Error::Status(Status::PEER_CLOSED)),
            status => return Err(Error::Status(status)),
        }
    }
}

/// Writes guest input, waiting while the port's bounded input queue is full.
pub fn write(serial: HandleRef<'_, VirtualSerialObject>, bytes: &[u8]) -> Result<usize> {
    let bytes = bytes
        .get(..bytes.len().min(MAX_TRANSFER_BYTES))
        .ok_or(Error::InvalidResponse)?;
    loop {
        // SAFETY: the borrowed handle and readable slice remain live for the call.
        let result = unsafe {
            hyper_sys::virtual_serial_write(serial.raw().get(), bytes.as_ptr(), bytes.len())
        };
        match Status::from_raw(result.status) {
            Status::OK => return checked_count(result.value0, bytes.len()),
            Status::BUSY => {
                let waits = [WaitItem::new(
                    serial,
                    ObjectSignals::<VirtualSerialObject>::WRITABLE
                        .union(ObjectSignals::<VirtualSerialObject>::DISCONNECTED),
                )];
                let observed = wait_many(&waits, crate::DEADLINE_INFINITE)?;
                if ObjectSignals::<VirtualSerialObject>::DISCONNECTED
                    .is_present_in(observed.observed)
                {
                    return Err(Error::Status(Status::PEER_CLOSED));
                }
            }
            Status::BAD_STATE => return Err(Error::Status(Status::PEER_CLOSED)),
            status => return Err(Error::Status(status)),
        }
    }
}

fn checked_count(raw: u64, capacity: usize) -> Result<usize> {
    let count = usize::try_from(raw).map_err(|_| Error::InvalidResponse)?;
    if count <= capacity {
        Ok(count)
    } else {
        Err(Error::InvalidResponse)
    }
}
