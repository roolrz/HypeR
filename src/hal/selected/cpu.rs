// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected CPU identity, lifecycle, and low-power capabilities.
//!
//! Kernel policy assigns logical CPU indices and decides when a processor
//! starts, idles, or stops. This facade selects the machine implementation
//! while preserving typed logical and firmware-visible CPU identities.

use hyper::cpu::CpuIndex;
use hyper::hal::cpu_power::{CpuHardwareId, ResumeAddress};
use hyper::platform::CpuPowerInfo;

pub(crate) use crate::arch::cpu::{PowerController, PowerError, SecondaryBootParameters};

#[inline]
pub(crate) fn initialize_power(info: CpuPowerInfo) -> Result<PowerController, PowerError> {
    crate::arch::cpu::initialize_power(info)
}

#[inline]
pub(crate) fn secondary_is_compatible() -> bool {
    crate::arch::cpu::secondary_is_compatible()
}

#[inline]
pub(crate) fn current_index() -> Option<CpuIndex> {
    crate::arch::cpu::current_index()
}

#[inline]
pub(crate) fn current_hardware_id() -> CpuHardwareId {
    crate::arch::cpu::current_hardware_id()
}

#[inline]
pub(crate) fn secondary_entry_address(
    image_physical_start: u64,
    kernel_virtual_base: u64,
) -> Option<ResumeAddress> {
    crate::arch::cpu::secondary_entry_address(image_physical_start, kernel_virtual_base)
}

#[inline]
pub(crate) fn register_secondary(index: CpuIndex, hardware_id: CpuHardwareId) -> bool {
    crate::arch::cpu::register_secondary(index, hardware_id)
}

#[inline]
pub(crate) fn mark_current_online() {
    crate::arch::cpu::mark_current_online();
}

#[inline]
pub(crate) fn send_event() {
    crate::arch::cpu::send_event();
}

/// Waits after scheduler work was checked with local interrupts masked.
///
/// The selected architecture must close its check-to-sleep race and return
/// with interrupts still masked so the caller can restore exact guard state.
#[inline]
pub(crate) fn wait_for_interrupt_masked() {
    crate::arch::cpu::wait_for_interrupt_masked();
}

#[inline]
pub(crate) fn halt() -> ! {
    crate::arch::cpu::halt()
}
