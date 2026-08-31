// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! End-to-end `AArch64` EL0 entry, Native syscall, and contained-fault proof.

use hyper::mm::PAGE_SIZE;

use crate::kernel::accounting::{ResourceDomain, ResourceLimits};
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::{UserAddress, UserSlice, prepare_native_entry_self_test};
use crate::kernel::object::Event;
use crate::kernel::process::{
    AddressSpaceRetirement, MachineAbi, PreparedProcess, Process, ProcessImage,
    ProcessRetirementStep, TaskGroup, TerminalReason,
};
use crate::kernel::task::scheduler::CpuMask;

const IMAGE_BASE: u64 = 0x40_0000;

const DIRECT_CALL_COUNT: usize = 64;

// Query the ABI repeatedly within one machine run and validate x0/x1/x2 after
// every direct return. An unknown call then exercises deferred unwind/re-entry
// and must return NOT_SUPPORTED before brk #0 reports success. Any comparison
// failure branches to brk #1 so its syndrome identifies a return-path error.
const PROGRAM: [u8; 64] = [
    0x09, 0x08, 0x80, 0xd2, // mov x9, #64; direct-call loop count.
    0x01, 0x00, 0x00, 0xd4, // svc #0; x8 starts at abi_query (zero).
    0x1f, 0x00, 0x00, 0xf1, // cmp x0, #0; status == OK.
    0x81, 0x01, 0x00, 0x54, // b.ne failure.
    0x3f, 0x00, 0x00, 0xf1, // cmp x1, #0; pre-release ABI revision.
    0x41, 0x01, 0x00, 0x54, // b.ne failure.
    0x5f, 0x04, 0x00, 0xf1, // cmp x2, #1; CORE feature bitmap.
    0x01, 0x01, 0x00, 0x54, // b.ne failure.
    0x29, 0x05, 0x00, 0xf1, // subs x9, x9, #1.
    0x01, 0xff, 0xff, 0x54, // b.ne svc loop.
    0x08, 0x00, 0x80, 0x92, // mov x8, #-1; unknown syscall number.
    0x01, 0x00, 0x00, 0xd4, // svc #0; deferred slow path.
    0x1f, 0x10, 0x00, 0xb1, // cmn x0, #4; status == NOT_SUPPORTED (-4).
    0x41, 0x00, 0x00, 0x54, // b.ne failure.
    0x00, 0x00, 0x20, 0xd4, // brk #0; success.
    0x20, 0x00, 0x20, 0xd4, // brk #1; result mismatch.
];

// Yield once, validate its completed result, then terminate the current Thread
// with status -17. Reaching either breakpoint means that a returning or
// no-return contract was violated.
const THREAD_EXIT_PROGRAM: [u8; 36] = [
    0xc8, 0x00, 0x80, 0xd2, // mov x8, #6; thread_yield.
    0x01, 0x00, 0x00, 0xd4, // svc #0; deferred scheduling path.
    0x1f, 0x00, 0x00, 0xf1, // cmp x0, #0; status == OK.
    0xa1, 0x00, 0x00, 0x54, // b.ne result_mismatch.
    0x00, 0x02, 0x80, 0x92, // mov x0, #-17; exit status.
    0xe8, 0x00, 0x80, 0xd2, // mov x8, #7; thread_exit.
    0x01, 0x00, 0x00, 0xd4, // svc #0; must not return.
    0x20, 0x00, 0x20, 0xd4, // brk #1; no-return contract failed.
    0x40, 0x00, 0x20, 0xd4, // result_mismatch: brk #2.
];

// Terminate the Process with status 23. The call must stop its current Thread
// without completing a machine-visible result.
const PROCESS_EXIT_PROGRAM: [u8; 16] = [
    0xe0, 0x02, 0x80, 0xd2, // mov x0, #23; exit status.
    0x08, 0x01, 0x80, 0xd2, // mov x8, #8; process_exit.
    0x01, 0x00, 0x00, 0xd4, // svc #0; must not return.
    0x20, 0x00, 0x20, 0xd4, // brk #1; no-return contract failed.
];

// Create an Event, assert its level through the deferred signal path, observe
// it with an infinite-deadline wait, then exit normally. This proves the
// generated ABI numbers, Process handle publication, rights checks, object
// dispatch, and the signal snapshot return convention through real EL0 entry.
const EVENT_WAIT_PROGRAM: [u8; 108] = [
    0x00, 0x00, 0x80, 0xd2, // mov x0, #0; event_create options.
    0x28, 0x01, 0x80, 0xd2, // mov x8, #9; event_create.
    0x01, 0x00, 0x00, 0xd4, // svc #0.
    0x1f, 0x00, 0x00, 0xf1, // cmp x0, #0.
    0xc1, 0x02, 0x00, 0x54, // b.ne failure.
    0xf3, 0x03, 0x01, 0xaa, // mov x19, x1; retain Event handle.
    0xe0, 0x03, 0x13, 0xaa, // mov x0, x19; Event handle.
    0x01, 0x00, 0x80, 0xd2, // mov x1, #0; clear mask.
    0x22, 0x00, 0x80, 0xd2, // mov x2, #1; set SIGNALED.
    0x48, 0x01, 0x80, 0xd2, // mov x8, #10; event_signal.
    0x01, 0x00, 0x00, 0xd4, // svc #0.
    0x1f, 0x00, 0x00, 0xf1, // cmp x0, #0.
    0xc1, 0x01, 0x00, 0x54, // b.ne failure.
    0xe0, 0x03, 0x13, 0xaa, // mov x0, x19; Event handle.
    0x21, 0x00, 0x80, 0xd2, // mov x1, #1; requested SIGNALED.
    0x02, 0x00, 0x80, 0x92, // mov x2, #-1; infinite deadline.
    0x68, 0x01, 0x80, 0xd2, // mov x8, #11; object_wait_one.
    0x01, 0x00, 0x00, 0xd4, // svc #0.
    0x1f, 0x00, 0x00, 0xf1, // cmp x0, #0.
    0xe1, 0x00, 0x00, 0x54, // b.ne failure.
    0x3f, 0x04, 0x00, 0xf1, // cmp x1, #1; observed SIGNALED.
    0xa1, 0x00, 0x00, 0x54, // b.ne failure.
    0x00, 0x00, 0x80, 0xd2, // mov x0, #0; exit status.
    0xe8, 0x00, 0x80, 0xd2, // mov x8, #7; thread_exit.
    0x01, 0x00, 0x00, 0xd4, // svc #0; must not return.
    0x20, 0x00, 0x20, 0xd4, // brk #1; no-return contract failed.
    0x40, 0x00, 0x20, 0xd4, // failure: brk #2.
];

// Block forever on a clear Event. The kernel self-test controller requests
// Process stop only after the Event publishes its waiter; returning to either
// breakpoint proves cancellation leaked a result back into EL0.
const CANCELLED_EVENT_WAIT_PROGRAM: [u8; 52] = [
    0x00, 0x00, 0x80, 0xd2, // mov x0, #0; event_create options.
    0x28, 0x01, 0x80, 0xd2, // mov x8, #9; event_create.
    0x01, 0x00, 0x00, 0xd4, // svc #0.
    0x1f, 0x00, 0x00, 0xf1, // cmp x0, #0.
    0x01, 0x01, 0x00, 0x54, // b.ne failure.
    0xf3, 0x03, 0x01, 0xaa, // mov x19, x1; retain Event handle.
    0xe0, 0x03, 0x13, 0xaa, // mov x0, x19; Event handle.
    0x21, 0x00, 0x80, 0xd2, // mov x1, #1; requested SIGNALED.
    0x02, 0x00, 0x80, 0x92, // mov x2, #-1; infinite deadline.
    0x68, 0x01, 0x80, 0xd2, // mov x8, #11; object_wait_one.
    0x01, 0x00, 0x00, 0xd4, // svc #0; controller cancels this wait.
    0x60, 0x00, 0x20, 0xd4, // brk #3; cancellation must not return.
    0x40, 0x00, 0x20, 0xd4, // failure: brk #2.
];

#[derive(Clone, Copy)]
enum SiblingSetup {
    None,
    Dormant,
}

#[derive(Clone, Copy)]
enum RunControl {
    Join,
    CancelPublishedEventWait,
}

struct ProgramOutcome {
    thread: TerminalReason,
    process: TerminalReason,
    sibling: Option<TerminalReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    AddressSpace,
    Construction,
    Group,
    Image,
    Lifecycle,
    Scheduler,
    Terminal,
}

pub(super) fn run() -> Result<(), Error> {
    let direct_calls = crate::hal::user::direct_native_call_count_for_test();
    let domain =
        ResourceDomain::try_new_root(ResourceLimits::UNLIMITED).map_err(|_| Error::Construction)?;
    let group = TaskGroup::try_new(&domain).map_err(|_| Error::Group)?;

    let outcome = run_program(
        &domain,
        &group,
        &PROGRAM,
        "selftest/el0-fault",
        SiblingSetup::None,
        RunControl::Join,
    )?;
    if !matches!(outcome.thread, TerminalReason::Fault { class: 6, code } if code & 0xffff == 0)
        || outcome.process != outcome.thread
        || outcome.sibling.is_some()
    {
        return Err(Error::Terminal);
    }
    if crate::hal::user::direct_native_call_count_for_test()
        != direct_calls.saturating_add(DIRECT_CALL_COUNT)
    {
        return Err(Error::Terminal);
    }

    let outcome = run_program(
        &domain,
        &group,
        &THREAD_EXIT_PROGRAM,
        "selftest/el0-thread-exit",
        SiblingSetup::None,
        RunControl::Join,
    )?;
    if outcome.thread != (TerminalReason::ThreadExited { status: -17 })
        || outcome.process != (TerminalReason::LastThreadExited { status: -17 })
        || outcome.sibling.is_some()
    {
        return Err(Error::Terminal);
    }

    let outcome = run_program(
        &domain,
        &group,
        &EVENT_WAIT_PROGRAM,
        "selftest/el0-event-wait",
        SiblingSetup::None,
        RunControl::Join,
    )?;
    if outcome.thread != (TerminalReason::ThreadExited { status: 0 })
        || outcome.process != (TerminalReason::LastThreadExited { status: 0 })
        || outcome.sibling.is_some()
    {
        return Err(Error::Terminal);
    }

    let outcome = run_program(
        &domain,
        &group,
        &CANCELLED_EVENT_WAIT_PROGRAM,
        "selftest/el0-event-cancel",
        SiblingSetup::None,
        RunControl::CancelPublishedEventWait,
    )?;
    if outcome.thread != TerminalReason::Requested
        || outcome.process != TerminalReason::Requested
        || outcome.sibling.is_some()
    {
        return Err(Error::Terminal);
    }

    let outcome = run_program(
        &domain,
        &group,
        &PROCESS_EXIT_PROGRAM,
        "selftest/el0-process-exit",
        SiblingSetup::Dormant,
        RunControl::Join,
    )?;
    let process_exit = TerminalReason::ProcessExited { status: 23 };
    if outcome.thread != process_exit
        || outcome.process != process_exit
        || outcome.sibling != Some(process_exit)
    {
        return Err(Error::Terminal);
    }

    group.request_stop().map_err(|_| Error::Group)?;
    group.finish_retirement().map_err(|_| Error::Group)?;
    Ok(())
}

fn run_program(
    domain: &ResourceDomain,
    group: &TaskGroup,
    program: &[u8],
    name: &str,
    sibling_setup: SiblingSetup,
    control: RunControl,
) -> Result<ProgramOutcome, Error> {
    let code_range =
        UserSlice::new(UserAddress::new(IMAGE_BASE), PAGE_SIZE).map_err(|_| Error::Construction)?;
    let stack_range = UserSlice::new(UserAddress::new(IMAGE_BASE + PAGE_SIZE * 2), PAGE_SIZE)
        .map_err(|_| Error::Construction)?;
    let image_range = UserSlice::new(UserAddress::new(IMAGE_BASE), PAGE_SIZE * 3)
        .map_err(|_| Error::Construction)?;
    let pin = crate::kernel::task::scheduler::preempt_disable().map_err(|_| Error::Scheduler)?;
    let address_space = prepare_native_entry_self_test(
        domain.clone(),
        image_range,
        code_range,
        stack_range,
        program,
        &pin,
    );
    crate::kernel::task::scheduler::preempt_enable_and_reschedule(pin)
        .map_err(|_| Error::Scheduler)?;
    let address_space = address_space.map_err(|_| Error::AddressSpace)?;
    let image = ProcessImage::try_native(
        MachineAbi::Aarch64,
        code_range.base(),
        stack_range.end(),
        UserAddress::new(0),
    )
    .map_err(|_| Error::Image)?;
    let prepared =
        match PreparedProcess::try_new(image, group.clone(), domain.clone(), address_space) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (_cause, mut address_space) = failure.into_parts();
                address_space.retire().map_err(|_| Error::AddressSpace)?;
                return Err(Error::Construction);
            }
        };
    let process = prepared.publish();
    process.start().map_err(|_| Error::Lifecycle)?;
    let thread = process
        .create_initial_user_thread(name, CpuMask::ALL)
        .map_err(|_| Error::Construction)?;
    let sibling = match sibling_setup {
        SiblingSetup::None => None,
        SiblingSetup::Dormant => Some(
            process
                .create_initial_user_thread("selftest/el0-dormant-sibling", CpuMask::ALL)
                .map_err(|_| Error::Construction)?,
        ),
    };
    thread.ready().map_err(|_| Error::Scheduler)?;
    if matches!(control, RunControl::CancelPublishedEventWait) {
        wait_for_event_registration(&process)?;
        let report = process.request_stop(TerminalReason::Requested);
        if !report.newly_requested || !report.dispatch_complete {
            return Err(Error::Lifecycle);
        }
    }
    let thread_reason = thread.join().map_err(|_| Error::Lifecycle)?;
    let sibling_reason = match sibling.as_ref() {
        Some(sibling) => Some(sibling.join().map_err(|_| Error::Lifecycle)?),
        None => None,
    };
    let process_reason = process.join().map_err(|_| Error::Lifecycle)?;
    drop(thread);
    drop(sibling);
    retire_process(&process)?;
    Ok(ProgramOutcome {
        thread: thread_reason,
        process: process_reason,
        sibling: sibling_reason,
    })
}

fn wait_for_event_registration(process: &Process) -> Result<(), Error> {
    const MAX_PROGRESS_PASSES: usize = 4_096;

    let handle = HandleValue::first_for_test();
    for _ in 0..MAX_PROGRESS_PASSES {
        if let Ok(event) = process.resolve_handle::<Event>(handle, Rights::WAIT)
            && event.object().waiter_count() == 1
        {
            return Ok(());
        }
        crate::kernel::task::scheduler::yield_now().map_err(|_| Error::Scheduler)?;
    }
    Err(Error::Lifecycle)
}

fn retire_process(process: &Process) -> Result<(), Error> {
    let mut retry: Option<AddressSpaceRetirement> = None;
    for _ in 0..64 {
        let step = match retry.take() {
            Some(token) => match token.retry() {
                Ok(()) => return Ok(()),
                Err((token, _)) => {
                    retry = Some(token);
                    crate::kernel::task::scheduler::cond_resched().map_err(|_| Error::Scheduler)?;
                    continue;
                }
            },
            None => process.retire().map_err(|_| Error::Lifecycle)?,
        };
        match step {
            ProcessRetirementStep::Complete => return Ok(()),
            ProcessRetirementStep::Retry(token) => retry = Some(token),
            ProcessRetirementStep::InProgress | ProcessRetirementStep::PendingReferences => {}
        }
        crate::kernel::task::scheduler::cond_resched().map_err(|_| Error::Scheduler)?;
    }
    // An armed retry token must not be abandoned. Reaching this bound signals
    // a kernel teardown invariant, so fail-stop rather than return it live.
    if retry.is_some() {
        crate::hal::cpu::halt();
    }
    Err(Error::Lifecycle)
}
