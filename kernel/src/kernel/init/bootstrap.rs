// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallible construction of one dormant boot Process.

use hyper::exec::startup::{Layout as StackLayout, StartupHandle};
use hyper::fs::NodeKind;

use crate::kernel::accounting::ResourceDomain;
use crate::kernel::capability::PreparedHandle;
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::process::{
    PreparedProcess, Process, ProcessError, ProcessObject, TaskGroup, UserThread, load_native,
};
use crate::kernel::task::scheduler::CpuMask;

use super::Error;

const ENVIRONMENT: &[&str] = &[];

pub(super) struct BootProcess {
    pub(super) process: Process,
    pub(super) thread: UserThread,
    pub(super) stack_layout: StackLayout,
}

pub(super) fn prepare(
    path: &'static str,
    arguments: &'static [&'static str],
    thread_name: &'static str,
    startup_handle_count: usize,
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<BootProcess, Error> {
    let executable = crate::kernel::vfs::lookup(path, domain)
        .map_err(Error::FileSystem)?
        .ok_or(Error::Missing)?;
    if executable.kind() != NodeKind::File {
        return Err(Error::NotRegularFile);
    }
    if !executable.is_executable() {
        return Err(Error::NotExecutable);
    }
    let executable = executable.into_executable().ok_or(Error::NotExecutable)?;
    let stack_layout = StackLayout::try_new(
        crate::kernel::process::INITIAL_STACK_TOP,
        arguments,
        ENVIRONMENT,
        startup_handle_count,
    )
    .map_err(Error::Stack)?;
    let loaded = load_native(&executable, domain.clone(), stack_layout).map_err(Error::Image)?;
    let prepared = match PreparedProcess::try_new(
        loaded.image,
        group.clone(),
        domain.clone(),
        loaded.address_space,
    ) {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (error, address_space) = failure.into_parts();
            if let Err(failure) =
                crate::kernel::mm::user_space::NativeAddressSpace::retire_unpublished(address_space)
            {
                let (cleanup_error, retained) = failure.into_parts();
                crate::pr_err!(
                    "HypeR: retaining unpublished boot-process address space after cleanup error: {cleanup_error:?}"
                );
                drop(retained);
            }
            return Err(Error::Process(error));
        }
    };
    let object = ProcessObject::try_service(prepared.process()).map_err(Error::TaskObject)?;
    let process = prepared.publish(
        object,
        crate::kernel::process::ProcessNameSnapshot::from_validated(thread_name),
    );
    let thread = process.create_initial_user_thread(thread_name, CpuMask::ALL)?;
    Ok(BootProcess {
        process,
        thread,
        stack_layout,
    })
}

pub(super) fn install_handles<const N: usize>(
    boot: &BootProcess,
    arguments: &[&str],
    purposes: [u32; N],
    prepare: impl FnOnce() -> Result<[PreparedHandle; N], Error>,
) -> Result<(), Error> {
    let reservation = boot.process.reserve_handles::<N>()?;
    let values = reservation.values();
    let startup_handles: [StartupHandle; N] = core::array::from_fn(|index| StartupHandle {
        purpose: purposes[index],
        handle: values[index].get(),
    });
    let stack = match boot.stack_layout.encode(
        boot.process.image().auxiliary(),
        arguments,
        ENVIRONMENT,
        &startup_handles,
    ) {
        Ok(stack) => stack,
        Err(error) => {
            boot.process.abort_handles(reservation);
            return Err(Error::Stack(error));
        }
    };
    let stack_length = match u64::try_from(stack.bytes().len()) {
        Ok(length) => length,
        Err(_) => {
            boot.process.abort_handles(reservation);
            return Err(Error::Stack(hyper::exec::startup::Error::TooLarge));
        }
    };
    let stack_range = match UserSlice::new(UserAddress::new(stack.base()), stack_length) {
        Ok(range) => range,
        Err(_) => {
            boot.process.abort_handles(reservation);
            return Err(Error::Stack(hyper::exec::startup::Error::AddressOverflow));
        }
    };
    let output = match boot.process.reserve_user_write(stack_range) {
        Ok(output) => output,
        Err(error) => {
            boot.process.abort_handles(reservation);
            return Err(Error::Process(error));
        }
    };
    if let Err(error) = output.copy_from(stack.bytes()) {
        drop(output);
        boot.process.abort_handles(reservation);
        return Err(Error::Process(ProcessError::UserMemory(error)));
    }
    output.complete();

    let prepared = match prepare() {
        Ok(handles) => handles,
        Err(error) => {
            boot.process.abort_handles(reservation);
            return Err(error);
        }
    };
    match boot.process.publish_handles(reservation, prepared) {
        Ok(published) if published == values => Ok(()),
        Ok(_) => crate::kernel::crash::fatal(format_args!(
            "HypeR: startup handle publication changed reserved values"
        )),
        Err(failure) => Err(Error::Process(failure.error)),
    }
}
