// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Live bootstrap authority inventory and typed capability offers.

use hyper_app::manifest::{CapabilityGrant, CapabilityOperation};
use hyper_os::fs::{Directory, FileRights};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, ConsoleObject, CpuInspectorObject, DirectoryObject,
    FileObject, MemoryInspectorObject, ObjectInspectorObject, OwnedHandle, ResourceDomainObject,
    Rights, RightsOffer, TaskFactoryObject, TaskGroupObject, TaskInspectorObject, TypedObject,
    VirtualMachineCreationAuthorityObject,
};
use hyper_os::task::ProcessBuilder;

use super::LaunchError;
use super::policy::*;

/// Live bootstrap capabilities from which service offers are prepared.
pub(super) struct AuthorityInventory {
    pub(super) root_directory: Directory,
    pub(super) library_directory: Directory,
    pub(super) factory: OwnedHandle<TaskFactoryObject>,
    pub(super) group: OwnedHandle<TaskGroupObject>,
    pub(super) domain: OwnedHandle<ResourceDomainObject>,
    pub(super) vm_fleet_group: OwnedHandle<TaskGroupObject>,
    pub(super) vm_fleet_domain: OwnedHandle<ResourceDomainObject>,
    pub(super) task_inspector: OwnedHandle<TaskInspectorObject>,
    pub(super) object_inspector: OwnedHandle<ObjectInspectorObject>,
    pub(super) memory_inspector: OwnedHandle<MemoryInspectorObject>,
    pub(super) cpu_inspector: OwnedHandle<CpuInspectorObject>,
    pub(super) vm_authority: OwnedHandle<VirtualMachineCreationAuthorityObject>,
    pub(super) console: OwnedHandle<ConsoleObject>,
    pub(super) console_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) console_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) session_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) session_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) session_client_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) session_client_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) session_client_error_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) shell_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) shell_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) shell_error_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) vm_provisioning_channel: Option<OwnedHandle<CapabilityChannelObject>>,
}

impl AuthorityInventory {
    pub(super) fn offer(
        &mut self,
        grant: CapabilityGrant,
        vm_fleet_scope: bool,
        builder: &ProcessBuilder,
    ) -> Result<(), LaunchError> {
        let authority =
            BootstrapAuthority::from_key(grant.authority()).ok_or(LaunchError::InvalidPlan)?;
        let operation = grant.operation();
        let kind = grant.object_kind();
        let purpose = grant.purpose();
        let rights = Rights::from_bits(grant.rights()).ok_or(LaunchError::InvalidPlan)?;
        let offer = RightsOffer::Exact(rights);
        match (authority, operation) {
            (BootstrapAuthority::Console, CapabilityOperation::Duplicate)
                if kind == ConsoleObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.console.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::RootDirectory, CapabilityOperation::Duplicate)
                if kind == DirectoryObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.root_directory.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::DynamicLibraryDirectory, CapabilityOperation::Duplicate)
                if kind == DirectoryObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.library_directory.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::TaskFactory, CapabilityOperation::Duplicate)
                if kind == TaskFactoryObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.factory.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::TaskGroup, CapabilityOperation::Duplicate)
                if kind == TaskGroupObject::KIND.as_raw() =>
            {
                let group = if vm_fleet_scope {
                    self.vm_fleet_group.as_handle_ref()
                } else {
                    self.group.as_handle_ref()
                };
                builder.add_handle_duplicate(group, purpose, offer)
            }
            (BootstrapAuthority::ResourceDomain, CapabilityOperation::Duplicate)
                if kind == ResourceDomainObject::KIND.as_raw() =>
            {
                let domain = if vm_fleet_scope {
                    self.vm_fleet_domain.as_handle_ref()
                } else {
                    self.domain.as_handle_ref()
                };
                builder.add_handle_duplicate(domain, purpose, offer)
            }
            (BootstrapAuthority::TaskInspector, CapabilityOperation::Duplicate)
                if kind == TaskInspectorObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.task_inspector.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::ObjectInspector, CapabilityOperation::Duplicate)
                if kind == ObjectInspectorObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.object_inspector.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::MemoryInspector, CapabilityOperation::Duplicate)
                if kind == MemoryInspectorObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.memory_inspector.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::CpuInspector, CapabilityOperation::Duplicate)
                if kind == CpuInspectorObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.cpu_inspector.as_handle_ref(), purpose, offer)
            }
            (BootstrapAuthority::VmAuthority, CapabilityOperation::Duplicate)
                if kind == VirtualMachineCreationAuthorityObject::KIND.as_raw() =>
            {
                builder.add_handle_duplicate(self.vm_authority.as_handle_ref(), purpose, offer)
            }
            (authority, CapabilityOperation::Move) if kind == ByteChannelObject::KIND.as_raw() => {
                return self.move_channel_into_builder(authority, builder, purpose, rights);
            }
            (BootstrapAuthority::VmProvisioningChannel, CapabilityOperation::Move)
                if kind == CapabilityChannelObject::KIND.as_raw() =>
            {
                return self.move_vm_provisioning_into_builder(builder, purpose, rights);
            }
            (BootstrapAuthority::VmRuntimeImage, CapabilityOperation::Create)
                if kind == FileObject::KIND.as_raw() =>
            {
                return self.open_vm_runtime_into_builder(builder, purpose, rights);
            }
            _ => return Err(LaunchError::UnsupportedAuthority),
        }
        .map_err(|_| LaunchError::OperatingSystem)
    }

    fn move_channel_into_builder(
        &mut self,
        authority: BootstrapAuthority,
        builder: &ProcessBuilder,
        purpose: u32,
        rights: Rights,
    ) -> Result<(), LaunchError> {
        let slot = match authority {
            BootstrapAuthority::ConsoleInputChannel => &mut self.console_input_channel,
            BootstrapAuthority::ConsoleOutputChannel => &mut self.console_output_channel,
            BootstrapAuthority::SessionInputChannel => &mut self.session_input_channel,
            BootstrapAuthority::SessionOutputChannel => &mut self.session_output_channel,
            BootstrapAuthority::SessionClientInputChannel => &mut self.session_client_input_channel,
            BootstrapAuthority::SessionClientOutputChannel => {
                &mut self.session_client_output_channel
            }
            BootstrapAuthority::SessionClientErrorChannel => &mut self.session_client_error_channel,
            BootstrapAuthority::ShellInputChannel => &mut self.shell_input_channel,
            BootstrapAuthority::ShellOutputChannel => &mut self.shell_output_channel,
            BootstrapAuthority::ShellErrorChannel => &mut self.shell_error_channel,
            _ => return Err(LaunchError::UnsupportedAuthority),
        };
        let channel = slot.take().ok_or(LaunchError::AuthorityConsumed)?;
        match builder.add_handle_move(channel, purpose, RightsOffer::Exact(rights)) {
            Ok(()) => Ok(()),
            Err(failure) => {
                let (_, channel) = failure.into_parts();
                *slot = Some(channel);
                Err(LaunchError::OperatingSystem)
            }
        }
    }

    fn move_vm_provisioning_into_builder(
        &mut self,
        builder: &ProcessBuilder,
        purpose: u32,
        rights: Rights,
    ) -> Result<(), LaunchError> {
        let channel = self
            .vm_provisioning_channel
            .take()
            .ok_or(LaunchError::AuthorityConsumed)?;
        match builder.add_handle_move(channel, purpose, RightsOffer::Exact(rights)) {
            Ok(()) => Ok(()),
            Err(failure) => {
                let (_, channel) = failure.into_parts();
                self.vm_provisioning_channel = Some(channel);
                Err(LaunchError::OperatingSystem)
            }
        }
    }

    fn open_vm_runtime_into_builder(
        &self,
        builder: &ProcessBuilder,
        purpose: u32,
        rights: Rights,
    ) -> Result<(), LaunchError> {
        // TRANSFER is an init-private transport right. The builder publishes
        // only the manifest-approved `rights` mask to the destination.
        let requested = FileRights::from_rights(rights.union(Rights::TRANSFER))
            .ok_or(LaunchError::InvalidPlan)?;
        let file = self
            .root_directory
            .open(VM_RUNTIME_IMAGE, requested)
            .map_err(|_| LaunchError::OperatingSystem)?;
        builder
            .add_handle_move(file.into_handle(), purpose, RightsOffer::Exact(rights))
            .map_err(|_| LaunchError::OperatingSystem)
    }
}
