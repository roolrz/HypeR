//! Per-VM interrupt-controller ownership and timer PPI policy.

use hyper::drivers::interrupt::vgic::{
    Error as VgicError, InterruptGroup, InterruptSnapshot, InterruptTrigger, VirtualCpuId,
    VirtualInterruptController, VirtualInterruptId,
};
use hyper::hal::interrupt::InterruptId;
use hyper::sync::InterruptSpinLock;

type ControllerLock =
    InterruptSpinLock<VirtualInterruptController, crate::arch::LocalInterruptMask>;

const TIMER_PRIORITY: u8 = 0x80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInterrupt,
    Vgic(VgicError),
}

impl From<VgicError> for Error {
    fn from(error: VgicError) -> Self {
        Self::Vgic(error)
    }
}

/// Serialized virtual interrupt state shared by all vCPUs in one VM.
pub struct VmInterruptController {
    controller: ControllerLock,
    timer_interrupt: VirtualInterruptId,
}

impl VmInterruptController {
    pub fn new(vcpu_count: u32, timer_interrupt: InterruptId) -> Result<Self, Error> {
        let timer_interrupt =
            VirtualInterruptId::new(timer_interrupt.get()).ok_or(Error::InvalidInterrupt)?;
        let mut controller = VirtualInterruptController::new(vcpu_count)?;
        for index in 0..vcpu_count {
            let vcpu = VirtualCpuId::new(index);
            controller.configure(
                timer_interrupt,
                vcpu,
                TIMER_PRIORITY,
                InterruptGroup::Group1,
                InterruptTrigger::Level,
            )?;
            controller.set_maintenance_on_eoi(timer_interrupt, vcpu, true)?;
            // GIC register emulation will eventually transfer ownership of
            // this enable bit to the guest Redistributor model.
            controller.set_enabled(timer_interrupt, vcpu, true)?;
        }
        Ok(Self {
            controller: InterruptSpinLock::new(controller),
            timer_interrupt,
        })
    }

    pub const fn timer_interrupt(&self) -> VirtualInterruptId {
        self.timer_interrupt
    }

    pub(crate) fn with<R>(
        &self,
        operation: impl FnOnce(&mut VirtualInterruptController) -> R,
    ) -> R {
        self.controller.with(operation)
    }

    pub(crate) fn timer_snapshot(
        &self,
        vcpu: VirtualCpuId,
    ) -> Result<InterruptSnapshot, VgicError> {
        self.controller
            .with(|controller| controller.snapshot(self.timer_interrupt, vcpu))
    }
}
