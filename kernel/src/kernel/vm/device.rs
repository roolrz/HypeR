// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected guest-platform device service.
//!
//! Reusable register models live under [`hyper::vm`]. The selected module owns
//! per-VM model instances, host bindings, and guest-ISA exit decoding. Device
//! policy deliberately remains in the kernel VM service rather than the HAL.

pub(in crate::kernel) mod selected;
pub use selected::Error;
pub(crate) use selected::VirtualDeviceSet;
pub(crate) fn prepare() -> Result<VirtualDeviceSet, Error> {
    selected::prepare()
}

/// Validates guest RAM against the selected immutable platform profile.
pub(crate) fn supports_configuration(profile: u32, memory_base: u64, memory_size: u64) -> bool {
    selected::supports_configuration(profile, memory_base, memory_size)
}

pub(crate) fn supports_userspace_mmio(profile: u32, base: u64, length: u64) -> bool {
    selected::supports_userspace_mmio(profile, base, length)
}

/// Returns the architected timer interrupt used by the selected guest board.
pub(crate) const fn default_timer_interrupt() -> hyper::vm::interrupt::VirtualInterruptId {
    selected::default_timer_interrupt()
}

/// Additional allocation charged before virtual-device construction.
pub(crate) fn dynamic_allocation_bytes() -> usize {
    selected::dynamic_allocation_bytes()
}

pub(crate) const fn timer_count() -> u64 {
    selected::timer_count()
}

/// Retires device callbacks after registry visibility has been cut.
/// Called in normal context without holding registry or device locks.
pub(crate) fn quiesce(devices: &VirtualDeviceSet) -> Result<(), Error> {
    selected::quiesce(devices)
}
