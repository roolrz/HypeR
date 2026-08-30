// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Registered upward policy services for architecture-owned guest entry.
//!
//! Architecture code copies fixed-width events out of private machine frames,
//! invokes one of these callbacks, and applies the returned typed action only
//! after the callback has returned. No raw frame, VMCS/VMCB borrow, or backend
//! completion reference crosses this boundary.

use hyper::vm::exit::{GuestMemoryFault, MemoryFaultAction};

/// Resolves one owned guest-memory fault through the installed VM.
///
/// Local interrupts remain masked. This is the only VM-exit service allowed to
/// allocate: demand-zero RAM may allocate and publish pages before returning
/// `Retry`. `ForwardToDevice` performs no frame mutation and lets the backend
/// decode an MMIO operation from its still-private exit state.
pub(crate) fn dispatch_memory_fault(fault: GuestMemoryFault) -> MemoryFaultAction {
    match crate::kernel::vm::active_vcpu::with(|execution, _| {
        let vm = execution
            .vm_binding()
            .ok_or(crate::kernel::vm::memory::Error::Registry(
                crate::kernel::vm::registry::Error::NotInstalled,
            ))?;
        crate::kernel::vm::memory::resolve_guest_memory_fault(vm, fault)
    }) {
        Ok(Some(Ok(true))) => MemoryFaultAction::Retry,
        Ok(Some(Ok(false))) => MemoryFaultAction::ForwardToDevice,
        Ok(Some(Err(error))) => {
            crate::pr_err!(
                "HypeR: guest memory fault resolution failed at {:#x} ({:?}, guest-page-walk={}): {error:?}",
                fault.address().get(),
                fault.access(),
                fault.during_guest_page_walk()
            );
            MemoryFaultAction::Stop
        }
        Ok(None) => {
            crate::pr_err!("HypeR: guest memory fault arrived without an active vCPU");
            MemoryFaultAction::Stop
        }
        Err(error) => {
            crate::pr_err!("HypeR: invalid guest memory-fault entry context: {error:?}");
            MemoryFaultAction::Stop
        }
    }
}

/// Dispatches one owned `AArch64` MMIO operation.
#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn dispatch_mmio(access: hyper::vm::exit::MmioAccess) -> hyper::vm::exit::MmioAction {
    match crate::kernel::vm::active_vcpu::with(|execution, interrupts| {
        crate::kernel::vm::device::dispatch_mmio(execution, interrupts, access)
    }) {
        Ok(Some(action)) => action,
        Ok(None) | Err(_) => hyper::vm::exit::MmioAction::Stop,
    }
}

/// Resolves an owned backend-specific synchronous exit.
///
/// The selected exit/action types contain only copied values. Backend machine
/// context is accessed through the already-published active vCPU for exactly
/// this callback and cannot escape it.
#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) fn dispatch_guest_sync(
    exit: crate::hal::vm::GuestSyncExit,
) -> crate::hal::vm::GuestSyncAction {
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
        Ok(None) | Err(_) => crate::hal::vm::GuestSyncAction::Stop,
    }
}

/// Dispatches one owned scalar x86 port-I/O operation.
#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn dispatch_port_io(
    exit: hyper::vm::x86::exit::PortIoExit,
) -> hyper::vm::x86::exit::PortIoAction {
    use hyper::vm::x86::exit::{PortIoAction, PortIoOperation};

    match crate::kernel::vm::device::access_port(exit) {
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

/// Selects one x86 virtual interrupt for the next guest-entry transaction.
#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn query_pending_interrupt(
    timer_pending: bool,
) -> hyper::vm::x86::exit::PendingInterruptAction {
    use hyper::vm::x86::device::legacy_pc::InterruptSource;
    use hyper::vm::x86::exit::PendingInterruptAction;

    match crate::kernel::vm::device::pending_interrupt(timer_pending) {
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
