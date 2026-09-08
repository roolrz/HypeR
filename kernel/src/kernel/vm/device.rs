// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected guest-platform device service.
//!
//! Reusable register models live under [`hyper::vm`]. The selected module owns
//! per-VM model instances, host bindings, and guest-ISA exit decoding. Device
//! policy deliberately remains in the kernel VM service rather than the HAL.

use crate::kernel::object::{KernelRef, VmDeviceBinding};
use crate::kernel::vm::virtual_serial::{Route, VirtualSerial};

pub(in crate::kernel) mod selected;

pub use selected::Error;
pub(crate) use selected::VirtualDeviceSet;

/// Virtual-serial authority committed into one VM's device set.
///
/// Production bindings retain a typed reference derived from an explicitly
/// supplied `VirtualSerial` handle. The VM-exit path can therefore exchange
/// bytes without a handle-table lookup or allocation. Kernel self-tests opt
/// into the physical host sink through a visibly test-only route.
pub(crate) enum VirtualSerialBinding {
    VirtualSerial(KernelRef<VirtualSerial, VmDeviceBinding>),
    #[cfg(feature = "kernel-self-test")]
    HostTest,
}

impl VirtualSerialBinding {
    pub(crate) const fn from_virtual_serial(
        serial: KernelRef<VirtualSerial, VmDeviceBinding>,
    ) -> Self {
        Self::VirtualSerial(serial)
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) const fn for_host_test() -> Self {
        Self::HostTest
    }

    /// Best-effort, allocation-free publication into the bounded host queue.
    pub(super) fn write_byte(&self, byte: u8) {
        match self {
            Self::VirtualSerial(serial) => serial.object().publish_guest_output(byte),
            #[cfg(feature = "kernel-self-test")]
            Self::HostTest => crate::kernel::log::console::write_test_guest_console_byte(byte),
        }
    }

    #[allow(
        dead_code,
        reason = "selected guest UART backends consume input only when they implement receive injection"
    )]
    pub(super) fn pop_guest_input(&self) -> Option<u8> {
        match self {
            Self::VirtualSerial(serial) => serial.object().pop_guest_input(),
            #[cfg(feature = "kernel-self-test")]
            Self::HostTest => None,
        }
    }

    pub(super) fn bind(&self, route: Route) {
        match self {
            Self::VirtualSerial(serial) => serial.object().bind(route),
            #[cfg(feature = "kernel-self-test")]
            Self::HostTest => {}
        }
    }

    pub(super) fn disconnect(&self, vm: super::registry::VmId) {
        match self {
            Self::VirtualSerial(serial) => serial.object().disconnect(vm),
            #[cfg(feature = "kernel-self-test")]
            Self::HostTest => {}
        }
    }
}

pub(crate) fn prepare(
    virtual_serial: Option<VirtualSerialBinding>,
) -> Result<VirtualDeviceSet, Error> {
    selected::prepare(virtual_serial)
}

/// Validates guest RAM against the selected immutable platform profile.
pub(crate) fn supports_configuration(profile: u32, memory_base: u64, memory_size: u64) -> bool {
    selected::supports_configuration(profile, memory_base, memory_size)
}

/// Returns the architected timer interrupt used by the selected guest board.
pub(crate) const fn default_timer_interrupt() -> hyper::vm::interrupt::VirtualInterruptId {
    selected::default_timer_interrupt()
}

/// Clears an optional host-console route for this VM.
pub(super) fn clear_console_route_for_vm(expected_vm: super::registry::VmId) {
    selected::clear_console_route_for_vm(expected_vm);
}

pub(super) fn kick_virtual_serial(route: Route) {
    selected::kick_virtual_serial(route);
}

/// Guest ownership decision for one byte received from the host console.
#[derive(Clone, Copy)]
pub(crate) struct ConsoleInputDisposition {
    claimed_by_guest: bool,
}

impl ConsoleInputDisposition {
    const fn from_guest_claim(claimed_by_guest: bool) -> Self {
        Self { claimed_by_guest }
    }

    /// Reports whether offering this byte to Native userspace would cross an
    /// established guest-console ownership boundary.
    pub(crate) const fn claimed_by_guest(self) -> bool {
        self.claimed_by_guest
    }
}

/// Attempts to deliver one host-console byte to the selected guest platform.
pub(super) fn receive_console_input(byte: u8) -> ConsoleInputDisposition {
    selected::receive_console_input(byte)
}

#[cfg(feature = "kernel-self-test")]
pub(super) fn try_publish_console_route(
    vm: super::registry::VmId,
    vcpu: u32,
    thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    selected::try_publish_console_route(vm, vcpu, thread)
}
