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

// svc #0 (abi_query), followed by brk #0. All initial registers are zero, so
// x8 already selects syscall zero and x0 requests the base ABI revision.
const PROGRAM: [u8; 8] = [0x01, 0x00, 0x00, 0xd4, 0x00, 0x00, 0x20, 0xd4];

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
    if !matches!(reason, TerminalReason::Fault { class: 6, .. }) {
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
