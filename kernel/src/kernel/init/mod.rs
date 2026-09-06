// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Construction and launch of the initial Native userspace supervisor.

use core::convert::Infallible;

use crate::kernel::accounting::{ResourceDomain, ResourceError, ResourceLimits};
use crate::kernel::capability::HandleError;
use crate::kernel::process::{ProcessError, TaskGroup, TaskGroupError};
use crate::kernel::task::scheduler;

mod bootstrap;
mod capabilities;

const INIT_PATH: &str = "/init";
const INIT_ARGUMENTS: &[&str] = &[INIT_PATH];
pub(crate) enum Error {
    BootFs(crate::kernel::fs::BootFsError),
    #[cfg(not(feature = "kernel-self-test"))]
    ConsoleObject(crate::kernel::device::console::ObjectError),
    FileSystem(crate::kernel::fs::LookupError),
    Handle(HandleError),
    Image(crate::kernel::process::LoaderError),
    IncompleteThreadPublication,
    Missing,
    NotExecutable,
    NotRegularFile,
    Object(crate::kernel::object::ObjectCreationError),
    Process(ProcessError),
    Resource(ResourceError),
    ResourceObject(crate::kernel::accounting::ResourceDomainObjectError),
    Stack(hyper::exec::startup::Error),
    TaskGroup(TaskGroupError),
    TaskObject(crate::kernel::process::TaskObjectError),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BootFs(error) => formatter.debug_tuple("BootFs").field(error).finish(),
            #[cfg(not(feature = "kernel-self-test"))]
            Self::ConsoleObject(error) => {
                formatter.debug_tuple("ConsoleObject").field(error).finish()
            }
            Self::FileSystem(error) => formatter.debug_tuple("FileSystem").field(error).finish(),
            Self::Handle(error) => formatter.debug_tuple("Handle").field(error).finish(),
            Self::Image(error) => formatter.debug_tuple("Image").field(error).finish(),
            Self::IncompleteThreadPublication => formatter.write_str("IncompleteThreadPublication"),
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
    let domain = ResourceDomain::try_new_root(ResourceLimits::UNLIMITED)?;
    let group = TaskGroup::try_new(&domain)?;
    let init = bootstrap::prepare(
        INIT_PATH,
        INIT_ARGUMENTS,
        "init",
        capabilities::HANDLE_COUNT,
        &group,
        &domain,
    )?;

    capabilities::install(&init, INIT_ARGUMENTS, &group, &domain)?;

    init.process.start()?;
    let process_id = init.process.id().get();
    let thread_id = init
        .thread
        .scheduler_id()
        .ok_or(Error::IncompleteThreadPublication)?;
    init.thread.ready()?;
    crate::pr_info!(
        "HypeR: starting Native init process {} as thread {}",
        process_id,
        thread_id.get()
    );

    // Scheduler and Process membership now own the runnable bootstrap task.
    // Keep no observer on this non-returning stack.
    drop(init);
    drop(group);
    drop(domain);
    scheduler::exit_current()
}
