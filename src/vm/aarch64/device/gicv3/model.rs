// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Reusable guest `GICv3` register state and interrupt-model effects.

use crate::vm::arm::gic::{
    GicInterruptId, InterruptGroup, InterruptSnapshot, InterruptTrigger, RuntimeError, VirtualGic,
};
use crate::vm::interrupt::VirtualCpuId;

use super::decode::ModelRegisterKind;
use super::{BitmapRegister, ModelRegister, ServiceRegister};

const DISTRIBUTOR_CONTROL_MASK: u32 = (1 << 4) | (1 << 1) | 1;

/// Service-register state that must share serialization with interrupt state.
pub struct RegisterState {
    distributor_control: u32,
}

impl RegisterState {
    pub const fn new() -> Self {
        Self {
            distributor_control: 0,
        }
    }

    pub const fn read(&self, register: ServiceRegister) -> u64 {
        match register {
            ServiceRegister::DistributorControl => self.distributor_control as u64,
            ServiceRegister::DistributorType => 1 | (15 << 19),
            // GICD_TYPER2 is optional and explicitly unimplemented.
            ServiceRegister::DistributorType2 => 0,
            ServiceRegister::DistributorImplementer | ServiceRegister::RedistributorImplementer => {
                0x43b
            }
            ServiceRegister::DistributorStatus | ServiceRegister::RedistributorStatus => 0,
            ServiceRegister::RedistributorControl | ServiceRegister::RedistributorWake => 0,
            ServiceRegister::RedistributorType => 1 << 4,
            ServiceRegister::PeripheralId2 => 0x30,
        }
    }

    pub fn write(&mut self, register: ServiceRegister, value: u64) {
        if register == ServiceRegister::DistributorControl {
            self.distributor_control = value as u32 & DISTRIBUTOR_CONTROL_MASK;
        }
        // Identification, status, and current Redistributor service registers
        // are read-only or explicitly WI in this model.
    }
}

impl Default for RegisterState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    InvalidDecodedSpan,
    Interrupt(RuntimeError),
    UnsupportedRouteValue(u64),
}

impl From<RuntimeError> for ModelError {
    fn from(error: RuntimeError) -> Self {
        Self::Interrupt(error)
    }
}

pub fn read_model_register(
    controller: &VirtualGic,
    vcpu: VirtualCpuId,
    register: ModelRegister,
) -> Result<u64, ModelError> {
    match register.kind {
        ModelRegisterKind::Bitmap {
            register,
            first_interrupt,
        } => read_bitmap(controller, vcpu, register, first_interrupt),
        ModelRegisterKind::Priority {
            first_interrupt,
            count,
        } => read_priority(controller, vcpu, first_interrupt, count),
        ModelRegisterKind::Configuration { first_interrupt } => {
            read_configuration(controller, vcpu, first_interrupt)
        }
        ModelRegisterKind::Route(_) => Ok(0),
    }
}

pub fn write_model_register(
    controller: &mut VirtualGic,
    vcpu: VirtualCpuId,
    register: ModelRegister,
    value: u64,
) -> Result<(), ModelError> {
    match register.kind {
        ModelRegisterKind::Bitmap {
            register,
            first_interrupt,
        } => write_bitmap(controller, vcpu, register, first_interrupt, value),
        ModelRegisterKind::Priority {
            first_interrupt,
            count,
        } => write_priority(controller, vcpu, first_interrupt, count, value),
        ModelRegisterKind::Configuration { first_interrupt } => {
            write_configuration(controller, vcpu, first_interrupt, value)
        }
        ModelRegisterKind::Route(route) => {
            if value != 0 {
                return Err(ModelError::UnsupportedRouteValue(value));
            }
            controller.route(interrupt(route.interrupt())?, VirtualCpuId::new(0))?;
            Ok(())
        }
    }
}

fn read_bitmap(
    controller: &VirtualGic,
    vcpu: VirtualCpuId,
    register: BitmapRegister,
    first_interrupt: u32,
) -> Result<u64, ModelError> {
    // Active state can reside in hardware list registers while a vCPU runs.
    // A correct ISACTIVER/ICACTIVER implementation must synchronize that
    // hardware state before mutation or readback. Model both exact instances
    // as RAZ/WI until that transaction exists; do not expose stale model state.
    if matches!(
        register,
        BitmapRegister::SetActive | BitmapRegister::ClearActive
    ) {
        return Ok(0);
    }
    let mut value = 0;
    for bit in 0..32u32 {
        let snapshot = snapshot(controller, vcpu, lane(first_interrupt, bit)?)?;
        let set = match register {
            BitmapRegister::Group => snapshot.group == InterruptGroup::Group1,
            BitmapRegister::SetEnable | BitmapRegister::ClearEnable => snapshot.enabled,
            BitmapRegister::SetPending | BitmapRegister::ClearPending => snapshot.pending,
            BitmapRegister::SetActive | BitmapRegister::ClearActive => false,
        };
        if set {
            value |= 1 << bit;
        }
    }
    Ok(value)
}

fn write_bitmap(
    controller: &mut VirtualGic,
    vcpu: VirtualCpuId,
    register: BitmapRegister,
    first_interrupt: u32,
    value: u64,
) -> Result<(), ModelError> {
    if matches!(
        register,
        BitmapRegister::SetActive | BitmapRegister::ClearActive
    ) {
        return Ok(());
    }

    // Validate the complete mutation set before changing the first lane. The
    // controller is exclusively borrowed, so these entries cannot disappear
    // between this pass and the infallible-by-construction update pass.
    for bit in 0..32u32 {
        if register == BitmapRegister::Group || value & (1 << bit) != 0 {
            let _ = snapshot(controller, vcpu, lane(first_interrupt, bit)?)?;
        }
    }
    for bit in 0..32u32 {
        let set = value & (1 << bit) != 0;
        if register != BitmapRegister::Group && !set {
            continue;
        }
        let id = lane(first_interrupt, bit)?;
        let interrupt = interrupt(id)?;
        let target = target_for(id, vcpu);
        match register {
            BitmapRegister::Group => controller.set_group(
                interrupt,
                target,
                if set {
                    InterruptGroup::Group1
                } else {
                    InterruptGroup::Group0
                },
            )?,
            BitmapRegister::SetEnable => controller.set_enabled(interrupt, target, true)?,
            BitmapRegister::ClearEnable => controller.set_enabled(interrupt, target, false)?,
            BitmapRegister::SetPending => controller.inject(interrupt, target)?,
            BitmapRegister::ClearPending => controller.clear_pending(interrupt, target)?,
            BitmapRegister::SetActive | BitmapRegister::ClearActive => {}
        }
    }
    Ok(())
}

fn read_priority(
    controller: &VirtualGic,
    vcpu: VirtualCpuId,
    first_interrupt: u32,
    count: u8,
) -> Result<u64, ModelError> {
    let mut value = 0;
    for byte in 0..u32::from(count) {
        let snapshot = snapshot(controller, vcpu, lane(first_interrupt, byte)?)?;
        value |= u64::from(snapshot.priority) << (byte * 8);
    }
    Ok(value)
}

fn write_priority(
    controller: &mut VirtualGic,
    vcpu: VirtualCpuId,
    first_interrupt: u32,
    count: u8,
    value: u64,
) -> Result<(), ModelError> {
    for byte in 0..u32::from(count) {
        let _ = snapshot(controller, vcpu, lane(first_interrupt, byte)?)?;
    }
    for byte in 0..u32::from(count) {
        let id = lane(first_interrupt, byte)?;
        controller.set_priority(
            interrupt(id)?,
            target_for(id, vcpu),
            (value >> (byte * 8)) as u8,
        )?;
    }
    Ok(())
}

fn read_configuration(
    controller: &VirtualGic,
    vcpu: VirtualCpuId,
    first_interrupt: u32,
) -> Result<u64, ModelError> {
    let mut value = 0;
    for field in 0..16u32 {
        let id = lane(first_interrupt, field)?;
        if snapshot(controller, vcpu, id)?.trigger == InterruptTrigger::Edge {
            value |= 2 << (field * 2);
        }
    }
    Ok(value)
}

fn write_configuration(
    controller: &mut VirtualGic,
    vcpu: VirtualCpuId,
    first_interrupt: u32,
    value: u64,
) -> Result<(), ModelError> {
    for field in 0..16u32 {
        let id = lane(first_interrupt, field)?;
        if id >= 16 {
            let _ = snapshot(controller, vcpu, id)?;
        }
    }
    for field in 0..16u32 {
        let id = lane(first_interrupt, field)?;
        // SGI trigger configuration is architecturally fixed and write-ignored.
        if id < 16 {
            continue;
        }
        let trigger = if value & (2 << (field * 2)) != 0 {
            InterruptTrigger::Edge
        } else {
            InterruptTrigger::Level
        };
        controller.set_trigger(interrupt(id)?, target_for(id, vcpu), trigger)?;
    }
    Ok(())
}

fn snapshot(
    controller: &VirtualGic,
    vcpu: VirtualCpuId,
    id: u32,
) -> Result<InterruptSnapshot, ModelError> {
    controller
        .snapshot(interrupt(id)?, target_for(id, vcpu))
        .map_err(Into::into)
}

fn interrupt(id: u32) -> Result<GicInterruptId, ModelError> {
    GicInterruptId::new(id).ok_or(ModelError::InvalidDecodedSpan)
}

fn lane(first_interrupt: u32, offset: u32) -> Result<u32, ModelError> {
    first_interrupt
        .checked_add(offset)
        .ok_or(ModelError::InvalidDecodedSpan)
}

fn target_for(id: u32, private_cpu: VirtualCpuId) -> VirtualCpuId {
    if id < 32 {
        private_cpu
    } else {
        VirtualCpuId::new(0)
    }
}
