//! Kernel ownership of the GIC virtualization maintenance interrupt.

use hyper::hal::interrupt::{InterruptId, InterruptTrigger};
use hyper::platform::{PlatformInterrupt, PlatformInterruptTrigger};
use hyper::sync::InterruptSpinLock;

use super::interrupt::{HandlerResult, IrqDomainId, Registration, VirtualInterrupt};

type VgicLock = InterruptSpinLock<Option<State>, crate::arch::LocalInterruptMask>;

static VGIC: VgicLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Architecture(crate::arch::VgicError),
    Interrupt(super::interrupt::Error),
    MissingMaintenanceInterrupt,
}

impl From<crate::arch::VgicError> for Error {
    fn from(error: crate::arch::VgicError) -> Self {
        Self::Architecture(error)
    }
}

impl From<super::interrupt::Error> for Error {
    fn from(error: super::interrupt::Error) -> Self {
        Self::Interrupt(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub list_registers: u8,
    pub priority_bits: u8,
    pub preemption_bits: u8,
    pub interrupt_id_bits: u8,
    pub maintenance_interrupt: VirtualInterrupt,
}

struct State {
    _registration: Registration,
}

pub fn initialize(
    domain: IrqDomainId,
    maintenance: Option<PlatformInterrupt>,
) -> Result<Capabilities, Error> {
    if VGIC.with(|slot| slot.is_some()) {
        return Err(Error::AlreadyInitialized);
    }
    let maintenance = maintenance.ok_or(Error::MissingMaintenanceInterrupt)?;
    let trigger = match maintenance.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let interrupt = super::interrupt::map(
        domain,
        InterruptId::new(maintenance.interrupt),
        0x20,
        trigger,
    )?;
    let registration = super::interrupt::register_shared(interrupt, 0, maintenance_handler)?;
    let architecture = crate::arch::validate_vgic()?;
    let capabilities = Capabilities {
        list_registers: architecture.list_registers,
        priority_bits: architecture.priority_bits,
        preemption_bits: architecture.preemption_bits,
        interrupt_id_bits: architecture.interrupt_id_bits,
        maintenance_interrupt: interrupt,
    };
    VGIC.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(State {
            _registration: registration,
        });
        Ok(capabilities)
    })
}

fn maintenance_handler(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    let state = crate::arch::vgic_maintenance_state();
    if state.status == 0 {
        return HandlerResult::NotHandled;
    }
    match crate::kernel::vm::handle_maintenance() {
        Ok(outcome) if outcome.handled && outcome.timer_deasserted => {
            let Some(timer) = super::timer::guest_virtual_host_interrupt() else {
                crate::arch::disable_vgic();
                crate::pr_err!("HypeR: vGIC maintenance cannot rearm the virtual timer PPI");
                return HandlerResult::Handled;
            };
            return HandlerResult::HandledAndUnmaskLocal(timer);
        }
        Ok(outcome) if outcome.handled => return HandlerResult::Handled,
        Ok(_) => {}
        Err(error) => {
            crate::arch::disable_vgic();
            crate::pr_err!("HypeR: vGIC maintenance reconciliation failed: {error:?}");
            return HandlerResult::Handled;
        }
    }
    crate::arch::disable_vgic();
    crate::pr_err!(
        "HypeR: vGIC maintenance without active vCPU: MISR {:#x}, EISR {:#x}, ELRSR {:#x}",
        state.status,
        state.eoi_list_registers,
        state.empty_list_registers
    );
    HandlerResult::Handled
}
