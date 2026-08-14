//! IRQ domains, mappings, registration, and dispatch policy.

use alloc::vec::Vec;

use hyper::drivers::interrupt::gicv3::{Error as GicError, GicV3};
use hyper::hal::interrupt::{InterruptController, InterruptId, InterruptTrigger};
use hyper::platform::InterruptControllerInfo;
use hyper::sync::InterruptSpinLock;

type BootInterruptController =
    GicV3<crate::arch::GicCpuInterface, crate::arch::ArchitectureBarrier>;
type InterruptLock = InterruptSpinLock<Option<InterruptState>, crate::arch::LocalInterruptMask>;

const UNHANDLED_QUARANTINE_THRESHOLD: u32 = 8;

static INTERRUPTS: InterruptLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AlreadyInitialized,
    DomainBusy,
    DomainNotFound,
    Gic(GicError),
    HandlerNotFound,
    InterruptAlreadyMapped,
    InterruptNotMapped,
    LocalInterruptLifecycleRequiresCrossCall,
    MappingBusy,
    NotInitialized,
    NumericExhaustion,
}

impl From<GicError> for Error {
    fn from(error: GicError) -> Self {
        Self::Gic(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrqDomainId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualInterrupt(u32);

impl VirtualInterrupt {
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registration {
    id: u64,
    interrupt: VirtualInterrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandlerResult {
    Handled,
    NotHandled,
}

pub type Handler = fn(VirtualInterrupt, usize) -> HandlerResult;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub interrupt_count: u32,
    pub root_domain: IrqDomainId,
}

struct HandlerEntry {
    id: u64,
    context: usize,
    handler: Handler,
}

struct IrqMapping {
    hardware: InterruptId,
    virtual_interrupt: VirtualInterrupt,
    priority: u8,
    trigger: InterruptTrigger,
    handlers: Vec<HandlerEntry>,
    enabled: bool,
    consecutive_unhandled: u32,
}

struct IrqDomain {
    id: IrqDomainId,
    mappings: Vec<IrqMapping>,
}

struct InterruptState {
    controller: BootInterruptController,
    domains: Vec<IrqDomain>,
    next_domain: u32,
    next_virtual_interrupt: u32,
    next_registration: u64,
}

enum DispatchOutcome {
    Spurious,
    Handled,
    Quarantined {
        hardware: InterruptId,
        virtual_interrupt: VirtualInterrupt,
    },
    Unmapped(InterruptId),
}

pub fn initialize(info: InterruptControllerInfo) -> Result<Capabilities, Error> {
    if INTERRUPTS.with(|slot| slot.is_some()) {
        return Err(Error::AlreadyInitialized);
    }
    let InterruptControllerInfo::GicV3(info) = info;
    // SAFETY: The architecture MMIO window maps every DTB-discovered device
    // range with Device attributes and the controller has a single owner.
    let mut controller =
        unsafe { BootInterruptController::bind(info, crate::kernel::mm::memory::mmio_address)? };
    // SAFETY: Boot runs on one CPU with DAIF masked. No other component can
    // access the controller before it is installed in this lock.
    unsafe { controller.initialize(crate::arch::current_gic_affinity())? };
    let interrupt_count = controller.interrupt_count();
    INTERRUPTS.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(InterruptState {
            controller,
            domains: Vec::new(),
            next_domain: 0,
            next_virtual_interrupt: 0,
            next_registration: 0,
        });
        Ok(())
    })?;
    let lifecycle_probe = create_domain()?;
    destroy_domain(lifecycle_probe)?;
    let root_domain = create_domain()?;
    Ok(Capabilities {
        interrupt_count,
        root_domain,
    })
}

/// Initializes the calling secondary CPU's local GIC state and PPIs.
pub fn initialize_local_cpu() -> Result<(), Error> {
    with_state(|state| {
        let InterruptState {
            controller,
            domains,
            ..
        } = state;
        // SAFETY: The shared Distributor is active, this CPU still has IRQs
        // masked, and each Redistributor is private to its matching affinity.
        unsafe { controller.initialize_local(crate::arch::current_gic_affinity())? };
        for mapping in domains.iter().flat_map(|domain| domain.mappings.iter()) {
            if mapping.enabled && mapping.hardware.get() < 32 {
                controller.configure(mapping.hardware, mapping.priority, mapping.trigger)?;
                controller.enable(mapping.hardware)?;
            }
        }
        Ok(())
    })
}

pub fn create_domain() -> Result<IrqDomainId, Error> {
    with_state(|state| {
        reserve_one(&mut state.domains)?;
        let id = IrqDomainId(state.next_domain);
        state.next_domain = state
            .next_domain
            .checked_add(1)
            .ok_or(Error::NumericExhaustion)?;
        state.domains.push(IrqDomain {
            id,
            mappings: Vec::new(),
        });
        Ok(id)
    })
}

/// Destroys an IRQ domain after all of its mappings have been removed.
pub fn destroy_domain(domain: IrqDomainId) -> Result<(), Error> {
    with_state(|state| {
        let index = state
            .domains
            .iter()
            .position(|candidate| candidate.id == domain)
            .ok_or(Error::DomainNotFound)?;
        if !state.domains[index].mappings.is_empty() {
            return Err(Error::DomainBusy);
        }
        state.domains.swap_remove(index);
        Ok(())
    })
}

/// Creates a controller mapping and allocates a kernel-global virtual IRQ.
pub fn map(
    domain: IrqDomainId,
    hardware: InterruptId,
    priority: u8,
    trigger: InterruptTrigger,
) -> Result<VirtualInterrupt, Error> {
    with_state(|state| {
        if state
            .domains
            .iter()
            .flat_map(|domain| domain.mappings.iter())
            .any(|mapping| mapping.hardware == hardware)
        {
            return Err(Error::InterruptAlreadyMapped);
        }
        let domain_index = state
            .domains
            .iter()
            .position(|candidate| candidate.id == domain)
            .ok_or(Error::DomainNotFound)?;
        let next_virtual_interrupt = state
            .next_virtual_interrupt
            .checked_add(1)
            .ok_or(Error::NumericExhaustion)?;
        reserve_one(&mut state.domains[domain_index].mappings)?;
        state.controller.configure(hardware, priority, trigger)?;
        let virtual_interrupt = VirtualInterrupt(state.next_virtual_interrupt);
        state.next_virtual_interrupt = next_virtual_interrupt;
        state.domains[domain_index].mappings.push(IrqMapping {
            hardware,
            virtual_interrupt,
            priority,
            trigger,
            handlers: Vec::new(),
            enabled: false,
            consecutive_unhandled: 0,
        });
        Ok(virtual_interrupt)
    })
}

/// Removes an unused mapping. Mappings with registered handlers stay pinned.
pub fn unmap(interrupt: VirtualInterrupt) -> Result<(), Error> {
    with_state(|state| {
        let (domain_index, mapping_index) = state
            .mapping_position_by_virtual(interrupt)
            .ok_or(Error::InterruptNotMapped)?;
        if !state.domains[domain_index].mappings[mapping_index]
            .handlers
            .is_empty()
        {
            return Err(Error::MappingBusy);
        }
        state.domains[domain_index]
            .mappings
            .swap_remove(mapping_index);
        Ok(())
    })
}

/// Adds one handler to a shared virtual IRQ.
///
/// Handlers run with local IRQs masked and the registry stabilized. They must
/// not mutate IRQ domains or registrations from inside the callback. Every
/// shared handler is invoked so devices sharing a level-triggered line can
/// inspect and clear their own interrupt source. `context` is an opaque value
/// owned by the registering driver and must remain valid until unregister.
pub fn register_shared(
    interrupt: VirtualInterrupt,
    context: usize,
    handler: Handler,
) -> Result<Registration, Error> {
    with_state(|state| {
        let (domain_index, mapping_index) = state
            .mapping_position_by_virtual(interrupt)
            .ok_or(Error::InterruptNotMapped)?;
        let next_registration = state
            .next_registration
            .checked_add(1)
            .ok_or(Error::NumericExhaustion)?;
        let mapping = &mut state.domains[domain_index].mappings[mapping_index];
        reserve_one(&mut mapping.handlers)?;
        if !mapping.enabled {
            require_local_lifecycle_available(mapping.hardware)?;
            state.controller.enable(mapping.hardware)?;
            mapping.enabled = true;
            mapping.consecutive_unhandled = 0;
        }
        let registration = Registration {
            id: state.next_registration,
            interrupt,
        };
        state.next_registration = next_registration;
        mapping.handlers.push(HandlerEntry {
            id: registration.id,
            context,
            handler,
        });
        Ok(registration)
    })
}

/// Removes exactly the handler identified by `registration`.
///
/// The hardware line is disabled automatically when its final handler is
/// removed. A stale or duplicate registration is rejected explicitly.
pub fn unregister(registration: Registration) -> Result<(), Error> {
    with_state(|state| {
        let (domain_index, mapping_index) = state
            .mapping_position_by_virtual(registration.interrupt)
            .ok_or(Error::InterruptNotMapped)?;
        let mapping = &mut state.domains[domain_index].mappings[mapping_index];
        let handler_index = mapping
            .handlers
            .iter()
            .position(|handler| handler.id == registration.id)
            .ok_or(Error::HandlerNotFound)?;
        if mapping.handlers.len() == 1 && mapping.enabled {
            require_local_lifecycle_available(mapping.hardware)?;
            state.controller.disable(mapping.hardware)?;
            mapping.enabled = false;
        }
        mapping.handlers.swap_remove(handler_index);
        Ok(())
    })
}

pub fn dispatch() {
    let outcome = match with_state(|state| Ok(state.dispatch_one())) {
        Ok(outcome) => outcome,
        Err(Error::NotInitialized) => {
            crate::kernel::exception::fatal_interrupt("controller not initialized", None)
        }
        Err(error) => crate::kernel::exception::fatal_interrupt_state(error),
    };
    match outcome {
        DispatchOutcome::Spurious | DispatchOutcome::Handled => {}
        DispatchOutcome::Quarantined {
            hardware,
            virtual_interrupt,
        } => crate::pr_warn!(
            "HypeR: quarantined unhandled IRQ: INTID {}, VIRQ {}",
            hardware.get(),
            virtual_interrupt.get()
        ),
        DispatchOutcome::Unmapped(hardware) => crate::pr_warn!(
            "HypeR: masked IRQ without a domain mapping: INTID {}",
            hardware.get()
        ),
    }
}

impl InterruptState {
    fn mapping_position_by_virtual(&self, interrupt: VirtualInterrupt) -> Option<(usize, usize)> {
        self.domains
            .iter()
            .enumerate()
            .find_map(|(domain, entries)| {
                entries
                    .mappings
                    .iter()
                    .position(|mapping| mapping.virtual_interrupt == interrupt)
                    .map(|mapping| (domain, mapping))
            })
    }

    fn mapping_position_by_hardware(&self, interrupt: InterruptId) -> Option<(usize, usize)> {
        self.domains
            .iter()
            .enumerate()
            .find_map(|(domain, entries)| {
                entries
                    .mappings
                    .iter()
                    .position(|mapping| mapping.hardware == interrupt)
                    .map(|mapping| (domain, mapping))
            })
    }

    fn dispatch_one(&mut self) -> DispatchOutcome {
        let Some(hardware) = self.controller.acknowledge() else {
            return DispatchOutcome::Spurious;
        };
        let Some((domain_index, mapping_index)) = self.mapping_position_by_hardware(hardware)
        else {
            let _ = self.controller.disable(hardware);
            self.controller.end(hardware);
            return DispatchOutcome::Unmapped(hardware);
        };
        let mapping = &mut self.domains[domain_index].mappings[mapping_index];
        let mut handled = false;
        for entry in &mapping.handlers {
            handled |=
                (entry.handler)(mapping.virtual_interrupt, entry.context) == HandlerResult::Handled;
        }
        let outcome = if handled {
            mapping.consecutive_unhandled = 0;
            DispatchOutcome::Handled
        } else {
            mapping.consecutive_unhandled = mapping.consecutive_unhandled.saturating_add(1);
            if mapping.consecutive_unhandled >= UNHANDLED_QUARANTINE_THRESHOLD
                && (mapping.enabled || hardware.get() < 32)
            {
                let _ = self.controller.disable(hardware);
                mapping.enabled = false;
                DispatchOutcome::Quarantined {
                    hardware,
                    virtual_interrupt: mapping.virtual_interrupt,
                }
            } else {
                DispatchOutcome::Handled
            }
        };
        self.controller.end(hardware);
        outcome
    }
}

fn with_state<R>(
    operation: impl FnOnce(&mut InterruptState) -> Result<R, Error>,
) -> Result<R, Error> {
    INTERRUPTS.with(|slot| {
        let state = slot.as_mut().ok_or(Error::NotInitialized)?;
        operation(state)
    })
}

fn reserve_one<T>(entries: &mut Vec<T>) -> Result<(), Error> {
    entries.try_reserve(1).map_err(|_| Error::Allocation)
}

fn require_local_lifecycle_available(interrupt: InterruptId) -> Result<(), Error> {
    if interrupt.get() < 32 && crate::kernel::cpu::online_cpu_count() > 1 {
        return Err(Error::LocalInterruptLifecycleRequiresCrossCall);
    }
    Ok(())
}
