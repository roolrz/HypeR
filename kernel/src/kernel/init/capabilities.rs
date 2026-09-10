// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel-owned bootstrap authorities for the initial Native process.

use crate::kernel::accounting::{ResourceDomain, ResourceDomainObject};
use crate::kernel::capability::{HandleFlags, PreparedHandle, Rights};
use crate::kernel::inspect::{CpuInspector, MemoryInspector, ObjectInspector, TaskInspector};
use crate::kernel::mm::user_space::VmarObject;
use crate::kernel::object::{ObjectPublication, UserExportableObject};
use crate::kernel::process::{TaskFactory, TaskGroup, TaskGroupObject};

use super::Error;
use super::bootstrap::{self, BootProcess};

#[cfg(not(feature = "kernel-self-test"))]
pub(super) const HANDLE_COUNT: usize = 12;
#[cfg(feature = "kernel-self-test")]
pub(super) const HANDLE_COUNT: usize = 11;

const PURPOSES: [u32; HANDLE_COUNT] = [
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DYNAMIC_LIBRARY_DIRECTORY),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_INSPECTOR),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_INSPECTOR),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_MEMORY_INSPECTOR),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CPU_INSPECTOR),
    purpose(
        hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_VIRTUAL_MACHINE_CREATION_AUTHORITY,
    ),
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR),
    #[cfg(not(feature = "kernel-self-test"))]
    purpose(hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE),
];

pub(super) fn install(
    init: &BootProcess,
    arguments: &[&str],
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<(), Error> {
    bootstrap::install_handles(init, arguments, PURPOSES, || {
        prepare_handles(init, group, domain)
    })
}

fn prepare_handles(
    init: &BootProcess,
    group: &TaskGroup,
    domain: &ResourceDomain,
) -> Result<[PreparedHandle; HANDLE_COUNT], Error> {
    let resource = prepare_handle(
        ResourceDomainObject::try_publication(domain.clone()).map_err(Error::ResourceObject)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::CREATE_RESOURCE_DOMAIN)
            .union(Rights::SET_LIMITS)
            .union(Rights::REVOKE)
            .union(Rights::RESOURCE_DOMAIN_SPONSOR),
    )?;
    let task_group = prepare_handle(
        TaskGroupObject::try_publication(group.clone()).map_err(Error::TaskObject)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::REQUEST_STOP)
            .union(Rights::TASK_GROUP_ATTACH_PROCESS),
    )?;
    let task_factory = prepare_handle(
        ObjectPublication::try_new(TaskFactory::try_new(domain).map_err(Error::TaskObject)?)
            .map_err(Error::Object)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::CREATE_PROCESS)
            .union(Rights::CREATE_TASK_GROUP),
    )?;
    let root = crate::kernel::vfs::root_directory(domain).map_err(Error::RootDirectory)?;
    let library_directory = prepare_handle(
        ObjectPublication::try_new(
            root.open_directory("/lib", domain)
                .map_err(Error::RootDirectory)?,
        )
        .map_err(Error::Object)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::READ)
            .union(Rights::EXECUTE),
    )?;
    let root_directory = prepare_handle(
        ObjectPublication::try_new(root).map_err(Error::Object)?,
        Rights::WRITE
            .union(Rights::SET_ATTRIBUTES)
            .union(Rights::LOCK_FILE)
            .union(Rights::DUPLICATE)
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::READ)
            .union(Rights::EXECUTE),
    )?;
    let task_inspector = prepare_handle(
        ObjectPublication::try_new(TaskInspector::try_system(domain).map_err(Error::Inspection)?)
            .map_err(Error::Object)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::DERIVE),
    )?;
    let object_inspector = prepare_handle(
        ObjectPublication::try_new(ObjectInspector::try_system(domain).map_err(Error::Inspection)?)
            .map_err(Error::Object)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT)
            .union(Rights::DERIVE),
    )?;
    let memory_inspector = prepare_handle(
        ObjectPublication::try_new(MemoryInspector::try_system(domain).map_err(Error::Inspection)?)
            .map_err(Error::Object)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT),
    )?;
    let cpu_inspector = prepare_handle(
        ObjectPublication::try_new(CpuInspector::try_system(domain).map_err(Error::Inspection)?)
            .map_err(Error::Object)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT),
    )?;
    let vm_authority = prepare_handle(
        ObjectPublication::try_new(
            crate::kernel::vm::objects::VirtualMachineCreationAuthority::try_new(domain)
                .map_err(Error::VirtualMachineObject)?,
        )
        .map_err(Error::Object)?,
        <crate::kernel::vm::objects::VirtualMachineCreationAuthority as crate::kernel::object::KernelObject>::SUPPORTED_RIGHTS,
    )?;
    #[cfg(not(feature = "kernel-self-test"))]
    let console = prepare_handle(
        crate::kernel::device::console::SystemConsole::try_publication(domain)
            .map_err(Error::ConsoleObject)?,
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::WAIT)
            .union(Rights::INSPECT)
            .union(Rights::READ)
            .union(Rights::WRITE),
    )?;
    // Prepare the root VMAR last. Its one-per-address-space publication claim
    // needs explicit rollback, while every earlier handle is self-contained.
    let address_space = init.process.address_space_owner()?;
    let root_vmar_publication = VmarObject::try_root_publication(address_space.clone(), domain)
        .map_err(Error::MemoryObject)?;
    let root_vmar = match PreparedHandle::try_from_new_object(
        root_vmar_publication,
        VmarObject::ROOT_RIGHTS,
        HandleFlags::NONE,
    ) {
        Ok(handle) => handle,
        Err(error) => {
            VmarObject::abort_root_publication(&address_space);
            return Err(Error::Handle(error));
        }
    };
    Ok([
        resource,
        task_group,
        task_factory,
        root_directory,
        library_directory,
        task_inspector,
        object_inspector,
        memory_inspector,
        cpu_inspector,
        vm_authority,
        root_vmar,
        #[cfg(not(feature = "kernel-self-test"))]
        console,
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
