// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Registered upward policy services for architecture-owned guest entry.
//!
//! Architecture code copies fixed-width events out of private machine frames,
//! invokes one of these callbacks, and applies the returned typed action only
//! after the callback has returned. No raw frame, VMCS/VMCB borrow, or backend
//! completion reference crosses this boundary.

use hyper::vm::exit::{GuestMemoryFault, MemoryFaultAction};

mod selected;

/// Builds the selected architecture's immutable upward VM-exit service table.
///
/// Target-specific event types remain inside the architecture/HAL boundary;
/// VM initialization receives only the opaque completed table.
pub(crate) const fn services() -> crate::hal::vm::ExitServices {
    selected::services()
}

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
        Ok(Some(Err(_error))) => {
            // The architecture captures fixed exit facts before terminal
            // unwind. Ordinary logging waits until local hardware detaches.
            MemoryFaultAction::Stop
        }
        Ok(None) => crate::kernel::crash::fatal(format_args!(
            "HypeR: guest memory fault arrived without an active vCPU"
        )),
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: invalid guest memory-fault entry context: {error:?}"
        )),
    }
}
