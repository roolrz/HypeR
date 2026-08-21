// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! IRQ domains, mappings, registration, and dispatch policy.

use alloc::vec::Vec;

use hyper::hal::interrupt::{
    InterruptController, InterruptId, InterruptPriority, InterruptTrigger,
    KernelInterruptController,
};
use hyper::platform::{InterruptControllerInfo, PlatformInterrupt};
use hyper::sync::InterruptSpinLock;

type BootInterruptController = crate::arch::irq::Controller;
type InterruptLock = InterruptSpinLock<Option<InterruptState>, crate::arch::irq::LocalMask>;

const UNHANDLED_QUARANTINE_THRESHOLD: u32 = 8;

static INTERRUPTS: InterruptLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AlreadyInitialized,
    DomainBusy,
    DomainNotFound,
    Controller(crate::arch::irq::ControllerError),
    HandlerNotFound,
    InterruptAlreadyMapped,
    InterruptNotMapped,
    LocalInterruptLifecycleRequiresCrossCall,
    MappingBusy,
    NotInitialized,
    NumericExhaustion,
}

impl From<crate::arch::irq::ControllerError> for Error {
    fn from(error: crate::arch::irq::ControllerError) -> Self {
        Self::Controller(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrqDomainId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualInterrupt(u32);

impl VirtualInterrupt {
    pub const fn from_raw(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Exclusive capability to remove one registered IRQ handler.
///
/// The capability has no implicit `Drop` behavior because dropping it may occur
/// while an unrelated lock is held or while local interrupts cannot safely be
/// changed. Its owner must either pass it to [`unregister`] or explicitly call
/// [`Registration::retain_permanently`] for a kernel-lifetime handler.
#[must_use = "an IRQ registration must be owned, unregistered, or explicitly retained permanently"]
#[derive(Debug, Eq, PartialEq)]
pub struct Registration {
    id: u64,
    interrupt: VirtualInterrupt,
}

impl Registration {
    /// Relinquishes the unregister capability for a kernel-lifetime handler.
    ///
    /// This is intentionally explicit and irreversible. The registry owns the
    /// handler entry until shutdown, and `HypeR` currently has no global IRQ
    /// teardown phase. Subsystems with a shorter lifetime must store this
    /// capability in their owning state instead.
    pub fn retain_permanently(self) {}
}

/// An unsuccessful unregister operation that preserves its capability.
///
/// A valid registration remains installed when unregistration fails. The
/// caller can retry with the returned capability or explicitly retain the
/// handler permanently if its failure path cannot safely retry.
#[must_use = "failed IRQ unregistration returns ownership of the still-active registration"]
#[derive(Debug, Eq, PartialEq)]
pub struct UnregisterFailure {
    error: Error,
    registration: Registration,
}

impl UnregisterFailure {
    pub fn into_parts(self) -> (Error, Registration) {
        (self.error, self.registration)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandlerResult {
    Handled,
    NotHandled,
    HandledAndMaskLocal,
    HandledAndUnmaskLocal(VirtualInterrupt),
}

pub type Handler = fn(VirtualInterrupt, usize) -> HandlerResult;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub interrupt_count: u32,
    pub root_domain: IrqDomainId,
    pub maintenance_interrupt: Option<PlatformInterrupt>,
}

struct HandlerEntry {
    id: u64,
    context: usize,
    handler: Handler,
}

struct IrqMapping {
    hardware: InterruptId,
    virtual_interrupt: VirtualInterrupt,
    priority: InterruptPriority,
    trigger: InterruptTrigger,
    handlers: Vec<HandlerEntry>,
    /// Registry intent shared by all CPUs. Per-CPU quarantine must not clear it.
    enabled_by_registry: bool,
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
    let maintenance_interrupt = match info {
        InterruptControllerInfo::GicV3(info) => info.maintenance_interrupt,
        InterruptControllerInfo::Plic(_) => None,
        InterruptControllerInfo::X2Apic(_) => None,
    };
    // SAFETY: The architecture MMIO window maps every DTB-discovered device
    // range with Device attributes and the controller has a single owner.
    let controller =
        unsafe { BootInterruptController::bind(info, crate::kernel::mm::memory::mmio_address)? };
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
        maintenance_interrupt,
    })
}

/// Initializes the calling secondary CPU's local interrupt-controller state.
pub fn initialize_local_cpu() -> Result<(), Error> {
    with_state(|state| {
        let InterruptState {
            controller,
            domains,
            ..
        } = state;
        // SAFETY: The shared Distributor is active, this CPU still has IRQs
        // masked, and each Redistributor is private to its matching affinity.
        unsafe { controller.initialize_local()? };
        for mapping in domains.iter().flat_map(|domain| domain.mappings.iter()) {
            if mapping.enabled_by_registry && controller.is_per_cpu(mapping.hardware) {
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
    priority: InterruptPriority,
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
            enabled_by_registry: false,
            consecutive_unhandled: 0,
        });
        Ok(virtual_interrupt)
    })
}

impl IrqDomainId {
    /// Creates a mapping and installs its first shared handler transactionally.
    ///
    /// All validation, allocation, and controller operations complete while
    /// the registry is locked. The populated mapping is published only by the
    /// final infallible insertion.
    ///
    /// This is a boot-phase installation interface. The caller must keep local
    /// IRQs masked and the device source quiesced until it has published every
    /// state object the handler reads. Enabling the controller route here does
    /// not make an unpublished handler context safe to observe.
    pub fn register_shared_mapping(
        self,
        hardware: InterruptId,
        priority: InterruptPriority,
        trigger: InterruptTrigger,
        context: usize,
        handler: Handler,
    ) -> Result<(VirtualInterrupt, Registration), Error> {
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
                .position(|candidate| candidate.id == self)
                .ok_or(Error::DomainNotFound)?;
            let next_virtual_interrupt = state
                .next_virtual_interrupt
                .checked_add(1)
                .ok_or(Error::NumericExhaustion)?;
            let next_registration = state
                .next_registration
                .checked_add(1)
                .ok_or(Error::NumericExhaustion)?;
            reserve_one(&mut state.domains[domain_index].mappings)?;
            let mut handlers = Vec::new();
            reserve_one(&mut handlers)?;
            require_local_lifecycle_available(&state.controller, hardware)?;
            state.controller.configure(hardware, priority, trigger)?;
            state.controller.enable(hardware)?;

            let virtual_interrupt = VirtualInterrupt(state.next_virtual_interrupt);
            let registration = Registration {
                id: state.next_registration,
                interrupt: virtual_interrupt,
            };
            handlers.push(HandlerEntry {
                id: registration.id,
                context,
                handler,
            });
            state.next_virtual_interrupt = next_virtual_interrupt;
            state.next_registration = next_registration;
            state.domains[domain_index].mappings.push(IrqMapping {
                hardware,
                virtual_interrupt,
                priority,
                trigger,
                handlers,
                enabled_by_registry: true,
                consecutive_unhandled: 0,
            });
            Ok((virtual_interrupt, registration))
        })
    }
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
/// Handlers run with local IRQs masked and the global registry lock held. They
/// must not allocate, block, mutate IRQ domains or registrations, or acquire a
/// lock ordered before the IRQ registry. Every shared handler is invoked so
/// devices sharing a level-triggered line can inspect and clear their own
/// interrupt source. `context` is an opaque value owned by the registering
/// driver and must remain valid until unregister.
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
        if !mapping.enabled_by_registry {
            require_local_lifecycle_available(&state.controller, mapping.hardware)?;
            state.controller.enable(mapping.hardware)?;
            mapping.enabled_by_registry = true;
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
/// removed. Failure leaves the handler installed and returns the exclusive
/// capability in [`UnregisterFailure`], allowing the owner to retry or retain
/// it deliberately.
pub fn unregister(registration: Registration) -> Result<(), UnregisterFailure> {
    let result = with_state(|state| {
        let (domain_index, mapping_index) = state
            .mapping_position_by_virtual(registration.interrupt)
            .ok_or(Error::InterruptNotMapped)?;
        let mapping = &mut state.domains[domain_index].mappings[mapping_index];
        let handler_index = mapping
            .handlers
            .iter()
            .position(|handler| handler.id == registration.id)
            .ok_or(Error::HandlerNotFound)?;
        if mapping.handlers.len() == 1 && mapping.enabled_by_registry {
            require_local_lifecycle_available(&state.controller, mapping.hardware)?;
            state.controller.disable(mapping.hardware)?;
            mapping.enabled_by_registry = false;
        }
        mapping.handlers.swap_remove(handler_index);
        Ok(())
    });
    result.map_err(|error| UnregisterFailure {
        error,
        registration,
    })
}

/// Dispatches an interrupt already acknowledged by architecture exception entry.
pub fn dispatch(hardware: InterruptId) {
    let outcome = match with_state(|state| Ok(state.dispatch_one(hardware))) {
        Ok(outcome) => outcome,
        Err(Error::NotInitialized) => {
            crate::kernel::irq::exception::fatal_interrupt("controller not initialized", None)
        }
        Err(error) => crate::kernel::irq::exception::fatal_interrupt_state(error),
    };
    match outcome {
        DispatchOutcome::Handled => {}
        DispatchOutcome::Quarantined {
            hardware,
            virtual_interrupt,
        } => {
            crate::pr_warn!(
                "HypeR: quarantined unhandled IRQ: INTID {}, VIRQ {}",
                hardware.get(),
                virtual_interrupt.get()
            );
        }
        DispatchOutcome::Unmapped(hardware) => {
            crate::pr_warn!(
                "HypeR: masked IRQ without a domain mapping: INTID {}",
                hardware.get()
            );
        }
    }
}

/// Claims one pending source from a memory-mapped controller CPU context.
/// Architecture trap entry uses this only for external-interrupt causes.
pub fn acknowledge_external() -> Option<InterruptId> {
    with_state(|state| Ok(state.controller.acknowledge()))
        .ok()
        .flatten()
}

/// Enables one already-mapped PPI on the calling CPU.
pub fn enable_local(interrupt: VirtualInterrupt) -> Result<(), Error> {
    with_state(|state| state.set_local_enabled(interrupt, true))
}

/// Disables one already-mapped PPI on the calling CPU.
pub fn disable_local(interrupt: VirtualInterrupt) -> Result<(), Error> {
    with_state(|state| state.set_local_enabled(interrupt, false))
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

    fn dispatch_one(&mut self, hardware: InterruptId) -> DispatchOutcome {
        let Some((domain_index, mapping_index)) = self.mapping_position_by_hardware(hardware)
        else {
            let _ = self.controller.disable(hardware);
            self.controller.end(hardware);
            return DispatchOutcome::Unmapped(hardware);
        };
        let (outcome, mask_local, unmask_local) = {
            let mapping = &mut self.domains[domain_index].mappings[mapping_index];
            let mut handled = false;
            let mut mask_local = false;
            let mut unmask_local = None;
            for entry in &mapping.handlers {
                match (entry.handler)(mapping.virtual_interrupt, entry.context) {
                    HandlerResult::Handled => handled = true,
                    HandlerResult::NotHandled => {}
                    HandlerResult::HandledAndMaskLocal => {
                        handled = true;
                        mask_local = true;
                    }
                    HandlerResult::HandledAndUnmaskLocal(interrupt) => {
                        handled = true;
                        unmask_local = Some(interrupt);
                    }
                }
            }
            let outcome = if handled {
                mapping.consecutive_unhandled = 0;
                DispatchOutcome::Handled
            } else {
                mapping.consecutive_unhandled = mapping.consecutive_unhandled.saturating_add(1);
                if mapping.consecutive_unhandled >= UNHANDLED_QUARANTINE_THRESHOLD
                    && mapping.enabled_by_registry
                {
                    let _ = self.controller.disable(hardware);
                    // A PPI was disabled only on this CPU. Preserve the
                    // registry's installed intent so secondary-CPU replay and
                    // an explicit local re-enable remain correct.
                    if !self.controller.is_per_cpu(hardware) {
                        mapping.enabled_by_registry = false;
                    }
                    DispatchOutcome::Quarantined {
                        hardware,
                        virtual_interrupt: mapping.virtual_interrupt,
                    }
                } else {
                    DispatchOutcome::Handled
                }
            };
            (outcome, mask_local, unmask_local)
        };
        if mask_local {
            let _ = self.controller.disable(hardware);
        }
        if let Some(interrupt) = unmask_local
            && let Some((domain, mapping)) = self.mapping_position_by_virtual(interrupt)
        {
            let target = self.domains[domain].mappings[mapping].hardware;
            if self.controller.is_per_cpu(target) {
                let _ = self.controller.enable(target);
            }
        }
        self.controller.end(hardware);
        outcome
    }

    fn set_local_enabled(
        &mut self,
        interrupt: VirtualInterrupt,
        enabled: bool,
    ) -> Result<(), Error> {
        let (domain, mapping) = self
            .mapping_position_by_virtual(interrupt)
            .ok_or(Error::InterruptNotMapped)?;
        let hardware = self.domains[domain].mappings[mapping].hardware;
        if !self.controller.is_per_cpu(hardware) {
            return Err(Error::LocalInterruptLifecycleRequiresCrossCall);
        }
        if enabled {
            self.controller.enable(hardware)?;
        } else {
            self.controller.disable(hardware)?;
        }
        Ok(())
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

fn require_local_lifecycle_available(
    controller: &BootInterruptController,
    interrupt: InterruptId,
) -> Result<(), Error> {
    if controller.is_per_cpu(interrupt) && crate::kernel::cpu::online_cpu_count() > 1 {
        Err(Error::LocalInterruptLifecycleRequiresCrossCall)
    } else {
        Ok(())
    }
}
