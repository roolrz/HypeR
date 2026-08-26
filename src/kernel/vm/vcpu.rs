// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! vCPU interrupt-interface activation and reconciliation.

use core::convert::Infallible;

pub(super) fn create_thread(
    vm: super::registry::VmBinding,
    vcpu_id: u32,
    context: crate::hal::vm::VcpuContext,
) -> Result<crate::kernel::task::scheduler::DormantVcpuThread, crate::kernel::task::scheduler::Error>
{
    crate::kernel::task::scheduler::vcpu_create("vcpu/0", vm, vcpu_id, context, thread_entry)
}

extern "C" fn thread_entry(_argument: usize) {
    run_current()
}

fn run_current() -> ! {
    match try_run_current() {
        Ok(never) => match never {},
        Err(error) => crate::kernel::boot::fail("vCPU execution startup", error),
    }
}

fn try_run_current() -> Result<Infallible, RunError> {
    crate::hal::irq::mask_local();
    let current = crate::kernel::task::scheduler::current_vcpu().map_err(RunError::Scheduler)?;
    let stack_marker = 0usize;
    let stack_pointer = (&stack_marker as *const usize) as usize;
    if stack_pointer < current.stack.0 || stack_pointer >= current.stack.1 {
        return Err(RunError::InvalidStack(current.stack));
    }
    let execution = current.execution;
    if execution.is_null() || !execution.is_aligned() {
        return Err(RunError::InvalidExecution);
    }
    // SAFETY: The scheduler-origin pointer identifies its pinned current vCPU.
    // The binding pointer targets a field in that pinned allocation and is
    // converted to a scoped reference only before active publication below.
    let (virtual_machine, vcpu_id) = unsafe {
        let execution_ref = &*execution;
        let vm = execution_ref
            .vm_binding()
            .ok_or(RunError::MissingVmBinding)?;
        (vm.id(), execution_ref.vcpu_id)
    };
    crate::println!(
        "HypeR: vCPU {} running as scheduler thread {} on guarded stack {:#x}-{:#x}; VM {:?}",
        vcpu_id,
        current.thread.get(),
        current.stack.0,
        current.stack.1,
        virtual_machine
    );
    // SAFETY: This current scheduler Thread exclusively owns the stopped vCPU,
    // the installed VM aggregate is pinned, and local interrupts remain masked.
    unsafe {
        activate(execution).map_err(RunError::VirtualHardware)?;
        crate::hal::vm::prepare_interrupts_for_entry();
        // No Rust reference to VcpuExecution or VcpuContext is live here. VM
        // exits and asynchronous guest exceptions may therefore reconstruct
        // short-lived exclusive references from the published raw owner token.
        let hardware = core::ptr::addr_of_mut!((*execution).hardware);
        crate::hal::vm::enter(hardware)
    }
}

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
    {
        // SAFETY: The caller supplies the valid, pinned, exclusive pointer.
        let execution = unsafe { &mut *execution };
        let cpu =
            crate::kernel::cpu::current_index().ok_or(HardwareTransitionError::InvalidExecution)?;
        let claimed = execution
            .claim_execution(cpu)
            .map_err(HardwareTransitionError::Execution)?;
        if claimed {
            let Some(binding) = execution.vm_binding() else {
                release_execution_or_fail(execution);
                return Err(HardwareTransitionError::InvalidExecution);
            };
            // SAFETY: The exclusive execution claim above proves that no other
            // CPU can execute this VM while the current CPU selects its
            // mapping epoch. The stopped vCPU and masked IRQ requirements are
            // inherited from this function.
            if let Err(error) = unsafe { super::memory::activate(binding) } {
                release_execution_or_fail(execution);
                return Err(HardwareTransitionError::Memory(error));
            }
        }
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
                release_execution_or_fail(execution);
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
            return match rollback {
                Ok(()) => {
                    release_execution_or_fail(execution);
                    Err(HardwareTransitionError::Timer(timer))
                }
                Err(hardware) => fatal_ambiguous_hardware(
                    "timer rollback could not detach vCPU hardware",
                    hardware,
                ),
            };
        }
    }
    // SAFETY: All temporary references ended; the scheduler-origin pointer
    // remains pinned and exclusive for the active run.
    if let Err(publication) = unsafe { super::active_vcpu::set_raw(execution) } {
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
                    release_execution_or_fail(execution);
                    Err(HardwareTransitionError::Active(publication))
                }
                Err(timer) => {
                    release_execution_or_fail(execution);
                    Err(HardwareTransitionError::TimerRollbackAfterPublication {
                        publication,
                        timer,
                    })
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
    super::active_vcpu::clear(execution).map_err(HardwareTransitionError::Active)?;
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
    release_execution_or_fail(execution);
    super::timer::set_host_timer_enabled(true).map_err(HardwareTransitionError::Timer)
}

fn release_execution_or_fail(execution: &mut crate::kernel::task::thread::VcpuExecution) {
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: vCPU execution capability release has no registered CPU"
        ));
    };
    if let Err(error) = execution.release_execution(cpu) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: vCPU execution capability release failed: {error:?}"
        ));
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
    Execution(hyper::vm::translation::ExecutionError),
    InvalidExecution,
    Memory(super::memory::Error),
    Timer(super::timer::Error),
    TimerRollbackAfterPublication {
        publication: super::active_vcpu::Error,
        timer: super::timer::Error,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunError {
    InvalidExecution,
    MissingVmBinding,
    InvalidStack((usize, usize)),
    Memory(super::memory::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
    VirtualHardware(HardwareTransitionError),
}

pub use crate::hal::vm::VcpuInterruptError;
