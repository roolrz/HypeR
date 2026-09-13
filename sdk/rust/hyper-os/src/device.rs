// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit authority to assign discovered physical devices to guest VMs.

use crate::handle::{
    DeviceAssignmentAuthorityObject, HandleRef, OwnedHandle, PendingVirtualMachineObject,
    PhysicalDeviceObject, Rights, VmoObject,
};
use crate::{Error, Result, Status};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Info {
    pub device_id: u32,
    pub transport_version: u32,
    pub mmio_size: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaExtent {
    pub physical_base: u64,
    pub length: u64,
}

/// Claims one firmware-enumerated, unbound device. The index is a discovery
/// selector, never an arbitrary physical address or interrupt number.
pub fn claim(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
    index: u32,
) -> Result<OwnedHandle<PhysicalDeviceObject>> {
    // SAFETY: Authority stays borrowed; the successful result transfers one owner.
    let result = unsafe { hyper_sys::device_claim(authority.raw().get(), index) };
    crate::guest_io::adopt_created(
        result,
        Rights::TRANSFER
            .union(Rights::DUPLICATE)
            .union(Rights::INSPECT)
            .union(Rights::WRITE),
        &[authority.raw()],
    )
}

pub fn info(device: HandleRef<'_, PhysicalDeviceObject>) -> Result<Info> {
    let mut record = hyper_abi::HyperNativePhysicalDeviceInfo {
        device_id: 0,
        transport_version: 0,
        mmio_size: 0,
    };
    // SAFETY: The typed handle and initialized output are live throughout the call.
    let result = unsafe {
        hyper_sys::physical_device_info(
            device.raw().get(),
            &mut record,
            core::mem::size_of_val(&record),
        )
    };
    crate::validate_info_result(
        result,
        hyper_abi::HYPER_NATIVE_PHYSICAL_DEVICE_INFO_MIN_SIZE,
    )?;
    if record.device_id == 0 || record.mmio_size == 0 {
        return Err(Error::InvalidResponse);
    }
    Ok(Info {
        device_id: record.device_id,
        transport_version: record.transport_version,
        mmio_size: record.mmio_size,
    })
}

/// Inspects an already resident contiguous extent for constructing DMA metadata.
/// The caller must retain the VMO. This does not enable DMA or replace the
/// frozen guest-memory grants required when attaching a device to a VM.
pub fn dma_extent(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
    vmo: HandleRef<'_, VmoObject>,
    offset: u64,
    length: u64,
) -> Result<DmaExtent> {
    let mut record = hyper_abi::HyperNativeDmaExtent {
        physical_base: 0,
        length: 0,
    };
    // SAFETY: Borrowed handles and the output record remain live for the call.
    let result = unsafe {
        hyper_sys::vmo_get_dma_extent(
            authority.raw().get(),
            vmo.raw().get(),
            offset,
            length,
            &mut record,
            core::mem::size_of_val(&record),
        )
    };
    crate::validate_info_result(result, hyper_abi::HYPER_NATIVE_DMA_EXTENT_MIN_SIZE)?;
    if record.length != length || length == 0 || record.physical_base.checked_add(length).is_none()
    {
        return Err(Error::InvalidResponse);
    }
    Ok(DmaExtent {
        physical_base: record.physical_base,
        length,
    })
}

/// Binds the claimed device before sealing a VM. The kernel owns physical reset
/// and retains the VM's DMA backing until hardware quiescence is established.
pub fn assign(
    pending: HandleRef<'_, PendingVirtualMachineObject>,
    device: HandleRef<'_, PhysicalDeviceObject>,
    base: u64,
    irq: u32,
) -> Result<()> {
    // SAFETY: Both handles stay borrowed; the remaining inputs are scalars.
    Status::from_raw(
        unsafe {
            hyper_sys::pending_virtual_machine_assign_device(
                pending.raw().get(),
                device.raw().get(),
                base,
                irq,
            )
        }
        .status,
    )
    .into_result()
}
