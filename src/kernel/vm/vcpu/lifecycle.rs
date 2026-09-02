// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Administrative lifecycle completion for inactive vCPU continuations.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum DetachedStopError {
    InvalidExecution,
    MissingBinding,
    Registry(crate::kernel::vm::registry::Error),
    ReapAlreadyArmed,
}

/// Completes a stop which became durable while an IRQ-tail continuation was
/// detached or descheduled.
///
/// Returns `true` only after publishing `HardwareDetached` and arming exact
/// scheduler-reap publication. The caller must then terminate this Thread and
/// must never reactivate guest hardware.
///
/// # Safety
///
/// `current` must be the scheduler's newly reobserved exact current vCPU. Its
/// active-vCPU publication and architecture hardware must be detached, its VM
/// execution claim released, and local interrupts must remain masked.
#[allow(dead_code)]
pub(crate) unsafe fn complete_detached_stop_if_requested(
    current: crate::kernel::task::scheduler::CurrentVcpu,
) -> Result<bool, DetachedStopError> {
    let execution = current.execution;
    if execution.is_null() || !execution.is_aligned() {
        return Err(DetachedStopError::InvalidExecution);
    }
    // SAFETY: the caller provides the scheduler-reobserved, pinned, exclusive
    // current vCPU after complete local hardware detachment.
    let execution = unsafe { &mut *execution };
    let binding = execution
        .vm_binding()
        .ok_or(DetachedStopError::MissingBinding)?;
    let Some(reason) = binding
        .administrative_stop_requested(execution.vcpu_id, current.thread)
        .map_err(DetachedStopError::Registry)?
    else {
        return Ok(false);
    };
    binding
        .publish_hardware_detached(execution.vcpu_id, current.thread, reason)
        .map_err(DetachedStopError::Registry)?;
    execution
        .arm_reap_publication(
            current.thread,
            crate::kernel::vm::registry::VcpuClosureReason::Administrative(reason),
        )
        .map_err(|()| DetachedStopError::ReapAlreadyArmed)?;
    Ok(true)
}
