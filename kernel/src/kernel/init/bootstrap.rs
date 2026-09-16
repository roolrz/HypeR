// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallible construction of one dormant boot Process.

use hyper::exec::startup::{Layout as StackLayout, StartupHandle};
use hyper::fs::NodeKind;

use crate::kernel::accounting::ResourceDomain;
use crate::kernel::capability::{HandleValue, PreparedHandle};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::process::{
    PreparedProcess, Process, ProcessError, ProcessHandleReservation, ProcessObject, TaskGroup,
    UserThread, load_native,
};
use crate::kernel::task::scheduler::CpuMask;

use super::Error;

const ENVIRONMENT: &[&str] = &[];

pub(super) struct BootProcess {
    pub(super) process: Process,
    pub(super) thread: UserThread,
    pub(super) stack_layout: StackLayout,
}

// Image loading and authority installation are consecutive boot phases. Their
// temporary owners must not accumulate on the same scheduled kernel stack.
#[inline(never)]
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
    prepare: impl FnOnce(&mut [Option<PreparedHandle>; N]) -> Result<(), Error>,
) -> Result<(), Error> {
    let reservation = boot.process.reserve_handles::<N>()?;
    let values = reservation.values();
    if let Err(error) = write_startup(boot, arguments, &purposes, &values) {
        boot.process.abort_handles(reservation);
        return Err(error);
    }

    // One owner table is filled in place. Partial preparation is automatically
    // retired on failure; no handle is visible before the batch publication.
    let mut slots = [const { None }; N];
    if let Err(error) = prepare(&mut slots) {
        drop(slots);
        boot.process.abort_handles(reservation);
        return Err(error);
    }
    finish_handles(boot, reservation, &values, &mut slots)
}

// Authority construction can enter VFS. Keep the publication array and its
// failure-owned handles off that call chain; only this final phase consumes
// the complete slot table and the linear reservation.
#[inline(never)]
fn finish_handles<const N: usize>(
    boot: &BootProcess,
    reservation: ProcessHandleReservation<N>,
    values: &[HandleValue; N],
    slots: &mut [Option<PreparedHandle>; N],
) -> Result<(), Error> {
    let prepared = core::array::from_fn(|index| match slots[index].take() {
        Some(handle) => handle,
        None => hyper::debug::invariant_failure("init::bootstrap::finish_handles invariant"),
    });
    match boot.process.publish_handles(reservation, prepared) {
        Ok(published) if &published == values => Ok(()),
        Ok(_) => crate::kernel::crash::fatal(format_args!(
            "HypeR: startup handle publication changed reserved values"
        )),
        Err(failure) => Err(Error::Process(failure.error)),
    }
}

// No capability constructors or publication run while the encoded stack and
// COW write reservation exist. Only the reserved handle values cross phases.
#[inline(never)]
fn write_startup<const N: usize>(
    boot: &BootProcess,
    arguments: &[&str],
    purposes: &[u32; N],
    values: &[HandleValue; N],
) -> Result<(), Error> {
    let startup_handles = core::array::from_fn::<_, N, _>(|index| StartupHandle {
        purpose: purposes[index],
        handle: values[index].get(),
    });
    let stack = boot
        .stack_layout
        .encode(
            boot.process.image().auxiliary(),
            arguments,
            ENVIRONMENT,
            &startup_handles,
        )
        .map_err(Error::Stack)?;
    let stack_length = u64::try_from(stack.bytes().len())
        .map_err(|_| Error::Stack(hyper::exec::startup::Error::TooLarge))?;
    let stack_range = UserSlice::new(UserAddress::new(stack.base()), stack_length)
        .map_err(|_| Error::Stack(hyper::exec::startup::Error::AddressOverflow))?;
    let output = boot.process.reserve_user_write(stack_range)?;
    output
        .copy_from(stack.bytes())
        .map_err(|error| Error::Process(ProcessError::UserMemory(error)))?;
    output.complete();
    Ok(())
}
