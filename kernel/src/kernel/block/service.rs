// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-bound block setup and bounded exclusive volume transfers.

use super::*;
use crate::kernel::accounting::{ResourceAmount, ResourceKind};
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle};
use crate::kernel::object::{ObjectPublication, object_allocation_size};
use crate::kernel::process::Process;
use crate::kernel::vm::objects::{GuestMemoryObject, VirtualMachineObject};
use crate::kernel::vm::service::Error as ServiceError;

pub(crate) fn create(
    process: &Process,
    memory: HandleValue,
    backend: HandleValue,
    guest_base: u64,
    notify_base: u64,
    notify_irq: u32,
) -> Result<HandleValue, ServiceError> {
    let memory = process.resolve_handle::<GuestMemoryObject>(memory, Rights::MAP)?;
    let backend = process.resolve_handle::<VirtualMachineObject>(backend, Rights::WRITE)?;
    let backing = memory.object().backing();
    if backing.size() != wire::MEMORY_BYTES
        || !guest_base.is_multiple_of(4096)
        || guest_base.checked_add(wire::MEMORY_BYTES).is_none()
    {
        return Err(ServiceError::InvalidArgument);
    }
    // No allocation or fault is permitted after queue publication.
    for offset in (0..wire::MEMORY_BYTES).step_by(4096) {
        backing
            .physical_page(offset)
            .map_err(|_| ServiceError::BadState)?;
    }
    let domain = process.resource_domain();
    let bytes = object_allocation_size::<NativeBlock>()
        .and_then(|n| n.checked_add(FallibleArc::<Device>::allocation_size()))
        .ok_or(ServiceError::NoMemory)?;
    let charge = domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelMemoryBytes, bytes as u64)
                .with(ResourceKind::KernelObjects, 1),
        )
        .map_err(|_| ServiceError::ResourceLimit)?
        .commit();
    let owner = backend.object().owner();
    let notification = Notification::prepare_native(&owner, notify_base, notify_irq, &domain)?;
    let device = FallibleArc::try_new(Device {
        memory: backing,
        guest_base,
        notification,
        domain,
        state: InterruptSpinLock::new(State {
            busy: false,
            failed: false,
            sectors: 0,
            indices: [0; wire::REQUEST_QUEUES],
            tag: 0,
            readonly: false,
            capability_alive: true,
        }),
        available: SignalState::new(),

        _charge: charge,
    })
    .map_err(|_| ServiceError::NoMemory)?;
    let reservation = process.reserve_handles::<1>()?;
    let prepare = || {
        let publication = ObjectPublication::try_new(NativeBlock {
            device: device.clone(),
        })
        .map_err(|_| ServiceError::NoMemory)?;
        let prepared = PreparedHandle::try_from_new_object(
            publication,
            NativeBlock::SUPPORTED_RIGHTS,
            HandleFlags::NONE,
        )
        .map_err(|_| ServiceError::NoMemory)?;
        if !memory.object().claim_native_initiator() {
            return Err(ServiceError::Busy);
        }
        // The region is now permanently dedicated to this initiator. A failed
        // setup cannot expose a partially initialized queue to another owner.
        let zeros = [0; 256];
        for offset in (0..wire::DATA).step_by(zeros.len()) {
            device
                .write(offset, &zeros)
                .map_err(|_| ServiceError::BadState)?;
        }
        device.notification.install_native(&owner)?;
        Ok(prepared)
    };
    let prepared = match prepare() {
        Ok(value) => value,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(error);
        }
    };
    process
        .publish_handles(reservation, [prepared])
        .map(|handles| handles[0])
        .map_err(|failure| failure.error.into())
}

pub(crate) enum ActivationError {
    Capability(crate::kernel::process::ProcessError),
    Device(Error),
}

pub(crate) fn activate(
    process: &Process,
    handle: HandleValue,
    readonly: bool,
) -> Result<(u64, bool), ActivationError> {
    let block = process
        .resolve_handle::<NativeBlock>(handle, Rights::WRITE)
        .map_err(ActivationError::Capability)?;
    block
        .object()
        .device
        .activate(readonly)
        .map(|sectors| (sectors, readonly))
        .map_err(ActivationError::Device)
}

/// Raw volume access is capability scoped. `NativeBlock` cannot be duplicated;
/// the I/O owner transfers its sole active handle to the filesystem service.
pub(crate) fn transfer(
    process: &Process,
    handle: HandleValue,
    operation: u32,
    first: u64,
    buffer: Option<crate::kernel::mm::user_space::UserSlice>,
) -> Result<(), TransferError> {
    use hyper::abi::native::*;
    if operation > 3 {
        return Err(TransferError::Status(HYPER_NATIVE_STATUS_INVALID_ARGUMENT));
    }
    let rights = if operation == 0 {
        Rights::READ
    } else {
        Rights::WRITE
    };
    let block = process
        .resolve_handle::<NativeBlock>(handle, rights)
        .map_err(TransferError::Capability)?;
    let mut device = BlockAccess {
        device: block.object().device.clone(),
    };
    if operation == 2 {
        return device.flush().map_err(TransferError::Device);
    }
    let length = buffer.map_or(0, |buffer| buffer.length() as usize);
    let maximum = if operation == 3 {
        HYPER_NATIVE_NATIVE_BLOCK_TRANSFER_FRAME_BYTES
    } else {
        HYPER_NATIVE_NATIVE_BLOCK_TRANSFER_BYTES
    };
    if length as u64 > maximum || (operation != 3 && !length.is_multiple_of(SECTOR_SIZE)) {
        return Err(TransferError::Status(HYPER_NATIVE_STATUS_INVALID_ARGUMENT));
    }
    if operation == 3 {
        if first == 0
            || first > HYPER_NATIVE_NATIVE_BLOCK_BATCH_MAX
            || length < first as usize * HYPER_NATIVE_NATIVE_BLOCK_BATCH_RECORD_BYTES as usize
        {
            return Err(TransferError::Status(HYPER_NATIVE_STATUS_INVALID_ARGUMENT));
        }
    } else {
        check_range(first, length, device.sector_count()).map_err(TransferError::Device)?;
    }
    if length == 0 {
        return Ok(());
    }
    let buffer = buffer.ok_or(TransferError::Status(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))?;
    let _charge = process
        .resource_domain()
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, length as u64))
        .map_err(|_| TransferError::Status(HYPER_NATIVE_STATUS_RESOURCE_LIMIT))?
        .commit();
    let mut bytes = alloc::vec::Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| TransferError::Status(HYPER_NATIVE_STATUS_NO_MEMORY))?;
    bytes.resize(length, 0);
    if operation == 0 {
        device
            .read_sectors(first, &mut bytes)
            .map_err(TransferError::Device)?;
        process
            .copy_to_user(buffer, &bytes)
            .map_err(TransferError::Capability)
    } else {
        process
            .copy_from_user(buffer, &mut bytes)
            .map_err(TransferError::Capability)?;
        if operation == 1 {
            device
                .write_sectors(first, &bytes)
                .map_err(TransferError::Device)
        } else {
            let batch =
                super::transfer::decode_write_batch(&bytes, first as usize, device.sector_count())
                    .map_err(TransferError::Device)?;
            device
                .write_batch(batch.requests())
                .map_err(TransferError::Device)
        }
    }
}
pub(crate) enum TransferError {
    Capability(crate::kernel::process::ProcessError),
    Device(Error),
    Status(i64),
}
