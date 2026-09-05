// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected typed VM-exit service table.
//!
//! Target-specific copied event values stay in this narrow boundary. Common
//! memory-fault policy remains in the parent module.

pub(super) const fn services() -> crate::hal::vm::ExitServices {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::hal::vm::ExitServices::aarch64(
            super::dispatch_memory_fault,
            dispatch_mmio,
            dispatch_guest_sync,
        )
    }
    #[cfg(CONFIG_ARCH_RISCV64)]
    {
        crate::hal::vm::ExitServices::riscv64(super::dispatch_memory_fault, dispatch_guest_sync)
    }
    #[cfg(CONFIG_ARCH_X86_64)]
    {
        crate::hal::vm::ExitServices::x86_64(
            super::dispatch_memory_fault,
            dispatch_port_io,
            query_pending_interrupt,
        )
    }
}

#[cfg(CONFIG_ARCH_AARCH64)]
fn dispatch_mmio(access: hyper::vm::exit::MmioAccess) -> hyper::vm::exit::MmioAction {
    match crate::kernel::vm::active_vcpu::with(|execution, interrupts| {
        crate::kernel::vm::device::selected::dispatch_mmio(execution, interrupts, access)
    }) {
        Ok(Some(dispatch)) => dispatch.into_action(),
        Ok(None) => crate::kernel::crash::fatal(format_args!(
            "HypeR: guest MMIO exit arrived without an active vCPU"
        )),
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: invalid guest MMIO entry context: {error:?}"
        )),
    }
}

#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
fn dispatch_guest_sync(exit: crate::hal::vm::GuestSyncExit) -> crate::hal::vm::GuestSyncAction {
    match crate::kernel::vm::active_vcpu::with(|execution, interrupts| {
        #[cfg(CONFIG_ARCH_RISCV64)]
        if let Some(byte) = exit.legacy_console_byte() {
            crate::kernel::log::console::write_raw_byte(byte);
            return crate::hal::vm::GuestSyncAction::complete_legacy_console();
        }
        crate::hal::vm::handle_guest_sync(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
            exit,
        )
    }) {
        Ok(Some(action)) => action,
        Ok(None) => crate::kernel::crash::fatal(format_args!(
            "HypeR: synchronous guest exit arrived without an active vCPU"
        )),
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: invalid synchronous guest entry context: {error:?}"
        )),
    }
}

#[cfg(CONFIG_ARCH_X86_64)]
fn dispatch_port_io(exit: hyper::vm::x86::exit::PortIoExit) -> hyper::vm::x86::exit::PortIoAction {
    use hyper::vm::x86::exit::{PortIoAction, PortIoOperation};

    match crate::kernel::vm::device::selected::access_port(exit) {
        Ok(value) => match (exit.operation(), value) {
            (PortIoOperation::Input, Some(value)) => PortIoAction::CompleteInput(value),
            (PortIoOperation::Output(_), _) => PortIoAction::CompleteOutput,
            (PortIoOperation::Input, None) => PortIoAction::Stop,
        },
        Err(error) => {
            crate::pr_err!("HypeR: x86 guest port-I/O dispatch failed: {error:?}");
            PortIoAction::Stop
        }
    }
}

#[cfg(CONFIG_ARCH_X86_64)]
fn query_pending_interrupt(timer_pending: bool) -> hyper::vm::x86::exit::PendingInterruptAction {
    use hyper::vm::x86::device::legacy_pc::InterruptSource;
    use hyper::vm::x86::exit::PendingInterruptAction;

    match crate::kernel::vm::device::selected::pending_interrupt(timer_pending) {
        Ok(Some(pending)) => PendingInterruptAction::Inject {
            vector: pending.vector,
            consumes_timer: pending.source == InterruptSource::Timer,
        },
        Ok(None) => PendingInterruptAction::None,
        Err(error) => {
            crate::pr_err!("HypeR: x86 guest interrupt routing failed: {error:?}");
            PendingInterruptAction::Stop
        }
    }
}
