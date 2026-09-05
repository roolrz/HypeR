// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Construction and launch of the first Native userspace process.

use core::convert::Infallible;

use hyper::exec::startup::{Layout as StackLayout, StartupHandle};
use hyper::fs::ramfs::NodeKind;

use crate::kernel::accounting::{
    ResourceDomain, ResourceDomainObject, ResourceDomainObjectError, ResourceError, ResourceLimits,
};
use crate::kernel::capability::{HandleError, HandleFlags, PreparedHandle, Rights};
use crate::kernel::mm::user_space::{
    ExecutableAuthority, ExecutableAuthorityError, MemoryObjectError, UserAddress, UserSlice,
    VmarObject,
};
use crate::kernel::object::{ObjectCreationError, ObjectPublication, UserExportableObject};
use crate::kernel::process::{
    LoaderError, PreparedProcess, Process, ProcessError, TaskFactory, TaskGroup, TaskGroupError,
    TaskGroupObject, TaskObjectError, load_native,
};
use crate::kernel::task::scheduler::{self, CpuMask};

const INIT_PATH: &str = "/init";
const INIT_ARGUMENTS: &[&str] = &[INIT_PATH];
const INIT_ENVIRONMENT: &[&str] = &[];
const REQUIRED_STARTUP_HANDLE_COUNT: usize = 5;
const COMPLETE_STARTUP_HANDLE_COUNT: usize = 6;
const REQUIRED_STARTUP_PURPOSES: [u32; REQUIRED_STARTUP_HANDLE_COUNT] = [
    startup_purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN),
    startup_purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP),
    startup_purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY),
    startup_purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY),
    startup_purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR),
];
const COMPLETE_STARTUP_PURPOSES: [u32; COMPLETE_STARTUP_HANDLE_COUNT] = [
    REQUIRED_STARTUP_PURPOSES[0],
    REQUIRED_STARTUP_PURPOSES[1],
    REQUIRED_STARTUP_PURPOSES[2],
    REQUIRED_STARTUP_PURPOSES[3],
    REQUIRED_STARTUP_PURPOSES[4],
    startup_purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE),
];

pub(crate) enum Error {
    FileSystem(crate::kernel::fs::LookupError),
    Console(crate::kernel::device::console::ObjectError),
    ExecutableAuthority(ExecutableAuthorityError),
    Handle(HandleError),
    Image(LoaderError),
    IncompleteThreadPublication,
    MemoryObject(MemoryObjectError),
    Missing,
    NotExecutable,
    NotRegularFile,
    Object(ObjectCreationError),
    Process(ProcessError),
    Resource(ResourceError),
    ResourceObject(ResourceDomainObjectError),
    Stack(hyper::exec::startup::Error),
    TaskGroup(TaskGroupError),
    TaskObject(TaskObjectError),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FileSystem(error) => formatter.debug_tuple("FileSystem").field(error).finish(),
            Self::Console(error) => formatter.debug_tuple("Console").field(error).finish(),
            Self::ExecutableAuthority(error) => formatter
                .debug_tuple("ExecutableAuthority")
                .field(error)
                .finish(),
            Self::Handle(error) => formatter.debug_tuple("Handle").field(error).finish(),
            Self::Image(error) => formatter.debug_tuple("Image").field(error).finish(),
            Self::IncompleteThreadPublication => formatter.write_str("IncompleteThreadPublication"),
            Self::MemoryObject(error) => {
                formatter.debug_tuple("MemoryObject").field(error).finish()
            }
            Self::Missing => formatter.write_str("Missing"),
            Self::NotExecutable => formatter.write_str("NotExecutable"),
            Self::NotRegularFile => formatter.write_str("NotRegularFile"),
            Self::Object(error) => formatter.debug_tuple("Object").field(error).finish(),
            Self::Process(error) => formatter.debug_tuple("Process").field(error).finish(),
            Self::Resource(error) => formatter.debug_tuple("Resource").field(error).finish(),
            Self::ResourceObject(error) => formatter
                .debug_tuple("ResourceObject")
                .field(error)
                .finish(),
            Self::Stack(error) => formatter.debug_tuple("Stack").field(error).finish(),
            Self::TaskGroup(error) => formatter.debug_tuple("TaskGroup").field(error).finish(),
            Self::TaskObject(error) => formatter.debug_tuple("TaskObject").field(error).finish(),
        }
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<TaskGroupError> for Error {
    fn from(error: TaskGroupError) -> Self {
        Self::TaskGroup(error)
    }
}

impl From<ProcessError> for Error {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

pub(crate) fn start() -> Result<Infallible, Error> {
    let init = crate::kernel::fs::lookup(INIT_PATH)
        .map_err(Error::FileSystem)?
        .ok_or(Error::Missing)?;
    if init.kind() != NodeKind::File {
        return Err(Error::NotRegularFile);
    }
    if !init.is_executable() {
        return Err(Error::NotExecutable);
    }

    let domain = ResourceDomain::try_new_root(ResourceLimits::UNLIMITED)?;
    let group = TaskGroup::try_new(&domain)?;
    let console_available = crate::kernel::device::console::SystemConsole::is_available();
    let startup_handle_count = if console_available {
        COMPLETE_STARTUP_HANDLE_COUNT
    } else {
        REQUIRED_STARTUP_HANDLE_COUNT
    };
    let stack_layout = StackLayout::try_new(
        crate::kernel::process::INITIAL_STACK_TOP,
        INIT_ARGUMENTS,
        INIT_ENVIRONMENT,
        startup_handle_count,
    )
    .map_err(Error::Stack)?;
    let loaded = load_native(init.data(), domain.clone(), stack_layout).map_err(Error::Image)?;
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
                crate::kernel::mm::user_space::NativeAddressSpace::retire(address_space)
            {
                let (cleanup_error, retained) = failure.into_parts();
                crate::pr_err!(
                    "HypeR: retaining the unpublished init address space after cleanup error: {cleanup_error:?}"
                );
                drop(retained);
            }
            return Err(Error::Process(error));
        }
    };
    let process = prepared.publish();
    install_startup_capabilities(&process, &group, &domain, stack_layout, console_available)?;
    let thread = process.create_initial_user_thread("init", CpuMask::ALL)?;
    process.start()?;
    let process_id = process.id().get();
    let thread_id = thread
        .scheduler_id()
        .ok_or(Error::IncompleteThreadPublication)?;
    thread.ready()?;
    crate::println!(
        "HypeR: starting Native init process {} as thread {}",
        process_id,
        thread_id.get()
    );
    // Scheduler and Process membership now own the runnable Thread and its
    // Process. Do not strand observer references in the non-returning bootstrap
    // Thread's stack frame.
    drop(thread);
    drop(process);
    scheduler::exit_current()
}

fn install_startup_capabilities(
    process: &Process,
    group: &TaskGroup,
    domain: &ResourceDomain,
    stack_layout: StackLayout,
    console_available: bool,
) -> Result<(), Error> {
    if console_available {
        install_startup_handle_set(process, stack_layout, COMPLETE_STARTUP_PURPOSES, || {
            prepare_complete_startup_handles(process, group, domain)
        })
    } else {
        install_startup_handle_set(process, stack_layout, REQUIRED_STARTUP_PURPOSES, || {
            prepare_required_startup_handles(process, group, domain)
        })
    }
}

fn install_startup_handle_set<const N: usize>(
    process: &Process,
    stack_layout: StackLayout,
    purposes: [u32; N],
    prepare: impl FnOnce() -> Result<[PreparedHandle; N], Error>,
) -> Result<(), Error> {
    let reservation = process.reserve_handles::<N>()?;
    let values = reservation.values();
    let startup_handles: [StartupHandle; N] = core::array::from_fn(|index| StartupHandle {
        purpose: purposes[index],
        handle: values[index].get(),
    });
    let stack = match stack_layout.encode(
        process.image().entry().get(),
        INIT_ARGUMENTS,
        INIT_ENVIRONMENT,
        &startup_handles,
    ) {
        Ok(stack) => stack,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(Error::Stack(error));
        }
    };
    let stack_length = match u64::try_from(stack.bytes().len()) {
        Ok(length) => length,
        Err(_) => {
            process.abort_handles(reservation);
            return Err(Error::Stack(hyper::exec::startup::Error::TooLarge));
        }
    };
    let stack_range = match UserSlice::new(UserAddress::new(stack.base()), stack_length) {
        Ok(range) => range,
        Err(_) => {
            process.abort_handles(reservation);
            return Err(Error::Stack(hyper::exec::startup::Error::AddressOverflow));
        }
    };
    let output = match process.reserve_user_write(stack_range) {
        Ok(output) => output,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(Error::Process(error));
        }
    };
    if let Err(error) = output.copy_from(stack.bytes()) {
        drop(output);
        process.abort_handles(reservation);
        return Err(Error::Process(ProcessError::UserMemory(error)));
    }
    output.complete();

    let prepared = match prepare() {
        Ok(handles) => handles,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(error);
        }
    };
    match process.publish_handles(reservation, prepared) {
        Ok(published) if published == values => Ok(()),
        Ok(_) => crate::kernel::crash::fatal(format_args!(
            "HypeR: startup handle publication changed reserved values"
        )),
        Err(failure) => Err(Error::Process(failure.error)),
    }
}

fn prepare_required_startup_handles(
    process: &Process,
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<[PreparedHandle; REQUIRED_STARTUP_HANDLE_COUNT], Error> {
    let resource =
        ResourceDomainObject::try_publication(domain.clone()).map_err(Error::ResourceObject)?;
    let task_group = TaskGroupObject::try_publication(group.clone()).map_err(Error::TaskObject)?;
    let task_factory =
        ObjectPublication::try_new(TaskFactory::try_new(domain).map_err(Error::TaskObject)?)
            .map_err(Error::Object)?;
    let executable = ObjectPublication::try_new(
        ExecutableAuthority::try_new(domain).map_err(Error::ExecutableAuthority)?,
    )
    .map_err(Error::Object)?;
    let root_vmar = VmarObject::try_root_publication(process.address_space_owner()?, domain)
        .map_err(Error::MemoryObject)?;
    Ok([
        prepare_handle(
            resource,
            Rights::TRANSFER
                .union(Rights::INSPECT)
                .union(Rights::CREATE_RESOURCE_DOMAIN)
                .union(Rights::SET_LIMITS)
                .union(Rights::REVOKE),
        )?,
        prepare_handle(
            task_group,
            Rights::TRANSFER
                .union(Rights::INSPECT)
                .union(Rights::REQUEST_STOP),
        )?,
        prepare_handle(
            task_factory,
            Rights::TRANSFER
                .union(Rights::INSPECT)
                .union(Rights::CREATE_PROCESS)
                .union(Rights::CREATE_TASK_GROUP),
        )?,
        prepare_handle(
            executable,
            Rights::TRANSFER
                .union(Rights::INSPECT)
                .union(Rights::CREATE_EXECUTABLE),
        )?,
        prepare_handle(
            root_vmar,
            Rights::TRANSFER.union(Rights::INSPECT).union(Rights::MAP),
        )?,
    ])
}

fn prepare_complete_startup_handles(
    process: &Process,
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<[PreparedHandle; COMPLETE_STARTUP_HANDLE_COUNT], Error> {
    let [resource, task_group, task_factory, executable, root_vmar] =
        prepare_required_startup_handles(process, group, domain)?;
    let console = crate::kernel::device::console::SystemConsole::try_publication(domain)
        .map_err(Error::Console)?;
    Ok([
        resource,
        task_group,
        task_factory,
        executable,
        root_vmar,
        prepare_handle(
            console,
            Rights::TRANSFER
                .union(Rights::WAIT)
                .union(Rights::INSPECT)
                .union(Rights::READ)
                .union(Rights::WRITE),
        )?,
    ])
}

fn prepare_handle<T: UserExportableObject>(
    publication: ObjectPublication<T>,
    rights: Rights,
) -> Result<PreparedHandle, Error> {
    PreparedHandle::try_from_new_object(publication, rights, HandleFlags::NONE)
        .map_err(Error::Handle)
}

const fn startup_purpose(purpose: u64) -> u32 {
    assert!(purpose <= u32::MAX as u64);
    purpose as u32
}
