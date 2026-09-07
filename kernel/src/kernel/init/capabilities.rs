// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel-owned bootstrap authorities for the initial Native process.

use crate::kernel::accounting::{ResourceDomain, ResourceDomainObject};
use crate::kernel::capability::{HandleFlags, PreparedHandle, Rights};
use crate::kernel::inspect::{ObjectInspector, TaskInspector};
use crate::kernel::object::{ObjectPublication, UserExportableObject};
use crate::kernel::process::{TaskFactory, TaskGroup, TaskGroupObject};

use super::Error;
use super::bootstrap::{self, BootProcess};

#[cfg(not(feature = "kernel-self-test"))]
pub(super) const HANDLE_COUNT: usize = 7;
#[cfg(feature = "kernel-self-test")]
pub(super) const HANDLE_COUNT: usize = 6;

const PURPOSES: [u32; HANDLE_COUNT] = [
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_INSPECTOR),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_INSPECTOR),
    #[cfg(not(feature = "kernel-self-test"))]
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE),
];

pub(super) fn install(
    init: &BootProcess,
    arguments: &[&str],
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<(), Error> {
    bootstrap::install_handles(init, arguments, PURPOSES, || prepare_handles(group, domain))
}

fn prepare_handles(
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<[PreparedHandle; HANDLE_COUNT], Error> {
    let resource =
        ResourceDomainObject::try_publication(domain.clone()).map_err(Error::ResourceObject)?;
    let task_group = TaskGroupObject::try_publication(group.clone()).map_err(Error::TaskObject)?;
    let task_factory =
        ObjectPublication::try_new(TaskFactory::try_new(domain).map_err(Error::TaskObject)?)
            .map_err(Error::Object)?;
    let root_directory = ObjectPublication::try_new(
        crate::kernel::vfs::root_directory(domain).map_err(Error::RootDirectory)?,
    )
    .map_err(Error::Object)?;
    let task_inspector =
        ObjectPublication::try_new(TaskInspector::try_system(domain).map_err(Error::Inspection)?)
            .map_err(Error::Object)?;
    let object_inspector =
        ObjectPublication::try_new(ObjectInspector::try_system(domain).map_err(Error::Inspection)?)
            .map_err(Error::Object)?;
    #[cfg(not(feature = "kernel-self-test"))]
    let console = crate::kernel::device::console::SystemConsole::try_publication(domain)
        .map_err(Error::ConsoleObject)?;
    Ok([
        prepare_handle(
            resource,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
                .union(Rights::INSPECT)
                .union(Rights::CREATE_RESOURCE_DOMAIN)
                .union(Rights::SET_LIMITS)
                .union(Rights::REVOKE)
                .union(Rights::RESOURCE_DOMAIN_SPONSOR),
        )?,
        prepare_handle(
            task_group,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
                .union(Rights::INSPECT)
                .union(Rights::REQUEST_STOP)
                .union(Rights::TASK_GROUP_ATTACH_PROCESS),
        )?,
        prepare_handle(
            task_factory,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
                .union(Rights::INSPECT)
                .union(Rights::CREATE_PROCESS)
                .union(Rights::CREATE_TASK_GROUP),
        )?,
        prepare_handle(
            root_directory,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
                .union(Rights::INSPECT)
                .union(Rights::READ)
                .union(Rights::EXECUTE),
        )?,
        prepare_handle(
            task_inspector,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
                .union(Rights::INSPECT)
                .union(Rights::DERIVE),
        )?,
        prepare_handle(
            object_inspector,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
                .union(Rights::INSPECT)
                .union(Rights::DERIVE),
        )?,
        #[cfg(not(feature = "kernel-self-test"))]
        prepare_handle(
            console,
            Rights::DUPLICATE
                .union(Rights::TRANSFER)
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

const fn purpose(purpose: u64) -> u32 {
    assert!(purpose <= u32::MAX as u64);
    purpose as u32
}
