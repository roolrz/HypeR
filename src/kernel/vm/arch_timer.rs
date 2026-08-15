//! Running-vCPU bridge for the Arm virtual timer and vGIC.

use hyper::drivers::interrupt::vgic::{Error as VgicError, VirtualCpuId};
use hyper::sync::InterruptSpinLock;

use super::VmInterruptController;
use crate::kernel::task::thread::VcpuExecution;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;

type ActiveLock =
    InterruptSpinLock<[Option<ActiveBinding>; MAX_CPUS], crate::arch::LocalInterruptMask>;

static ACTIVE: ActiveLock = InterruptSpinLock::new([None; MAX_CPUS]);

#[derive(Clone, Copy)]
struct ActiveBinding {
    execution: usize,
    interrupts: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ActiveVcpuMissing,
    CpuAlreadyActive,
    InvalidCpu,
    VgicArchitecture(crate::arch::VgicError),
    VgicModel(VgicError),
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

/// Publishes the pinned objects owned by the local guest run loop.
///
/// # Safety
///
/// Both objects must remain at these addresses and exclusively associated
/// with the calling CPU until [`clear_active`] succeeds. Local IRQs must be
/// masked across publication and guest entry.
pub unsafe fn set_active(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
) -> Result<(), Error> {
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| {
        let slot = slots.get_mut(cpu).ok_or(Error::InvalidCpu)?;
        if slot.is_some() {
            return Err(Error::CpuAlreadyActive);
        }
        *slot = Some(ActiveBinding {
            execution: execution as *mut VcpuExecution as usize,
            interrupts: interrupts as *const VmInterruptController as usize,
        });
        Ok(())
    })
}

pub fn clear_active(execution: &mut VcpuExecution) -> Result<(), Error> {
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| {
        let slot = slots.get_mut(cpu).ok_or(Error::InvalidCpu)?;
        let binding = slot.ok_or(Error::ActiveVcpuMissing)?;
        if binding.execution != execution as *mut VcpuExecution as usize {
            return Err(Error::ActiveVcpuMissing);
        }
        *slot = None;
        Ok(())
    })
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
    let Some(binding) = active_binding()? else {
        return Ok(InterruptOutcome {
            active: false,
            asserted: false,
        });
    };
    // SAFETY: set_active requires both objects to remain pinned and exclusively
    // associated with this CPU. IRQ entry masks local IRQs before this access.
    let (execution, interrupts) = unsafe { binding.objects() };
    let asserted = execution
        .context
        .virtual_timer_interrupt_asserted_hardware();
    refresh_running_vgic(execution, interrupts, asserted)?;
    Ok(InterruptOutcome {
        active: true,
        asserted,
    })
}

pub(super) fn inject_active_for_validation() -> Result<bool, Error> {
    let Some(binding) = active_binding()? else {
        return Ok(false);
    };
    // SAFETY: The boot validation obeys the active-binding contract.
    let (execution, interrupts) = unsafe { binding.objects() };
    refresh_running_vgic(execution, interrupts, true)?;
    Ok(true)
}

/// Reconciles guest EOI progress and reports when the physical PPI can rearm.
pub fn handle_maintenance() -> Result<MaintenanceOutcome, Error> {
    let Some(binding) = active_binding()? else {
        return Ok(MaintenanceOutcome {
            handled: false,
            timer_deasserted: false,
        });
    };
    // SAFETY: See handle_interrupt.
    let (execution, interrupts) = unsafe { binding.objects() };
    let asserted = execution
        .context
        .virtual_timer_interrupt_asserted_hardware();
    refresh_running_vgic(execution, interrupts, asserted)?;
    Ok(MaintenanceOutcome {
        handled: true,
        timer_deasserted: !asserted,
    })
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

fn active_binding() -> Result<Option<ActiveBinding>, Error> {
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| slots.get(cpu).copied().ok_or(Error::InvalidCpu))
}

fn current_cpu() -> Result<usize, Error> {
    let cpu = crate::arch::current_cpu_index();
    if cpu < MAX_CPUS {
        Ok(cpu)
    } else {
        Err(Error::InvalidCpu)
    }
}

impl ActiveBinding {
    unsafe fn objects<'a>(self) -> (&'a mut VcpuExecution, &'a VmInterruptController) {
        // SAFETY: Enforced by set_active's contract and clear_active pairing.
        unsafe {
            (
                &mut *(self.execution as *mut VcpuExecution),
                &*(self.interrupts as *const VmInterruptController),
            )
        }
    }
}
