// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V guest-platform device service.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {}

pub(crate) struct VirtualDeviceSet {
    virtual_serial: Option<super::super::VirtualSerialBinding>,
}

impl VirtualDeviceSet {
    pub(super) fn write_console_byte(&self, byte: u8) {
        if let Some(output) = &self.virtual_serial {
            output.write_byte(byte);
        }
    }

    pub(in crate::kernel::vm) fn bind_virtual_serial(
        &self,
        vm: super::super::super::registry::VmId,
        vcpu: u32,
        thread: crate::kernel::task::thread::ThreadId,
    ) {
        if let Some(output) = &self.virtual_serial {
            output.bind(crate::kernel::vm::virtual_serial::Route { vm, vcpu, thread });
        }
    }

    pub(in crate::kernel::vm) fn disconnect_virtual_serial(
        &self,
        vm: super::super::super::registry::VmId,
    ) {
        if let Some(output) = &self.virtual_serial {
            output.disconnect(vm);
        }
    }
}

pub(super) const fn prepare(
    virtual_serial: Option<super::super::VirtualSerialBinding>,
) -> Result<VirtualDeviceSet, Error> {
    Ok(VirtualDeviceSet { virtual_serial })
}

pub(super) const fn supports_configuration(
    _profile: u32,
    _memory_base: u64,
    _memory_size: u64,
) -> bool {
    false
}

pub(super) const fn default_timer_interrupt() -> hyper::vm::interrupt::VirtualInterruptId {
    hyper::vm::interrupt::VirtualInterruptId::new(5)
}

pub(super) const fn clear_console_route_for_vm(_expected_vm: super::super::super::registry::VmId) {}

pub(super) fn kick_virtual_serial(route: crate::kernel::vm::virtual_serial::Route) {
    let _ = (route.vcpu, route.thread);
    let _ = super::super::super::registry::with_binding(route.vm, |binding| {
        binding.devices().disconnect_virtual_serial(route.vm);
    });
}

pub(super) const fn receive_console_input(_byte: u8) -> super::super::ConsoleInputDisposition {
    super::super::ConsoleInputDisposition::from_guest_claim(false)
}

#[cfg(feature = "kernel-self-test")]
pub(super) const fn try_publish_console_route(
    _vm: super::super::super::registry::VmId,
    _vcpu: u32,
    _thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    false
}
