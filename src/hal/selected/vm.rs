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
    DeviceError, InterruptInitializationError, LegacySyncAction, LegacySyncFrame,
    RegisterValidationError, Stage2AddressSpace, Stage2Error, VcpuContext, VirtualInterruptError,
};
pub use crate::arch::vm::{InterruptController, InterruptError, VcpuInterruptError};

/// Selected per-vCPU machine state retained by one scheduler execution.
pub struct VcpuHardwareState {
    context: VcpuContext,
}

impl VcpuHardwareState {
    pub(crate) const fn new(context: VcpuContext) -> Self {
        Self { context }
    }
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

pub(crate) fn enable_interrupts_for_entry() {
    crate::arch::vm::enable_interrupts_for_entry();
}

/// Activates the local machine state for a stopped vCPU.
///
/// # Safety
///
/// `state` must be pinned and exclusively owned by the stopped vCPU. No guest
/// may execute concurrently, and local interrupts must remain masked. A caller
/// which can proceed to guest entry must activate the selected second-stage
/// hierarchy before entry; machine-state-only validation need not do so.
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
/// hardware must already be active. Guest exits may mutate the state.
pub(crate) unsafe fn enter(state: *mut VcpuHardwareState) -> ! {
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
        Ok(Some((interrupts, VcpuHardwareState::new(context))))
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (timer_interrupt, physical_count);
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

pub(crate) fn decode_legacy_sync(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    frame: &mut LegacySyncFrame<'_>,
) -> LegacySyncAction {
    crate::arch::vm::decode_legacy_sync(&mut state.context, vcpu_id, frame)
}

pub(crate) fn deliver_legacy_software_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    request: u64,
) -> Result<(), VcpuInterruptError> {
    crate::arch::vm::deliver_legacy_software_interrupt(
        &mut state.context,
        vcpu_id,
        interrupts,
        request,
    )
}

pub(crate) fn handle_legacy_device_access(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    frame: &mut LegacySyncFrame<'_>,
    fallback: LegacySyncAction,
) -> LegacySyncAction {
    crate::arch::vm::handle_legacy_device_access(
        &mut state.context,
        vcpu_id,
        interrupts,
        frame,
        fallback,
    )
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use crate::arch::vm::{complete_legacy_mmio, decode_legacy_mmio};

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn update_legacy_device_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    interrupt: hyper::vm::interrupt::VirtualInterruptId,
    asserted: bool,
) -> Result<(), VcpuInterruptError> {
    crate::arch::vm::update_legacy_device_interrupt(
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
