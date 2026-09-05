// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process-facing Channel transactions and user-memory commit ordering.

use alloc::vec::Vec;

use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceError, ResourceKind};
use crate::kernel::authority::Rights;
use crate::kernel::capability::{HandleTransferRequest, HandleValue};
use crate::kernel::mm::user_space::{UserAddress, UserSlice, UserWriteReservation};
use crate::kernel::object::{KernelObject, ObjectKind};
use crate::kernel::process::{Process, ProcessError};

use super::{ChannelEndpoint, ChannelError, PreparedMessage};

const DISPOSITION_SIZE: usize =
    core::mem::size_of::<hyper::abi::native::HyperNativeChannelDisposition>();
const HANDLE_VALUE_SIZE: usize = core::mem::size_of::<u64>();
const MAX_HANDLE_OUTPUT_BYTES: usize =
    hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES as usize * HANDLE_VALUE_SIZE;

/// Output address ranges already validated against the declared capacities.
#[derive(Clone, Copy)]
pub(crate) struct ReadBuffers {
    pub(crate) bytes: Option<UserSlice>,
    pub(crate) handles: Option<UserSlice>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChannelReadOutcome {
    Received { bytes: u64, handles: u64 },
    BufferTooSmall { bytes: u64, handles: u64 },
}

#[derive(Debug)]
pub(crate) enum ChannelServiceError {
    InvalidDisposition,
    Process(ProcessError),
    Channel(ChannelError),
    Resource(ResourceError),
}

struct DispositionBatch {
    requests: Vec<HandleTransferRequest>,
    _storage_charge: Option<CommittedCharge>,
}

impl From<ProcessError> for ChannelServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<ChannelError> for ChannelServiceError {
    fn from(error: ChannelError) -> Self {
        Self::Channel(error)
    }
}

impl From<ResourceError> for ChannelServiceError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

pub(crate) fn channel_create(process: &Process) -> Result<[HandleValue; 2], ChannelServiceError> {
    let (first, second) = ChannelEndpoint::try_pair(&process.resource_domain())?;
    Ok(process.create_object_pair(
        first,
        second,
        <ChannelEndpoint as KernelObject>::SUPPORTED_RIGHTS,
    )?)
}

/// Copies and commits one message without consuming a source on failure.
pub(crate) fn channel_write(
    process: &Process,
    endpoint_value: HandleValue,
    bytes: Option<UserSlice>,
    dispositions: Option<UserSlice>,
    disposition_count: usize,
) -> Result<(), ChannelServiceError> {
    let endpoint = process.resolve_handle::<ChannelEndpoint>(endpoint_value, Rights::WRITE)?;
    let domain = process.resource_domain();
    let handle_count =
        u64::try_from(disposition_count).map_err(|_| ChannelServiceError::InvalidDisposition)?;
    let byte_count = bytes
        .map(|range| usize::try_from(range.length()))
        .transpose()
        .map_err(|_| ChannelServiceError::InvalidDisposition)?
        .unwrap_or(0);
    let mut message = PreparedMessage::try_new(&domain, byte_count, handle_count)?;
    if let Some(source) = bytes {
        process.copy_from_user(source, message.bytes_mut())?;
    }

    let requests = copy_dispositions(process, dispositions, disposition_count)?;
    let write = endpoint.object().prepare_write(&message)?;
    if requests.requests.is_empty() {
        write.publish(message);
        return Ok(());
    }

    let transfer = match process.prepare_handle_transfer(
        &requests.requests,
        Some(endpoint.koid()),
        Some(ObjectKind::CHANNEL),
    ) {
        Ok(transfer) => transfer,
        Err(error) => {
            write.abort();
            return Err(error.into());
        }
    };
    let capabilities = match transfer.commit() {
        Ok(capabilities) => capabilities,
        Err(failure) => {
            failure.transfer.rollback();
            write.abort();
            return Err(failure.error.into());
        }
    };
    // Every fallible operation precedes the source-generation commit above.
    message.attach_handles(capabilities);
    write.publish(message);
    Ok(())
}

/// Receives one FIFO head, publishing every returned handle in one commit.
pub(crate) fn channel_read(
    process: &Process,
    endpoint_value: HandleValue,
    buffers: ReadBuffers,
) -> Result<ChannelReadOutcome, ChannelServiceError> {
    if let (Some(bytes), Some(handles)) = (buffers.bytes, buffers.handles)
        && !bytes.is_disjoint(handles)
    {
        return Err(ChannelServiceError::InvalidDisposition);
    }
    let endpoint = process.resolve_handle::<ChannelEndpoint>(endpoint_value, Rights::READ)?;
    let info = endpoint.object().peek()?;
    let required_bytes =
        u64::try_from(info.bytes()).map_err(|_| ChannelServiceError::InvalidDisposition)?;
    let required_handles = info.handles();
    let byte_capacity = buffers.bytes.map_or(0, UserSlice::length);
    let handle_capacity = buffers
        .handles
        .map_or(0, |range| range.length() / HANDLE_VALUE_SIZE as u64);
    if byte_capacity < required_bytes || handle_capacity < required_handles {
        return Ok(ChannelReadOutcome::BufferTooSmall {
            bytes: required_bytes,
            handles: required_handles,
        });
    }
    let handle_bytes = match required_handles.checked_mul(HANDLE_VALUE_SIZE as u64) {
        Some(bytes) => bytes,
        None => service_invariant("receiver handle byte count overflowed"),
    };
    let required_handle_count = match usize::try_from(required_handles) {
        Ok(count) => count,
        Err(_) => service_invariant("receiver handle count exceeded usize"),
    };

    let mut claim = endpoint.object().claim(info)?;
    let mut receiver_slots = if required_handles == 0 {
        None
    } else {
        match process.reserve_handle_batch(required_handle_count) {
            Ok(reservation) => Some(reservation),
            Err(error) => {
                claim.abort();
                return Err(error.into());
            }
        }
    };

    let byte_write = match reserve_exact_output(process, buffers.bytes, required_bytes) {
        Ok(reservation) => reservation,
        Err(error) => {
            abort_receive_preparation(process, receiver_slots.take(), claim);
            return Err(error.into());
        }
    };
    let handle_write = match reserve_exact_output(process, buffers.handles, handle_bytes) {
        Ok(reservation) => reservation,
        Err(error) => {
            drop(byte_write);
            abort_receive_preparation(process, receiver_slots.take(), claim);
            return Err(error.into());
        }
    };

    if let Some(write) = byte_write.as_ref()
        && let Err(error) = write.copy_from(claim.bytes())
    {
        drop(handle_write);
        drop(byte_write);
        abort_receive_preparation(process, receiver_slots.take(), claim);
        return Err(ProcessError::UserMemory(error).into());
    }

    let mut encoded_handles = [0_u8; MAX_HANDLE_OUTPUT_BYTES];
    if let (Some(reservation), Some(write)) = (receiver_slots.as_ref(), handle_write.as_ref()) {
        encode_handle_values(reservation.values(), &mut encoded_handles);
        let byte_count = match reservation.values().len().checked_mul(HANDLE_VALUE_SIZE) {
            Some(bytes) => bytes,
            None => service_invariant("receiver handle output byte count overflowed"),
        };
        let Some(encoded) = encoded_handles.get(..byte_count) else {
            service_invariant("receiver handle output exceeded its ABI maximum");
        };
        if let Err(error) = write.copy_from(encoded) {
            drop(handle_write);
            drop(byte_write);
            abort_receive_preparation(process, receiver_slots.take(), claim);
            return Err(ProcessError::UserMemory(error).into());
        }
    }

    let received = if let Some(reservation) = receiver_slots.take() {
        let Some(capabilities) = claim.take_capabilities() else {
            service_invariant("capability-bearing Channel message had no active owners");
        };
        match process.publish_handle_batch(reservation, capabilities) {
            Ok(()) => claim.commit_after_handle_publication(),
            Err(failure) => {
                claim.restore_capabilities(failure.handles);
                claim.abort();
                drop(handle_write);
                drop(byte_write);
                return Err(failure.error.into());
            }
        }
    } else {
        claim.commit()
    };
    received.release();
    if let Some(write) = handle_write {
        write.complete();
    }
    if let Some(write) = byte_write {
        write.complete();
    }
    Ok(ChannelReadOutcome::Received {
        bytes: required_bytes,
        handles: required_handles,
    })
}

fn copy_dispositions(
    process: &Process,
    source: Option<UserSlice>,
    count: usize,
) -> Result<DispositionBatch, ChannelServiceError> {
    if count == 0 {
        return Ok(DispositionBatch {
            requests: Vec::new(),
            _storage_charge: None,
        });
    }
    let source = source.ok_or(ChannelServiceError::InvalidDisposition)?;
    let raw_bytes = count
        .checked_mul(DISPOSITION_SIZE)
        .ok_or(ChannelServiceError::InvalidDisposition)?;
    if source.length() != raw_bytes as u64 {
        return Err(ChannelServiceError::InvalidDisposition);
    }
    let request_bytes = count
        .checked_mul(core::mem::size_of::<HandleTransferRequest>())
        .and_then(|bytes| bytes.checked_add(raw_bytes))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ChannelServiceError::InvalidDisposition)?;
    let storage_charge = process
        .resource_domain()
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, request_bytes))?
        .commit();
    let mut raw = Vec::new();
    raw.try_reserve_exact(raw_bytes)
        .map_err(|_| ChannelServiceError::Process(ProcessError::Allocation))?;
    raw.resize(raw_bytes, 0);
    process.copy_from_user(source, &mut raw)?;
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(count)
        .map_err(|_| ChannelServiceError::Process(ProcessError::Allocation))?;
    for record in raw.chunks_exact(DISPOSITION_SIZE) {
        requests.push(parse_disposition(process, record)?);
    }
    Ok(DispositionBatch {
        requests,
        _storage_charge: Some(storage_charge),
    })
}

fn parse_disposition(
    process: &Process,
    record: &[u8],
) -> Result<HandleTransferRequest, ChannelServiceError> {
    let raw_handle = read_u64(record, 0).ok_or(ChannelServiceError::InvalidDisposition)?;
    let raw_rights = read_u64(record, 8).ok_or(ChannelServiceError::InvalidDisposition)?;
    let raw_kind = read_u32(record, 16).ok_or(ChannelServiceError::InvalidDisposition)?;
    let reserved = read_u32(record, 20).ok_or(ChannelServiceError::InvalidDisposition)?;
    if reserved != 0 {
        return Err(ChannelServiceError::InvalidDisposition);
    }
    let value = HandleValue::try_from_raw(raw_handle)
        .map_err(|error| ChannelServiceError::Process(ProcessError::Handle(error)))?;
    let expected_kind = match raw_kind {
        hyper::abi::native::HYPER_NATIVE_OBJECT_NONE => None,
        hyper::abi::native::HYPER_NATIVE_OBJECT_EVENT => Some(ObjectKind::EVENT),
        hyper::abi::native::HYPER_NATIVE_OBJECT_CHANNEL => Some(ObjectKind::CHANNEL),
        hyper::abi::native::HYPER_NATIVE_OBJECT_CONSOLE => Some(ObjectKind::CONSOLE),
        _ => return Err(ChannelServiceError::InvalidDisposition),
    };
    let rights = if raw_rights == hyper::abi::native::HYPER_NATIVE_CHANNEL_DISPOSITION_SAME_RIGHTS {
        process.handle_info(value, Rights::TRANSFER)?.rights
    } else {
        Rights::from_bits(raw_rights).ok_or(ChannelServiceError::InvalidDisposition)?
    };
    Ok(HandleTransferRequest {
        value,
        rights,
        expected_kind,
    })
}

fn reserve_exact_output(
    process: &Process,
    capacity: Option<UserSlice>,
    length: u64,
) -> Result<Option<UserWriteReservation>, ProcessError> {
    if length == 0 {
        return Ok(None);
    }
    let capacity = match capacity {
        Some(capacity) => capacity,
        None => service_invariant("nonempty Channel output had no validated capacity"),
    };
    let range = UserSlice::new(capacity.base(), length)
        .map_err(|error| ProcessError::UserMemory(error.into()))?;
    Ok(Some(process.reserve_user_write(range)?))
}

fn abort_receive_preparation(
    process: &Process,
    receiver_slots: Option<crate::kernel::process::ProcessHandleBatchReservation>,
    claim: super::ReceiveClaim,
) {
    if let Some(reservation) = receiver_slots {
        process.abort_handle_batch(reservation);
    }
    claim.abort();
}

fn encode_handle_values(values: &[HandleValue], output: &mut [u8; MAX_HANDLE_OUTPUT_BYTES]) {
    for (index, value) in values.iter().enumerate() {
        let Some(offset) = index.checked_mul(HANDLE_VALUE_SIZE) else {
            service_invariant("receiver handle output offset overflowed");
        };
        let Some(target) = output.get_mut(offset..offset + HANDLE_VALUE_SIZE) else {
            service_invariant("receiver handle output exceeded its ABI maximum");
        };
        target.copy_from_slice(&value.get().to_ne_bytes());
    }
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let source = bytes.get(offset..offset.checked_add(8)?)?;
    let mut encoded = [0_u8; 8];
    encoded.copy_from_slice(source);
    Some(u64::from_ne_bytes(encoded))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let source = bytes.get(offset..offset.checked_add(4)?)?;
    let mut encoded = [0_u8; 4];
    encoded.copy_from_slice(source);
    Some(u32::from_ne_bytes(encoded))
}

#[cold]
fn service_invariant(message: &str) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: Channel service invariant failed: {message}"
    ))
}
