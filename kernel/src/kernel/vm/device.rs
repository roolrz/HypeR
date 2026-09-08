// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected guest-platform device service.
//!
//! Reusable register models live under [`hyper::vm`]. The selected module owns
//! per-VM model instances, host bindings, and guest-ISA exit decoding. Device
//! policy deliberately remains in the kernel VM service rather than the HAL.

use crate::kernel::device::console::SystemConsole;
use crate::kernel::object::{KernelRef, VmDeviceBinding};

pub(in crate::kernel) mod selected;

pub use selected::Error;
pub(crate) use selected::VirtualDeviceSet;

/// Console output authority committed into one VM's device set.
///
/// Production bindings retain a typed reference derived from an explicitly
/// supplied Console handle. The VM-exit path can therefore enqueue output
/// without a handle-table lookup or allocation. Kernel self-tests opt into the
/// same host sink through a visibly test-only route.
pub(crate) enum ConsoleOutputBinding {
    Console(KernelRef<SystemConsole, VmDeviceBinding>),
    #[cfg(feature = "kernel-self-test")]
    HostTest,
}

impl ConsoleOutputBinding {
    pub(crate) const fn from_console(console: KernelRef<SystemConsole, VmDeviceBinding>) -> Self {
        Self::Console(console)
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) const fn for_host_test() -> Self {
        Self::HostTest
    }

    /// Best-effort, allocation-free publication into the bounded host queue.
    pub(super) fn write_byte(&self, byte: u8) {
        match self {
            Self::Console(console) => {
                let _ = console.object().try_write(core::slice::from_ref(&byte));
            }
            #[cfg(feature = "kernel-self-test")]
            Self::HostTest => crate::kernel::log::console::write_test_guest_console_byte(byte),
        }
    }
}

pub(crate) fn prepare(
    console_output: Option<ConsoleOutputBinding>,
) -> Result<VirtualDeviceSet, Error> {
    selected::prepare(console_output)
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
