// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Transactional process construction from a validated launch plan.

use hyper_init::manifest::{LaunchPlan, Manifest, Service};
use hyper_os::fs::FileRights;
use hyper_os::handle::{OwnedHandle, ProcessObject, Rights, RightsOffer};
use hyper_os::startup;
use hyper_os::task::{ProcessBuilder, StartFailure};

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
        vm_manager_index: Option<usize>,
        plan: &LaunchPlan<'_>,
    ) -> Result<OwnedHandle<ProcessObject>, LaunchError> {
        let executable = self
            .authorities
            .root_directory
            .open(service.image(), FileRights::EXECUTE)
            .map_err(LaunchError::service_operation)?;
        let vm_fleet_scope = Some(service_index) == vm_manager_index
            || service.image() == hyper_init::bootstrap_policy::IO_RUNTIME_IMAGE;
        let builder = ProcessBuilder::create(
            self.authorities.factory.as_handle_ref(),
            if vm_fleet_scope {
                self.authorities.vm()?.group.as_handle_ref()
            } else {
                self.authorities.group.as_handle_ref()
            },
            if vm_fleet_scope {
                self.authorities.vm()?.domain.as_handle_ref()
            } else {
                self.authorities.domain.as_handle_ref()
            },
            executable.as_handle_ref(),
        )
        .map_err(LaunchError::service_operation)?;
        builder
            .set_name(service.name())
            .map_err(LaunchError::service_resources)?;
        builder
            .add_argument(service.image())
            .map_err(LaunchError::service_resources)?;
        builder
            .add_handle_duplicate(
                self.authorities.library_directory.as_handle_ref(),
                startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
                RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE)),
            )
            .map_err(LaunchError::service_resources)?;

        for (capability_index, _) in service.capabilities().enumerate() {
            let grant = plan
                .capability_grant(service_index, capability_index)
                .ok_or(LaunchError::InvalidPlan)?;
            self.authorities.offer(grant, vm_fleet_scope, &builder)?;
        }

        builder.seal().map_err(LaunchError::service_image)?;
        builder.start().map_err(|failure| {
            let (error, committed) = match failure {
                StartFailure::Rejected { error, .. } => (error, false),
                StartFailure::Committed(error) => (error, true),
            };
            let message = format!(
                "HypeR init: service '{}' {} failed: {error:?}\n",
                service.name(),
                if committed { "loader" } else { "start" }
            );
            let _ = self
                .authorities
                .console
                .as_emergency_console()
                .write_all(message.as_bytes());
            if committed {
                LaunchError::service_image(error)
            } else {
                LaunchError::service_resources(error)
            }
        })
    }
}

impl ServiceLauncher {
    pub(super) fn start_initial_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
        vm_manager_index: Option<usize>,
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
                        if !service.critical() && matches!(error, LaunchError::ServiceUnavailable) {
                            continue;
                        }
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
