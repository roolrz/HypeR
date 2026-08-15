//! Running-vCPU bridge for the Arm virtual timer and vGIC.

use hyper::drivers::interrupt::vgic::{Error as VgicError, VirtualCpuId};

use super::{VmInterruptController, active_vcpu};
use crate::kernel::task::thread::VcpuExecution;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Active(active_vcpu::Error),
    VgicArchitecture(crate::arch::VgicError),
    VgicModel(VgicError),
}

impl From<active_vcpu::Error> for Error {
    fn from(error: active_vcpu::Error) -> Self {
        Self::Active(error)
    }
}

impl From<crate::arch::VgicError> for Error {
    fn from(error: crate::arch::VgicError) -> Self {
        Self::VgicArchitecture(error)
    }
}

impl From<VgicError> for Error {
    fn from(error: VgicError) -> Self {
        Self::VgicModel(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceOutcome {
    pub handled: bool,
    pub timer_deasserted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptOutcome {
    pub active: bool,
    pub asserted: bool,
}

pub fn reconcile_saved(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    physical_count: u64,
) -> Result<(), Error> {
    let asserted = execution
        .context
        .virtual_timer_interrupt_asserted_at(physical_count);
    update_model(execution, interrupts, asserted)
}

/// Converts a live CNTV PPI into a level interrupt in the active vGIC.
pub fn handle_interrupt() -> Result<InterruptOutcome, Error> {
    let Some(result) = active_vcpu::with(|execution, interrupts| {
        let asserted = execution
            .context
            .virtual_timer_interrupt_asserted_hardware();
        refresh_running_vgic(execution, interrupts, asserted)?;
        Ok::<InterruptOutcome, Error>(InterruptOutcome {
            active: true,
            asserted,
        })
    })?
    else {
        return Ok(InterruptOutcome {
            active: false,
            asserted: false,
        });
    };
    result
}

pub(super) fn inject_active_for_validation() -> Result<bool, Error> {
    let Some(result) = active_vcpu::with(|execution, interrupts| {
        refresh_running_vgic(execution, interrupts, true)
    })?
    else {
        return Ok(false);
    };
    result?;
    Ok(true)
}

/// Reconciles guest EOI progress and reports when the physical PPI can rearm.
pub fn handle_maintenance() -> Result<MaintenanceOutcome, Error> {
    let Some(result) = active_vcpu::with(|execution, interrupts| {
        let asserted = execution
            .context
            .virtual_timer_interrupt_asserted_hardware();
        refresh_running_vgic(execution, interrupts, asserted)?;
        Ok::<MaintenanceOutcome, Error>(MaintenanceOutcome {
            handled: true,
            timer_deasserted: !asserted,
        })
    })?
    else {
        return Ok(MaintenanceOutcome {
            handled: false,
            timer_deasserted: false,
        });
    };
    result
}

fn refresh_running_vgic(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    timer_asserted: bool,
) -> Result<(), Error> {
    // SAFETY: The active binding identifies the vCPU whose hardware interface
    // is loaded on this CPU, and IRQ entry has masked local interrupts.
    unsafe { execution.context.deactivate_vgic()? };
    let result = interrupts.with(|controller| {
        let vcpu = VirtualCpuId::new(execution.vcpu_id);
        controller.synchronize(vcpu, execution.context.vgic.slots())?;
        set_timer_level(controller, interrupts, vcpu, timer_asserted)?;
        let _ = controller.refill(vcpu, execution.context.vgic.slots_mut())?;
        Ok::<(), VgicError>(())
    });
    if let Err(error) = result {
        crate::arch::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The complete model has been reconciled into this vCPU's slots.
    unsafe { execution.context.activate_vgic()? };
    Ok(())
}

fn update_model(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    asserted: bool,
) -> Result<(), Error> {
    interrupts
        .with(|controller| {
            set_timer_level(
                controller,
                interrupts,
                VirtualCpuId::new(execution.vcpu_id),
                asserted,
            )
        })
        .map_err(Into::into)
}

fn set_timer_level(
    controller: &mut hyper::drivers::interrupt::vgic::VirtualInterruptController,
    interrupts: &VmInterruptController,
    vcpu: VirtualCpuId,
    asserted: bool,
) -> Result<(), VgicError> {
    if asserted {
        controller.inject(interrupts.timer_interrupt(), vcpu)
    } else {
        controller.clear_pending(interrupts.timer_interrupt(), vcpu)
    }
}
