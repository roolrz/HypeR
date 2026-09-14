// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability checks for physical assignment; no arbitrary host address input.

use super::{Assignment, DeviceAssignmentAuthority, PhysicalDevice};
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle, Rights};
use crate::kernel::mm::user_space::VmoObject;
use crate::kernel::object::{KernelObject, ObjectPublication};
use crate::kernel::process::Process;
use crate::kernel::vm::service::Error;

pub(crate) struct Info {
    pub(crate) device_id: u32,
    pub(crate) transport_version: u32,
    pub(crate) mmio_size: u64,
}
pub(crate) struct DmaExtent {
    pub(crate) physical_base: u64,
    pub(crate) length: u64,
}

pub(crate) fn claim(
    process: &Process,
    authority: HandleValue,
    index: u32,
) -> Result<HandleValue, Error> {
    let _authority =
        process.resolve_handle::<DeviceAssignmentAuthority>(authority, Rights::INSPECT)?;
    let reservation = process.reserve_handles::<1>()?;
    let object =
        PhysicalDevice::claim(index as usize, &process.resource_domain()).map_err(classify)?;
    let publication = ObjectPublication::try_new(object).map_err(|_| Error::NoMemory)?;
    let prepared = PreparedHandle::try_from_new_object(
        publication,
        PhysicalDevice::SUPPORTED_RIGHTS,
        HandleFlags::NONE,
    )
    .map_err(|_| Error::NoMemory)?;
    process
        .publish_handles(reservation, [prepared])
        .map(|handles| handles[0])
        .map_err(|failure| failure.error.into())
}
pub(crate) fn info(process: &Process, device: HandleValue) -> Result<Info, Error> {
    Ok(process
        .resolve_handle::<PhysicalDevice>(device, Rights::INSPECT)?
        .object()
        .info())
}
pub(crate) fn dma_extent(
    process: &Process,
    authority: HandleValue,
    vmo: HandleValue,
    offset: u64,
    length: u64,
) -> Result<DmaExtent, Error> {
    let _authority =
        process.resolve_handle::<DeviceAssignmentAuthority>(authority, Rights::INSPECT)?;
    let vmo = process.resolve_handle::<VmoObject>(vmo, Rights::READ.union(Rights::MAP))?;
    let storage = vmo.object().writable().ok_or(Error::InvalidArgument)?;
    let first = super::model::contiguous_extent(
        offset,
        length,
        storage.size(),
        hyper::mm::PAGE_SIZE,
        |offset| {
            storage
                .resident_physical_page(offset)
                .ok()
                .map(|page| page.get())
        },
    )
    .map_err(|error| match error {
        super::model::ExtentError::Range => Error::InvalidArgument,
        super::model::ExtentError::Nonresident => Error::BadState,
        super::model::ExtentError::Noncontiguous => Error::NotSupported,
    })?;
    // This retained ordinary VMO never substitutes resident frames. This is
    // information for DT construction, not permission to start hardware DMA.
    Ok(DmaExtent {
        physical_base: first,
        length,
    })
}
pub(crate) fn assign(
    process: &Process,
    pending: HandleValue,
    device: HandleValue,
    base: u64,
    irq: u32,
) -> Result<(), Error> {
    let pending = process.resolve_handle::<crate::kernel::vm::objects::PendingVirtualMachine>(
        pending,
        Rights::WRITE,
    )?;
    let device = process.resolve_handle::<PhysicalDevice>(device, Rights::WRITE)?;
    let assignment = Assignment::new(
        device.into_operation_pin().into_vm_device_binding(),
        base,
        irq,
    )
    .map_err(classify)?;
    pending
        .object()
        .assign_physical(assignment)
        .map_err(Into::into)
}
pub(crate) const fn classify(error: super::Error) -> Error {
    match error {
        super::Error::Unsupported => Error::NotSupported,
        super::Error::InvalidArgument => Error::InvalidArgument,
        super::Error::Resource => Error::NoMemory,
        super::Error::BadState | super::Error::Interrupt | super::Error::Quarantined => {
            Error::BadState
        }
    }
}
