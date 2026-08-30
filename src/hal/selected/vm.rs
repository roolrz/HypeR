// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected hardware-virtualization capabilities.
//!
//! Kernel VM policy owns identity, publication, scheduling, demand paging,
//! devices, and exit disposition. This facade owns only the selected vCPU
//! machine state and the mechanisms which operate on it.
//!
//! `VcpuContext`, `Stage2AddressSpace`, and `InterruptController` are selected
//! layout-state re-exports rather than opaque wrappers. Assembly and backend
//! code consume their addresses and layouts directly, so wrapping them would
//! require unchecked pointer conversion or an architecture-to-HAL callback.
//! Kernel policy must still use only the operations exposed by this module.

use hyper::hal::interrupt::{HostInterruptBinding, InterruptId};

pub(crate) use crate::arch::vm::{
    DeviceError, ExitServiceError, ExitServices, ExitServicesReady, InterruptInitializationError,
    RegisterValidationError, Stage2AddressSpace, Stage2Error, VcpuContext, VirtualInterruptError,
};
#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) use crate::arch::vm::{GuestSyncAction, GuestSyncExit};
pub use crate::arch::vm::{InterruptController, InterruptError, VcpuInterruptError};

/// Selected per-vCPU machine state retained by one scheduler execution.
pub struct VcpuHardwareState {
    context: VcpuContext,
    runtime_authorized: bool,
}

/// Proof that every dependency required for normal guest entry is active.
///
/// The kernel may mint this only after exit services, register validation,
/// virtual devices, timer routing, and interrupt virtualization have all
/// completed. Copies describe the same irreversible boot publication.
#[derive(Clone, Copy)]
#[must_use]
pub(crate) struct VmEntryReady {
    _private: (),
}

impl VcpuHardwareState {
    pub(crate) const fn new(context: VcpuContext, _entry: &VmEntryReady) -> Self {
        Self {
            context,
            runtime_authorized: true,
        }
    }

    #[cfg(CONFIG_ARCH_AARCH64)]
    const fn for_validation(context: VcpuContext, _services: &ExitServicesReady) -> Self {
        Self {
            context,
            runtime_authorized: false,
        }
    }
}

pub(crate) fn install_exit_services(
    services: ExitServices,
) -> Result<ExitServicesReady, ExitServiceError> {
    crate::arch::vm::install_exit_services(services)
}

/// Commits the one-way transition from callback publication to guest entry.
///
/// # Safety
///
/// Register validation and selected virtual-device initialization must have
/// completed. Every host timer route and interrupt-virtualization dependency
/// must be active and no longer eligible for rollback. The caller must own the
/// only VM initialization transaction and invoke this commit at most once.
pub(crate) unsafe fn commit_entry_initialization(_services: ExitServicesReady) -> VmEntryReady {
    VmEntryReady { _private: () }
}

pub(crate) fn validate_register_interface() -> Result<(), RegisterValidationError> {
    crate::arch::vm::validate_register_interface()
}

pub(crate) fn initialize_devices(
    timer_interrupt: InterruptId,
    host_timer_interrupt: Option<HostInterruptBinding>,
) -> Result<(), DeviceError> {
    crate::arch::vm::initialize_devices(timer_interrupt, host_timer_interrupt)
}

pub(crate) fn initialize_interrupts(
    host_timer_interrupt: Option<HostInterruptBinding>,
) -> Result<(), InterruptInitializationError> {
    crate::arch::vm::initialize_interrupts(host_timer_interrupt)
}

pub(crate) fn initialize_vcpu_interrupts(
    state: &mut VcpuHardwareState,
) -> Result<(), VirtualInterruptError> {
    state.context.initialize_virtual_interrupts().map(|_| ())
}

/// Sets the guest-visible virtual counter before the context becomes runnable.
pub(crate) fn set_virtual_count(context: &mut VcpuContext, physical: u64, value: u64) {
    context.set_virtual_count(physical, value);
}

/// Applies the selected architecture's local interrupt state for guest entry.
///
/// Callers must not assume this unmasks interrupts. In particular, `AArch64`
/// keeps IRQs masked across the non-atomic EL2-to-guest context transaction.
pub(crate) fn prepare_interrupts_for_entry() {
    crate::arch::vm::prepare_interrupts_for_entry();
}

/// Activates the local machine state for a stopped vCPU.
///
/// # Safety
///
/// `state` must be pinned and exclusively owned by the stopped vCPU. No guest
/// may execute concurrently, and local interrupts must remain masked. A caller
/// which can proceed to guest entry must activate the selected second-stage
/// hierarchy before entry; machine-state-only validation need not do so. An
/// error must leave local vCPU hardware detached so ownership can be released.
pub(crate) unsafe fn activate_hardware(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    physical_count: u64,
) -> Result<bool, VcpuInterruptError> {
    // SAFETY: The facade preserves stopped-state ownership, stage-2, and
    // interrupt-mask requirements while hiding the backend context layout.
    unsafe {
        crate::arch::vm::activate_vcpu_hardware(
            &mut state.context,
            vcpu_id,
            interrupts,
            physical_count,
        )
    }
}

/// Saves and detaches the current vCPU's local machine state.
///
/// # Safety
///
/// `state` must exclusively own the active local machine state. Guest execution
/// must have stopped and local interrupts must remain masked.
pub(crate) unsafe fn deactivate_hardware(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    physical_count: u64,
) -> Result<(), VcpuInterruptError> {
    // SAFETY: The facade preserves active ownership and interrupt masking.
    unsafe {
        crate::arch::vm::deactivate_vcpu_hardware(
            &mut state.context,
            vcpu_id,
            interrupts,
            physical_count,
        )
    }
}

/// Transfers control to a pinned, exclusively owned vCPU machine state.
///
/// # Safety
///
/// `state` must be non-null, aligned, pinned, and exclusively owned by the
/// active vCPU for the entire guest-run lifetime. Stage-2 and local virtual
/// hardware must already be active, and the state must carry final
/// [`VmEntryReady`] authorization rather than validation-only authorization.
/// Guest exits may mutate the state.
pub(crate) unsafe fn enter(state: *mut VcpuHardwareState) -> ! {
    // SAFETY: The caller guarantees a valid exclusive state pointer.
    if !unsafe { (*state).runtime_authorized } {
        crate::hal::cpu::halt()
    }
    // SAFETY: The caller pins and exclusively owns the complete state.
    let context = unsafe { core::ptr::addr_of_mut!((*state).context) };
    // SAFETY: The facade preserves the backend entry contract unchanged.
    unsafe { VcpuContext::enter(context) }
}

pub(crate) fn handle_virtual_timer_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<bool, VcpuInterruptError> {
    crate::arch::vm::handle_virtual_timer_interrupt(&mut state.context, vcpu_id, interrupts)
}

pub(crate) fn handle_maintenance_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<bool, VcpuInterruptError> {
    crate::arch::vm::handle_maintenance_interrupt(&mut state.context, vcpu_id, interrupts)
}

pub(crate) fn maintenance_interrupt_pending() -> bool {
    crate::arch::vm::maintenance_interrupt_pending()
}

/// Disables local virtual-interrupt delivery after ownership is lost.
pub(crate) fn quiesce_virtual_interrupt_delivery() {
    crate::arch::vm::quiesce_virtual_interrupt_delivery();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InterruptVirtualizationDescription {
    pub(crate) list_registers: u8,
    pub(crate) priority_bits: u8,
    pub(crate) preemption_bits: u8,
    pub(crate) interrupt_id_bits: u8,
}

pub(crate) fn interrupt_virtualization_description() -> Option<InterruptVirtualizationDescription> {
    crate::arch::vm::interrupt_virtualization_description().map(
        |(list_registers, priority_bits, preemption_bits, interrupt_id_bits)| {
            InterruptVirtualizationDescription {
                list_registers,
                priority_bits,
                preemption_bits,
                interrupt_id_bits,
            }
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimerValidationError {
    #[cfg(CONFIG_ARCH_AARCH64)]
    InterruptController(InterruptError),
    #[cfg(CONFIG_ARCH_AARCH64)]
    InvalidInterrupt,
    #[cfg(CONFIG_ARCH_AARCH64)]
    VirtualInterrupt(VirtualInterruptError),
}

pub(crate) fn prepare_timer_validation(
    timer_interrupt: InterruptId,
    physical_count: u64,
    services: &ExitServicesReady,
) -> Result<Option<(InterruptController, VcpuHardwareState)>, TimerValidationError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let timer = hyper::vm::interrupt::VirtualInterruptId::new(timer_interrupt.get())
            .ok_or(TimerValidationError::InvalidInterrupt)?;
        let interrupts = InterruptController::new(1, timer)
            .map_err(TimerValidationError::InterruptController)?;
        let mut context = VcpuContext::new(0);
        context
            .initialize_virtual_interrupts()
            .map_err(TimerValidationError::VirtualInterrupt)?;
        context.set_virtual_count(physical_count, physical_count);
        context.set_virtual_timer_deadline(physical_count.wrapping_add(1_000_000));
        context.set_virtual_timer_enabled(true);
        Ok(Some((
            interrupts,
            VcpuHardwareState::for_validation(context, services),
        )))
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (timer_interrupt, physical_count, services);
        Ok(None)
    }
}

pub(crate) fn inject_timer_for_validation(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<bool, VcpuInterruptError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::arch::vm::inject_timer_for_validation(&mut state.context, vcpu_id, interrupts)?;
        Ok(true)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (state, vcpu_id, interrupts);
        Ok(false)
    }
}

pub(crate) fn timer_validation_succeeded(
    interrupts: &InterruptController,
) -> Result<bool, InterruptError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let snapshot = interrupts
            .timer_snapshot(hyper::vm::interrupt::VirtualCpuId::new(0))
            .map_err(InterruptError::Vgic)?;
        Ok(snapshot.pending && snapshot.listed)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = interrupts;
        Ok(true)
    }
}

#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) fn handle_guest_sync(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    exit: GuestSyncExit,
) -> GuestSyncAction {
    crate::arch::vm::handle_guest_sync(&mut state.context, vcpu_id, interrupts, exit)
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn update_guest_device_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    interrupt: hyper::vm::interrupt::VirtualInterruptId,
    asserted: bool,
) -> Result<(), VcpuInterruptError> {
    crate::arch::vm::update_guest_device_interrupt(
        &mut state.context,
        vcpu_id,
        interrupts,
        interrupt,
        asserted,
    )
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn guest_execution_available() -> bool {
    crate::arch::vm::guest_execution_available()
}
