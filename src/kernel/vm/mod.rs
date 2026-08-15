//! Virtual-machine execution policy.

mod active_vcpu;
mod arch_timer;
mod gicv3;
mod interrupt;
mod linux;
mod vcpu;

pub(crate) use arch_timer::{handle_interrupt as handle_arch_timer_interrupt, handle_maintenance};
pub use hyper::vm::bundle::{Error as VmBundleError, VmBundle};
pub use interrupt::{Error as VmInterruptError, VmInterruptController};
pub use linux::Error as LinuxBootError;
pub use vcpu::VcpuInterruptError;

pub fn select_default(ramdisk: &[u8]) -> Result<VmBundle<'_>, VmBundleError> {
    hyper::vm::bundle::select_default(ramdisk)
}

pub fn boot_linux(guest: VmBundle<'_>) -> Result<core::convert::Infallible, LinuxBootError> {
    linux::boot(guest)
}

pub(crate) fn handle_guest_sync(frame: &mut crate::arch::GuestSyncFrame<'_>) -> bool {
    match active_vcpu::with(|execution, interrupts| {
        let action =
            crate::arch::handle_guest_sync(&mut execution.context, execution.vcpu_id, frame);
        match action {
            crate::arch::GuestSyncAction::SoftwareInterrupt(request) => {
                return match vcpu::deliver_software_interrupt(execution, interrupts, request) {
                    Ok(()) => crate::arch::GuestSyncAction::Resume,
                    Err(error) => {
                        crate::pr_err!("HypeR: failed to deliver guest SGI: {error:?}");
                        crate::arch::GuestSyncAction::Unhandled
                    }
                };
            }
            crate::arch::GuestSyncAction::Unhandled => {}
            _ => return action,
        }
        let Some(access) = frame.data_access() else {
            return action;
        };
        if !gicv3::handles(access.address) {
            return action;
        }
        let vcpu = VirtualCpuId::new(execution.vcpu_id);
        let result = if access.write {
            gicv3::write(interrupts, vcpu, access.address, access.size, access.value).map(|()| None)
        } else {
            gicv3::read(interrupts, vcpu, access.address, access.size).map(Some)
        };
        match result {
            Ok(value) => {
                frame.complete_data_access(access, value);
                crate::arch::GuestSyncAction::Resume
            }
            Err(error) => {
                crate::pr_err!(
                    "HypeR: unsupported guest GIC access at {:#x}, FAR {:#x}: {error:?}",
                    access.address,
                    frame.fault_address()
                );
                action
            }
        }
    }) {
        Ok(Some(crate::arch::GuestSyncAction::Resume | crate::arch::GuestSyncAction::Injected)) => {
            true
        }
        Ok(Some(
            crate::arch::GuestSyncAction::SoftwareInterrupt(_)
            | crate::arch::GuestSyncAction::Unhandled,
        ))
        | Ok(None)
        | Err(_) => false,
    }
}

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
