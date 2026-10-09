// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallible construction of one dormant boot Process.

use hyper::fs::NodeKind;

use crate::kernel::accounting::ResourceDomain;
use crate::kernel::capability::{HandleValue, PreparedHandle};
use crate::kernel::process::{
    PreparedProcess, Process, ProcessHandleReservation, ProcessObject, TaskGroup, load_native,
};

use super::Error;

pub(super) struct BootProcess {
    pub(super) process: Process,
    executable: crate::kernel::vfs::ExecutableSnapshot,
}

// Image loading and authority installation are consecutive boot phases. Their
// temporary owners must not accumulate on the same scheduled kernel stack.
#[inline(never)]
pub(super) fn prepare(
    path: &'static str,
    thread_name: &'static str,
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
    let loaded = load_native(domain.clone()).map_err(Error::Image)?;
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
    Ok(BootProcess {
        process,
        executable,
    })
}

pub(super) fn install_handles<const N: usize>(
    boot: &BootProcess,
    purposes: [u32; N],
    prepare: impl FnOnce(&mut [Option<PreparedHandle>; N]) -> Result<(), Error>,
) -> Result<u64, Error> {
    let reservation = boot.process.reserve_handles::<N>()?;
    let values = reservation.values();
    // One owner table is filled in place. Partial preparation is automatically
    // retired on failure; no handle is visible before the batch publication.
    let mut slots = [const { None }; N];
    if let Err(error) = prepare(&mut slots) {
        drop(slots);
        boot.process.abort_handles(reservation);
        return Err(error);
    }
    let channel = match install_bootstrap(boot, &purposes, &values) {
        Ok(channel) => channel,
        Err(error) => {
            drop(slots);
            boot.process.abort_handles(reservation);
            return Err(error);
        }
    };
    finish_handles(boot, reservation, &values, &mut slots)?;
    Ok(channel)
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

// Init has no parent awaiting a result. An empty opaque payload asks the
// installed userspace runtime to supply its own initial argument policy.
#[inline(never)]
fn install_bootstrap<const N: usize>(
    boot: &BootProcess,
    purposes: &[u32; N],
    values: &[HandleValue; N],
) -> Result<u64, Error> {
    use hyper::abi::native::*;
    let reservation = boot.process.reserve_handles::<3>()?;
    let extra = reservation.values();
    let root = purposes
        .iter()
        .position(|p| u64::from(*p) == HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR);
    let stack = purposes
        .iter()
        .position(|p| u64::from(*p) == HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR);
    let (root, stack) = match (root, stack) {
        (Some(root), Some(stack)) => (root, stack),
        _ => hyper::debug::invariant_failure("init startup VMARs missing"),
    };
    let layout = boot.process.image().bootstrap();
    let header = HyperNativeLoaderStartup {
        size: 88,
        handle_count: (N - 2) as u32,
        data_size: 0,
        flags: 0,
        runtime_directory: extra[2].get(),
        executable: extra[0].get(),
        executable_size: boot.executable.bytes().len() as u64,
        root_vmar: values[root].get(),
        stack_vmar: values[stack].get(),
        stack_base: layout.stack_base,
        stack_size: layout.stack_size,
        loader_base: layout.loader_base,
        loader_size: layout.loader_size,
    };
    let records = purposes
        .iter()
        .zip(values)
        .enumerate()
        .filter(|(i, _)| *i != root && *i != stack)
        .map(|(_, (purpose, value))| HyperNativeStartupHandle {
            purpose: *purpose,
            flags: 0,
            handle: value.get(),
        });
    let prepared = match crate::kernel::process::bootstrap::prepare(
        &boot.process.resource_domain(),
        &boot.executable,
        header,
        records,
        &[],
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            boot.process.abort_handles(reservation);
            return Err(Error::Bootstrap(error));
        }
    };
    drop(prepared.parent_channel);
    let published = boot
        .process
        .publish_handles(
            reservation,
            [
                prepared.executable,
                prepared.child_channel,
                prepared.runtime_directory,
            ],
        )
        .map_err(|failure| Error::Process(failure.error))?;
    Ok(published[1].get())
}
