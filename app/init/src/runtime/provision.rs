// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One-shot VM fleet configuration and admission acknowledgement.

use hyper_init::manifest::Manifest;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::fs::{Directory, FileRights};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, ConsoleObject, OwnedHandle, Rights, RightsOffer,
};
use hyper_os::wait::{ObjectSignals, WaitItem};
use hyper_service::vm as vm_contract;

use super::LaunchError;
use super::report::report_fleet_configured;
use super::supervisor::SupervisorSet;

const UNAVAILABLE_HANDSHAKE_SECONDS: u64 = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FleetConfiguration {
    Configured,
    Unavailable,
}

/// Owns init's single opportunity to configure the fleet manager.
pub(super) struct FleetProvisioner {
    pub(super) channel: CapabilityChannel,
}

impl FleetProvisioner {
    /// Transfers configuration once and waits only for its admission result.
    /// Guest startup and lifetime remain the manager's responsibility.
    pub(super) fn configure_fleet(
        self,
        manifest: &Manifest<'_>,
        manager_index: usize,
        config_path: &str,
        root_directory: &Directory,
        console: &OwnedHandle<ConsoleObject>,
        supervisors: &mut SupervisorSet,
    ) -> Result<FleetConfiguration, LaunchError> {
        let requested =
            FileRights::from_rights(vm_contract::PROVISIONED_CONFIG_RIGHTS.union(Rights::TRANSFER))
                .ok_or(LaunchError::InvalidPlan)?;
        let config = match root_directory.open(config_path, requested) {
            Ok(config) => config,
            Err(error) if hyper_init::supervision::service_unavailable(&error) => {
                let _ = console
                    .as_emergency_console()
                    .write_all(b"HypeR init: cannot open VM fleet configuration\n");
                return self.configuration_unavailable(
                    manifest,
                    manager_index,
                    console,
                    supervisors,
                );
            }
            Err(_) => return Err(LaunchError::OperatingSystem),
        };
        let mut config = Some(config.into_handle());
        let (result_reader, result_writer) =
            hyper_os::channel::create_pair().map_err(|_| LaunchError::OperatingSystem)?;
        let result_reader = result_reader
            .replace(Rights::READ.union(Rights::WAIT))
            .map_err(|_| LaunchError::OperatingSystem)?;
        let mut result_writer = Some(result_writer);

        loop {
            // Readiness is not a reservation. A lost rendezvous race retains
            // both capabilities and returns to the complete service wait set.
            if !self.wait_for_receiver(
                manifest,
                manager_index,
                console,
                supervisors,
                hyper_os::DEADLINE_INFINITE,
            )? {
                return Ok(FleetConfiguration::Unavailable);
            }
            let config_disposition = CapabilityDisposition::move_handle(
                &mut config,
                RightsOffer::Exact(vm_contract::PROVISIONED_CONFIG_RIGHTS),
            )
            .map_err(|_| LaunchError::OperatingSystem)?;
            let result_disposition = CapabilityDisposition::move_handle(
                &mut result_writer,
                RightsOffer::Exact(vm_contract::PROVISIONED_RESULT_RIGHTS),
            )
            .map_err(|_| LaunchError::OperatingSystem)?;
            match self.channel.try_send(
                &vm_contract::ProvisionRequest::ConfigureFleet.encode(),
                &mut [config_disposition, result_disposition],
            ) {
                Ok(()) => break,
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {
                    return provisioning_closed(manifest, manager_index);
                }
                Err(_) => return Err(LaunchError::OperatingSystem),
            }
        }
        drop(self);
        wait_for_result(
            manifest,
            manager_index,
            &result_reader,
            console,
            supervisors,
        )
    }

    /// Explicitly ends provisioning without reading configuration or allowing
    /// the manager to discover an unavailable I/O broker. No reply is needed:
    /// the capability-channel rendezvous itself proves request delivery.
    pub(super) fn configuration_unavailable(
        self,
        manifest: &Manifest<'_>,
        manager_index: usize,
        console: &OwnedHandle<ConsoleObject>,
        supervisors: &mut SupervisorSet,
    ) -> Result<FleetConfiguration, LaunchError> {
        let deadline = hyper_os::time::deadline_after(std::time::Duration::from_secs(
            UNAVAILABLE_HANDSHAKE_SECONDS,
        ))
        .map_err(|_| LaunchError::OperatingSystem)?
        .as_raw();
        loop {
            if !self.wait_for_receiver(manifest, manager_index, console, supervisors, deadline)? {
                return Ok(FleetConfiguration::Unavailable);
            }
            match self.channel.try_send(
                &vm_contract::ProvisionRequest::ConfigurationUnavailable.encode(),
                &mut [],
            ) {
                Ok(()) => return Ok(FleetConfiguration::Unavailable),
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {
                    return provisioning_closed(manifest, manager_index);
                }
                Err(_) => return Err(LaunchError::OperatingSystem),
            }
        }
    }

    fn wait_for_receiver(
        &self,
        manifest: &Manifest<'_>,
        manager_index: usize,
        console: &OwnedHandle<ConsoleObject>,
        supervisors: &mut SupervisorSet,
        deadline: u64,
    ) -> Result<bool, LaunchError> {
        let observed = match supervisors.wait_for_service_event(
            manifest,
            manager_index,
            WaitItem::new(
                self.channel.as_handle_ref(),
                ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
            ),
            deadline,
            console,
        ) {
            Ok(observed) => observed,
            Err(LaunchError::RequiredServiceUnavailable) => return Ok(false),
            Err(LaunchError::WaitTimedOut) => {
                // No configuration has been delivered at this boundary. An
                // optional nonreceiving manager cannot delay Native forever.
                if manifest
                    .service(manager_index)
                    .ok_or(LaunchError::InvalidPlan)?
                    .critical()
                {
                    return Err(LaunchError::WaitTimedOut);
                }
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        if ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING.is_present_in(observed) {
            Ok(true)
        } else {
            provisioning_closed(manifest, manager_index).map(|_| false)
        }
    }
}

fn provisioning_closed(
    manifest: &Manifest<'_>,
    manager_index: usize,
) -> Result<FleetConfiguration, LaunchError> {
    let service = manifest
        .service(manager_index)
        .ok_or(LaunchError::InvalidPlan)?;
    if service.critical() {
        Err(LaunchError::VmProvisioningClosed)
    } else {
        Ok(FleetConfiguration::Unavailable)
    }
}

fn wait_for_result(
    manifest: &Manifest<'_>,
    manager_index: usize,
    reader: &OwnedHandle<ByteChannelObject>,
    console: &OwnedHandle<ConsoleObject>,
    supervisors: &mut SupervisorSet,
) -> Result<FleetConfiguration, LaunchError> {
    loop {
        let observed = match supervisors.wait_for_service_event(
            manifest,
            manager_index,
            WaitItem::new(
                reader.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            ),
            hyper_os::DEADLINE_INFINITE,
            console,
        ) {
            Ok(observed) => observed,
            Err(LaunchError::RequiredServiceUnavailable) => {
                return Ok(FleetConfiguration::Unavailable);
            }
            Err(error) => return Err(error),
        };
        // A result can be queued when the manager closes its writer. Drain
        // READABLE before interpreting EOF; a close alone is never acceptance.
        if !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) {
            return Err(LaunchError::VmFleetReplyClosed);
        }
        let mut message = [0; vm_contract::MESSAGE_BYTES];
        let length = match reader.as_byte_channel().try_receive(&mut message) {
            Ok(length) => length,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
            Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {
                return Err(LaunchError::VmFleetReplyClosed);
            }
            Err(_) => return Err(LaunchError::VmFleetReplyInvalid),
        };
        match message
            .get(..length)
            .and_then(vm_contract::ProvisionResult::decode)
        {
            Some(vm_contract::ProvisionResult::Configured) => {
                report_fleet_configured(console);
                return Ok(FleetConfiguration::Configured);
            }
            Some(vm_contract::ProvisionResult::Rejected) => {
                let _ = console
                    .as_emergency_console()
                    .write_all(b"HypeR init: VM fleet configuration rejected\n");
                return Ok(FleetConfiguration::Unavailable);
            }
            None => return Err(LaunchError::VmFleetReplyInvalid),
        }
    }
}
