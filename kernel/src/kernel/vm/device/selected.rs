// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected guest-platform device implementation.
//!
//! This is the only host-build selection point for guest device models. The
//! parent module exposes architecture-neutral ownership and console lifecycle;
//! typed exit callbacks enter the selected implementation directly.

#[cfg(CONFIG_ARCH_AARCH64)]
#[path = "gicv3.rs"]
mod gicv3;
#[cfg(CONFIG_ARCH_AARCH64)]
#[path = "aarch64.rs"]
mod platform;
#[cfg(CONFIG_ARCH_RISCV64)]
#[path = "riscv64.rs"]
mod platform;
#[cfg(CONFIG_ARCH_X86_64)]
#[path = "x86_64.rs"]
mod platform;

pub use platform::Error;
pub(crate) use platform::VirtualDeviceSet;

pub(super) fn prepare() -> Result<VirtualDeviceSet, Error> {
    platform::prepare()
}

pub(super) fn clear_console_route_for_vm(vm: super::super::registry::VmId) {
    platform::clear_console_route_for_vm(vm);
}

pub(super) fn receive_console_input(byte: u8) -> super::ConsoleInputDisposition {
    platform::receive_console_input(byte)
}

pub(super) fn try_publish_console_route(
    vm: super::super::registry::VmId,
    vcpu: u32,
    thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    platform::try_publish_console_route(vm, vcpu, thread)
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(in crate::kernel) use platform::MmioDispatch;

#[cfg(CONFIG_ARCH_AARCH64)]
pub(in crate::kernel) fn dispatch_mmio(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    interrupts: &crate::kernel::vm::VmInterruptController,
    access: hyper::vm::exit::MmioAccess,
) -> MmioDispatch {
    platform::dispatch_mmio(execution, interrupts, access)
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(in crate::kernel) fn access_port(
    access: hyper::vm::x86::exit::PortIoExit,
) -> Result<Option<u32>, Error> {
    platform::access_port(access)
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(in crate::kernel) fn pending_interrupt(
    timer_pending: bool,
) -> Result<Option<hyper::vm::x86::device::legacy_pc::PendingInterrupt>, Error> {
    platform::pending_interrupt(timer_pending)
}
