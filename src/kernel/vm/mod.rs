//! Virtual-machine execution policy.

mod arch_timer;
mod interrupt;
mod vcpu;

pub(crate) use arch_timer::{handle_interrupt as handle_arch_timer_interrupt, handle_maintenance};
pub use interrupt::{Error as VmInterruptError, VmInterruptController};
pub use vcpu::VcpuInterruptError;

use hyper::drivers::interrupt::vgic::VirtualCpuId;
use hyper::hal::interrupt::InterruptId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    ActiveBridge(arch_timer::Error),
    Context(crate::arch::VgicError),
    Interrupts(interrupt::Error),
    Model(hyper::drivers::interrupt::vgic::Error),
    StateMismatch,
    Vcpu(vcpu::VcpuInterruptError),
}

pub fn validate_arch_timer(timer_interrupt: InterruptId) -> Result<(), ValidationError> {
    let interrupts =
        VmInterruptController::new(1, timer_interrupt).map_err(ValidationError::Interrupts)?;
    let mut context = crate::arch::VcpuContext::new(0);
    let _ = context
        .initialize_vgic()
        .map_err(ValidationError::Context)?;
    let now = crate::kernel::time::monotonic_ticks();
    context.set_virtual_count(now, now);
    context.set_virtual_timer_deadline(now.wrapping_add(1_000_000));
    context.set_virtual_timer_enabled(true);
    let mut execution = crate::kernel::task::thread::VcpuExecution {
        virtual_machine: crate::kernel::task::thread::VirtualMachineId(u64::MAX),
        vcpu_id: 0,
        context,
    };
    // SAFETY: Boot validation owns these pinned stack objects, runs with local
    // IRQs masked, and pairs activation with deactivation before returning.
    unsafe {
        execution
            .activate_virtual_hardware(&interrupts)
            .map_err(ValidationError::Vcpu)?;
    }
    if !arch_timer::inject_active_for_validation().map_err(ValidationError::ActiveBridge)? {
        return Err(ValidationError::StateMismatch);
    }
    let snapshot = interrupts
        .timer_snapshot(VirtualCpuId::new(0))
        .map_err(ValidationError::Model)?;
    if !snapshot.pending || !snapshot.listed {
        return Err(ValidationError::StateMismatch);
    }
    // SAFETY: This is the active validation vCPU and local IRQs remain masked.
    unsafe {
        execution
            .deactivate_virtual_hardware(&interrupts)
            .map_err(ValidationError::Vcpu)?;
    }
    Ok(())
}
