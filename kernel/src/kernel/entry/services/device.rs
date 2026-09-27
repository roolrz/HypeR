// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native device service calls under borrowed Process authority.

use super::DeferredProcessServices;
use crate::kernel::capability::HandleValue;

impl crate::kernel::abi::native::DeviceServices for DeferredProcessServices<'_> {
    fn device_firmware_read(
        &self,
        authority: HandleValue,
        node: u32,
        field: u32,
        name: &str,
    ) -> Result<alloc::vec::Vec<u8>, crate::kernel::device::assigned::service::MatchError> {
        crate::kernel::device::assigned::service::firmware_read(
            self.process,
            authority,
            node,
            field,
            name,
        )
    }
    fn device_claim_bundle(
        &self,
        authority: HandleValue,
        entries: &[(u32, u32, u64)],
        irq_node: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::claim_bundle(
            self.process,
            authority,
            entries,
            irq_node,
        )
    }
    fn device_mmio(
        &self,
        device: HandleValue,
        offset: u64,
        width: u32,
        write: bool,
        value: u64,
    ) -> Result<u64, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::mmio(
            self.process,
            device,
            offset,
            width,
            write,
            value,
        )
    }
    fn device_irq_pending(
        &self,
        device: HandleValue,
    ) -> Result<u64, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::irq_pending(self.process, device)
    }
    fn device_irq_complete(
        &self,
        device: HandleValue,
        sequence: u64,
        asserted: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::irq_complete(
            self.process,
            device,
            sequence,
            asserted,
        )
    }

    fn device_profile_info(
        &self,
        device: HandleValue,
    ) -> Result<[u8; 32], crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::profile_info(self.process, device)
    }
    fn device_resource_info(
        &self,
        device: HandleValue,
        index: u32,
    ) -> Result<[u8; 32], crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::resource_info(self.process, device, index)
    }
    fn claim_device_matching(
        &self,
        authority: HandleValue,
        profile: u32,
        identity_kind: u32,
        identity: &str,
    ) -> Result<HandleValue, crate::kernel::device::assigned::service::MatchError> {
        crate::kernel::device::assigned::service::claim_matching(
            self.process,
            authority,
            profile,
            identity_kind,
            identity,
        )
    }
    fn claim_device(
        &self,
        authority: HandleValue,
        index: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::claim(self.process, authority, index)
    }
    fn physical_device_info(
        &self,
        device: HandleValue,
    ) -> Result<crate::kernel::device::assigned::Info, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::info(self.process, device)
    }
    fn vmo_dma_extent(
        &self,
        authority: HandleValue,
        vmo: HandleValue,
        offset: u64,
        length: u64,
    ) -> Result<crate::kernel::device::assigned::DmaExtent, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::dma_extent(
            self.process,
            authority,
            vmo,
            offset,
            length,
        )
    }
    fn assign_physical_device(
        &self,
        pending: HandleValue,
        device: HandleValue,
        base: u64,
        irq: u32,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::assign(self.process, pending, device, base, irq)
    }
}
