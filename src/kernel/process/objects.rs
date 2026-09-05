// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Userspace-exportable task identity and construction authorities.

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectCreationError, ObjectKind, ObjectPublication, SignalSource,
    object_allocation_size, private,
};

use super::{Process, TaskGroup, TaskGroupError};
use crate::kernel::accounting::ResourceDomainObject;

/// Failure while preparing an accounted task capability object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskObjectError {
    AlreadyPublished,
    AllocationSize,
    Object(ObjectCreationError),
    Resource(ResourceError),
    TaskGroup(TaskGroupError),
}

impl From<ObjectCreationError> for TaskObjectError {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<TaskGroupError> for TaskObjectError {
    fn from(error: TaskGroupError) -> Self {
        Self::TaskGroup(error)
    }
}

impl From<ResourceError> for TaskObjectError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

/// Canonical userspace authority over one existing Process lifecycle.
///
/// The contained Process owner is the same owner used by `TaskGroup` membership
/// and scheduler execution. The object adds a KOID and handle rights; it does
/// not create a second task lifecycle or an independently reclaimable task.
pub(crate) struct ProcessObject {
    process: Process,
    _object_charge: CommittedCharge,
}

impl ProcessObject {
    fn try_new(process: Process) -> Result<Self, TaskObjectError> {
        let charge = reserve_object_charge::<Self>(&process.resource_domain())?;
        Ok(Self {
            process,
            _object_charge: charge,
        })
    }

    pub(crate) const fn process(&self) -> &Process {
        &self.process
    }

    /// Constructs the single userspace object identity for this `Process`.
    pub(crate) fn try_publication(
        process: Process,
    ) -> Result<ObjectPublication<Self>, TaskObjectError> {
        if !process.claim_object_publication() {
            return Err(TaskObjectError::AlreadyPublished);
        }
        let result = Self::try_new(process.clone())
            .and_then(|payload| ObjectPublication::try_new(payload).map_err(Into::into));
        if result.is_err() {
            process.abort_object_publication();
        }
        result
    }
}

impl private::Sealed for ProcessObject {}
impl private::UserExportable for ProcessObject {}

impl KernelObject for ProcessObject {
    const KIND: ObjectKind = ObjectKind::PROCESS;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::START)
        .union(Rights::REQUEST_STOP)
        .union(Rights::CREATE_THREAD);

    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(self.process.signal_source())
    }
}

/// Userspace authority over grouped Process lifecycle operations.
pub(crate) struct TaskGroupObject {
    group: TaskGroup,
    _object_charge: CommittedCharge,
}

impl TaskGroupObject {
    fn try_new(group: TaskGroup) -> Result<Self, TaskObjectError> {
        let charge = reserve_object_charge::<Self>(&group.resource_domain())?;
        Ok(Self {
            group,
            _object_charge: charge,
        })
    }

    pub(crate) const fn group(&self) -> &TaskGroup {
        &self.group
    }

    /// Constructs the single userspace object identity for this `TaskGroup`.
    pub(crate) fn try_publication(
        group: TaskGroup,
    ) -> Result<ObjectPublication<Self>, TaskObjectError> {
        if !group.claim_object_publication() {
            return Err(TaskObjectError::AlreadyPublished);
        }
        let result = Self::try_new(group.clone())
            .and_then(|payload| ObjectPublication::try_new(payload).map_err(Into::into));
        if result.is_err() {
            group.abort_object_publication();
        }
        result
    }
}

impl private::Sealed for TaskGroupObject {}
impl private::UserExportable for TaskGroupObject {}

impl KernelObject for TaskGroupObject {
    const KIND: ObjectKind = ObjectKind::TASK_GROUP;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::REQUEST_STOP);
}

/// Stateless authority required to construct task hierarchy objects.
///
/// `ResourceDomain` and `TaskGroup` handles remain separate arguments to creation
/// operations. Possessing this factory never grants access to either object;
/// all participating handles must independently resolve with their required
/// rights before construction begins.
pub(crate) struct TaskFactory {
    _object_charge: CommittedCharge,
}

impl TaskFactory {
    pub(crate) fn try_new(sponsor: &ResourceDomain) -> Result<Self, TaskObjectError> {
        Ok(Self {
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn try_create_task_group(
        &self,
        domain: &ResourceDomainObject,
    ) -> Result<ObjectPublication<TaskGroupObject>, TaskObjectError> {
        TaskGroupObject::try_publication(TaskGroup::try_new(domain.domain())?)
    }
}

impl private::Sealed for TaskFactory {}
impl private::UserExportable for TaskFactory {}

impl KernelObject for TaskFactory {
    const KIND: ObjectKind = ObjectKind::TASK_FACTORY;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::CREATE_PROCESS)
        .union(Rights::CREATE_TASK_GROUP);
}

fn reserve_object_charge<T: KernelObject>(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, TaskObjectError> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(TaskObjectError::AllocationSize)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )?
        .commit())
}
