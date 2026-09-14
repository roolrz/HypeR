// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability checks for physical assignment; no arbitrary host address input.

use super::{Assignment, DeviceAssignmentAuthority, PhysicalDevice};
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle, Rights};
use crate::kernel::mm::user_space::VmoObject;
use crate::kernel::object::{KernelObject, ObjectPublication};
use crate::kernel::process::Process;
use crate::kernel::vm::service::Error;

#[derive(Debug)]
pub(crate) enum MatchError {
    Service(Error),
    Missing,
    Ambiguous,
}
impl From<Error> for MatchError {
    fn from(error: Error) -> Self {
        Self::Service(error)
    }
}
impl From<super::model::SelectionError> for MatchError {
    fn from(error: super::model::SelectionError) -> Self {
        match error {
            super::model::SelectionError::Missing => Self::Missing,
            super::model::SelectionError::Ambiguous => Self::Ambiguous,
        }
    }
}

pub(crate) fn claim_matching(
    process: &Process,
    authority: HandleValue,
    profile: u32,
    identity_kind: u32,
    identity: &str,
) -> Result<HandleValue, MatchError> {
    if identity.is_empty()
        || identity.len() > 512
        || identity.bytes().any(|byte| byte <= 32 || byte == 127)
        || !matches!(identity_kind, 1 | 2)
        || (identity_kind == 2
            && (!identity.starts_with('/')
                || identity.ends_with('/')
                || identity
                    .split('/')
                    .skip(1)
                    .any(|part| matches!(part, "" | "." | ".."))))
    {
        return Err(Error::InvalidArgument.into());
    }
    process
        .resolve_handle::<DeviceAssignmentAuthority>(authority, Rights::INSPECT)
        .map_err(Error::from)?;
    if profile != 1 {
        return Err(Error::NotSupported.into());
    }
    if !crate::hal::vm::supports_guest_device_assignment() {
        return Err(Error::NotSupported.into());
    }
    let reservation = process.reserve_handles::<1>().map_err(Error::from)?;
    let (reservation, prepared) = super::transaction::prepare(
        reservation,
        || -> Result<_, MatchError> {
            let object = PhysicalDevice::claim_matching(
                profile,
                identity_kind,
                identity,
                &process.resource_domain(),
            )?;
            let publication = ObjectPublication::try_new(object).map_err(|_| Error::NoMemory)?;
            let prepared = PreparedHandle::try_from_new_object(
                publication,
                Rights::TRANSFER
                    .union(Rights::DUPLICATE)
                    .union(Rights::INSPECT)
                    .union(Rights::WRITE),
                HandleFlags::NONE,
            )
            .map_err(|_| Error::NoMemory)?;
            Ok(prepared)
        },
        |reservation| process.abort_handles(reservation),
    )?;
    process
        .publish_handles(reservation, [prepared])
        .map(|handles| handles[0])
        .map_err(|failure| MatchError::Service(failure.error.into()))
}

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
    let (reservation, prepared) = super::transaction::prepare(
        reservation,
        || -> Result<_, Error> {
            let object = PhysicalDevice::claim(index as usize, &process.resource_domain())
                .map_err(classify)?;
            let publication = ObjectPublication::try_new(object).map_err(|_| Error::NoMemory)?;
            let prepared = PreparedHandle::try_from_new_object(
                publication,
                Rights::TRANSFER
                    .union(Rights::DUPLICATE)
                    .union(Rights::INSPECT)
                    .union(Rights::WRITE),
                HandleFlags::NONE,
            )
            .map_err(|_| Error::NoMemory)?;
            Ok(prepared)
        },
        |reservation| process.abort_handles(reservation),
    )?;
    process
        .publish_handles(reservation, [prepared])
        .map(|handles| handles[0])
        .map_err(|failure| failure.error.into())
}
pub(crate) fn profile_info(process: &Process, device: HandleValue) -> Result<[u8; 32], Error> {
    let device = process.resolve_handle::<PhysicalDevice>(device, Rights::INSPECT)?;
    Ok(device.object().profile_info())
}
pub(crate) fn resource_info(
    process: &Process,
    device: HandleValue,
    index: u32,
) -> Result<[u8; 32], Error> {
    let device = process.resolve_handle::<PhysicalDevice>(device, Rights::INSPECT)?;
    device.object().resource_info(index).map_err(classify)
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
        super::Error::Busy => Error::Busy,
        super::Error::BadState | super::Error::Interrupt | super::Error::Quarantined => {
            Error::BadState
        }
    }
}

pub(crate) fn firmware_read(
    process: &Process,
    authority: HandleValue,
    node: u32,
    field: u32,
    name: &str,
) -> Result<alloc::vec::Vec<u8>, MatchError> {
    process
        .resolve_handle::<DeviceAssignmentAuthority>(authority, Rights::INSPECT)
        .map_err(Error::from)?;
    super::super::platform_bus::catalogue()
        .map_err(classify)?
        .read(node, field, name)
}

pub(crate) fn claim_bundle(
    process: &Process,
    authority: HandleValue,
    entries: &[(u32, u32, u64)],
    irq_node: u32,
) -> Result<HandleValue, Error> {
    process.resolve_handle::<DeviceAssignmentAuthority>(authority, Rights::INSPECT)?;
    let reservation = process.reserve_handles::<1>()?;
    let (reservation, prepared) = super::transaction::prepare(
        reservation,
        || -> Result<_, Error> {
            let object =
                PhysicalDevice::claim_bundle(entries, irq_node, &process.resource_domain())
                    .map_err(classify)?;
            let publication = ObjectPublication::try_new(object).map_err(|_| Error::NoMemory)?;
            PreparedHandle::try_from_new_object(
                publication,
                PhysicalDevice::SUPPORTED_RIGHTS,
                HandleFlags::NONE,
            )
            .map_err(|_| Error::NoMemory)
        },
        |reservation| process.abort_handles(reservation),
    )?;
    process
        .publish_handles(reservation, [prepared])
        .map(|handles| handles[0])
        .map_err(|failure| failure.error.into())
}
pub(crate) fn mmio(
    process: &Process,
    device: HandleValue,
    offset: u64,
    width: u32,
    write: bool,
    value: u64,
) -> Result<u64, Error> {
    process
        .resolve_handle::<PhysicalDevice>(device, Rights::WRITE)?
        .object()
        .mmio(offset, width, write, value)
        .map_err(classify)
}
pub(crate) fn irq_pending(process: &Process, device: HandleValue) -> Result<u64, Error> {
    process
        .resolve_handle::<PhysicalDevice>(device, Rights::WAIT)?
        .object()
        .irq_pending()
        .map_err(classify)
}
pub(crate) fn irq_complete(
    process: &Process,
    device: HandleValue,
    sequence: u64,
    asserted: bool,
) -> Result<(), Error> {
    process
        .resolve_handle::<PhysicalDevice>(device, Rights::WRITE)?
        .object()
        .irq_complete(sequence, asserted)
        .map_err(classify)
}
