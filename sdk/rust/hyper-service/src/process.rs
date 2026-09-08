// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Symbolic manifest names for standard process-construction authorities.

use hyper_os::handle::{DirectoryObject, Rights};
use hyper_os::startup::{self, StartupPurpose};

use crate::StartupContract;

pub const ROOT_DIRECTORY_NAME: &str = "process.root-directory";
pub const TASK_FACTORY_NAME: &str = "process.task-factory";
pub const TASK_GROUP_NAME: &str = "process.task-group";
pub const RESOURCE_DOMAIN_NAME: &str = "process.resource-domain";
pub const TASK_INSPECTOR_NAME: &str = "process.task-inspector";
pub const OBJECT_INSPECTOR_NAME: &str = "process.object-inspector";
pub const MEMORY_INSPECTOR_NAME: &str = "process.memory-inspector";
pub const CPU_INSPECTOR_NAME: &str = "process.cpu-inspector";
pub const CHILD_LIBRARY_DIRECTORY_NAME: &str = "process.child-library-directory";

/// Conventional current-directory authority delegated to command processes.
pub const WORKING_DIRECTORY: StartupPurpose<DirectoryObject> = StartupPurpose::new(0x8004_0001);

/// Library namespace which a process-construction service may attenuate into
/// child startup tables.
pub const CHILD_LIBRARY_DIRECTORY: StartupPurpose<DirectoryObject> =
    StartupPurpose::new(0x8004_0002);

const fn delegated(rights: Rights) -> Rights {
    rights.union(Rights::DUPLICATE).union(Rights::TRANSFER)
}

pub const SHELL_ROOT_DIRECTORY_CONTRACT: StartupContract = StartupContract::exact(
    ROOT_DIRECTORY_NAME,
    startup::ROOT_DIRECTORY,
    Rights::READ
        .union(Rights::EXECUTE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const SHELL_TASK_FACTORY_CONTRACT: StartupContract = StartupContract::exact(
    TASK_FACTORY_NAME,
    startup::TASK_FACTORY,
    Rights::CREATE_PROCESS,
);
pub const SHELL_TASK_GROUP_CONTRACT: StartupContract = StartupContract::exact(
    TASK_GROUP_NAME,
    startup::TASK_GROUP,
    Rights::TASK_GROUP_ATTACH_PROCESS,
);
pub const SHELL_RESOURCE_DOMAIN_CONTRACT: StartupContract = StartupContract::exact(
    RESOURCE_DOMAIN_NAME,
    startup::RESOURCE_DOMAIN,
    Rights::RESOURCE_DOMAIN_SPONSOR,
);
pub const SHELL_TASK_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    TASK_INSPECTOR_NAME,
    startup::TASK_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const SHELL_OBJECT_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    OBJECT_INSPECTOR_NAME,
    startup::OBJECT_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const SHELL_MEMORY_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    MEMORY_INSPECTOR_NAME,
    startup::MEMORY_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const SHELL_CPU_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    CPU_INSPECTOR_NAME,
    startup::CPU_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const CHILD_LIBRARY_DIRECTORY_CONTRACT: StartupContract = StartupContract::exact(
    CHILD_LIBRARY_DIRECTORY_NAME,
    CHILD_LIBRARY_DIRECTORY,
    Rights::READ
        .union(Rights::EXECUTE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const VM_MANAGER_TASK_FACTORY_CONTRACT: StartupContract = StartupContract::exact(
    TASK_FACTORY_NAME,
    startup::TASK_FACTORY,
    Rights::CREATE_PROCESS.union(Rights::CREATE_TASK_GROUP),
);
pub const VM_MANAGER_RESOURCE_DOMAIN_CONTRACT: StartupContract = StartupContract::exact(
    RESOURCE_DOMAIN_NAME,
    startup::RESOURCE_DOMAIN,
    Rights::CREATE_RESOURCE_DOMAIN,
);

pub const SHELL_STARTUP_CONTRACTS: &[StartupContract] = &[
    SHELL_ROOT_DIRECTORY_CONTRACT,
    SHELL_TASK_FACTORY_CONTRACT,
    SHELL_TASK_GROUP_CONTRACT,
    SHELL_RESOURCE_DOMAIN_CONTRACT,
    SHELL_TASK_INSPECTOR_CONTRACT,
    SHELL_OBJECT_INSPECTOR_CONTRACT,
    SHELL_MEMORY_INSPECTOR_CONTRACT,
    SHELL_CPU_INSPECTOR_CONTRACT,
    CHILD_LIBRARY_DIRECTORY_CONTRACT,
];

pub const VM_MANAGER_STARTUP_CONTRACTS: &[StartupContract] = &[
    CHILD_LIBRARY_DIRECTORY_CONTRACT,
    VM_MANAGER_TASK_FACTORY_CONTRACT,
    VM_MANAGER_RESOURCE_DOMAIN_CONTRACT,
];
