// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit ownership of a firmware-discovered physical virtio transport.

mod model;
mod object;
pub(crate) mod service;
pub(crate) use object::{Assignment, DeviceAssignmentAuthority, PhysicalDevice};
pub(crate) use service::{DmaExtent, Info};

#[cfg_attr(feature = "kernel-self-test", allow(dead_code))]
pub(crate) const fn available() -> bool {
    crate::hal::vm::supports_guest_device_assignment()
}

use crate::kernel::irq::interrupt::IrqDomainId;
use hyper::drivers::platform::{DriverServices, PermanentMmioMapping, PlatformDevice};
use hyper::hal::interrupt::{InterruptId, InterruptTrigger};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Unsupported,
    BadState,
    InvalidArgument,
    Resource,
    Interrupt,
    Quarantined,
}

#[derive(Clone, Copy)]
struct Hardware {
    mapping: PermanentMmioMapping,
    domain: IrqDomainId,
    interrupt: InterruptId,
    trigger: InterruptTrigger,
}

impl Hardware {
    fn read(self, offset: usize) -> u32 {
        // SAFETY: Discovery validates the permanently mapped transport register window;
        // callers use aligned constant/register-window-checked offsets only.
        unsafe {
            core::ptr::with_exposed_provenance::<u32>(self.mapping.virtual_start() + offset)
                .read_volatile()
        }
    }
    fn read_access(self, offset: usize, width: usize) -> u64 {
        let address = self.mapping.virtual_start() + offset;
        // SAFETY: access_at validates width, alignment and mapped extent. Device
        // configuration fields use byte accesses in upstream virtio-mmio.
        unsafe {
            match width {
                1 => core::ptr::with_exposed_provenance::<u8>(address).read_volatile() as u64,
                2 => core::ptr::with_exposed_provenance::<u16>(address).read_volatile() as u64,
                _ => core::ptr::with_exposed_provenance::<u32>(address).read_volatile() as u64,
            }
        }
    }
    fn write_access(self, offset: usize, width: usize, value: u64) {
        let address = self.mapping.virtual_start() + offset;
        // SAFETY: Same checked register extent as read_access. The device state
        // lock owns register mutation through the exclusive physical claim.
        unsafe {
            match width {
                1 => core::ptr::with_exposed_provenance_mut::<u8>(address)
                    .write_volatile(value as u8),
                2 => core::ptr::with_exposed_provenance_mut::<u16>(address)
                    .write_volatile(value as u16),
                _ => core::ptr::with_exposed_provenance_mut::<u32>(address)
                    .write_volatile(value as u32),
            }
        }
    }
    fn write(self, offset: usize, value: u32) {
        // SAFETY: Same mapping proof as read; the exclusive claim serializes
        // guest register access and final reset. No native driver binds it.
        unsafe {
            core::ptr::with_exposed_provenance_mut::<u32>(self.mapping.virtual_start() + offset)
                .write_volatile(value)
        }
    }
}

pub(super) struct Resource {
    hardware: Hardware,
    claimed: bool,
}
impl Resource {
    pub(super) fn discover(
        device: &PlatformDevice,
        services: &dyn DriverServices,
        domain: IrqDomainId,
    ) -> Option<Self> {
        if !crate::hal::vm::supports_guest_device_assignment() {
            return None;
        }
        let mapping = services
            .map_mmio(*device.registers().first()?)
            .ok()?
            .validate_window(0x200, 4)
            .ok()?;
        let irq = crate::hal::irq::decode_platform(device.interrupt_cells()).ok()?;
        if irq.interrupt < 32 {
            return None;
        }
        let trigger = match irq.trigger {
            hyper::platform::PlatformInterruptTrigger::Level => InterruptTrigger::Level,
            hyper::platform::PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
        };
        let hardware = Hardware {
            mapping,
            domain,
            interrupt: InterruptId::new(irq.interrupt),
            trigger,
        };
        // Empty QEMU virtio slots are not advertised as assignable devices.
        // This driver supports the modern SCSI transport's reset semantics.
        if hardware.read(0) != 0x7472_6976
            || hardware.read(4) != 2
            || hardware.read(8) != 8
            || hardware.read(0x70) != 0
        {
            return None;
        }
        hardware.write(0x14, 1);
        let dma_api = hardware.read(0x10) & 2 != 0;
        hardware.write(0x14, 0);
        if !dma_api {
            return None;
        }
        Some(Self {
            hardware,
            claimed: false,
        })
    }
    pub(super) fn claim(&mut self, index: usize) -> Option<Claim> {
        if self.claimed {
            return None;
        }
        self.claimed = true;
        Some(Claim {
            index,
            hardware: self.hardware,
        })
    }
    pub(super) fn release(&mut self) {
        self.claimed = false;
    }
}

pub(super) struct Claim {
    index: usize,
    hardware: Hardware,
}
impl Drop for Claim {
    fn drop(&mut self) {
        super::platform_bus::release(self.index);
    }
}
