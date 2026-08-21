// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest-exit entry adapter.
//!
//! This is the sole upward call from architecture guest-exit mechanisms into
//! kernel VM policy. The current `GuestSyncFrame` argument is a migration
//! adapter: individual exit classes must replace it with owned events from
//! `hyper::vm::exit`, after which this raw-frame entry point will be removed.

use hyper::vm::exit::GuestMemoryFault;

/// Kernel disposition for an owned guest-memory-fault event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub(crate) enum MemoryFaultAction {
    /// Re-enter the guest at the faulting instruction after mapping memory.
    Retry,
    /// Continue backend-local decoding, normally to identify an MMIO access.
    Forward,
    /// Enter the architecture fail-stop path for an invalid active context or
    /// a memory-policy failure.
    Stop,
}

/// Dispatches one owned guest-memory fault without exposing its raw frame.
///
/// The architecture caller keeps local interrupts masked and must have
/// published the current vCPU binding before guest entry. This path takes the
/// active-vCPU and VM address-space locks; it must not be called while either
/// lock is already held. The owned event may outlive the raw frame, but no
/// reference to that frame crosses this boundary.
///
/// The common path performs one active-vCPU lookup. Resolving demand-zero RAM
/// may allocate pages and update the active second-stage translation tables;
/// failure reporting may take the kernel log lock. Forwarding performs no
/// allocation and drops the active borrow before the backend continues
/// decoding the raw frame.
#[allow(dead_code)]
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
        Ok(Some(Ok(false))) => MemoryFaultAction::Forward,
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

/// Dispatches a legacy architecture guest-synchronous frame.
///
/// Guest entry owns the raw frame and keeps local interrupts masked. Kernel
/// policy borrows it only for this call and cannot retain the frame. Dispatch
/// may enter the explicit guest-memory slow path, which can allocate pages and
/// update active second-stage translation tables.
pub(crate) fn dispatch_legacy(frame: &mut crate::arch::vm::LegacySyncFrame<'_>) -> bool {
    crate::kernel::vm::handle_guest_sync(frame)
}

/// Continues legacy backend decoding after typed memory policy forwarded a
/// non-RAM access. The memory event is not dispatched a second time.
#[allow(dead_code)]
pub(crate) fn dispatch_legacy_after_memory_fault(
    frame: &mut crate::arch::vm::LegacySyncFrame<'_>,
) -> bool {
    crate::kernel::vm::handle_guest_sync_after_memory_fault(frame)
}

/// Dispatches one x86 port access through the active VM's device set.
#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn dispatch_port_io(
    port: u16,
    size: usize,
    write: bool,
    value: u32,
) -> Result<Option<u32>, crate::kernel::vm::device::Error> {
    crate::kernel::vm::device::access_port(port, size, write, value)
}

/// Queries the active VM's legacy interrupt router after host timer polling.
#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn pending_legacy_interrupt(
    timer_pending: bool,
) -> Result<
    Option<hyper::vm::x86::device::legacy_pc::PendingInterrupt>,
    crate::kernel::vm::device::Error,
> {
    crate::kernel::vm::device::pending_interrupt(timer_pending)
}
