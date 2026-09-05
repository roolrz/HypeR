// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fixed scheduler entry, wait, and terminal lifecycle policy for vCPU Threads.

pub(crate) fn create_thread(
    vm: super::registry::VmBinding,
    vcpu_id: u32,
    context: crate::hal::vm::VcpuContext,
) -> Result<crate::kernel::task::scheduler::DormantVcpuThread, crate::kernel::task::scheduler::Error>
{
    let Some(entry_ready) = crate::kernel::vm::entry_ready() else {
        return Err(crate::kernel::task::scheduler::Error::VmEntryUnavailable);
    };
    crate::kernel::task::scheduler::vcpu_create(
        "vcpu/0",
        vm,
        vcpu_id,
        context,
        &entry_ready,
        thread_entry,
    )
}

extern "C" fn thread_entry(_argument: usize) {
    run_current()
}

/// Runs one vCPU across typed wait exits until terminal disposition.
fn run_current() {
    // ThreadContext owns this mask across every switch and migration. A
    // CPU-affine lexical guard must never span the migratable guest run.
    crate::hal::irq::mask_local();
    let current = match crate::kernel::task::scheduler::current_vcpu() {
        Ok(current) => current,
        Err(error) => fail_start(RunError::Scheduler(error)),
    };
    validate_stack(current);
    let execution = current.execution;
    if execution.is_null() || !execution.is_aligned() {
        fail_start(RunError::InvalidExecution);
    }
    // SAFETY: The scheduler-origin pointer is pinned and exclusively owned.
    let execution_ref = unsafe { &*execution };
    let Some(binding) = execution_ref.vm_binding() else {
        fail_start(RunError::MissingVmBinding);
    };
    if execution_ref.terminal_mmio_report_pending() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: vCPU {} entered with a stale terminal MMIO report",
            execution_ref.vcpu_id
        ));
    }
    let virtual_machine = binding.id();
    let vcpu_id = execution_ref.vcpu_id;
    report_start(current, virtual_machine, vcpu_id);

    loop {
        // Administrative ownership is checked before every activation. A
        // Dormant/Ready or WFI-resumed vCPU therefore never enters hardware
        // after its endpoint accepted a stop request.
        if let Some(reason) = administrative_stop_reason(execution, current.thread) {
            finish_inactive_administrative_stop(execution, current.thread, reason);
            return;
        }
        // SAFETY: No reference survives into guest entry. The pointer stays
        // pinned, exclusive, and IRQ-masked through run and exact detachment.
        unsafe {
            if let Err(error) = super::activate(execution) {
                if error
                    == super::transition::HardwareTransitionError::Execution(
                        crate::kernel::vm::registry::VmExecutionError::AdmissionClosed,
                    )
                {
                    let Some(reason) = administrative_stop_reason(execution, current.thread) else {
                        crate::kernel::crash::fatal(format_args!(
                            "HypeR: VM run admission closed without an exact vCPU stop request"
                        ));
                    };
                    finish_inactive_administrative_stop(execution, current.thread, reason);
                    return;
                }
                fail_start(RunError::VirtualHardware(error));
            }
            crate::hal::vm::prepare_interrupts_for_entry();
            let hardware = core::ptr::addr_of_mut!((*execution).hardware);
            let stopped = match crate::hal::vm::run(hardware) {
                Ok(stopped) => stopped,
                Err(error) => crate::kernel::crash::fatal(format_args!(
                    "HypeR: active vCPU run returned invalid terminal ownership: {error:?}"
                )),
            };
            let exit = stopped.exit();
            let detached = super::transition::detach_stopped(&mut *execution, stopped);
            match exit.disposition() {
                crate::hal::vm::VcpuRunDisposition::Wait(
                    crate::hal::vm::VcpuWaitReason::Interrupt,
                ) => {
                    let Some(binding) = detached.vm_binding() else {
                        crate::kernel::crash::fatal(format_args!(
                            "HypeR: waiting vCPU lost its installed VM binding"
                        ));
                    };
                    let ticket = match binding.wfi_wait_ticket(vcpu_id) {
                        Ok(ticket) => ticket,
                        Err(error) => crate::kernel::crash::fatal(format_args!(
                            "HypeR: waiting vCPU could not snapshot its endpoint: {error:?}"
                        )),
                    };
                    let state = match detached.wfi_state() {
                        Ok(state) => state,
                        Err(error) => crate::kernel::crash::fatal(format_args!(
                            "HypeR: waiting vCPU state query failed: {error:?}"
                        )),
                    };
                    detached.finish();
                    if state.interrupt_may_wake
                        || state.timer == crate::hal::vm::VcpuTimerWake::PendingNow
                    {
                        continue;
                    }
                    // SAFETY: token completion released the execution claim, but
                    // the scheduler still owns and pins this current execution.
                    let execution_ref = &*execution;
                    let Some(binding) = execution_ref.vm_binding() else {
                        crate::kernel::crash::fatal(format_args!(
                            "HypeR: waiting vCPU lost its binding before park"
                        ));
                    };
                    let timer = match state.timer {
                        crate::hal::vm::VcpuTimerWake::Deadline(deadline) => {
                            match binding.arm_wfi_timer(vcpu_id, deadline) {
                                Ok(timer) => Some(timer),
                                Err(error) => crate::kernel::crash::fatal(format_args!(
                                    "HypeR: waiting vCPU timer arm failed: {error:?}"
                                )),
                            }
                        }
                        crate::hal::vm::VcpuTimerWake::None
                        | crate::hal::vm::VcpuTimerWake::PendingNow => None,
                    };
                    match binding.prepare_wfi_wait(vcpu_id, ticket) {
                        Ok(crate::kernel::vm::endpoint::PreparedWait::Changed) => {}
                        Ok(crate::kernel::vm::endpoint::PreparedWait::Completed(outcome)) => {
                            complete_wfi_wait(outcome);
                        }
                        Ok(crate::kernel::vm::endpoint::PreparedWait::Park(park)) => {
                            complete_wfi_wait(park.complete());
                        }
                        Err(error) => crate::kernel::crash::fatal(format_args!(
                            "HypeR: waiting vCPU park failed: {error:?}"
                        )),
                    }
                    if let Some(timer) = timer
                        && let Err(error) = timer.retire()
                    {
                        crate::kernel::crash::fatal(format_args!(
                            "HypeR: waiting vCPU timer retirement failed: {error:?}"
                        ));
                    }
                    if let Some(reason) = administrative_stop_reason(execution, current.thread) {
                        finish_inactive_administrative_stop(execution, current.thread, reason);
                        return;
                    }
                    continue;
                }
                crate::hal::vm::VcpuRunDisposition::Terminal(terminal) => {
                    let reason = terminal.reason();
                    let Some(binding) = detached.vm_binding() else {
                        crate::kernel::crash::fatal(format_args!(
                            "HypeR: terminal vCPU lost its installed VM binding"
                        ));
                    };
                    let close = match binding.close_vcpu_endpoint(vcpu_id, current.thread, reason) {
                        Ok(close) => close,
                        Err(error) => crate::kernel::crash::fatal(format_args!(
                            "HypeR: terminal vCPU endpoint close failed: {error:?}"
                        )),
                    };
                    if close
                        == crate::kernel::vm::endpoint_state::GuestCloseOutcome::AdministrativeStopPending
                    {
                        finish_detached_administrative_stop(
                            execution,
                            current.thread,
                            detached,
                            crate::kernel::vm::registry::AdministrativeStopReason::Requested,
                        );
                        return;
                    }
                    detached.finish();

                    let execution_ref = &mut *execution;
                    arm_reap(
                        execution_ref,
                        current.thread,
                        crate::kernel::vm::registry::VcpuClosureReason::Guest(reason),
                    );
                    let report = execution_ref.take_terminal_mmio_report();
                    // Remain IRQ-masked until the scheduler thread-exit trampoline commits;
                    // this Thread is still classified as a vCPU until then.
                    if let Some(report) = report {
                        crate::pr_err!("{report}");
                    }
                    crate::pr_err!(
                        "HypeR: vCPU {} stopped: {}; cause={:?}, vector={:#x}, pc={:#x}, pstate={:#x}, esr={:#x}, far={:#x}",
                        vcpu_id,
                        reason,
                        terminal.cause(),
                        terminal.vector(),
                        terminal.program_counter(),
                        terminal.processor_state(),
                        terminal.syndrome(),
                        terminal.fault_address()
                    );
                    return;
                }
                crate::hal::vm::VcpuRunDisposition::AdministrativeStop(
                    crate::hal::vm::VcpuAdministrativeStopReason::Requested,
                ) => {
                    finish_detached_administrative_stop(
                        execution,
                        current.thread,
                        detached,
                        crate::kernel::vm::registry::AdministrativeStopReason::Requested,
                    );
                    return;
                }
            }
        }
    }
}

fn administrative_stop_reason(
    execution: *mut crate::kernel::task::thread::VcpuExecution,
    thread: crate::kernel::task::thread::ThreadId,
) -> Option<crate::kernel::vm::registry::AdministrativeStopReason> {
    // SAFETY: the scheduler owns and pins this inactive execution while its
    // fixed runner is current. This borrow ends before any activation.
    let execution = unsafe { &*execution };
    let Some(binding) = execution.vm_binding() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: administrative stop check lost VM binding"
        ));
    };
    match binding.administrative_stop_requested(execution.vcpu_id, thread) {
        Ok(reason) => reason,
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: administrative vCPU stop observation failed: {error:?}"
        )),
    }
}

fn finish_inactive_administrative_stop(
    execution: *mut crate::kernel::task::thread::VcpuExecution,
    thread: crate::kernel::task::thread::ThreadId,
    reason: crate::kernel::vm::registry::AdministrativeStopReason,
) {
    // SAFETY: no architecture hardware or execution claim is active at runner
    // checkpoints outside `activate`/`detach_stopped`.
    let execution = unsafe { &mut *execution };
    publish_hardware_detached_and_arm_reap(execution, thread, reason);
}

fn finish_detached_administrative_stop(
    execution: *mut crate::kernel::task::thread::VcpuExecution,
    thread: crate::kernel::task::thread::ThreadId,
    detached: super::transition::DetachedVcpuExecution,
    reason: crate::kernel::vm::registry::AdministrativeStopReason,
) {
    // HardwareDetached means every architecture transition, host-timer
    // restoration, exclusive-execution release, and run-admission release has
    // completed. StopRequested already prevents a later runner activation, so
    // publication must not overclaim quiescence while this token is armed.
    detached.finish();
    // SAFETY: token completion released architecture/execution ownership. The
    // scheduler still pins this current Thread payload until runner return.
    let execution = unsafe { &mut *execution };
    publish_hardware_detached_and_arm_reap(execution, thread, reason);
}

fn publish_hardware_detached_and_arm_reap(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    thread: crate::kernel::task::thread::ThreadId,
    reason: crate::kernel::vm::registry::AdministrativeStopReason,
) {
    let Some(binding) = execution.vm_binding() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: inactive administrative stop lost VM binding"
        ));
    };
    if let Err(error) = binding.publish_hardware_detached(execution.vcpu_id, thread, reason) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: inactive vCPU detach publication failed: {error:?}"
        ));
    }
    arm_reap(
        execution,
        thread,
        crate::kernel::vm::registry::VcpuClosureReason::Administrative(reason),
    );
}

fn arm_reap(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    thread: crate::kernel::task::thread::ThreadId,
    reason: crate::kernel::vm::registry::VcpuClosureReason,
) {
    if execution.arm_reap_publication(thread, reason).is_err() {
        crate::kernel::crash::fatal(format_args!("HypeR: vCPU reap publication was armed twice"));
    }
}

fn complete_wfi_wait(outcome: crate::kernel::task::WaitOutcome) {
    match outcome {
        crate::kernel::task::WaitOutcome::Notified => {}
        crate::kernel::task::WaitOutcome::Cancelled
        | crate::kernel::task::WaitOutcome::TimedOut => crate::kernel::crash::fatal(format_args!(
            "HypeR: WFI wait completed with unsupported outcome: {outcome:?}"
        )),
    }
}

fn validate_stack(current: crate::kernel::task::scheduler::CurrentVcpu) {
    let marker = 0usize;
    let pointer = (&marker as *const usize) as usize;
    if pointer < current.stack.0 || pointer >= current.stack.1 {
        fail_start(RunError::InvalidStack(current.stack));
    }
}

fn report_start(
    current: crate::kernel::task::scheduler::CurrentVcpu,
    virtual_machine: crate::kernel::vm::registry::VmId,
    vcpu_id: u32,
) {
    crate::pr_info!(
        "HypeR: vCPU {} running as scheduler thread {} on guarded stack {:#x}-{:#x}; VM {:?}",
        vcpu_id,
        current.thread.get(),
        current.stack.0,
        current.stack.1,
        virtual_machine
    );
}

fn fail_start(error: RunError) -> ! {
    crate::kernel::boot::fail("vCPU execution startup", error)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunError {
    InvalidExecution,
    MissingVmBinding,
    InvalidStack((usize, usize)),
    Memory(super::memory::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
    VirtualHardware(super::HardwareTransitionError),
}
