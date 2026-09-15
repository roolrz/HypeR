// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-bound setup and exclusive filesystem mount admission.

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
            mounted: false,
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
) -> Result<u64, ActivationError> {
    let block = process
        .resolve_handle::<NativeBlock>(handle, Rights::WRITE)
        .map_err(ActivationError::Capability)?;
    block
        .object()
        .device
        .activate(readonly)
        .map_err(ActivationError::Device)
}

pub(crate) fn claim_mount(
    process: &Process,
    handle: HandleValue,
) -> Result<MountedDevice, ServiceError> {
    let block = process.resolve_handle::<NativeBlock>(handle, Rights::MAP)?;
    let device = &block.object().device;
    device.state.with(|state| {
        if state.sectors == 0 || state.failed || !state.capability_alive {
            return Err(ServiceError::BadState);
        }
        if state.mounted {
            return Err(ServiceError::Busy);
        }
        state.mounted = true;
        Ok(())
    })?;
    Ok(MountedDevice {
        device: device.clone(),
    })
}
