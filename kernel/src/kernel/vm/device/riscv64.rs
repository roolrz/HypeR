// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V interrupt-controller access and generic guest MMIO dispatch.

use hyper::abi::native::*;
use hyper::vm::exit::{MmioAccess, MmioAction};
use hyper::vm::interrupt::VirtualInterruptId;
const PLIC_BASE: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_BASE;
const PLIC_SIZE: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_SIZE;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAccess,
}
pub(crate) struct VirtualDeviceSet;
pub(super) fn prepare() -> Result<VirtualDeviceSet, Error> {
    Ok(VirtualDeviceSet)
}
pub(super) fn supports_userspace_mmio(profile: u32, base: u64, length: u64) -> bool {
    profile == HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE as u32
        && base == HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_BASE
        && length == HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_SIZE
}
pub(super) fn supports_configuration(profile: u32, memory_base: u64, memory_size: u64) -> bool {
    profile == hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE as u32
        && memory_base == HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_GUEST_RAM_BASE
        && memory_size != 0
        && memory_base.checked_add(memory_size).is_some()
}
pub(super) const fn default_timer_interrupt() -> VirtualInterruptId {
    VirtualInterruptId::new(5)
}

#[must_use]
pub(in crate::kernel) struct MmioDispatch {
    action: MmioAction,
}
impl MmioDispatch {
    pub(in crate::kernel) const fn into_action(self) -> MmioAction {
        self.action
    }
}

pub(super) fn dispatch_mmio(
    execution: &mut crate::kernel::vm::vcpu::VcpuExecution,
    access: MmioAccess,
) -> MmioDispatch {
    let Some((action, report)) = (|| {
        let (binding, hardware, vcpu, interrupts) = execution.device_context()?;
        let result = match access
            .address()
            .get()
            .checked_sub(PLIC_BASE)
            .filter(|offset| *offset < PLIC_SIZE)
        {
            Some(offset) => crate::hal::vm::access_plic(
                hardware,
                interrupts,
                vcpu,
                offset,
                access.size(),
                access.operation(),
            )
            .map(Some)
            .map_err(|_| Error::InvalidAccess),
            None => Ok(None),
        };
        Some(match result {
            Ok(Some(Some(value))) => (MmioAction::CompleteRead(value), None),
            Ok(Some(None)) => (MmioAction::CompleteWrite, None),
            Ok(None) => match binding.lifecycle().route_mmio(vcpu, access) {
                Some(action) => (action, None),
                None => (
                    MmioAction::Unhandled,
                    binding.admit_unhandled_mmio(vcpu, access),
                ),
            },
            Err(_) => (MmioAction::Stop, None),
        })
    })() else {
        return MmioDispatch {
            action: MmioAction::Stop,
        };
    };
    if let Some(report) = report
        && execution.publish_terminal_mmio_report(report).is_err()
    {
        crate::kernel::crash::fatal(format_args!("HypeR: duplicate terminal RISC-V MMIO report"));
    }
    MmioDispatch { action }
}
