// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Handle-visible VM objects and their shared construction errors.

mod authority;
mod installed;
mod pending;

pub(crate) use crate::kernel::vm::installed::{
    VirtualCpuSnapshot, VirtualMachineConfiguration, VirtualMachineSnapshot,
};
pub(crate) use authority::{VirtualMachineCreationAuthority, VirtualMachineCreationLease};
pub(crate) use installed::{VirtualCpuObject, VirtualMachineObject};
pub(crate) use pending::PendingVirtualMachine;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::mm::user_space::MemoryObjectError;
use crate::kernel::object::{KernelObject, ObjectCreationError, object_allocation_size};

/// Immutable initial machine state for the boot vCPU.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VirtualCpuBootstrap {
    pub(crate) entry: u64,
    pub(crate) stack: u64,
    pub(crate) arguments: [u64; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    BadState,
    InvalidConfiguration,
    Memory(MemoryObjectError),
    MemoryLayout(super::memory::Error),
    Object(ObjectCreationError),
    Registry(super::registry::Error),
    Resource(ResourceError),
    Scheduler(crate::kernel::task::scheduler::Error),
    UnsupportedArchitecture,
    VirtualDevice(super::device::Error),
    VirtualInterrupt(crate::hal::vm::InterruptError),
}

impl From<MemoryObjectError> for Error {
    fn from(error: MemoryObjectError) -> Self {
        Self::Memory(error)
    }
}

impl From<super::memory::Error> for Error {
    fn from(error: super::memory::Error) -> Self {
        Self::MemoryLayout(error)
    }
}

impl From<ObjectCreationError> for Error {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<super::registry::Error> for Error {
    fn from(error: super::registry::Error) -> Self {
        Self::Registry(error)
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<crate::kernel::task::scheduler::Error> for Error {
    fn from(error: crate::kernel::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<super::registry::VcpuPreparationError> for Error {
    fn from(error: super::registry::VcpuPreparationError) -> Self {
        match error {
            super::registry::VcpuPreparationError::Registry(error) => Self::Registry(error),
            super::registry::VcpuPreparationError::Scheduler(error) => Self::Scheduler(error),
        }
    }
}

impl From<super::device::Error> for Error {
    fn from(error: super::device::Error) -> Self {
        Self::VirtualDevice(error)
    }
}

impl From<crate::hal::vm::InterruptError> for Error {
    fn from(error: crate::hal::vm::InterruptError) -> Self {
        Self::VirtualInterrupt(error)
    }
}

impl From<super::installed::Error> for Error {
    fn from(error: super::installed::Error) -> Self {
        match error {
            super::installed::Error::Allocation => Self::Allocation,
            super::installed::Error::BadState => Self::BadState,
        }
    }
}

fn reserve_object_charge<T: KernelObject>(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, Error> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(Error::Allocation)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )?
        .commit())
}
