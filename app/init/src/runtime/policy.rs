// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable bootstrap authority and service-contract policy.

use hyper_init::manifest::{
    AuthorityDeclaration, AuthorityKey, AuthorityPolicy, StartupPurposeDeclaration,
};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, ConsoleObject, CpuInspectorObject, DirectoryObject,
    FileObject, MemoryInspectorObject, ObjectInspectorObject, ResourceDomainObject, Rights,
    TaskFactoryObject, TaskGroupObject, TaskInspectorObject, TypedObject,
    VirtualMachineCreationAuthorityObject,
};
use hyper_service::{
    StartupContract, console as console_contract, process as process_contract,
    session as session_contract, stdio as stdio_contract, vm as vm_contract,
};

const CONSOLE_INPUT_IMAGE: &str = "/svc/console-input";
const CONSOLE_OUTPUT_IMAGE: &str = "/svc/console-output";
const SESSION_IMAGE: &str = "/svc/session";
const SHELL_IMAGE: &str = "/bin/sh";
const VM_MANAGER_IMAGE: &str = "/svc/vm-manager";
pub(super) const VM_RUNTIME_IMAGE: &str = "/svc/vm-runtime";

macro_rules! define_bootstrap_authorities {
    ($( $variant:ident = $value:literal => $source:literal ),+ $(,)?) => {
        #[repr(u16)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(super) enum BootstrapAuthority {
            $($variant = $value),+
        }

        impl BootstrapAuthority {
            pub(super) const fn key(self) -> AuthorityKey {
                AuthorityKey::new(self as u16)
            }

            pub(super) fn from_source(source: &str) -> Option<Self> {
                match source {
                    $($source => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub(super) const fn from_key(key: AuthorityKey) -> Option<Self> {
                match key.as_raw() {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

define_bootstrap_authorities! {
    Console = 1 => "bootstrap.console",
    ConsoleInputChannel = 2 => "bootstrap.console-input-channel",
    ConsoleOutputChannel = 3 => "bootstrap.console-output-channel",
    SessionInputChannel = 4 => "bootstrap.session-input-channel",
    SessionOutputChannel = 5 => "bootstrap.session-output-channel",
    SessionClientInputChannel = 6 => "bootstrap.session-client-input-channel",
    SessionClientOutputChannel = 7 => "bootstrap.session-client-output-channel",
    SessionClientErrorChannel = 8 => "bootstrap.session-client-error-channel",
    ShellInputChannel = 9 => "bootstrap.shell-input-channel",
    ShellOutputChannel = 10 => "bootstrap.shell-output-channel",
    ShellErrorChannel = 11 => "bootstrap.shell-error-channel",
    RootDirectory = 12 => "bootstrap.root-directory",
    DynamicLibraryDirectory = 13 => "bootstrap.dynamic-library-directory",
    TaskFactory = 14 => "bootstrap.task-factory",
    TaskGroup = 15 => "bootstrap.task-group",
    ResourceDomain = 16 => "bootstrap.resource-domain",
    TaskInspector = 17 => "bootstrap.task-inspector",
    ObjectInspector = 18 => "bootstrap.object-inspector",
    MemoryInspector = 19 => "bootstrap.memory-inspector",
    CpuInspector = 20 => "bootstrap.cpu-inspector",
    VmAuthority = 21 => "bootstrap.vm-creation-authority",
    VmRuntimeImage = 22 => "bootstrap.vm-runtime-image",
    VmProvisioningChannel = 23 => "bootstrap.vm-provisioning-channel",
    VmManagerConnectionChannel = 24 => "bootstrap.vm-manager-connection-channel",
    VmClientConnectionChannel = 25 => "bootstrap.vm-client-connection-channel",
}

/// Stateless policy used to validate a manifest before touching live handles.
pub(super) struct BootstrapPolicy;

impl AuthorityPolicy for BootstrapPolicy {
    fn authority<'policy>(&'policy self, source: &str) -> Option<AuthorityDeclaration<'policy>> {
        let authority = BootstrapAuthority::from_source(source)?;
        Some(match authority {
            BootstrapAuthority::Console => {
                duplicate_authority(authority, ConsoleObject::KIND.as_raw(), console_rights())
            }
            BootstrapAuthority::ConsoleInputChannel
            | BootstrapAuthority::ConsoleOutputChannel
            | BootstrapAuthority::SessionInputChannel
            | BootstrapAuthority::SessionOutputChannel
            | BootstrapAuthority::SessionClientInputChannel
            | BootstrapAuthority::SessionClientOutputChannel
            | BootstrapAuthority::SessionClientErrorChannel
            | BootstrapAuthority::ShellInputChannel
            | BootstrapAuthority::ShellOutputChannel
            | BootstrapAuthority::ShellErrorChannel => move_authority(authority),
            BootstrapAuthority::RootDirectory => duplicate_authority(
                authority,
                DirectoryObject::KIND.as_raw(),
                root_directory_rights(),
            ),
            BootstrapAuthority::DynamicLibraryDirectory => duplicate_authority(
                authority,
                DirectoryObject::KIND.as_raw(),
                root_directory_rights(),
            ),
            BootstrapAuthority::TaskFactory => duplicate_authority(
                authority,
                TaskFactoryObject::KIND.as_raw(),
                task_factory_rights(),
            ),
            BootstrapAuthority::TaskGroup => duplicate_authority(
                authority,
                TaskGroupObject::KIND.as_raw(),
                task_group_rights(),
            ),
            BootstrapAuthority::ResourceDomain => duplicate_authority(
                authority,
                ResourceDomainObject::KIND.as_raw(),
                resource_domain_rights(),
            ),
            BootstrapAuthority::TaskInspector => duplicate_authority(
                authority,
                TaskInspectorObject::KIND.as_raw(),
                inspector_rights(),
            ),
            BootstrapAuthority::ObjectInspector => duplicate_authority(
                authority,
                ObjectInspectorObject::KIND.as_raw(),
                inspector_rights(),
            ),
            BootstrapAuthority::MemoryInspector => duplicate_authority(
                authority,
                MemoryInspectorObject::KIND.as_raw(),
                observation_rights(),
            ),
            BootstrapAuthority::CpuInspector => duplicate_authority(
                authority,
                CpuInspectorObject::KIND.as_raw(),
                observation_rights(),
            ),
            BootstrapAuthority::VmAuthority => duplicate_authority(
                authority,
                VirtualMachineCreationAuthorityObject::KIND.as_raw(),
                vm_authority_rights(),
            ),
            BootstrapAuthority::VmRuntimeImage => {
                create_authority(authority, FileObject::KIND.as_raw(), Rights::EXECUTE)
            }
            BootstrapAuthority::VmProvisioningChannel => AuthorityDeclaration {
                key: authority.key(),
                provider: None,
                object_kind: CapabilityChannelObject::KIND.as_raw(),
                rights: Rights::WAIT
                    .union(Rights::READ)
                    .union(Rights::WRITE)
                    .union(Rights::TRANSFER)
                    .bits(),
                movable: true,
                duplicable: false,
                creatable: false,
            },
            BootstrapAuthority::VmManagerConnectionChannel => AuthorityDeclaration {
                key: authority.key(),
                provider: None,
                object_kind: CapabilityChannelObject::KIND.as_raw(),
                rights: Rights::WAIT
                    .union(Rights::READ)
                    .union(Rights::TRANSFER)
                    .bits(),
                movable: true,
                duplicable: false,
                creatable: false,
            },
            BootstrapAuthority::VmClientConnectionChannel => duplicate_authority(
                authority,
                CapabilityChannelObject::KIND.as_raw(),
                vm_contract::MANAGER_CONNECTION_RIGHTS,
            ),
        })
    }

    fn startup_purpose(&self, image: &str, name: &str) -> Option<StartupPurposeDeclaration> {
        let contract = match image {
            CONSOLE_INPUT_IMAGE => find_contract(console_contract::INPUT_STARTUP_CONTRACTS, name),
            CONSOLE_OUTPUT_IMAGE => find_contract(console_contract::OUTPUT_STARTUP_CONTRACTS, name),
            SESSION_IMAGE => find_contract(session_contract::STARTUP_CONTRACTS, name),
            SHELL_IMAGE => find_contract(stdio_contract::STARTUP_CONTRACTS, name)
                .or_else(|| find_contract(process_contract::SHELL_STARTUP_CONTRACTS, name))
                .or_else(|| find_contract(vm_contract::CLIENT_STARTUP_CONTRACTS, name)),
            VM_MANAGER_IMAGE => find_contract(vm_contract::MANAGER_STARTUP_CONTRACTS, name)
                .or_else(|| find_contract(process_contract::VM_MANAGER_STARTUP_CONTRACTS, name)),
            _ => None,
        }?;
        Some(contract_declaration(contract))
    }

    fn right(&self, name: &str) -> Option<u64> {
        match name {
            "duplicate" => Some(Rights::DUPLICATE.bits()),
            "transfer" => Some(Rights::TRANSFER.bits()),
            "wait" => Some(Rights::WAIT.bits()),
            "inspect" => Some(Rights::INSPECT.bits()),
            "read" => Some(Rights::READ.bits()),
            "write" => Some(Rights::WRITE.bits()),
            "execute" => Some(Rights::EXECUTE.bits()),
            "create-process" => Some(Rights::CREATE_PROCESS.bits()),
            "create-task-group" => Some(Rights::CREATE_TASK_GROUP.bits()),
            "create-resource-domain" => Some(Rights::CREATE_RESOURCE_DOMAIN.bits()),
            "create-virtual-machine" => Some(Rights::CREATE_VIRTUAL_MACHINE.bits()),
            "attach-process" => Some(Rights::TASK_GROUP_ATTACH_PROCESS.bits()),
            "sponsor" => Some(Rights::RESOURCE_DOMAIN_SPONSOR.bits()),
            "derive" => Some(Rights::DERIVE.bits()),
            _ => None,
        }
    }
}

fn find_contract(contracts: &[StartupContract], name: &str) -> Option<StartupContract> {
    contracts
        .iter()
        .copied()
        .find(|contract| contract.name() == name)
}

const fn contract_declaration(contract: StartupContract) -> StartupPurposeDeclaration {
    StartupPurposeDeclaration {
        value: contract.purpose(),
        object_kind: contract.object_kind(),
        required_rights: contract.required_rights().bits(),
        allowed_rights: contract.allowed_rights().bits(),
    }
}

const fn duplicate_authority(
    authority: BootstrapAuthority,
    object_kind: u32,
    rights: Rights,
) -> AuthorityDeclaration<'static> {
    AuthorityDeclaration {
        key: authority.key(),
        provider: None,
        object_kind,
        rights: rights.bits(),
        movable: false,
        duplicable: true,
        creatable: false,
    }
}

const fn move_authority(authority: BootstrapAuthority) -> AuthorityDeclaration<'static> {
    AuthorityDeclaration {
        key: authority.key(),
        provider: None,
        object_kind: ByteChannelObject::KIND.as_raw(),
        rights: byte_channel_rights().bits(),
        movable: true,
        duplicable: false,
        creatable: false,
    }
}

const fn create_authority(
    authority: BootstrapAuthority,
    object_kind: u32,
    rights: Rights,
) -> AuthorityDeclaration<'static> {
    AuthorityDeclaration {
        key: authority.key(),
        provider: None,
        object_kind,
        rights: rights.bits(),
        movable: false,
        duplicable: false,
        creatable: true,
    }
}

const fn console_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
}

const fn byte_channel_rights() -> Rights {
    Rights::TRANSFER
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
}

const fn root_directory_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::EXECUTE)
}

const fn task_factory_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::CREATE_PROCESS)
        .union(Rights::CREATE_TASK_GROUP)
}

const fn task_group_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::REQUEST_STOP)
        .union(Rights::TASK_GROUP_ATTACH_PROCESS)
}

const fn resource_domain_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::CREATE_RESOURCE_DOMAIN)
        .union(Rights::RESOURCE_DOMAIN_SPONSOR)
}

const fn vm_authority_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::DERIVE)
        .union(Rights::CREATE_VIRTUAL_MACHINE)
}

const fn inspector_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::DERIVE)
}

const fn observation_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
}
