// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Physical-interrupt entry policy.

use hyper::hal::interrupt::{EntryAction as Action, InterruptId};

/// Dispatches one architecture-acknowledged interrupt.
///
/// The caller owns interrupt acknowledgement and raw-frame lifetime. This
/// adapter completes the acknowledged source exactly once before returning
/// either action. Local interrupts must remain masked. Dispatch is
/// allocation-free but may invoke registered interrupt-safe handlers under the
/// IRQ registry lock.
pub(crate) fn dispatch(
    interrupt: InterruptId,
    native_unwind: Option<unsafe extern "C" fn()>,
) -> Action {
    let irq = match crate::kernel::task::preempt::enter_irq() {
        Ok(irq) => irq,
        Err(error) => {
            crate::kernel::irq::exception::fatal_interrupt(error.description(), Some(interrupt))
        }
    };
    let action = if crate::kernel::crash::is_stop_interrupt(interrupt) {
        crate::kernel::irq::interrupt::complete(interrupt);
        Action::Stop
    } else {
        crate::kernel::irq::interrupt::dispatch(interrupt);
        Action::Resume { postlude: None }
    };
    match irq.complete() {
        Ok(true)
            if matches!(action, Action::Resume { .. })
                && native_unwind.is_some()
                && match crate::kernel::task::preempt::should_unwind_user_after_irq() {
                    Ok(pending) => pending,
                    Err(error) => crate::kernel::irq::exception::fatal_interrupt(
                        error.description(),
                        Some(interrupt),
                    ),
                } =>
        {
            Action::Resume {
                postlude: native_unwind,
            }
        }
        Ok(true)
            if matches!(action, Action::Resume { .. })
                && native_unwind.is_none()
                && match crate::kernel::task::preempt::should_reschedule_after_irq() {
                    Ok(pending) => pending,
                    Err(error) => crate::kernel::irq::exception::fatal_interrupt(
                        error.description(),
                        Some(interrupt),
                    ),
                } =>
        {
            Action::Resume {
                postlude: preemption_postlude(),
            }
        }
        Ok(_) => action,
        Err(_) => crate::kernel::irq::exception::fatal_interrupt(
            "failed to complete IRQ preemption accounting",
            Some(interrupt),
        ),
    }
}

/// Runs the outermost interrupt-exit scheduling point on a Thread-owned stack.
///
/// Local interrupts must remain masked and architecture entry must have
/// completed interrupt acknowledgement before calling this adapter. No raw
/// exception-frame reference crosses this boundary. If the interrupted Thread
/// owns an active vCPU, its local virtual hardware is unpublished and saved
/// before the scheduler commits a switch, then restored by the same suspended
/// continuation before architecture exception return.
#[cfg(CONFIG_ARCH_AARCH64)]
fn preemption_tail() {
    let current_vcpu = match crate::kernel::task::scheduler::current_vcpu_if_present() {
        Ok(current) => current,
        Err(error) => fail_preemption_tail("failed to identify interrupted Thread", error),
    };

    if let Some(current) = current_vcpu {
        // SAFETY: The scheduler supplied its pinned current-vCPU owner pointer.
        // IRQ dispatch has returned, so no active-vCPU callback borrow remains,
        // and local interrupts stay masked across unpublication and save.
        if let Err(error) = unsafe { crate::kernel::vm::vcpu::deactivate(&mut *current.execution) }
        {
            fail_vcpu_tail("failed to deactivate interrupted vCPU", error)
        }
    }

    if let Err(error) = crate::kernel::task::scheduler::cond_resched_from_irq_tail() {
        fail_preemption_tail("IRQ-tail scheduling failed", error)
    }

    if let Some(current) = current_vcpu {
        // SAFETY: This continuation can resume only when its pinned vCPU Thread
        // is current again. The preceding deactivation removed all local
        // architectural ownership, and interrupts are still masked.
        if let Err(error) = unsafe { crate::kernel::vm::vcpu::activate(current.execution) } {
            fail_vcpu_tail("failed to reactivate interrupted vCPU", error)
        }
    }
}

/// Runs the kernel-selected `AArch64` interrupt postlude.
///
/// # Safety
///
/// The caller must satisfy the postlude contract documented by
/// [`Action::Resume`].
#[cfg(CONFIG_ARCH_AARCH64)]
unsafe extern "C" fn aarch64_preemption_postlude() {
    preemption_tail();
}

#[inline]
fn preemption_postlude() -> Option<unsafe extern "C" fn()> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        Some(aarch64_preemption_postlude)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        // Secondary architectures retain the pending request until a
        // cooperative scheduling point; they do not yet provide an IRQ-tail
        // context-switch contract.
        None
    }
}

#[cfg(CONFIG_ARCH_AARCH64)]
fn fail_preemption_tail(operation: &str, error: crate::kernel::task::scheduler::Error) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {operation}: {error:?}"))
}

#[cfg(CONFIG_ARCH_AARCH64)]
fn fail_vcpu_tail(operation: &str, error: crate::kernel::vm::vcpu::HardwareTransitionError) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {operation}: {error:?}"))
}

/// Claims and dispatches one external controller interrupt, when pending.
///
/// This is the RISC-V external-interrupt entry seam. Controller claim and
/// registry dispatch are kernel-owned policy; architecture code only decodes
/// the architectural trap cause.
// Only the RISC-V trap backend uses a controller claim distinct from its
// architectural interrupt acknowledgement. Keeping target selection below
// `arch` leaves this narrow adapter intentionally unused on other targets.
#[allow(dead_code)]
pub(crate) fn claim_and_dispatch_external() -> Option<Action> {
    crate::kernel::irq::acknowledge_external().map(|interrupt| dispatch(interrupt, None))
}

/// Publishes a remote CPU's exact interrupt snapshot and stops that CPU.
pub(crate) fn stop(context: crate::hal::exception::CrashContext) -> ! {
    crate::kernel::crash::stop_this_cpu(context)
}
