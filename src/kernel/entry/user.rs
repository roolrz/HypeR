// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native-user execution session and synchronous-call policy entry.

use core::ptr::NonNull;

use hyper::abi::native::{NativeInvocation, NativeResult};
use hyper::hal::user::{
    NativeCallAction, NativeCallHandler, NativeCallService, UserFault, UserFaultKind,
};
use hyper::sync::InterruptMaskGuard;

use crate::kernel::abi::native::{self, ImmediateServices};
use crate::kernel::capability::{HandleInfo, HandleValue, Rights};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::process::{
    AbiFamily, ExecutionRoute, Process, ProcessError, RunAdmissionError, TerminalReason,
    UserExecution, UserThread, UserThreadPhase,
};

struct ProcessServices<'process> {
    process: &'process Process,
}

impl ImmediateServices for ProcessServices<'_> {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        self.process.close_handle(value)
    }

    fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.process.duplicate_handle(value, rights)
    }

    fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.process.replace_handle(value, rights)
    }

    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        self.process.handle_info(value, required_rights)
    }

    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError> {
        self.process.copy_to_user(destination, source)
    }
}

struct NativeProcessCalls<'process> {
    process: &'process Process,
}

impl NativeProcessCalls<'_> {
    fn dispatch_immediate(&self, invocation: NativeInvocation) -> NativeResult {
        native::dispatch_immediate(
            &ProcessServices {
                process: self.process,
            },
            invocation,
        )
    }
}

// SAFETY: The current Native ABI dispatcher contains only Never-blocking
// operations. Its Process services use non-sleeping spinlocked transactions,
// fallible nonblocking allocation, and resident-page copies; none enters the
// scheduler or retains the invocation. A future blocking ABI operation must
// return NativeCallAction::Unwind before it performs any work.
unsafe impl NativeCallHandler for NativeProcessCalls<'_> {
    unsafe fn dispatch(&self, invocation: NativeInvocation) -> NativeCallAction {
        if native::is_immediate(invocation.number()) {
            NativeCallAction::Return(self.dispatch_immediate(invocation))
        } else {
            NativeCallAction::Unwind
        }
    }
}

/// Stable ownership established once for the scheduler Thread's entire entry.
struct UserSession {
    thread: UserThread,
    process: Process,
}

impl UserSession {
    fn attach(
        current: crate::kernel::task::scheduler::CurrentUser,
        pin: &crate::kernel::task::scheduler::UserRunGuard,
    ) -> (Self, NonNull<UserExecution>) {
        validate_current_stack(current.stack);
        let execution = current.execution;
        // SAFETY: `current_user` returned the scheduler-owned payload of this
        // pinned current Thread. This borrow ends before `pin` is consumed by
        // run admission; later machine runs obtain a fresh pointer and pin.
        let owner = unsafe { execution.as_ref() };
        let thread = owner.thread().clone();
        if thread.scheduler_id() != Some(current.thread) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user scheduler identity is inconsistent"
            ));
        }
        let process = thread.process().clone();
        if process.image().family() != AbiFamily::Native
            || process.image().route() != ExecutionRoute::NativeKernel
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user Thread has an invalid execution route"
            ));
        }
        let _ = pin;
        (Self { thread, process }, execution)
    }

    fn refresh_execution(
        &self,
        current: crate::kernel::task::scheduler::CurrentUser,
        pin: &crate::kernel::task::scheduler::UserRunGuard,
    ) -> NonNull<UserExecution> {
        if self.thread.scheduler_id() != Some(current.thread) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: resumed native-user scheduler identity is inconsistent"
            ));
        }
        let execution = current.execution;
        // SAFETY: `current_user` returned this payload under `pin`. Reading its
        // immutable owner identity does not enter a machine-active borrow.
        if unsafe { execution.as_ref() }.thread().scheduler_id() != Some(current.thread) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: resumed native-user execution owner is inconsistent"
            ));
        }
        let _ = pin;
        execution
    }
}

/// Fixed scheduler entry for every native user Thread.
///
/// Process construction supplies no arbitrary kernel callback. Returning from
/// this function transfers to the ordinary scheduler thread-exit trampoline.
pub(in crate::kernel) extern "C" fn thread_entry(_argument: usize) {
    run_current()
}

fn run_current() {
    let pin = acquire_pin();
    let current = match crate::kernel::task::scheduler::current_user(&pin) {
        Ok(current) => current,
        Err(error) => fail_run("failed to identify current native-user Thread", error),
    };
    let (session, execution) = UserSession::attach(current, &pin);
    run_session(&session, pin, execution)
}

fn run_session(
    session: &UserSession,
    mut pin: crate::kernel::task::scheduler::UserRunGuard,
    mut execution: NonNull<UserExecution>,
) {
    loop {
        // SAFETY: `execution` was obtained with this run's `pin`. The prepared
        // and active run tokens below consume that same pin and retain its
        // no-migration/no-reclamation guarantee until machine exit.
        let execution_owner = unsafe { execution.as_ref() };
        if execution_owner.stop_requested() {
            finish_pin(pin);
            return;
        }

        let prepared = match session
            .thread
            .prepare_run(pin, session.process.image_generation())
        {
            Ok(prepared) => prepared,
            Err((pin, RunAdmissionError::AdmissionClosed)) => {
                finish_pin(pin);
                return;
            }
            Err((_pin, error)) => fail_run("failed to reserve native-user generation", error),
        };

        // Masking closes the final pending-check-to-ERET window. Assembly
        // restores this masked state after capture; it is released before
        // address-space leave so local replacement IPIs can make progress.
        // SAFETY: The guard remains on this pinned continuation and is dropped
        // before any scheduling point.
        let interrupt_mask = unsafe { InterruptMaskGuard::<crate::hal::irq::LocalMask>::acquire() };
        let cpu = match crate::kernel::cpu::current_index() {
            Some(cpu) => cpu,
            None => crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user Thread lost its CPU identity"
            )),
        };
        match crate::kernel::task::preempt::pending(cpu) {
            Ok(true) => {
                let aborted_pin = prepared.abort();
                drop(interrupt_mask);
                finish_pin(aborted_pin);
                (pin, execution) = reacquire_execution(session);
                continue;
            }
            Ok(false) => {}
            Err(error) => fail_run("failed to inspect native-user preemption state", error),
        }
        let kernel_access = match crate::hal::user::prepare_kernel_access(&interrupt_mask) {
            Ok(access) => access,
            Err(error) => {
                let pin = prepared.abort();
                drop(interrupt_mask);
                finish_pin(pin);
                fail_run("failed to establish native-user access isolation", error);
            }
        };

        let active_run = match prepared.commit() {
            Ok(active) => active,
            Err((pin, error)) => {
                drop(interrupt_mask);
                finish_pin(pin);
                if error == RunAdmissionError::AdmissionClosed {
                    return;
                }
                fail_run("failed to publish native-user generation", error);
            }
        };
        let active_address = match execution_owner
            .address_space()
            .activate(active_run.pin(), &kernel_access)
        {
            Ok(active) => active,
            Err(error) => fail_run("failed to activate native-user address space", error),
        };
        let binding = active_run.binding();
        // SAFETY: The scheduler pin, admitted generation, and active address
        // root uniquely own the stopped context until its return capability is
        // consumed. UserExecution uses UnsafeCell solely for this seam.
        let context = unsafe { &mut *execution_owner.context_ptr() };
        let stopped = {
            // Keep the CPU-affine service wholly inside this pinned machine
            // run. Architecture exit closes its publication before returning,
            // so both borrowed values are gone before the pin can be released.
            let calls = NativeProcessCalls {
                process: &session.process,
            };
            let service = NativeCallService::new(&calls);
            active_address.run_user(context, binding, kernel_access, &service)
        };

        drop(interrupt_mask);
        let (exit, proof) = stopped.leave();
        let (stopped_run, returned_pin) = active_run.stop_after_machine_exit(proof);
        if let Err(error) = crate::kernel::task::scheduler::finish_user_run(returned_pin) {
            fail_run("failed to release native-user execution pin", error);
        }

        let stop_requested = session.thread.snapshot().phase == UserThreadPhase::StopRequested;
        match exit {
            crate::hal::user::UserExit::NativeCall {
                invocation,
                completion,
            } => {
                let result = if stop_requested {
                    completion.discard(binding)
                } else {
                    completion.complete_native(binding, dispatch_native(session, invocation))
                };
                if let Err(failure) = result {
                    fail_completion(failure);
                }
                stopped_run.acknowledge_architecture_exit();
                if stop_requested {
                    return;
                }
            }
            crate::hal::user::UserExit::Interrupted { completion } => {
                let result = if stop_requested {
                    completion.discard(binding)
                } else {
                    completion.resume_interrupted(binding)
                };
                if let Err(failure) = result {
                    fail_completion(failure);
                }
                stopped_run.acknowledge_architecture_exit();
                if stop_requested {
                    return;
                }
            }
            crate::hal::user::UserExit::Fault { fault, completion } => {
                session.process.request_stop(fault_reason(fault));
                if let Err(failure) = completion.discard(binding) {
                    fail_completion(failure);
                }
                stopped_run.acknowledge_architecture_exit();
                return;
            }
        }
        (pin, execution) = reacquire_execution(session);
    }
}

fn dispatch_native(session: &UserSession, invocation: NativeInvocation) -> NativeResult {
    NativeProcessCalls {
        process: &session.process,
    }
    .dispatch_immediate(invocation)
}

fn reacquire_execution(
    session: &UserSession,
) -> (
    crate::kernel::task::scheduler::UserRunGuard,
    NonNull<UserExecution>,
) {
    let pin = acquire_pin();
    let current = match crate::kernel::task::scheduler::current_user(&pin) {
        Ok(current) => current,
        Err(error) => fail_run("failed to refresh current native-user Thread", error),
    };
    let execution = session.refresh_execution(current, &pin);
    (pin, execution)
}

fn acquire_pin() -> crate::kernel::task::scheduler::UserRunGuard {
    match crate::kernel::task::scheduler::user_run_guard() {
        Ok(pin) => pin,
        Err(error) => fail_run("failed to pin native-user execution", error),
    }
}

fn validate_current_stack(stack: (usize, usize)) {
    let marker = 0usize;
    let pointer = core::ptr::from_ref(&marker).expose_provenance();
    if pointer < stack.0 || pointer >= stack.1 {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: native-user runner is outside its scheduler stack"
        ));
    }
}

fn finish_pin(pin: crate::kernel::task::scheduler::UserRunGuard) {
    if let Err(error) = crate::kernel::task::scheduler::finish_user_run(pin) {
        fail_run("failed to finish native-user pin", error);
    }
}

fn fault_reason(fault: UserFault) -> TerminalReason {
    let class = match fault.kind() {
        UserFaultKind::InstructionAbort => 1,
        UserFaultKind::DataAbort => 2,
        UserFaultKind::Alignment => 3,
        UserFaultKind::IllegalInstruction => 4,
        UserFaultKind::SystemAccess => 5,
        UserFaultKind::Breakpoint => 6,
        UserFaultKind::OtherSynchronous => 7,
    };
    TerminalReason::Fault {
        class,
        code: fault.syndrome(),
    }
}

fn fail_completion(failure: crate::hal::user::CompletionFailure<'_>) -> ! {
    let (error, completion) = failure.into_parts();
    // The machine context cannot be reused after a binding failure. Retain its
    // armed owner while the fatal path stops the system.
    #[cfg(CONFIG_ARCH_AARCH64)]
    core::mem::forget(completion);
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    let _ = completion;
    fail_run("native-user return ownership is inconsistent", error)
}

fn fail_run(context: &str, error: impl core::fmt::Debug) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {context}: {error:?}"))
}
