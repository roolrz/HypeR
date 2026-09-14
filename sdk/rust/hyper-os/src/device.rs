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
    /// Virtio device identifier, or zero for a userspace-managed device.
    pub device_id: u32,
    /// Virtio transport version, or zero when no kernel transport is identified.
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

/// Assignment profiles encode the kernel's validated device/reset contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Profile {
    VirtioMmioScsi = hyper_abi::HYPER_NATIVE_DEVICE_PROFILE_VIRTIO_MMIO_SCSI as u32,
    /// Register semantics and IRQ acknowledgement are owned by userspace.
    Userspace = hyper_abi::HYPER_NATIVE_DEVICE_PROFILE_USERSPACE as u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareIdentity<'a> {
    Compatible(&'a str),
    FdtPath(&'a str),
}

/// Claims a unique profile/firmware match. Ambiguous selectors fail instead of
/// falling back to a different device when the first match is already claimed.
pub fn claim_matching(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
    profile: Profile,
    identity: FirmwareIdentity<'_>,
) -> Result<OwnedHandle<PhysicalDeviceObject>> {
    let (kind, text) = match identity {
        FirmwareIdentity::Compatible(text) => (
            hyper_abi::HYPER_NATIVE_DEVICE_IDENTITY_COMPATIBLE as u32,
            text,
        ),
        FirmwareIdentity::FdtPath(text) => (
            hyper_abi::HYPER_NATIVE_DEVICE_IDENTITY_FDT_PATH as u32,
            text,
        ),
    };
    // SAFETY: Authority and identity remain borrowed throughout this call.
    let result = unsafe {
        hyper_sys::device_claim_matching(
            authority.raw().get(),
            profile as u32,
            kind,
            text.as_ptr(),
            text.len(),
        )
    };
    crate::guest_io::adopt_created(
        result,
        Rights::TRANSFER
            .union(Rights::DUPLICATE)
            .union(Rights::INSPECT)
            .union(Rights::WRITE),
        &[authority.raw()],
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileInfo {
    pub profile: Profile,
    pub resource_count: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceInfo {
    pub kind: u32,
    pub offset: u64,
    pub length: u64,
}

pub fn profile_info(device: HandleRef<'_, PhysicalDeviceObject>) -> Result<ProfileInfo> {
    let mut record = hyper_abi::HyperNativeDeviceProfileInfo {
        profile: 0,
        resource_count: 0,
        reserved0: 0,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
    };
    // SAFETY: Typed handle and initialized output remain live throughout the call.
    let result = unsafe {
        hyper_sys::device_profile_info(
            device.raw().get(),
            &mut record,
            core::mem::size_of_val(&record),
        )
    };
    crate::validate_info_result(result, hyper_abi::HYPER_NATIVE_DEVICE_PROFILE_INFO_MIN_SIZE)?;
    let profile = match record.profile {
        1 => Profile::VirtioMmioScsi,
        2 => Profile::Userspace,
        _ => return Err(Error::InvalidResponse),
    };
    if record.resource_count == 0
        || record.resource_count > 8
        || record.reserved0 != 0
        || record.reserved1 != 0
        || record.reserved2 != 0
        || record.reserved3 != 0
    {
        return Err(Error::InvalidResponse);
    }
    Ok(ProfileInfo {
        profile,
        resource_count: record.resource_count,
    })
}

pub fn resource_info(
    device: HandleRef<'_, PhysicalDeviceObject>,
    index: u32,
) -> Result<ResourceInfo> {
    let mut record = hyper_abi::HyperNativeDeviceResourceInfo {
        kind: 0,
        reserved: 0,
        offset: 0,
        length: 0,
        reserved2: 0,
    };
    // SAFETY: Typed handle and initialized output remain live throughout the call.
    let result = unsafe {
        hyper_sys::device_resource_info(
            device.raw().get(),
            index,
            &mut record,
            core::mem::size_of_val(&record),
        )
    };
    crate::validate_info_result(
        result,
        hyper_abi::HYPER_NATIVE_DEVICE_RESOURCE_INFO_MIN_SIZE,
    )?;
    if record.reserved != 0
        || record.reserved2 != 0
        || record.length == 0
        || record
            .offset
            .checked_add(record.length)
            .is_none_or(|end| end > 65536)
    {
        return Err(Error::InvalidResponse);
    }
    Ok(ResourceInfo {
        kind: record.kind,
        offset: record.offset,
        length: record.length,
    })
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
    decode_info(record)
}

fn decode_info(record: hyper_abi::HyperNativePhysicalDeviceInfo) -> Result<Info> {
    if (record.device_id == 0 && record.transport_version != 0) || record.mmio_size == 0 {
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

#[path = "device/firmware.rs"]
mod firmware;
pub use firmware::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_info_accepts_userspace_and_virtio_identity() {
        for (device_id, transport_version) in [(0, 0), (8, 2)] {
            let expected = Info {
                device_id,
                transport_version,
                mmio_size: 65536,
            };
            assert_eq!(
                decode_info(hyper_abi::HyperNativePhysicalDeviceInfo {
                    device_id,
                    transport_version,
                    mmio_size: 65536,
                }),
                Ok(expected)
            );
        }
        for (device_id, transport_version, mmio_size) in [(0, 2, 65536), (0, 0, 0), (8, 2, 0)] {
            assert_eq!(
                decode_info(hyper_abi::HyperNativePhysicalDeviceInfo {
                    device_id,
                    transport_version,
                    mmio_size,
                }),
                Err(Error::InvalidResponse)
            );
        }
    }
}
