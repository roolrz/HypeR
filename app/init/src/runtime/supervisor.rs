// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Service process supervision during startup handshakes and steady operation.

use std::convert::Infallible;

use hyper_init::manifest::{MAX_SERVICES, Manifest};
use hyper_init::supervision::{self, TerminationAction};
use hyper_os::handle::{ByteChannelObject, ConsoleObject, OwnedHandle, ProcessObject};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};

use super::LaunchError;
use super::report::report_service_termination;

const _: () = assert!(MAX_SERVICES < hyper_os::wait::MAX_ITEMS);

/// Stable service-index ownership and wait-set mapping.
pub(super) struct SupervisorSet {
    pub(super) processes: [Option<OwnedHandle<ProcessObject>>; MAX_SERVICES],
}

impl SupervisorSet {
    /// Waits for one startup endpoint while continuing to supervise every
    /// service. The endpoint's provider must remain alive until completion.
    pub(super) fn wait_for_service_event(
        &mut self,
        manifest: &Manifest<'_>,
        required_service: usize,
        event: WaitItem<'_>,
        deadline: u64,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<u64, LaunchError> {
        if self
            .processes
            .get(required_service)
            .and_then(Option::as_ref)
            .is_none()
        {
            return Err(LaunchError::InvalidPlan);
        }
        loop {
            let selected = {
                let mut items = Vec::with_capacity(MAX_SERVICES + 1);
                let mut services = Vec::with_capacity(MAX_SERVICES);
                for (index, process) in self.processes.iter().enumerate() {
                    if let Some(process) = process {
                        items.push(WaitItem::new(
                            process.as_handle_ref(),
                            ObjectSignals::<ProcessObject>::TERMINATED,
                        ));
                        services.push(index);
                    }
                }
                // Process events precede endpoint readiness, so a queued
                // successful reply cannot hide an observed critical failure.
                items.push(event);
                let observation =
                    wait_many(&items, deadline).map_err(|_| LaunchError::OperatingSystem)?;
                if observation.index == services.len() {
                    return Ok(observation.observed);
                }
                *services
                    .get(observation.index)
                    .ok_or(LaunchError::InvalidPlan)?
            };
            self.observe_service_termination(manifest, selected, console)?;
            if selected == required_service {
                return Err(LaunchError::CriticalServiceTerminated);
            }
        }
    }

    pub(super) fn wait_for_storage(
        &mut self,
        manifest: &Manifest<'_>,
        ready_service: usize,
        reader: &OwnedHandle<ByteChannelObject>,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<(), LaunchError> {
        let deadline = hyper_os::time::deadline_after(std::time::Duration::from_secs(
            hyper_service::io::READY_TIMEOUT_SECONDS,
        ))
        .map_err(|_| LaunchError::OperatingSystem)?
        .as_raw();
        loop {
            let observed = self
                .wait_for_service_event(
                    manifest,
                    ready_service,
                    WaitItem::new(
                        reader.as_handle_ref(),
                        ObjectSignals::<ByteChannelObject>::READABLE
                            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                    ),
                    deadline,
                    console,
                )
                .map_err(|error| match error {
                    LaunchError::OperatingSystem => LaunchError::StorageNotReady,
                    other => other,
                })?;
            if !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) {
                return Err(LaunchError::StorageNotReady);
            }
            let mut message = [0; hyper_service::io::READY_MESSAGE.len()];
            match reader.as_byte_channel().try_receive(&mut message) {
                Ok(length) if message.get(..length) == Some(hyper_service::io::READY_MESSAGE) => {
                    return Ok(());
                }
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
                _ => return Err(LaunchError::StorageNotReady),
            }
        }
    }

    pub(super) fn supervise(
        &mut self,
        manifest: &Manifest<'_>,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<Infallible, LaunchError> {
        loop {
            let selected = {
                let mut items = Vec::with_capacity(MAX_SERVICES);
                let mut services = Vec::with_capacity(MAX_SERVICES);
                for (index, process) in self.processes.iter().enumerate() {
                    if let Some(process) = process {
                        items.push(WaitItem::new(
                            process.as_handle_ref(),
                            ObjectSignals::<ProcessObject>::TERMINATED,
                        ));
                        services.push(index);
                    }
                }
                let observation = wait_many(&items, hyper_os::DEADLINE_INFINITE)
                    .map_err(|_| LaunchError::OperatingSystem)?;
                *services
                    .get(observation.index)
                    .ok_or(LaunchError::InvalidPlan)?
            };
            self.observe_service_termination(manifest, selected, console)?;
        }
    }

    fn observe_service_termination(
        &mut self,
        manifest: &Manifest<'_>,
        index: usize,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<(), LaunchError> {
        let service = manifest.service(index).ok_or(LaunchError::InvalidPlan)?;
        let process = self
            .processes
            .get_mut(index)
            .and_then(Option::take)
            .ok_or(LaunchError::InvalidPlan)?;
        report_service_termination(console, service.name(), service.critical(), &process);
        if supervision::service_termination_action(service.critical())
            == TerminationAction::FailSystem
        {
            Err(LaunchError::CriticalServiceTerminated)
        } else {
            Ok(())
        }
    }

    pub(super) fn request_service_stop(&self) -> Result<(), LaunchError> {
        let mut failed = false;
        for supervisor in self.processes.iter().filter_map(Option::as_ref) {
            if supervisor.as_process_supervisor().request_stop().is_err() {
                failed = true;
            }
        }
        if failed {
            Err(LaunchError::StopRollbackFailed)
        } else {
            Ok(())
        }
    }
}
