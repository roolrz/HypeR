// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Process and initial-VM supervision wait set.

use std::convert::Infallible;

use hyper_init::manifest::{MAX_SERVICES, Manifest};
use hyper_init::supervision::{self, TerminationAction};
use hyper_os::handle::{ByteChannelObject, ConsoleObject, OwnedHandle, ProcessObject};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::vm as vm_contract;

use super::LaunchError;
use super::report::{report_service_termination, report_vm_event, report_vm_protocol_failure};

const _: () = assert!(MAX_SERVICES < hyper_os::wait::MAX_ITEMS);

/// Stable service-index ownership and wait-set mapping.
pub(super) struct SupervisorSet {
    pub(super) processes: [Option<OwnedHandle<ProcessObject>>; MAX_SERVICES],
}

#[derive(Clone, Copy)]
enum SupervisedItem {
    Service(usize),
    InitialVm,
}

impl SupervisorSet {
    pub(super) fn supervise(
        &mut self,
        manifest: &Manifest<'_>,
        vm_instance_control: &mut Option<OwnedHandle<ByteChannelObject>>,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<Infallible, LaunchError> {
        loop {
            let selected = {
                let mut wait_items = Vec::with_capacity(MAX_SERVICES + 1);
                let mut supervised = Vec::with_capacity(MAX_SERVICES + 1);

                for (service_index, supervisor) in self.processes.iter().enumerate() {
                    let Some(supervisor) = supervisor.as_ref() else {
                        continue;
                    };
                    wait_items.push(WaitItem::new(
                        supervisor.as_handle_ref(),
                        ObjectSignals::<ProcessObject>::TERMINATED,
                    ));
                    supervised.push(SupervisedItem::Service(service_index));
                }
                if let Some(control) = vm_instance_control.as_ref() {
                    wait_items.push(WaitItem::new(
                        control.as_handle_ref(),
                        ObjectSignals::<ByteChannelObject>::READABLE
                            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                    ));
                    supervised.push(SupervisedItem::InitialVm);
                }

                let observation = wait_many(&wait_items, hyper_os::DEADLINE_INFINITE)
                    .map_err(|_| LaunchError::OperatingSystem)?;
                (
                    *supervised
                        .get(observation.index)
                        .ok_or(LaunchError::InvalidPlan)?,
                    observation.observed,
                )
            };

            match selected {
                (SupervisedItem::Service(service_index), _) => {
                    let service = manifest
                        .service(service_index)
                        .ok_or(LaunchError::InvalidPlan)?;
                    let supervisor = self
                        .processes
                        .get(service_index)
                        .and_then(Option::as_ref)
                        .ok_or(LaunchError::InvalidPlan)?;
                    report_service_termination(
                        console,
                        service.name(),
                        service.critical(),
                        supervisor,
                    );
                    drop(
                        self.processes
                            .get_mut(service_index)
                            .ok_or(LaunchError::InvalidPlan)?
                            .take(),
                    );
                    if supervision::service_termination_action(service.critical())
                        == TerminationAction::FailSystem
                    {
                        return Err(LaunchError::CriticalServiceTerminated);
                    }
                }
                (SupervisedItem::InitialVm, observed) => {
                    let readable =
                        ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed);
                    if !readable {
                        drop(vm_instance_control.take());
                        report_vm_protocol_failure(
                            console,
                            b"control endpoint closed without event",
                        );
                        return Err(LaunchError::VmInstanceProtocol);
                    }
                    let mut message = [0u8; vm_contract::MESSAGE_BYTES];
                    let received = vm_instance_control
                        .as_ref()
                        .ok_or(LaunchError::InvalidPlan)?
                        .as_byte_channel()
                        .try_receive(&mut message);
                    let length = match received {
                        Ok(length) => length,
                        Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
                        Err(_) => {
                            drop(vm_instance_control.take());
                            report_vm_protocol_failure(console, b"terminal event is unavailable");
                            return Err(LaunchError::VmInstanceProtocol);
                        }
                    };
                    let Some(event) = message
                        .get(..length)
                        .and_then(vm_contract::InstanceEvent::decode)
                    else {
                        drop(vm_instance_control.take());
                        report_vm_protocol_failure(console, b"terminal event is malformed");
                        return Err(LaunchError::VmInstanceProtocol);
                    };
                    report_vm_event(console, event);
                    drop(vm_instance_control.take());
                    if supervision::instance_termination_action(event)
                        == TerminationAction::FailSystem
                    {
                        return Err(LaunchError::VmInstanceFailed);
                    }
                }
            }
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
