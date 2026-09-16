// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-owned continuation of a userspace-emulated MMIO instruction.

use super::VcpuExecution;

/// The raw payload is scheduler-pinned and hardware-detached for this entire
/// continuation, just as for WFI and PSCI waits. No raw-frame borrow survives.
pub(super) unsafe fn wait(
    execution: *mut VcpuExecution,
    thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    loop {
        if let Some(reason) = super::runner::administrative_stop_reason(execution, thread) {
            super::runner::finish_inactive_administrative_stop(execution, thread, reason);
            return false;
        }
        // SAFETY: The scheduler exclusively owns this pinned, detached payload.
        let current = unsafe { &mut *execution };
        let Some(binding) = current.vm_binding() else {
            hyper::debug::invariant_failure(format_args!("vm::vcpu::mmio::wait invariant"));
        };
        let ticket = binding
            .wfi_wait_ticket(current.vcpu_id)
            .unwrap_or_else(|_| {
                hyper::debug::invariant_failure(format_args!("vm::vcpu::mmio::wait invariant"))
            });
        let lifecycle = binding.lifecycle();
        match lifecycle.take_mmio_completion(current.vcpu_id) {
            Ok(Some(hyper::vm::exit::MmioAction::Stop)) => {
                crate::kernel::vm::installed::InstalledMachine::request_stop(&lifecycle);
            }
            Ok(Some(action)) => {
                if let Err(error) =
                    crate::hal::vm::complete_mmio_call(&mut current.hardware, action)
                {
                    crate::kernel::crash::fatal(format_args!(
                        "invalid detached MMIO completion: {error:?}"
                    ));
                }
                return true;
            }
            Ok(None) => {}
            Err(_) => {
                // Administrative stop may have cancelled the slot after the
                // loop's first observation; the published endpoint stop wins.
                if super::runner::administrative_stop_reason(execution, thread).is_some() {
                    continue;
                }
                crate::kernel::crash::fatal(format_args!(
                    "MMIO continuation lost its pending request"
                ));
            }
        }
        match binding.prepare_wfi_wait(current.vcpu_id, ticket) {
            Ok(crate::kernel::vm::endpoint::PreparedWait::Park(park)) => {
                let _ = park.complete();
            }
            Ok(_) => {}
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "MMIO continuation park failed: {error:?}"
            )),
        }
    }
}
