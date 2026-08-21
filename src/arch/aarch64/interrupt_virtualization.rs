//! `GICv3` virtualization ownership at the architecture/kernel IRQ boundary.

use hyper::hal::interrupt::{InterruptId, InterruptPriority, InterruptTrigger};
use hyper::platform::{PlatformInterrupt, PlatformInterruptTrigger};
use hyper::sync::InterruptSpinLock;

use crate::kernel::irq::interrupt::Error as InterruptError;
use crate::kernel::irq::interrupt::HandlerResult;
use crate::kernel::irq::interrupt::IrqDomainId;
use crate::kernel::irq::interrupt::Registration;
use crate::kernel::irq::interrupt::VirtualInterrupt;

type VgicLock = InterruptSpinLock<Option<State>, super::LocalInterruptMask>;

static VGIC: VgicLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Architecture(super::VgicError),
    Interrupt(InterruptError),
    MissingMaintenanceInterrupt,
}

impl From<super::VgicError> for Error {
    fn from(error: super::VgicError) -> Self {
        Self::Architecture(error)
    }
}

impl From<InterruptError> for Error {
    fn from(error: InterruptError) -> Self {
        Self::Interrupt(error)
    }
}

struct State {
    // The architecture virtualization service owns the maintenance handler.
    _maintenance_registration: Registration,
}

pub fn initialize(
    domain: IrqDomainId,
    maintenance: Option<PlatformInterrupt>,
) -> Result<(), Error> {
    let maintenance = maintenance.ok_or(Error::MissingMaintenanceInterrupt)?;
    let trigger = match maintenance.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let capabilities = super::validate_vgic()?;
    let interrupt = VGIC.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        let (interrupt, registration) = domain.register_shared_mapping(
            InterruptId::new(maintenance.interrupt),
            InterruptPriority::High,
            trigger,
            0,
            maintenance_handler,
        )?;
        *slot = Some(State {
            _maintenance_registration: registration,
        });
        Ok(interrupt)
    })?;
    crate::println!(
        "HypeR: vGICv3 active with {} LRs, {} priority bits, {} preemption bits, {} INTID bits, maintenance VIRQ {}",
        capabilities.list_registers,
        capabilities.priority_bits,
        capabilities.preemption_bits,
        capabilities.interrupt_id_bits,
        interrupt.get()
    );
    Ok(())
}

fn maintenance_handler(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    let state = super::vgic_maintenance_state();
    if state.status == 0 {
        return HandlerResult::NotHandled;
    }
    match super::vm_timer::handle_maintenance() {
        Ok(outcome) if outcome.handled && outcome.timer_deasserted => {
            let Some(timer) = crate::kernel::irq::timer::guest_virtual_host_interrupt() else {
                super::disable_vgic();
                crate::pr_err!("HypeR: vGIC maintenance cannot rearm the virtual timer PPI");
                return HandlerResult::Handled;
            };
            return HandlerResult::HandledAndUnmaskLocal(timer);
        }
        Ok(outcome) if outcome.handled => return HandlerResult::Handled,
        Ok(_) => {}
        Err(error) => {
            super::disable_vgic();
            crate::pr_err!("HypeR: vGIC maintenance reconciliation failed: {error:?}");
            return HandlerResult::Handled;
        }
    }
    super::disable_vgic();
    crate::pr_err!(
        "HypeR: vGIC maintenance without active vCPU: MISR {:#x}, EISR {:#x}, ELRSR {:#x}",
        state.status,
        state.eoi_list_registers,
        state.empty_list_registers
    );
    HandlerResult::Handled
}
