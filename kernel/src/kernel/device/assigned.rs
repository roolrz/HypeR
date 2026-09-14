// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exclusive ownership of firmware-discovered physical register resources.

mod model;
mod transaction;
pub(super) use model::unique_match as select_unique;
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
    Busy,
}

#[derive(Clone, Copy)]
struct Window {
    mapping: PermanentMmioMapping,
    offset: usize,
}
#[derive(Clone, Copy)]
enum Profile {
    Virtio,
    Userspace,
}
impl Profile {
    const fn id(self) -> u32 {
        match self {
            Self::Virtio => 1,
            Self::Userspace => 2,
        }
    }
}

#[derive(Clone, Copy)]
struct Hardware {
    extra: [Option<Window>; 7],
    profile: Profile,
    mapping: PermanentMmioMapping,
    domain: IrqDomainId,
    interrupt: InterruptId,
    trigger: InterruptTrigger,
}

impl Hardware {
    fn line_asserted(self) -> bool {
        match self.profile {
            Profile::Virtio => self.read(0x60) != 0,
            Profile::Userspace => false,
        }
    }
    fn register(self, offset: usize, width: usize) -> Option<(PermanentMmioMapping, usize)> {
        if let Some(local) =
            model::register_offset(offset, width, 0, self.mapping.resource().size())
        {
            return Some((self.mapping, local));
        }
        self.extra.iter().flatten().find_map(|window| {
            model::register_offset(
                offset,
                width,
                window.offset,
                window.mapping.resource().size(),
            )
            .map(|local| (window.mapping, local))
        })
    }

    fn read(self, offset: usize) -> u32 {
        // SAFETY: Discovery validates the permanently mapped transport register window;
        // callers use aligned constant/register-window-checked offsets only.
        unsafe {
            core::ptr::with_exposed_provenance::<u32>(self.mapping.virtual_start() + offset)
                .read_volatile()
        }
    }
    fn read_access(self, offset: usize, width: usize) -> Option<u64> {
        let (mapping, offset) = self.register(offset, width)?;
        let address = mapping.virtual_start() + offset;
        // SAFETY: access_at validates width, alignment and mapped extent. Device
        // configuration fields use byte accesses in upstream virtio-mmio.
        Some(unsafe {
            match width {
                1 => core::ptr::with_exposed_provenance::<u8>(address).read_volatile() as u64,
                2 => core::ptr::with_exposed_provenance::<u16>(address).read_volatile() as u64,
                _ => core::ptr::with_exposed_provenance::<u32>(address).read_volatile() as u64,
            }
        })
    }
    fn write_access(self, offset: usize, width: usize, value: u64) -> bool {
        let Some((mapping, offset)) = self.register(offset, width) else {
            return false;
        };
        let address = mapping.virtual_start() + offset;
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
        true
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
    firmware: hyper::platform::fdt::NodeId,
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
            extra: [None; 7],
            profile: Profile::Virtio,
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
            firmware: device.id(),
            claimed: false,
        })
    }
    /// No probe or register interpretation: only firmware-owned extents.
    pub(super) fn discover_userspace(
        device: &PlatformDevice,
        services: &dyn DriverServices,
        domain: IrqDomainId,
    ) -> Option<Self> {
        if device.registers().is_empty() || device.registers().len() > 8 {
            return None;
        }
        let mut windows = [None; 8];
        for (slot, range) in windows.iter_mut().zip(device.registers()) {
            if range.size() == 0 || !range.start().is_multiple_of(4) {
                return None;
            }
            *slot = Some(Window {
                mapping: services.map_mmio(*range).ok()?,
                offset: 0,
            });
        }
        let irq = crate::hal::irq::decode_platform(device.interrupt_cells()).ok();
        let (interrupt, trigger) = match irq {
            Some(irq) => (
                irq.interrupt,
                match irq.trigger {
                    hyper::platform::PlatformInterruptTrigger::Level => InterruptTrigger::Level,
                    hyper::platform::PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
                },
            ),
            None => (0, InterruptTrigger::Level),
        };
        let mut extra = [None; 7];
        extra.copy_from_slice(&windows[1..]);
        Some(Self {
            hardware: Hardware {
                mapping: windows[0]?.mapping,
                extra,
                profile: Profile::Userspace,
                domain,
                interrupt: InterruptId::new(interrupt),
                trigger,
            },
            firmware: device.id(),
            claimed: false,
        })
    }

    fn window(&self, index: u32) -> Option<Window> {
        if index == 0 {
            Some(Window {
                mapping: self.hardware.mapping,
                offset: 0,
            })
        } else {
            self.hardware
                .extra
                .get(index as usize - 1)
                .copied()
                .flatten()
        }
    }

    /// Validate a whole resource bundle before publishing any exclusive claim.
    pub(super) fn claim_bundle(
        resources: &mut [Self],
        selected: &[(usize, u32, u64)],
        interrupt_owner: usize,
    ) -> Result<Claim, Error> {
        if selected.is_empty() || selected.len() > 8 {
            return Err(Error::InvalidArgument);
        }
        let irq = resources
            .get(interrupt_owner)
            .ok_or(Error::InvalidArgument)?
            .hardware;
        if !matches!(irq.profile, Profile::Userspace)
            || irq.interrupt.get() < 32
            || !matches!(irq.trigger, InterruptTrigger::Level)
            || !selected.iter().any(|entry| entry.0 == interrupt_owner)
        {
            return Err(Error::Unsupported);
        }
        let mut indices = [None; 8];
        let mut windows = [None; 8];
        for (position, &(index, register, offset)) in selected.iter().enumerate() {
            let source = resources.get(index).ok_or(Error::InvalidArgument)?;
            if !matches!(source.hardware.profile, Profile::Userspace) {
                return Err(Error::InvalidArgument);
            }
            if source.claimed
                || resources
                    .iter()
                    .any(|other| other.claimed && source.conflicts(other))
            {
                return Err(Error::Busy);
            }
            let mut window = source.window(register).ok_or(Error::InvalidArgument)?;
            let size = window.mapping.resource().size();
            let end = offset.checked_add(size).ok_or(Error::InvalidArgument)?;
            if end > 65536 || !offset.is_multiple_of(4) {
                return Err(Error::InvalidArgument);
            }
            for prior in windows[..position].iter().flatten() {
                let prior: &Window = prior;
                let prior_end = prior.offset as u64 + prior.mapping.resource().size();
                let physical = window.mapping.resource();
                let other = prior.mapping.resource();
                if (offset < prior_end && (prior.offset as u64) < end)
                    || (physical.start() < other.start() + other.size()
                        && other.start() < physical.start() + physical.size())
                {
                    return Err(Error::InvalidArgument);
                }
            }
            // The legacy primary field is based at offset zero. Require it
            // explicitly, rather than silently granting an unrequested window.
            if position == 0 && offset != 0 {
                return Err(Error::InvalidArgument);
            }
            window.offset = offset as usize;
            windows[position] = Some(window);
            if !indices.contains(&Some(index)) {
                indices[position] = Some(index);
            }
        }
        let mut extra = [None; 7];
        extra.copy_from_slice(&windows[1..]);
        let first = windows[0].ok_or(Error::InvalidArgument)?;
        let hardware = Hardware {
            mapping: first.mapping,
            extra,
            ..irq
        };
        for index in indices.iter().flatten() {
            resources[*index].claimed = true;
        }
        Ok(Claim { indices, hardware })
    }

    pub(super) fn profile(&self) -> u32 {
        self.hardware.profile.id()
    }
    pub(super) fn conflicts(&self, other: &Self) -> bool {
        if self.hardware.interrupt.get() >= 32
            && self.hardware.interrupt == other.hardware.interrupt
        {
            return true;
        }
        let ranges = |hardware: Hardware| {
            core::iter::once(hardware.mapping.resource()).chain(
                hardware
                    .extra
                    .into_iter()
                    .flatten()
                    .map(|window| window.mapping.resource()),
            )
        };
        ranges(self.hardware).any(|left| {
            ranges(other.hardware).any(|right| {
                let left_start = left.start() & !4095;
                let right_start = right.start() & !4095;
                left_start
                    < (right
                        .start()
                        .saturating_add(right.size())
                        .saturating_add(4095)
                        & !4095)
                    && right_start
                        < (left
                            .start()
                            .saturating_add(left.size())
                            .saturating_add(4095)
                            & !4095)
            })
        })
    }
    pub(super) fn claimed(&self) -> bool {
        self.claimed
    }
    pub(super) fn firmware(&self) -> hyper::platform::fdt::NodeId {
        self.firmware
    }
    pub(super) fn claim(&mut self, index: usize) -> Option<Claim> {
        if self.claimed {
            return None;
        }
        self.claimed = true;
        Some(Claim {
            indices: [Some(index), None, None, None, None, None, None, None],
            hardware: self.hardware,
        })
    }
    pub(super) fn release(&mut self) {
        self.claimed = false;
    }
}

pub(super) struct Claim {
    indices: [Option<usize>; 8],
    hardware: Hardware,
}
impl Drop for Claim {
    fn drop(&mut self) {
        for index in self.indices.into_iter().flatten() {
            super::platform_bus::release(index);
        }
    }
}
