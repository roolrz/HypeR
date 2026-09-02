// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! vCPU hardware activation, reconciliation, and exact teardown transactions.

/// Claims VM execution, selects the current mapping epoch, activates local
/// hardware, then publishes the pinned execution for exception callbacks.
/// Publication is last so callbacks cannot observe partial state.
///
/// # Safety
///
/// `execution` must be the scheduler-origin, non-null, aligned, pinned, and
/// exclusively owned current-vCPU pointer. Local interrupts must be masked.
pub(crate) unsafe fn activate(
    execution: *mut crate::kernel::task::thread::VcpuExecution,
) -> Result<(), HardwareTransitionError> {
    if execution.is_null() || !execution.is_aligned() {
        return Err(HardwareTransitionError::InvalidExecution);
    }
    let mut execution_claim;
    let consumed_reconcile;
    {
        // SAFETY: The caller supplies the valid, pinned, exclusive pointer.
        let execution = unsafe { &mut *execution };
        let cpu =
            crate::kernel::cpu::current_index().ok_or(HardwareTransitionError::InvalidExecution)?;
        execution_claim = claim_execution(execution, cpu)?;
        if let Some(claim) = execution_claim.as_mut() {
            let Some(binding) = execution.vm_binding() else {
                release_execution_or_fail(execution, execution_claim);
                return Err(HardwareTransitionError::InvalidExecution);
            };
            // SAFETY: The exclusive execution claim above proves that no other
            // CPU can execute this VM while the current CPU selects its
            // mapping epoch. The stopped vCPU and masked IRQ requirements are
            // inherited from this function.
            let residency = match unsafe { super::memory::activate(binding) } {
                Ok(residency) => residency,
                Err(error) => {
                    release_execution_or_fail(execution, execution_claim);
                    return Err(HardwareTransitionError::Memory(error));
                }
            };
            // The residency capability becomes part of the exact composite VM
            // execution claim before any later fallible activation step.
            if let Err(_residency) = claim.attach_residency(residency) {
                crate::hal::cpu::halt()
            }
        }
        // Clear publication before acquiring the controller lock. A producer
        // which mutates after this exchange publishes a new bit that survives
        // this activation and prompts the scheduler-authoritative running CPU.
        let reconcile_claimed = match execution.vm_binding() {
            Some(binding) => match binding.take_interrupt_reconcile(execution.vcpu_id) {
                Ok(claimed) => claimed,
                Err(error) => {
                    release_execution_or_fail(execution, execution_claim);
                    return Err(HardwareTransitionError::Registry(error));
                }
            },
            None => false,
        };
        consumed_reconcile = reconcile_claimed;
        let interrupts = core::ptr::from_ref(execution.interrupts());
        // SAFETY: The VM binding keeps the controller fixed and live; this
        // reference ends before raw active-vCPU publication.
        let interrupts = unsafe { &*interrupts };
        // SAFETY: The caller owns this stopped vCPU with IRQs masked.
        let timer_asserted = match unsafe {
            crate::hal::vm::activate_hardware(
                &mut execution.hardware,
                execution.vcpu_id,
                interrupts,
                crate::kernel::time::monotonic_ticks(),
            )
        } {
            Ok(asserted) => asserted,
            Err(error) => {
                restore_reconcile_if_claimed(execution, reconcile_claimed);
                release_execution_or_fail(execution, execution_claim);
                return Err(HardwareTransitionError::Hardware(error));
            }
        };
        if let Err(timer) = super::timer::set_host_timer_enabled(!timer_asserted) {
            // SAFETY: Publication has not occurred, hardware is active, and
            // the caller still owns this stopped execution with IRQs masked.
            let rollback = unsafe {
                crate::hal::vm::deactivate_hardware(
                    &mut execution.hardware,
                    execution.vcpu_id,
                    interrupts,
                    crate::kernel::time::monotonic_ticks(),
                )
            };
            match rollback {
                Ok(()) => {
                    restore_reconcile_if_claimed(execution, reconcile_claimed);
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: vCPU activation could not program host timer: {timer:?}"
                    ));
                }
                Err(hardware) => fatal_ambiguous_hardware(
                    "timer rollback could not detach vCPU hardware",
                    hardware,
                ),
            };
        }
    }
    // Close the first controller-refill window before local callback
    // publication. A producer racing after this claim observes the scheduler's
    // already-Running Thread, sends a guest-exit prompt for its durable VM
    // work, and leaves that work set until IRQ-tail reactivation.
    let final_reconcile = {
        // SAFETY: Hardware is active but unpublished, and this scheduler-owned
        // pointer remains exclusively owned with IRQs masked.
        let execution = unsafe { &mut *execution };
        match execution.vm_binding() {
            Some(binding) => match binding.take_interrupt_reconcile(execution.vcpu_id) {
                Ok(claimed) => claimed,
                Err(error) => {
                    rollback_unpublished_activation(execution, execution_claim, consumed_reconcile);
                    return Err(HardwareTransitionError::Registry(error));
                }
            },
            None => false,
        }
    };
    if final_reconcile {
        // SAFETY: Hardware is active locally, guest execution has not begun,
        // and this scheduler-owned pointer remains exclusive with IRQs masked.
        let execution_ref = unsafe { &mut *execution };
        let interrupts = core::ptr::from_ref(execution_ref.interrupts());
        // SAFETY: The strong VM binding retains the fixed controller.
        let interrupts = unsafe { &*interrupts };
        if let Err(error) = crate::hal::vm::reconcile_active_interrupts(
            &mut execution_ref.hardware,
            execution_ref.vcpu_id,
            interrupts,
        ) {
            restore_reconcile_if_claimed(execution_ref, true);
            rollback_unpublished_activation(execution_ref, execution_claim, consumed_reconcile);
            return Err(HardwareTransitionError::Reconcile(error));
        }
    }
    // SAFETY: All temporary references ended; the scheduler-origin pointer
    // remains pinned and exclusive for the active run.
    if let Err(publication) = unsafe { super::active_vcpu::set_raw(execution, execution_claim) } {
        let publication_error = publication.error();
        execution_claim = publication.into_claim();
        // SAFETY: Publication failed, so no callback can borrow this execution.
        let execution = unsafe { &mut *execution };
        let interrupts = core::ptr::from_ref(execution.interrupts());
        // SAFETY: The VM binding retains the controller through rollback.
        let interrupts = unsafe { &*interrupts };
        // SAFETY: Hardware was activated above and IRQs remain masked.
        let rollback = unsafe {
            crate::hal::vm::deactivate_hardware(
                &mut execution.hardware,
                execution.vcpu_id,
                interrupts,
                crate::kernel::time::monotonic_ticks(),
            )
        };
        return match rollback {
            Ok(()) => match super::timer::set_host_timer_enabled(true) {
                Ok(()) => {
                    restore_reconcile_if_claimed(execution, consumed_reconcile || final_reconcile);
                    release_execution_or_fail(execution, execution_claim);
                    Err(HardwareTransitionError::Active(publication_error))
                }
                Err(timer) => {
                    restore_reconcile_if_claimed(execution, consumed_reconcile || final_reconcile);
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: publication rollback restored hardware but not host timer: publication={publication_error:?}, timer={timer:?}"
                    ));
                }
            },
            Err(hardware) => fatal_ambiguous_hardware(
                "publication rollback could not detach vCPU hardware",
                hardware,
            ),
        };
    }
    Ok(())
}

/// Removes active publication before saving and detaching local hardware.
///
/// # Safety
///
/// `execution` must exclusively own the active local vCPU and local interrupts
/// must remain masked throughout this transaction.
pub(crate) unsafe fn deactivate(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
) -> Result<(), HardwareTransitionError> {
    let claim = super::active_vcpu::clear(execution).map_err(HardwareTransitionError::Active)?;
    let interrupts = core::ptr::from_ref(execution.interrupts());
    // SAFETY: The retained VM binding keeps the controller fixed and live.
    let interrupts = unsafe { &*interrupts };
    // SAFETY: Publication is gone, guest execution stopped, and IRQs are masked.
    if let Err(error) = unsafe {
        crate::hal::vm::deactivate_hardware(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
            crate::kernel::time::monotonic_ticks(),
        )
    } {
        // Active callback publication is already gone, but local hardware may
        // still refer to this execution. Retain its claim and stop globally.
        fatal_ambiguous_hardware("could not detach active vCPU hardware", error);
    }
    if let Err(error) = super::timer::set_host_timer_enabled(true) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: active vCPU detach could not restore host timer: {error:?}"
        ));
    }
    release_execution_or_fail(execution, claim);
    Ok(())
}

/// Detaches a vector-stopped vCPU and consumes its linear proof.
///
/// # Safety
///
/// `execution` must be the exact active local vCPU represented by `stopped`.
/// Local interrupts must remain masked throughout the transaction.
pub(super) unsafe fn detach_stopped(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    stopped: crate::hal::vm::StoppedVcpuRun,
) -> DetachedVcpuExecution {
    let claim = match super::active_vcpu::clear(execution) {
        Ok(claim) => claim,
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: stopped vCPU callback publication could not be cleared: {error:?}"
        )),
    };
    let interrupts = core::ptr::from_ref(execution.interrupts());
    // SAFETY: The retained VM binding keeps the controller fixed and live.
    let interrupts = unsafe { &*interrupts };
    // SAFETY: Vector capture closed lower-world ownership and returned the
    // exact linear stopped proof while this vCPU and its IRQ mask stayed local.
    if let Err(failure) = unsafe {
        crate::hal::vm::deactivate_stopped_hardware(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
            crate::kernel::time::monotonic_ticks(),
            stopped,
        )
    } {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: could not detach stopped vCPU hardware: {:?}",
            failure.error()
        ));
    }
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: stopped vCPU detach lost its registered CPU"
        ));
    };
    DetachedVcpuExecution {
        execution: core::ptr::NonNull::from(execution),
        claim,
        cpu,
        armed: true,
        not_send_or_sync: core::marker::PhantomData,
    }
}

/// Linear interval after stopped hardware detach but before claim release.
#[must_use = "stopped-vCPU policy must complete before execution release"]
pub(super) struct DetachedVcpuExecution {
    execution: core::ptr::NonNull<crate::kernel::task::thread::VcpuExecution>,
    claim: Option<super::registry::VmExecutionClaim>,
    cpu: hyper::cpu::CpuIndex,
    armed: bool,
    not_send_or_sync: core::marker::PhantomData<alloc::rc::Rc<()>>,
}

impl DetachedVcpuExecution {
    /// Borrows only the installed VM binding while its execution claim is held.
    pub(super) fn vm_binding(&self) -> Option<&super::registry::VmBinding> {
        // SAFETY: Construction retained the pinned execution and the armed
        // token prevents claim release or payload destruction during borrow.
        unsafe { self.execution.as_ref() }.vm_binding()
    }

    pub(super) fn wfi_state(
        &self,
    ) -> Result<crate::hal::vm::VcpuWfiState, crate::hal::vm::StoppedVcpuQueryError> {
        // SAFETY: The armed token retains exclusive ownership of this stopped
        // execution and its hardware state until `finish` consumes the token.
        let execution = unsafe { self.execution.as_ref() };
        crate::hal::vm::stopped_wfi_state(
            &execution.hardware,
            execution.vcpu_id,
            execution.interrupts(),
            crate::kernel::time::monotonic_ticks(),
        )
    }

    /// Restores host timing and releases the exact retained execution claim.
    pub(super) fn finish(mut self) {
        if crate::kernel::cpu::current_index() != Some(self.cpu) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: detached vCPU migrated before claim release"
            ));
        }
        if let Err(error) = super::timer::set_host_timer_enabled(true) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: stopped vCPU detach could not restore host timer: {error:?}"
            ));
        }
        // SAFETY: CPU identity matched and this armed token retains the exact
        // exclusive execution pointer until release completes.
        let execution = unsafe { self.execution.as_mut() };
        release_execution_or_fail(execution, self.claim.take());
        self.armed = false;
    }
}

impl Drop for DetachedVcpuExecution {
    fn drop(&mut self) {
        if self.armed {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: detached vCPU proof was dropped before claim release"
            ));
        }
    }
}

fn release_execution_or_fail(
    execution: &crate::kernel::task::thread::VcpuExecution,
    mut claim: Option<super::registry::VmExecutionClaim>,
) {
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: vCPU execution capability release has no registered CPU"
        ));
    };
    let Some(mut claim) = claim.take() else {
        if execution.vm_binding().is_some() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: installed vCPU lost its active execution capability"
            ));
        }
        return;
    };
    let Some(binding) = execution.vm_binding() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: active execution capability lost its VM binding"
        ));
    };
    if let Some(residency) = claim.take_residency()
        && let Err(failure) = super::memory::leave(binding, residency)
    {
        let error = failure.error();
        claim.restore_residency(failure.into_claim());
        crate::kernel::crash::fatal(format_args!(
            "HypeR: guest residency capability release failed: {error:?}"
        ));
    }
    if let Err(failure) = binding.release_execution(claim, cpu) {
        let error = failure.error();
        let _retained_claim = failure.into_claim();
        crate::kernel::crash::fatal(format_args!(
            "HypeR: VM execution capability release failed: {error:?}"
        ));
    }
}

fn restore_reconcile_if_claimed(
    execution: &crate::kernel::task::thread::VcpuExecution,
    claimed: bool,
) {
    if !claimed {
        return;
    }
    let Some(binding) = execution.vm_binding() else {
        crate::hal::cpu::halt()
    };
    if binding
        .restore_interrupt_reconcile(execution.vcpu_id)
        .is_err()
    {
        crate::hal::cpu::halt()
    }
}

fn rollback_unpublished_activation(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    claim: Option<super::registry::VmExecutionClaim>,
    restore_reconcile: bool,
) {
    let interrupts = core::ptr::from_ref(execution.interrupts());
    // SAFETY: The strong VM binding retains this fixed controller through the
    // rollback transaction.
    let interrupts = unsafe { &*interrupts };
    // SAFETY: Hardware is active but callback publication and guest execution
    // have not begun; local IRQs remain masked.
    if let Err(error) = unsafe {
        crate::hal::vm::deactivate_hardware(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
            crate::kernel::time::monotonic_ticks(),
        )
    } {
        fatal_ambiguous_hardware(
            "unpublished activation rollback could not detach hardware",
            error,
        )
    }
    restore_reconcile_if_claimed(execution, restore_reconcile);
    if let Err(error) = super::timer::set_host_timer_enabled(true) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: unpublished activation rollback could not restore host timer: {error:?}"
        ));
    }
    release_execution_or_fail(execution, claim);
}

fn claim_execution(
    execution: &crate::kernel::task::thread::VcpuExecution,
    cpu: hyper::cpu::CpuIndex,
) -> Result<Option<super::registry::VmExecutionClaim>, HardwareTransitionError> {
    match execution.vm_binding() {
        Some(binding) => binding
            .claim_execution(cpu)
            .map(Some)
            .map_err(HardwareTransitionError::Execution),
        None => Ok(None),
    }
}

fn fatal_ambiguous_hardware(
    operation: &'static str,
    error: crate::hal::vm::VcpuInterruptError,
) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {operation}: {error:?}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareTransitionError {
    Active(super::active_vcpu::Error),
    Hardware(crate::hal::vm::VcpuInterruptError),
    Execution(super::registry::VmExecutionError),
    InvalidExecution,
    Memory(super::memory::Error),
    Reconcile(crate::hal::vm::ActiveInterruptReconcileError),
    Registry(super::registry::Error),
}

#[allow(dead_code)]
pub(crate) fn current_interrupt_reconcile_pending() -> Result<bool, super::ReconcileObservationError>
{
    match super::active_vcpu::with(|execution, _| {
        let Some(binding) = execution.vm_binding() else {
            return Ok(false);
        };
        binding.interrupt_reconcile_pending(execution.vcpu_id)
    }) {
        Ok(Some(Ok(pending))) => Ok(pending),
        Ok(Some(Err(error))) => Err(super::ReconcileObservationError::Registry(error)),
        Ok(None) => Ok(false),
        Err(error) => Err(super::ReconcileObservationError::Active(error)),
    }
}

#[allow(dead_code)]
pub(crate) fn current_administrative_stop_requested()
-> Result<bool, super::ReconcileObservationError> {
    let current = crate::kernel::task::scheduler::current_vcpu_if_present()
        .map_err(super::ReconcileObservationError::Scheduler)?;
    let Some(current) = current else {
        return Ok(false);
    };
    match super::active_vcpu::with(|execution, _| {
        let Some(binding) = execution.vm_binding() else {
            return Ok(false);
        };
        binding
            .administrative_stop_requested(execution.vcpu_id, current.thread)
            .map(|reason| reason.is_some())
    }) {
        Ok(Some(Ok(pending))) => Ok(pending),
        Ok(Some(Err(error))) => Err(super::ReconcileObservationError::Registry(error)),
        Ok(None) => Ok(false),
        Err(error) => Err(super::ReconcileObservationError::Active(error)),
    }
}
