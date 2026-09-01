// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-VM `GICv3` interrupt-controller state.

use hyper::sync::InterruptSpinLock;
use hyper::vm::aarch64::device::gicv3::{
    DecodedRegister, ModelError, RegisterState, read_model_register, write_model_register,
};
use hyper::vm::arm::gic::ListEntry;
use hyper::vm::arm::gic::{
    BuildError as VgicBuildError, GicInterruptId, InterruptGroup, InterruptSnapshot,
    InterruptTrigger, RuntimeError as VgicError, VirtualGic, VirtualGicBuilder,
};
use hyper::vm::exit::MmioOperation;
use hyper::vm::interrupt::VirtualCpuId;

type ControllerLock = InterruptSpinLock<ControllerState, super::LocalInterruptMask>;

const TIMER_PRIORITY: u8 = 0x80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Build(VgicBuildError),
    InvalidInterrupt,
    MissingCapabilities,
    Vgic(VgicError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessError {
    Controller(VgicError),
    Model(ModelError),
}

impl From<VgicBuildError> for Error {
    fn from(error: VgicBuildError) -> Self {
        Self::Build(error)
    }
}

impl From<VgicError> for Error {
    fn from(error: VgicError) -> Self {
        Self::Vgic(error)
    }
}

pub struct VmInterruptController {
    state: ControllerLock,
    timer_interrupt: GicInterruptId,
    vcpu_count: u32,
}

struct ControllerState {
    controller: VirtualGic,
    registers: RegisterState,
}

impl VmInterruptController {
    pub fn new(
        vcpu_count: u32,
        timer_interrupt: GicInterruptId,
        list_registers: usize,
    ) -> Result<Self, Error> {
        let mut builder = VirtualGicBuilder::new(vcpu_count)?;
        for index in 0..vcpu_count {
            let vcpu = VirtualCpuId::new(index);
            for id in 0..32 {
                let interrupt = GicInterruptId::new(id).ok_or(Error::InvalidInterrupt)?;
                builder.configure(
                    interrupt,
                    vcpu,
                    TIMER_PRIORITY,
                    InterruptGroup::Group1,
                    if id < 16 {
                        InterruptTrigger::Edge
                    } else {
                        InterruptTrigger::Level
                    },
                )?;
            }
        }
        for id in 32..64 {
            builder.configure(
                GicInterruptId::new(id).ok_or(Error::InvalidInterrupt)?,
                VirtualCpuId::new(0),
                TIMER_PRIORITY,
                InterruptGroup::Group1,
                InterruptTrigger::Level,
            )?;
        }
        let mut controller = builder.finish(list_registers)?;
        for index in 0..vcpu_count {
            let vcpu = VirtualCpuId::new(index);
            controller.set_maintenance_on_eoi(timer_interrupt, vcpu, true)?;
            controller.set_enabled(timer_interrupt, vcpu, true)?;
        }
        Ok(Self {
            state: InterruptSpinLock::new(ControllerState {
                controller,
                registers: RegisterState::new(),
            }),
            timer_interrupt,
            vcpu_count,
        })
    }

    pub const fn timer_interrupt(&self) -> GicInterruptId {
        self.timer_interrupt
    }

    pub const fn vcpu_count(&self) -> u32 {
        self.vcpu_count
    }

    pub(super) fn with<R>(&self, operation: impl FnOnce(&mut VirtualGic) -> R) -> R {
        self.state.with(|state| operation(&mut state.controller))
    }

    pub fn timer_snapshot(&self, vcpu: VirtualCpuId) -> Result<InterruptSnapshot, VgicError> {
        self.state
            .with(|state| state.controller.snapshot(self.timer_interrupt, vcpu))
    }

    pub fn may_wake_wfi(&self, vcpu: VirtualCpuId) -> Result<bool, VgicError> {
        self.state.with(|state| state.controller.may_wake_wfi(vcpu))
    }

    pub(super) fn access_saved_bank(
        &self,
        vcpu: VirtualCpuId,
        slots: &mut [Option<ListEntry>],
        register: DecodedRegister,
        operation: MmioOperation,
    ) -> Result<Option<u64>, AccessError> {
        self.state.with(|state| {
            state
                .controller
                .synchronize(vcpu, slots)
                .map_err(AccessError::Controller)?;
            let value = match (register, operation) {
                (DecodedRegister::Service(register), MmioOperation::Read) => {
                    Some(state.registers.read(register))
                }
                (DecodedRegister::Service(register), MmioOperation::Write(value)) => {
                    state.registers.write(register, value);
                    None
                }
                (DecodedRegister::Model(register), MmioOperation::Read) => Some(
                    read_model_register(&state.controller, vcpu, register)
                        .map_err(AccessError::Model)?,
                ),
                (DecodedRegister::Model(register), MmioOperation::Write(value)) => {
                    write_model_register(&mut state.controller, vcpu, register, value)
                        .map_err(AccessError::Model)?;
                    None
                }
                (DecodedRegister::Reserved, MmioOperation::Read) => Some(0),
                (DecodedRegister::Reserved, MmioOperation::Write(_)) => None,
            };
            let _ = state
                .controller
                .refill(vcpu, slots)
                .map_err(AccessError::Controller)?;
            Ok(value)
        })
    }
}
