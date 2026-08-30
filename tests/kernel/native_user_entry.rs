// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! End-to-end `AArch64` EL0 entry, Native syscall, and contained-fault proof.

use hyper::mm::PAGE_SIZE;

use crate::kernel::accounting::{ResourceDomain, ResourceLimits};
use crate::kernel::mm::user_space::{UserAddress, UserSlice, prepare_native_entry_self_test};
use crate::kernel::process::{
    AddressSpaceRetirement, MachineAbi, PreparedProcess, ProcessImage, ProcessRetirementStep,
    TaskGroup, TerminalReason,
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
    let code_range =
        UserSlice::new(UserAddress::new(IMAGE_BASE), PAGE_SIZE).map_err(|_| Error::Construction)?;
    let stack_range = UserSlice::new(UserAddress::new(IMAGE_BASE + PAGE_SIZE * 2), PAGE_SIZE)
        .map_err(|_| Error::Construction)?;
    let image_range = UserSlice::new(UserAddress::new(IMAGE_BASE), PAGE_SIZE * 3)
        .map_err(|_| Error::Construction)?;
    let domain =
        ResourceDomain::try_new_root(ResourceLimits::UNLIMITED).map_err(|_| Error::Construction)?;
    let group = TaskGroup::try_new(&domain).map_err(|_| Error::Group)?;
    let pin = crate::kernel::task::scheduler::preempt_disable().map_err(|_| Error::Scheduler)?;
    let address_space = prepare_native_entry_self_test(
        domain.clone(),
        image_range,
        code_range,
        stack_range,
        &PROGRAM,
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
    let prepared = match PreparedProcess::try_new(image, group.clone(), domain, address_space) {
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
        .create_initial_user_thread("selftest/el0", CpuMask::ALL)
        .map_err(|_| Error::Construction)?;
    thread.ready().map_err(|_| Error::Scheduler)?;
    let reason = thread.join().map_err(|_| Error::Lifecycle)?;
    if !matches!(reason, TerminalReason::Fault { class: 6, code } if code & 0xffff == 0) {
        return Err(Error::Terminal);
    }
    if crate::hal::user::direct_native_call_count_for_test()
        != direct_calls.saturating_add(DIRECT_CALL_COUNT)
    {
        return Err(Error::Terminal);
    }
    if process.join().map_err(|_| Error::Lifecycle)? != reason {
        return Err(Error::Terminal);
    }
    drop(thread);
    group.request_stop().map_err(|_| Error::Group)?;
    retire_process(&process)?;
    group.finish_retirement().map_err(|_| Error::Group)?;
    Ok(())
}

fn retire_process(process: &crate::kernel::process::Process) -> Result<(), Error> {
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
