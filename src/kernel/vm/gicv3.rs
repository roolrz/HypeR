//! Minimal GICv3 distributor and Redistributor MMIO model for Linux boot.

use hyper::drivers::interrupt::vgic::{
    InterruptGroup, InterruptTrigger, VirtualCpuId, VirtualInterruptId,
};

use super::VmInterruptController;

pub const DISTRIBUTOR_BASE: u64 = 0x0800_0000;
pub const DISTRIBUTOR_SIZE: u64 = 0x0001_0000;
pub const REDISTRIBUTOR_BASE: u64 = 0x080a_0000;
pub const REDISTRIBUTOR_SIZE: u64 = 0x0002_0000;

const GICR_SGI_BASE: u64 = 0x0001_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAccess,
    Model(hyper::drivers::interrupt::vgic::Error),
}

impl From<hyper::drivers::interrupt::vgic::Error> for Error {
    fn from(error: hyper::drivers::interrupt::vgic::Error) -> Self {
        Self::Model(error)
    }
}

pub fn handles(address: u64) -> bool {
    in_range(address, DISTRIBUTOR_BASE, DISTRIBUTOR_SIZE)
        || in_range(address, REDISTRIBUTOR_BASE, REDISTRIBUTOR_SIZE)
}

pub fn read(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    address: u64,
    size: usize,
) -> Result<u64, Error> {
    if in_range(address, DISTRIBUTOR_BASE, DISTRIBUTOR_SIZE) {
        return read_distributor(interrupts, address - DISTRIBUTOR_BASE, size);
    }
    if in_range(address, REDISTRIBUTOR_BASE, REDISTRIBUTOR_SIZE) {
        return read_redistributor(interrupts, vcpu, address - REDISTRIBUTOR_BASE, size);
    }
    Err(Error::InvalidAccess)
}

pub fn write(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    address: u64,
    size: usize,
    value: u64,
) -> Result<(), Error> {
    if in_range(address, DISTRIBUTOR_BASE, DISTRIBUTOR_SIZE) {
        return write_distributor(interrupts, address - DISTRIBUTOR_BASE, size, value);
    }
    if in_range(address, REDISTRIBUTOR_BASE, REDISTRIBUTOR_SIZE) {
        return write_redistributor(interrupts, vcpu, address - REDISTRIBUTOR_BASE, size, value);
    }
    Err(Error::InvalidAccess)
}

fn read_distributor(
    interrupts: &VmInterruptController,
    offset: u64,
    size: usize,
) -> Result<u64, Error> {
    match (offset, size) {
        (0x0000, 4) => Ok(u64::from(interrupts.distributor_control())),
        (0x0004, 4) => Ok(1 | (15 << 19)),
        (0x0008, 4) => Ok(0x43b),
        (0x000c, 4) => Ok(0),
        (0xffe8, 4) => Ok(0x30),
        _ => read_interrupt_register(interrupts, VirtualCpuId::new(0), offset, size, 32, 64),
    }
}

fn write_distributor(
    interrupts: &VmInterruptController,
    offset: u64,
    size: usize,
    value: u64,
) -> Result<(), Error> {
    if size == 4 {
        match offset {
            0 => interrupts.set_distributor_control(value as u32),
            // GICD_STATUSR is optional. Model it as RAZ/WI with no reported
            // distributor errors.
            0x000c => {}
            _ => {
                return write_interrupt_register(
                    interrupts,
                    VirtualCpuId::new(0),
                    offset,
                    size,
                    value,
                    32,
                    64,
                );
            }
        }
        return Ok(());
    }
    write_interrupt_register(
        interrupts,
        VirtualCpuId::new(0),
        offset,
        size,
        value,
        32,
        64,
    )
}

fn read_redistributor(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    size: usize,
) -> Result<u64, Error> {
    match (offset, size) {
        (0x0000, 4) => Ok(0),
        (0x0004, 4) => Ok(0x43b),
        (0x0008, 8) => Ok(1 << 4),
        (0x0014, 4) => Ok(0),
        (0xffe8, 4) => Ok(0x30),
        _ if offset >= GICR_SGI_BASE => {
            read_interrupt_register(interrupts, vcpu, offset - GICR_SGI_BASE, size, 0, 32)
        }
        _ => Err(Error::InvalidAccess),
    }
}

fn write_redistributor(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    size: usize,
    value: u64,
) -> Result<(), Error> {
    if offset == 0x0014 && size == 4 {
        return Ok(());
    }
    if offset >= GICR_SGI_BASE {
        return write_interrupt_register(
            interrupts,
            vcpu,
            offset - GICR_SGI_BASE,
            size,
            value,
            0,
            32,
        );
    }
    Err(Error::InvalidAccess)
}

fn read_interrupt_register(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    size: usize,
    first: u32,
    end: u32,
) -> Result<u64, Error> {
    if (0x0080..0x0100).contains(&offset) && size == 4 {
        return bitmap(interrupts, vcpu, offset, 0x80, (first, end), |snapshot| {
            snapshot.group == InterruptGroup::Group1
        });
    }
    if (0x0100..0x0180).contains(&offset) && size == 4 {
        return bitmap(interrupts, vcpu, offset, 0x100, (first, end), |snapshot| {
            snapshot.enabled
        });
    }
    if (0x0180..0x0200).contains(&offset) && size == 4 {
        return bitmap(interrupts, vcpu, offset, 0x180, (first, end), |snapshot| {
            snapshot.enabled
        });
    }
    if (0x0200..0x0280).contains(&offset) && size == 4 {
        return bitmap(interrupts, vcpu, offset, 0x200, (first, end), |snapshot| {
            snapshot.pending
        });
    }
    if (0x0280..0x0300).contains(&offset) && size == 4 {
        return bitmap(interrupts, vcpu, offset, 0x280, (first, end), |snapshot| {
            snapshot.pending
        });
    }
    if (0x0400..0x0800).contains(&offset) && matches!(size, 1 | 2 | 4 | 8) {
        let mut value = 0;
        for byte in 0..size {
            let id = ((offset - 0x400) as u32 + byte as u32).max(first);
            if id >= end {
                continue;
            }
            let snapshot = snapshot(interrupts, vcpu, id)?;
            value |= u64::from(snapshot.priority) << (byte * 8);
        }
        return Ok(value);
    }
    if (0x0c00..0x0d00).contains(&offset) && size == 4 {
        let register = ((offset - 0xc00) / 4) as u32;
        let mut value = 0;
        for field in 0..16u32 {
            let id = register * 16 + field;
            if id < first || id >= end {
                continue;
            }
            if snapshot(interrupts, vcpu, id)?.trigger == InterruptTrigger::Edge {
                value |= 2 << (field * 2);
            }
        }
        return Ok(value);
    }
    if (0x6100..0x8000).contains(&offset) && size == 8 {
        return Ok(0);
    }
    Err(Error::InvalidAccess)
}

fn write_interrupt_register(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    size: usize,
    value: u64,
    first: u32,
    end: u32,
) -> Result<(), Error> {
    let range = (first, end);
    if let Some(result) = write_bitmap_register(interrupts, vcpu, offset, size, value, range) {
        return result;
    }
    if (0x0300..0x0400).contains(&offset) && size == 4 {
        return Ok(());
    }
    if (0x0400..0x0800).contains(&offset) && matches!(size, 1 | 2 | 4 | 8) {
        return write_priority_register(interrupts, vcpu, offset, size, value, range);
    }
    if (0x0c00..0x0d00).contains(&offset) && size == 4 {
        return write_configuration_register(interrupts, vcpu, offset, value, range);
    }
    if (0x6100..0x8000).contains(&offset) && size == 8 {
        return write_route_register(interrupts, offset, end);
    }
    Err(Error::InvalidAccess)
}

fn write_bitmap_register(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    size: usize,
    value: u64,
    range: (u32, u32),
) -> Option<Result<(), Error>> {
    if size != 4 {
        return None;
    }
    if (0x0080..0x0100).contains(&offset) {
        return Some(apply_bitmap(
            interrupts,
            vcpu,
            offset,
            0x80,
            value,
            range,
            |controller, id, cpu, set| {
                let group = if set {
                    InterruptGroup::Group1
                } else {
                    InterruptGroup::Group0
                };
                controller.set_group(id, cpu, group)
            },
        ));
    }
    if (0x0100..0x0180).contains(&offset) {
        return Some(apply_bitmap(
            interrupts,
            vcpu,
            offset,
            0x100,
            value,
            range,
            |controller, id, cpu, _| controller.set_enabled(id, cpu, true),
        ));
    }
    if (0x0180..0x0200).contains(&offset) {
        return Some(apply_bitmap(
            interrupts,
            vcpu,
            offset,
            0x180,
            value,
            range,
            |controller, id, cpu, _| controller.set_enabled(id, cpu, false),
        ));
    }
    if (0x0200..0x0280).contains(&offset) {
        return Some(apply_bitmap(
            interrupts,
            vcpu,
            offset,
            0x200,
            value,
            range,
            |controller, id, cpu, _| controller.inject(id, cpu),
        ));
    }
    if (0x0280..0x0300).contains(&offset) {
        return Some(apply_bitmap(
            interrupts,
            vcpu,
            offset,
            0x280,
            value,
            range,
            |controller, id, cpu, _| controller.clear_pending(id, cpu),
        ));
    }
    None
}

fn write_priority_register(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    size: usize,
    value: u64,
    range: (u32, u32),
) -> Result<(), Error> {
    interrupts.with(|controller| {
        for byte in 0..size {
            let id = (offset - 0x400) as u32 + byte as u32;
            if id < range.0 || id >= range.1 {
                continue;
            }
            let interrupt = VirtualInterruptId::new(id).ok_or(Error::InvalidAccess)?;
            controller.set_priority(
                interrupt,
                target_for(id, vcpu),
                (value >> (byte * 8)) as u8,
            )?;
        }
        Ok(())
    })
}

fn write_configuration_register(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    value: u64,
    range: (u32, u32),
) -> Result<(), Error> {
    let register = ((offset - 0xc00) / 4) as u32;
    interrupts.with(|controller| {
        for field in 0..16u32 {
            let id = register * 16 + field;
            if id < range.0 || id >= range.1 || id < 16 {
                continue;
            }
            let interrupt = VirtualInterruptId::new(id).ok_or(Error::InvalidAccess)?;
            let trigger = if value & (2 << (field * 2)) != 0 {
                InterruptTrigger::Edge
            } else {
                InterruptTrigger::Level
            };
            controller.set_trigger(interrupt, target_for(id, vcpu), trigger)?;
        }
        Ok(())
    })
}

fn write_route_register(
    interrupts: &VmInterruptController,
    offset: u64,
    end: u32,
) -> Result<(), Error> {
    let id = 32 + ((offset - 0x6100) / 8) as u32;
    if id >= end {
        return Err(Error::InvalidAccess);
    }
    interrupts.with(|controller| {
        controller.route(
            VirtualInterruptId::new(id).ok_or(Error::InvalidAccess)?,
            VirtualCpuId::new(0),
        )?;
        Ok(())
    })
}

fn bitmap(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    base: u64,
    range: (u32, u32),
    predicate: impl Fn(hyper::drivers::interrupt::vgic::InterruptSnapshot) -> bool,
) -> Result<u64, Error> {
    let register = ((offset - base) / 4) as u32;
    let mut value = 0;
    for bit in 0..32u32 {
        let id = register * 32 + bit;
        if id >= range.0 && id < range.1 && predicate(snapshot(interrupts, vcpu, id)?) {
            value |= 1 << bit;
        }
    }
    Ok(value)
}

fn apply_bitmap(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    offset: u64,
    base: u64,
    value: u64,
    range: (u32, u32),
    operation: impl Fn(
        &mut hyper::drivers::interrupt::vgic::VirtualInterruptController,
        VirtualInterruptId,
        VirtualCpuId,
        bool,
    ) -> Result<(), hyper::drivers::interrupt::vgic::Error>,
) -> Result<(), Error> {
    let register = ((offset - base) / 4) as u32;
    interrupts.with(|controller| {
        for bit in 0..32u32 {
            if value & (1 << bit) == 0 {
                continue;
            }
            let id = register * 32 + bit;
            if id < range.0 || id >= range.1 {
                continue;
            }
            let interrupt = VirtualInterruptId::new(id).ok_or(Error::InvalidAccess)?;
            operation(controller, interrupt, target_for(id, vcpu), true)?;
        }
        Ok(())
    })
}

fn snapshot(
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    id: u32,
) -> Result<hyper::drivers::interrupt::vgic::InterruptSnapshot, Error> {
    let interrupt = VirtualInterruptId::new(id).ok_or(Error::InvalidAccess)?;
    interrupts
        .with(|controller| controller.snapshot(interrupt, target_for(id, vcpu)))
        .map_err(Into::into)
}

fn target_for(id: u32, private_cpu: VirtualCpuId) -> VirtualCpuId {
    if id < 32 {
        private_cpu
    } else {
        VirtualCpuId::new(0)
    }
}

const fn in_range(address: u64, start: u64, size: u64) -> bool {
    address >= start && address < start + size
}
