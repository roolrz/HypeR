// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial virtual-machine capability rendezvous.

use hyper_init::manifest::Manifest;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::fs::{Directory, FileRights};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, ConsoleObject, OwnedHandle, ProcessObject,
    RightsOffer,
};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::vm as vm_contract;

use super::LaunchError;
use super::report::report_service_termination;
use super::supervisor::SupervisorSet;

/// Owns init's endpoints for initial-VM provisioning and supervision.
pub(super) struct InitialVmProvisioner {
    pub(super) init_vm_provisioning_channel: CapabilityChannel,
    pub(super) vm_instance_control_channel: Option<OwnedHandle<ByteChannelObject>>,
    pub(super) manager_vm_instance_control_channel: Option<OwnedHandle<ByteChannelObject>>,
}

impl InitialVmProvisioner {
    /// Provisions the initial VM through a capability rendezvous.
    ///
    /// `PEER_RECEIVING` is the commit precondition for `try_send`; a lost race
    /// blocks again on signals rather than polling or guessing scheduler turns.
    pub(super) fn provision_initial_vm(
        &mut self,
        manifest: &Manifest<'_>,
        manager_index: usize,
        config_path: &str,
        root_directory: &Directory,
        console: &OwnedHandle<ConsoleObject>,
        supervisors: &mut SupervisorSet,
    ) -> Result<(), LaunchError> {
        let manager = supervisors
            .processes
            .get(manager_index)
            .and_then(Option::as_ref)
            .ok_or(LaunchError::InvalidPlan)?;
        let requested = FileRights::from_rights(
            vm_contract::PROVISIONED_CONFIG_RIGHTS.union(hyper_os::handle::Rights::TRANSFER),
        )
        .ok_or(LaunchError::InvalidPlan)?;
        let mut config = Some(
            root_directory
                .open(config_path, requested)
                .map_err(|_| LaunchError::OperatingSystem)?
                .into_handle(),
        );
        let mut control = Some(
            self.manager_vm_instance_control_channel
                .take()
                .ok_or(LaunchError::AuthorityConsumed)?,
        );

        loop {
            let waits = [
                WaitItem::new(
                    self.init_vm_provisioning_channel.as_handle_ref(),
                    ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                        .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
                ),
                WaitItem::new(
                    manager.as_handle_ref(),
                    ObjectSignals::<ProcessObject>::TERMINATED,
                ),
            ];
            let observation = wait_many(&waits, hyper_os::DEADLINE_INFINITE)
                .map_err(|_| LaunchError::OperatingSystem)?;
            if observation.index == 1 {
                let service = manifest
                    .service(manager_index)
                    .ok_or(LaunchError::InvalidPlan)?;
                report_service_termination(console, service.name(), true, manager);
                drop(
                    supervisors
                        .processes
                        .get_mut(manager_index)
                        .ok_or(LaunchError::InvalidPlan)?
                        .take(),
                );
                return Err(LaunchError::VmManagerTerminated);
            }
            if observation.index != 0
                || !ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .is_present_in(observation.observed)
            {
                return Err(LaunchError::VmProvisioningClosed);
            }
            let config_disposition = CapabilityDisposition::move_handle(
                &mut config,
                RightsOffer::Exact(vm_contract::PROVISIONED_CONFIG_RIGHTS),
            )
            .map_err(|_| LaunchError::OperatingSystem)?;
            let control_disposition = CapabilityDisposition::move_handle(
                &mut control,
                RightsOffer::Exact(vm_contract::PROVISIONED_INSTANCE_CONTROL_RIGHTS),
            )
            .map_err(|_| LaunchError::OperatingSystem)?;
            match self.init_vm_provisioning_channel.try_send(
                &vm_contract::ProvisionRequest::ConfigureFleet.encode(),
                &mut [config_disposition, control_disposition],
            ) {
                Ok(()) => return Ok(()),
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {
                    return Err(LaunchError::VmProvisioningClosed);
                }
                Err(_) => return Err(LaunchError::OperatingSystem),
            }
        }
    }
}
