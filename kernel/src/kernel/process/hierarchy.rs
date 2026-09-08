// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-checked construction of process-accounting hierarchy objects.

use crate::kernel::accounting::{ResourceDomainObject, ResourceDomainObjectError, ResourceLimits};
use crate::kernel::authority::Rights;
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle};
use crate::kernel::object::KernelObject;

use super::{Process, ProcessError, TaskFactory, TaskGroupObject, TaskObjectError};

#[derive(Debug)]
pub(crate) enum Error {
    NotSupported,
    Process(ProcessError),
    ResourceDomain(ResourceDomainObjectError),
    Task(TaskObjectError),
}

impl From<ProcessError> for Error {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<ResourceDomainObjectError> for Error {
    fn from(error: ResourceDomainObjectError) -> Self {
        Self::ResourceDomain(error)
    }
}

impl From<TaskObjectError> for Error {
    fn from(error: TaskObjectError) -> Self {
        Self::Task(error)
    }
}

pub(crate) fn create_resource_domain(
    process: &Process,
    parent: HandleValue,
    limits: ResourceLimits,
) -> Result<HandleValue, Error> {
    let parent =
        process.resolve_handle::<ResourceDomainObject>(parent, Rights::CREATE_RESOURCE_DOMAIN)?;
    let reservation = process.reserve_handles::<1>()?;
    let publication = match parent.object().try_new_child(limits) {
        Ok(publication) => publication,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(error.into());
        }
    };
    let prepared = match PreparedHandle::try_from_new_object(
        publication,
        <ResourceDomainObject as KernelObject>::SUPPORTED_RIGHTS,
        HandleFlags::NONE,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(ProcessError::Handle(error).into());
        }
    };
    process
        .publish_handles(reservation, [prepared])
        .map(|values| values[0])
        .map_err(|failure| Error::Process(failure.error))
}

pub(crate) fn create_task_group(
    process: &Process,
    factory: HandleValue,
    domain: HandleValue,
) -> Result<HandleValue, Error> {
    let factory = process.resolve_handle::<TaskFactory>(factory, Rights::CREATE_TASK_GROUP)?;
    let domain =
        process.resolve_handle::<ResourceDomainObject>(domain, Rights::RESOURCE_DOMAIN_SPONSOR)?;
    let reservation = process.reserve_handles::<1>()?;
    let publication = match factory.object().try_create_task_group(domain.object()) {
        Ok(publication) => publication,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(error.into());
        }
    };
    let prepared = match PreparedHandle::try_from_new_object(
        publication,
        <TaskGroupObject as KernelObject>::SUPPORTED_RIGHTS,
        HandleFlags::NONE,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(ProcessError::Handle(error).into());
        }
    };
    process
        .publish_handles(reservation, [prepared])
        .map(|values| values[0])
        .map_err(|failure| Error::Process(failure.error))
}
