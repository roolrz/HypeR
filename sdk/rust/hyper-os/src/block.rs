// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native filesystem block initiator setup. Runtime policy negotiates the
//! Linux backend; the kernel owns subsequent virtio-scsi requests and waits.

use crate::handle::{
    AnyObject, DirectoryObject, GuestMemoryObject, HandleRef, NativeBlockObject, OwnedHandle,
    Rights, VirtualMachineObject,
};
use crate::{Error, Result, Status};

pub const PEER_CLOSED: u64 = hyper_abi::HYPER_NATIVE_SIGNAL_NATIVE_BLOCK_PEER_CLOSED;
pub const MEMORY_BYTES: u64 = hyper_abi::HYPER_NATIVE_NATIVE_BLOCK_MEMORY_BYTES;
pub const QUEUE_SIZE: u32 = hyper_abi::HYPER_NATIVE_NATIVE_BLOCK_QUEUE_SIZE as u32;
pub const QUEUE_STRIDE: u64 = hyper_abi::HYPER_NATIVE_NATIVE_BLOCK_QUEUE_STRIDE;
pub const AVAILABLE_OFFSET: u64 = hyper_abi::HYPER_NATIVE_NATIVE_BLOCK_AVAILABLE_OFFSET;
pub const USED_OFFSET: u64 = hyper_abi::HYPER_NATIVE_NATIVE_BLOCK_USED_OFFSET;

pub struct NativeBlock {
    handle: OwnedHandle<NativeBlockObject>,
}
impl NativeBlock {
    /// Dedicates an exactly `MEMORY_BYTES` resident grant to one initiator.
    /// Map that grant into the backend before installation. Queue addresses in
    /// ACTIVATE use `guest_base`; the Linux alias may have a different address.
    pub fn create(
        memory: HandleRef<'_, GuestMemoryObject>,
        backend: HandleRef<'_, VirtualMachineObject>,
        guest_base: u64,
        notification_base: u64,
        notification_irq: u32,
    ) -> Result<Self> {
        // SAFETY: Both capabilities stay borrowed for this non-retaining call.
        let result = unsafe {
            hyper_sys::native_block_create(
                memory.raw().get(),
                backend.raw().get(),
                guest_base,
                notification_base,
                notification_irq,
            )
        };
        Status::from_raw(result.status).into_result()?;
        // SAFETY: A successful create returns an independently owned handle.
        let owner = unsafe {
            crate::handle::adopt_produced_handle_excluding::<AnyObject>(
                result.value0,
                &[memory.raw(), backend.raw()],
            )?
        };
        let rights = Rights::WRITE
            .union(Rights::MAP)
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::WAIT);
        if result.value1 != 0 || owner.info()?.rights != rights {
            return Err(Error::InvalidResponse);
        }
        Ok(Self {
            handle: owner
                .downcast::<NativeBlockObject>()
                .map_err(|failure| failure.error())?,
        })
    }
    pub fn as_handle_ref(&self) -> HandleRef<'_, NativeBlockObject> {
        self.handle.as_handle_ref()
    }
    /// Call only after backend ACTIVATE succeeds. Returns verified 512-byte
    /// sector count, rather than trusting a userspace-supplied geometry.
    pub fn activate(&self, readonly: bool) -> Result<u64> {
        // SAFETY: The block capability remains borrowed through discovery.
        let result = unsafe {
            hyper_sys::native_block_activate(self.handle.as_handle_ref().raw().get(), readonly)
        };
        Status::from_raw(result.status).into_result()?;
        if result.value0 == 0 || result.value1 != 0 {
            return Err(Error::InvalidResponse);
        }
        Ok(result.value0)
    }
    /// Mount authority is carried by this handle's MAP right. The directory
    /// must additionally grant namespace modification and traversal rights.
    pub fn mount(&self, directory: HandleRef<'_, DirectoryObject>, path: &str) -> Result<()> {
        // SAFETY: Borrowed handles and path stay valid throughout the call.
        let result = unsafe {
            hyper_sys::native_block_mount(
                self.handle.as_handle_ref().raw().get(),
                directory.raw().get(),
                path.as_ptr(),
                path.len(),
            )
        };
        Status::from_raw(result.status).into_result()?;
        if result.value0 != 0 || result.value1 != 0 {
            return Err(Error::InvalidResponse);
        }
        Ok(())
    }
}
