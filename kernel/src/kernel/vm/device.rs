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
/// bytes without a handle-table lookup or allocation.
pub(crate) struct VirtualSerialBinding {
    serial: KernelRef<VirtualSerial, VmDeviceBinding>,
}

impl VirtualSerialBinding {
    pub(crate) const fn from_virtual_serial(
        serial: KernelRef<VirtualSerial, VmDeviceBinding>,
    ) -> Self {
        Self { serial }
    }

    /// Allocation-free publication into the registered userspace output buffer.
    pub(super) fn write_byte(&self, byte: u8) {
        self.serial.object().publish_guest_output(byte)
    }

    #[allow(
        dead_code,
        reason = "selected guest UART backends consume input only when they implement receive injection"
    )]
    pub(super) fn pop_guest_input(&self) -> Option<u8> {
        self.serial.object().pop_guest_input()
    }

    pub(super) fn bind(&self, route: Route) {
        self.serial.object().bind(route)
    }

    pub(super) fn disconnect(&self, vm: super::registry::VmId) {
        self.serial.object().disconnect(vm)
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

pub(super) fn kick_virtual_serial(route: Route) {
    selected::kick_virtual_serial(route);
}
