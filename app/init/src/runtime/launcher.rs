// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Transactional process construction from a validated launch plan.

use hyper_init::manifest::{LaunchPlan, Manifest, Service};
use hyper_os::fs::FileRights;
use hyper_os::handle::{OwnedHandle, ProcessObject, Rights, RightsOffer};
use hyper_os::startup;
use hyper_os::task::ProcessBuilder;

use super::LaunchError;
use super::authority::AuthorityInventory;
use super::report::report_service_launch_failure;
use super::supervisor::SupervisorSet;

/// Applies validated service declarations to the live authority inventory.
pub(super) struct ServiceLauncher {
    pub(super) authorities: AuthorityInventory,
}

impl ServiceLauncher {
    fn launch_service(
        &mut self,
        service: &Service<'_>,
        service_index: usize,
        vm_manager_index: usize,
        plan: &LaunchPlan<'_>,
    ) -> Result<OwnedHandle<ProcessObject>, LaunchError> {
        let executable = self
            .authorities
            .root_directory
            .open(service.image(), FileRights::EXECUTE)
            .map_err(|_| LaunchError::OperatingSystem)?;
        let builder = ProcessBuilder::create(
            self.authorities.factory.as_handle_ref(),
            if service_index == vm_manager_index {
                self.authorities.vm_fleet_group.as_handle_ref()
            } else {
                self.authorities.group.as_handle_ref()
            },
            if service_index == vm_manager_index {
                self.authorities.vm_fleet_domain.as_handle_ref()
            } else {
                self.authorities.domain.as_handle_ref()
            },
            executable.as_handle_ref(),
        )
        .map_err(|_| LaunchError::OperatingSystem)?;
        builder
            .set_name(service.name())
            .map_err(|_| LaunchError::OperatingSystem)?;
        builder
            .add_argument(service.image())
            .map_err(|_| LaunchError::OperatingSystem)?;
        builder
            .add_handle_duplicate(
                self.authorities.library_directory.as_handle_ref(),
                startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
                RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE)),
            )
            .map_err(|_| LaunchError::OperatingSystem)?;

        for (capability_index, _) in service.capabilities().enumerate() {
            let grant = plan
                .capability_grant(service_index, capability_index)
                .ok_or(LaunchError::InvalidPlan)?;
            self.authorities
                .offer(grant, service_index == vm_manager_index, &builder)?;
        }

        builder.seal().map_err(|_| LaunchError::OperatingSystem)?;
        builder.start().map_err(|_| LaunchError::OperatingSystem)
    }
}

impl ServiceLauncher {
    pub(super) fn start_initial_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
        vm_manager_index: usize,
        supervisors: &mut SupervisorSet,
    ) -> Result<(), LaunchError> {
        for position in 0..plan.service_count() {
            let service_index = plan
                .service_index(position)
                .ok_or(LaunchError::InvalidPlan)?;
            let service = manifest
                .service(service_index)
                .ok_or(LaunchError::InvalidPlan)?;
            if supervisors
                .processes
                .get(service_index)
                .ok_or(LaunchError::InvalidPlan)?
                .is_some()
            {
                return Err(LaunchError::InvalidPlan);
            }
            let supervisor =
                match self.launch_service(service, service_index, vm_manager_index, plan) {
                    Ok(supervisor) => supervisor,
                    Err(error) => {
                        report_service_launch_failure(
                            self.authorities.console,
                            service.name(),
                            &error,
                        );
                        return Err(error);
                    }
                };
            let slot = supervisors
                .processes
                .get_mut(service_index)
                .ok_or(LaunchError::InvalidPlan)?;
            *slot = Some(supervisor);
        }
        Ok(())
    }
}
