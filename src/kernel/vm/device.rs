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

/// Clears an optional host-console route for this VM.
pub(super) fn clear_console_route_for_vm(expected_vm: super::registry::VmId) {
    selected::clear_console_route_for_vm(expected_vm);
}

/// Delivers one host-console byte to the selected guest platform.
pub(super) fn receive_console_input(byte: u8) -> bool {
    selected::receive_console_input(byte)
}

pub(super) fn try_publish_console_route(
    vm: super::registry::VmId,
    vcpu: u32,
    thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    selected::try_publish_console_route(vm, vcpu, thread)
}
