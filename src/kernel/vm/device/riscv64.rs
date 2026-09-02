// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V guest-platform device service.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {}

pub(crate) struct VirtualDeviceSet;

pub(super) const fn prepare() -> Result<VirtualDeviceSet, Error> {
    Ok(VirtualDeviceSet)
}

pub(super) const fn clear_console_route_for_vm(_expected_vm: super::super::super::registry::VmId) {}

pub(super) const fn receive_console_input(_byte: u8) -> bool {
    false
}

pub(super) const fn try_publish_console_route(
    _vm: super::super::super::registry::VmId,
    _vcpu: u32,
    _thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    false
}
