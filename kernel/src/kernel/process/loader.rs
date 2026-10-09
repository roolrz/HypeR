// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native executable loading into unpublished Process address spaces.

use hyper::exec::bootstrap::{Image, Machine};
use hyper::mm::{PAGE_SIZE, UniqueFallibleArc};
use hyper::sync::PublishedOnce;

use super::{ImageError, MachineAbi, ProcessImage};
use crate::kernel::accounting::{ResourceDomain, ResourceError};
use crate::kernel::mm::user_space::{
    MachineError, NativeAddressSpace, NativeImageSegment, Permissions, UserAddress, UserSlice,
};
use crate::kernel::vfs::ExecutableSnapshot;

const LOADER_BASE: u64 = 0x10_0000;
const STACK_SIZE: u64 = 128 * 1024;
const STACK_TOP: u64 = 0x3f_f000;

struct BootstrapProgram {
    snapshot: ExecutableSnapshot,
    image: Image,
    runtime_directory: crate::kernel::vfs::DirectoryObject,
}
static PROGRAM: PublishedOnce<BootstrapProgram> = PublishedOnce::new();

/// Published once by init, before any userspace task can create children.
/// Retaining the snapshot shares immutable code pages on every subsequent start.
#[cfg_attr(
    feature = "kernel-self-test",
    allow(dead_code, reason = "Self-test images do not launch init")
)]
pub(crate) fn initialize_loader(domain: &ResourceDomain) -> Result<(), Error> {
    use crate::hal::user::HostMachine;
    let path = match crate::hal::user::host_machine() {
        HostMachine::Aarch64 => "/lib64/userspace-loader-hyper-aarch64",
        HostMachine::Riscv64 => "/lib64/userspace-loader-hyper-riscv64",
        _ => return Err(Error::UnsupportedMachine),
    };
    let snapshot = crate::kernel::vfs::lookup(path, domain)
        .map_err(|_| Error::Address)?
        .ok_or(Error::Address)?
        .into_executable()
        .ok_or(Error::Address)?;
    let image = Image::parse(snapshot.bytes()).map_err(Error::Elf)?;
    validate_host_machine(image.machine)?;
    let root = crate::kernel::vfs::root_directory(domain).map_err(|_| Error::Address)?;
    let directory = root
        .open_directory("/lib64", domain)
        .map_err(|_| Error::Address)?;
    PROGRAM
        .publish(BootstrapProgram {
            snapshot,
            image,
            runtime_directory: directory,
        })
        .map_err(|_| Error::Address)
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BootstrapLayout {
    pub(crate) stack_base: u64,
    pub(crate) stack_size: u64,
    pub(crate) loader_base: u64,
    pub(crate) loader_size: u64,
}

#[derive(Debug)]
pub(crate) enum Error {
    Address,
    #[cfg_attr(
        feature = "kernel-self-test",
        allow(
            dead_code,
            reason = "Only boot loader admission constructs this failure"
        )
    )]
    Elf(
        #[allow(
            dead_code,
            reason = "Retains bootstrap validation details for diagnostics"
        )]
        hyper::exec::bootstrap::Error,
    ),
    Image(
        #[expect(
            dead_code,
            reason = "Retains image failure details for Debug diagnostics"
        )]
        ImageError,
    ),
    Machine(MachineError),
    Resource(ResourceError),
    Scheduler(
        #[expect(
            dead_code,
            reason = "Retains scheduler failure details for Debug diagnostics"
        )]
        crate::kernel::task::scheduler::Error,
    ),
    #[cfg_attr(
        feature = "kernel-self-test",
        allow(
            dead_code,
            reason = "Only boot loader admission constructs this failure"
        )
    )]
    UnsupportedMachine,
}

impl From<MachineError> for Error {
    fn from(error: MachineError) -> Self {
        Self::Machine(error)
    }
}

impl From<crate::kernel::task::scheduler::Error> for Error {
    fn from(error: crate::kernel::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

pub(crate) struct LoadedProcessImage {
    pub(crate) image: ProcessImage,
    pub(crate) address_space: UniqueFallibleArc<NativeAddressSpace>,
}

/// Installs only the fixed bootstrap program and an empty guarded stack.
/// Application ELF interpretation is deliberately absent from this boundary.
pub(crate) fn load_native(domain: ResourceDomain) -> Result<LoadedProcessImage, Error> {
    let program = PROGRAM.get().ok_or(Error::Address)?;
    let limit = crate::hal::user::address_space_plan()
        .map_err(MachineError::from)?
        .application_limit();
    let root = UserSlice::new(
        UserAddress::new(PAGE_SIZE),
        limit.checked_sub(PAGE_SIZE).ok_or(Error::Address)?,
    )
    .map_err(|_| Error::Address)?;
    let address_space = NativeAddressSpace::try_new(domain, root)?;
    match prepare_address_space(program, &address_space) {
        Ok(image) => Ok(LoadedProcessImage {
            image,
            address_space,
        }),
        Err(error) => {
            retire_failed_address_space(address_space);
            Err(error)
        }
    }
}

fn prepare_address_space(
    program: &BootstrapProgram,
    address_space: &NativeAddressSpace,
) -> Result<ProcessImage, Error> {
    let stack_base = STACK_TOP - STACK_SIZE;
    let reservation = UserSlice::new(
        UserAddress::new(stack_base - PAGE_SIZE),
        STACK_SIZE + 2 * PAGE_SIZE,
    )
    .map_err(|_| Error::Address)?;
    let stack_vmar = address_space.reserve_initial_stack(reservation)?;
    // Every mapping installation, including executable cache maintenance, uses
    // the existing address-space transaction and CPU pinning mechanisms.
    for segment in program.image.segments() {
        let range = UserSlice::new(
            UserAddress::new(LOADER_BASE + segment.address),
            segment.size,
        )
        .map_err(|_| Error::Address)?;
        let permissions = if segment.executable {
            Permissions::read_execute()
        } else if segment.writable {
            Permissions::read_write()
        } else {
            Permissions::read_only()
        };
        let prepared = NativeImageSegment::try_from_snapshot(
            address_space,
            range,
            permissions,
            program.snapshot.storage().clone(),
            segment.file_offset,
            segment.file_size,
            segment.data_offset,
        )?;
        let pin = crate::kernel::task::scheduler::preempt_disable()?;
        let result = prepared.install(address_space, &pin);
        crate::kernel::task::scheduler::preempt_enable_without_reschedule(pin)?;
        result?;
    }
    let stack = NativeImageSegment::try_new(
        address_space,
        UserSlice::new(UserAddress::new(stack_base), STACK_SIZE).map_err(|_| Error::Address)?,
        Permissions::read_write(),
    )?;
    let pin = crate::kernel::task::scheduler::preempt_disable()?;
    let result = stack.install_in_vmar(address_space, stack_vmar, &pin);
    crate::kernel::task::scheduler::preempt_enable_without_reschedule(pin)?;
    result?;
    ProcessImage::try_native(
        machine_abi(program.image.machine),
        UserAddress::new(LOADER_BASE + program.image.entry),
        UserAddress::new(STACK_TOP),
        UserAddress::new(0),
    )
    .map(|image| {
        image.with_bootstrap(
            stack_vmar,
            BootstrapLayout {
                stack_base,
                stack_size: STACK_SIZE,
                loader_base: LOADER_BASE,
                loader_size: program.image.size,
            },
        )
    })
    .map_err(Error::Image)
}

fn machine_abi(machine: Machine) -> MachineAbi {
    match machine {
        Machine::Aarch64 => MachineAbi::Aarch64,
        Machine::Riscv64 => MachineAbi::Riscv64,
    }
}

#[cfg_attr(
    feature = "kernel-self-test",
    allow(dead_code, reason = "Self-test images do not launch init")
)]
fn validate_host_machine(machine: Machine) -> Result<(), Error> {
    use crate::hal::user::HostMachine;
    if matches!(
        (machine, crate::hal::user::host_machine()),
        (Machine::Aarch64, HostMachine::Aarch64) | (Machine::Riscv64, HostMachine::Riscv64)
    ) {
        Ok(())
    } else {
        Err(Error::UnsupportedMachine)
    }
}

fn retire_failed_address_space(address_space: UniqueFallibleArc<NativeAddressSpace>) {
    if let Err(failure) = NativeAddressSpace::retire_unpublished(address_space) {
        let (error, retained) = failure.into_parts();
        crate::pr_err!(
            "HypeR: retaining a failed Native image address space after cleanup error: {error:?}"
        );
        drop(retained);
    }
}

/// Creates a fresh bootstrap-only directory object for each launch. Caching a
/// publication would try to reactivate the object after an earlier loader closed
/// its last handle. Retain the immutable location, not a userspace handle lifecycle.
pub(crate) fn runtime_directory(
    domain: &ResourceDomain,
) -> Result<crate::kernel::vfs::DirectoryObject, crate::kernel::vfs::VfsError> {
    let program = PROGRAM.get().ok_or(crate::kernel::vfs::VfsError::Missing)?;
    program.runtime_directory.try_clone(domain)
}
