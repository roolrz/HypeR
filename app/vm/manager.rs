// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet policy and supervision for Native virtual-machine runtimes.

#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::time::Duration;

use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::channel;
use hyper_os::fs::{Directory, File};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, ResourceDomainObject,
    Rights, RightsOffer, TaskGroupObject, VirtualSerialObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::{
    ProcessBuilder, ProcessTermination, create_resource_domain, create_task_group,
};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_rt::ExitCode;
use hyper_service::process as process_contract;
use hyper_service::vm as vm_contract;

const RUNTIME_ARGUMENT: &str = "/svc/vm-runtime";
const MAX_CLIENTS: usize = 8;
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(5);
const CAPABILITY_REPLY_DEADLINE: Duration = Duration::from_millis(100);
const INSTANCE_EXIT_GRACE: Duration = Duration::from_secs(2);

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let mut manager = match FleetManager::from_startup(&mut startup) {
        Ok(manager) => manager,
        Err(_) => return ExitCode::FAILURE,
    };
    match manager.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// Long-lived fleet authority separated from disposable VM instances.
struct FleetManager {
    runtime_image: File,
    libraries: Directory,
    factory: OwnedHandle<hyper_os::handle::TaskFactoryObject>,
    fleet_domain: OwnedHandle<ResourceDomainObject>,
    authority: OwnedHandle<hyper_os::handle::VirtualMachineCreationAuthorityObject>,
    provisioning: CapabilityChannel,
    connections: CapabilityChannel,
    definition: Option<VmDefinition>,
    instance: Option<VmInstance>,
    clients: [Option<Client>; MAX_CLIENTS],
    restart_pending: bool,
    failed: bool,
}

impl FleetManager {
    fn from_startup(startup: &mut Startup<'_>) -> hyper_os::Result<Self> {
        Ok(Self {
            runtime_image: File::from_handle(startup.take(vm_contract::RUNTIME_IMAGE)?),
            libraries: Directory::from_handle(
                startup.take(process_contract::CHILD_LIBRARY_DIRECTORY)?,
            ),
            factory: startup.take(startup::TASK_FACTORY)?,
            fleet_domain: startup.take(startup::RESOURCE_DOMAIN)?,
            authority: startup.take(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)?,
            provisioning: CapabilityChannel::from_handle(startup.take(vm_contract::PROVISIONING)?),
            connections: CapabilityChannel::from_handle(
                startup.take(vm_contract::MANAGER_CONNECTION)?,
            ),
            definition: None,
            instance: None,
            clients: core::array::from_fn(|_| None),
            restart_pending: false,
            failed: false,
        })
    }

    fn run(&mut self) -> hyper_os::Result<()> {
        let provision = self.receive_provision()?;
        self.definition = Some(VmDefinition {
            image: provision.image,
        });
        self.clients[0] = Some(Client::initial(provision.control));
        if self.start_instance().is_err() {
            self.failed = true;
            self.publish_initial_event(vm_contract::InstanceEvent::Failed(
                vm_contract::InstanceFailure::Runtime,
            ));
        }
        loop {
            self.accept_client()?;
            self.observe_one_event()?;
            self.complete_restart_if_ready();
        }
    }

    fn receive_provision(&self) -> hyper_os::Result<Provision> {
        let mut bytes = [MaybeUninit::<u8>::uninit(); vm_contract::MESSAGE_BYTES];
        let mut slots = [
            CapabilityReceiveSlot::new::<hyper_os::handle::FileObject>(
                vm_contract::PROVISIONED_IMAGE_RIGHTS,
            ),
            CapabilityReceiveSlot::new::<ByteChannelObject>(
                vm_contract::PROVISIONED_INSTANCE_CONTROL_RIGHTS,
            ),
        ];
        let message =
            self.provisioning
                .receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)?;
        let request = vm_contract::ProvisionRequest::decode(message.bytes())
            .ok_or(hyper_os::Error::InvalidResponse)?;
        if message.capability_count() != request.capability_count() {
            return Err(hyper_os::Error::InvalidResponse);
        }
        Ok(Provision {
            image: slots[0]
                .take::<hyper_os::handle::FileObject>()?
                .ok_or(hyper_os::Error::MissingHandle)?,
            control: slots[1]
                .take::<ByteChannelObject>()?
                .ok_or(hyper_os::Error::MissingHandle)?,
        })
    }

    /// Opens one short rendezvous window. Accepted clients immediately leave
    /// the shared connector and use private control and capability channels.
    fn accept_client(&mut self) -> hyper_os::Result<()> {
        let deadline = hyper_os::time::deadline_after(EVENT_POLL_INTERVAL)?.as_raw();
        let mut bytes = [MaybeUninit::<u8>::uninit(); vm_contract::MESSAGE_BYTES];
        let mut slots = [
            CapabilityReceiveSlot::new::<ByteChannelObject>(
                Rights::WAIT.union(Rights::READ).union(Rights::WRITE),
            ),
            CapabilityReceiveSlot::new::<CapabilityChannelObject>(
                Rights::WAIT.union(Rights::WRITE),
            ),
        ];
        let message = match self.connections.receive(deadline, &mut bytes, &mut slots) {
            Ok(message) => message,
            Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) => return Ok(()),
            Err(error) => return Err(error),
        };
        if vm_contract::ManagerConnectionRequest::decode(message.bytes()).is_none()
            || message.capability_count() != vm_contract::ManagerConnectionRequest::CAPABILITY_COUNT
        {
            return Ok(());
        }
        let Some(index) = self.clients.iter().position(Option::is_none) else {
            return Ok(());
        };
        self.clients[index] = Some(Client::command(
            slots[0]
                .take::<ByteChannelObject>()?
                .ok_or(hyper_os::Error::MissingHandle)?,
            CapabilityChannel::from_handle(
                slots[1]
                    .take::<CapabilityChannelObject>()?
                    .ok_or(hyper_os::Error::MissingHandle)?,
            ),
        ));
        Ok(())
    }

    fn observe_one_event(&mut self) -> hyper_os::Result<()> {
        const WAIT_CAPACITY: usize = MAX_CLIENTS + 2;
        let filler = WaitItem::new(
            self.connections.as_handle_ref(),
            ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED,
        );
        let mut waits = [filler; WAIT_CAPACITY];
        let mut sources = [WaitSource::Connector; WAIT_CAPACITY];
        let mut count = 0usize;
        if let Some(instance) = self.instance.as_ref() {
            waits[count] = WaitItem::new(
                instance.runtime.as_handle_ref(),
                ObjectSignals::<ProcessObject>::TERMINATED,
            );
            sources[count] = WaitSource::RuntimeProcess;
            count += 1;
            if let Some(control) = instance.runtime_control.as_ref() {
                waits[count] = WaitItem::new(
                    control.as_handle_ref(),
                    ObjectSignals::<ByteChannelObject>::READABLE
                        .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                );
                sources[count] = WaitSource::RuntimeControl;
                count += 1;
            }
        }
        for (index, client) in self.clients.iter().enumerate() {
            let Some(client) = client else {
                continue;
            };
            waits[count] = WaitItem::new(
                client.control.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            );
            sources[count] = WaitSource::Client(index);
            count += 1;
        }
        let deadline = self
            .instance
            .as_ref()
            .and_then(|instance| instance.exit_deadline)
            .map_or(
                hyper_os::time::deadline_after(EVENT_POLL_INTERVAL)?.as_raw(),
                hyper_os::time::FiniteDeadline::as_raw,
            );
        let observation = match wait_many(&waits[..count], deadline) {
            Ok(observation) => observation,
            Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) => {
                let now = hyper_os::time::monotonic_now()?.as_nanoseconds();
                if let Some(instance) = self.instance.as_mut()
                    && instance
                        .exit_deadline
                        .is_some_and(|deadline| deadline.as_raw() <= now)
                {
                    instance.grace_period_expired();
                }
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        match sources[observation.index] {
            WaitSource::RuntimeProcess => self.finish_instance(),
            WaitSource::RuntimeControl => self.handle_runtime_control(observation.observed),
            WaitSource::Client(index) => self.handle_client(index, observation.observed),
            WaitSource::Connector => Ok(()),
        }
    }

    fn handle_runtime_control(&mut self, observed: u64) -> hyper_os::Result<()> {
        let Some(instance) = self.instance.as_mut() else {
            return Ok(());
        };
        if !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) {
            drop(instance.runtime_control.take());
            if !instance.tracker.is_terminal() {
                instance.tracker.reject_protocol();
                instance.force_stop();
            }
        } else if instance.receive_runtime_status().is_err() {
            instance.tracker.reject_protocol();
            instance.force_stop();
        } else if instance.tracker.is_terminal() {
            instance.arm_exit_deadline()?;
        }
        Ok(())
    }

    fn handle_client(&mut self, index: usize, observed: u64) -> hyper_os::Result<()> {
        if !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) {
            self.disconnect_client(index);
            return Ok(());
        }
        let mut bytes = [0u8; vm_contract::MESSAGE_BYTES];
        let length = self.clients[index]
            .as_ref()
            .ok_or(hyper_os::Error::InvalidResponse)?
            .control
            .as_byte_channel()
            .receive(&mut bytes)?;
        let message = bytes
            .get(..length)
            .ok_or(hyper_os::Error::InvalidResponse)?;
        if self.clients[index]
            .as_ref()
            .is_some_and(|client| client.initial)
        {
            if vm_contract::InstanceCommand::decode(message)
                == Some(vm_contract::InstanceCommand::Stop)
            {
                self.request_stop()?;
            } else {
                self.disconnect_client(index);
            }
            return Ok(());
        }
        let Some(command) = vm_contract::FleetCommand::decode(message) else {
            self.disconnect_client(index);
            return Ok(());
        };
        self.execute_command(index, command)
    }

    fn execute_command(
        &mut self,
        client: usize,
        command: vm_contract::FleetCommand,
    ) -> hyper_os::Result<()> {
        match command {
            vm_contract::FleetCommand::List | vm_contract::FleetCommand::Status => self.reply(
                client,
                vm_contract::FleetResponse::State(self.fleet_state()),
            ),
            vm_contract::FleetCommand::Start => {
                if self.instance.is_some() {
                    return self.reply(client, vm_contract::FleetResponse::Busy);
                }
                let response = if self.start_instance().is_ok() {
                    vm_contract::FleetResponse::Accepted
                } else {
                    self.failed = true;
                    vm_contract::FleetResponse::Failed
                };
                self.reply(client, response)
            }
            vm_contract::FleetCommand::Stop => {
                if self.instance.is_none() {
                    return self.reply(
                        client,
                        vm_contract::FleetResponse::State(vm_contract::FleetState::Stopped),
                    );
                }
                self.request_stop()?;
                self.reply(client, vm_contract::FleetResponse::Accepted)
            }
            vm_contract::FleetCommand::Restart => {
                if self.instance.is_none() {
                    let response = if self.start_instance().is_ok() {
                        vm_contract::FleetResponse::Accepted
                    } else {
                        self.failed = true;
                        vm_contract::FleetResponse::Failed
                    };
                    return self.reply(client, response);
                }
                self.restart_pending = true;
                self.request_stop()?;
                self.reply(client, vm_contract::FleetResponse::Accepted)
            }
            vm_contract::FleetCommand::Console => self.attach_console(client),
        }
    }

    fn reply(
        &mut self,
        client: usize,
        response: vm_contract::FleetResponse,
    ) -> hyper_os::Result<()> {
        if self.clients[client]
            .as_ref()
            .ok_or(hyper_os::Error::InvalidResponse)?
            .control
            .as_byte_channel()
            .try_send(&response.encode())
            .is_err()
        {
            self.disconnect_client(client);
        }
        Ok(())
    }

    fn attach_console(&mut self, client: usize) -> hyper_os::Result<()> {
        let Some(instance) = self.instance.as_ref() else {
            return self.reply(client, vm_contract::FleetResponse::Busy);
        };
        if instance.console_client.is_some()
            || self.fleet_state() != vm_contract::FleetState::Running
        {
            return self.reply(client, vm_contract::FleetResponse::Busy);
        }
        let mut serial = Some(
            instance
                .serial
                .duplicate(vm_contract::VIRTUAL_SERIAL_SESSION_RIGHTS)?,
        );
        if let Some(instance) = self.instance.as_mut() {
            instance.console_client = Some(client);
        }
        self.reply(client, vm_contract::FleetResponse::Accepted)?;
        if self.clients[client].is_none() {
            return Ok(());
        }
        let deadline = hyper_os::time::deadline_after(CAPABILITY_REPLY_DEADLINE)?.as_raw();
        let endpoint = self.clients[client]
            .as_ref()
            .and_then(|client| client.capabilities.as_ref())
            .ok_or(hyper_os::Error::InvalidResponse)?;
        loop {
            let waits = [WaitItem::new(
                endpoint.as_handle_ref(),
                ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
            )];
            let observation = match wait_many(&waits, deadline) {
                Ok(observation) => observation,
                Err(_) => {
                    self.disconnect_client(client);
                    return Ok(());
                }
            };
            if !ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                .is_present_in(observation.observed)
            {
                self.disconnect_client(client);
                return Ok(());
            }
            let disposition = CapabilityDisposition::move_handle(
                &mut serial,
                RightsOffer::Exact(vm_contract::VIRTUAL_SERIAL_SESSION_RIGHTS),
            )?;
            match endpoint.try_send(&vm_contract::ConsoleCapability.encode(), &mut [disposition]) {
                Ok(()) => break,
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(_) => {
                    self.disconnect_client(client);
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn disconnect_client(&mut self, index: usize) {
        drop(self.clients[index].take());
        if let Some(instance) = self.instance.as_mut()
            && instance.console_client == Some(index)
        {
            instance.console_client = None;
        }
    }

    fn request_stop(&mut self) -> hyper_os::Result<()> {
        if let Some(instance) = self.instance.as_mut() {
            instance.request_cooperative_stop()?;
        }
        Ok(())
    }

    fn start_instance(&mut self) -> hyper_os::Result<()> {
        if self.instance.is_some() {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let definition = self
            .definition
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?;
        let domain = create_resource_domain(
            self.fleet_domain.as_handle_ref(),
            hyper_app::vm_policy::INITIAL_VM_LIMITS,
        )?;
        let group = create_task_group(self.factory.as_handle_ref(), domain.as_handle_ref())?;
        let lease = hyper_os::vm::derive_creation_lease(
            self.authority.as_handle_ref(),
            domain.as_handle_ref(),
        )?;
        let serial = hyper_os::virtual_serial::create()?;
        let serial_binding = serial.duplicate(vm_contract::RUNTIME_VIRTUAL_SERIAL_RIGHTS)?;
        let (manager_runtime, runtime_control) = channel::create_pair()?;
        let builder = ProcessBuilder::create(
            self.factory.as_handle_ref(),
            group.as_handle_ref(),
            domain.as_handle_ref(),
            self.runtime_image.as_handle_ref(),
        )?;
        builder.set_name("vm-runtime")?;
        builder.add_argument(RUNTIME_ARGUMENT)?;
        builder.add_handle_duplicate(
            self.libraries.as_handle_ref(),
            startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
            RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE)),
        )?;
        builder.add_handle_duplicate(
            definition.image.as_handle_ref(),
            vm_contract::RUNTIME_IMAGE_CONTRACT.purpose(),
            RightsOffer::Exact(vm_contract::RUNTIME_IMAGE_CONTRACT.required_rights()),
        )?;
        builder
            .add_handle_move(
                lease,
                vm_contract::RUNTIME_CREATION_LEASE_CONTRACT.purpose(),
                RightsOffer::Exact(vm_contract::RUNTIME_CREATION_LEASE_CONTRACT.required_rights()),
            )
            .map_err(|failure| failure.error())?;
        builder
            .add_handle_move(
                runtime_control,
                vm_contract::RUNTIME_INSTANCE_CONTROL_CONTRACT.purpose(),
                RightsOffer::Exact(
                    vm_contract::RUNTIME_INSTANCE_CONTROL_CONTRACT.required_rights(),
                ),
            )
            .map_err(|failure| failure.error())?;
        builder
            .add_handle_move(
                serial_binding,
                vm_contract::RUNTIME_VIRTUAL_SERIAL_CONTRACT.purpose(),
                RightsOffer::Exact(vm_contract::RUNTIME_VIRTUAL_SERIAL_CONTRACT.required_rights()),
            )
            .map_err(|failure| failure.error())?;
        builder.seal()?;
        let runtime = builder.start().map_err(|failure| failure.error())?;
        self.instance = Some(VmInstance {
            _resource_domain: domain,
            _task_group: group,
            runtime,
            runtime_control: Some(manager_runtime),
            serial,
            console_client: None,
            tracker: vm_contract::InstanceTracker::new(),
            stop: vm_contract::InstanceStopState::new(),
            exit_deadline: None,
        });
        self.failed = false;
        Ok(())
    }

    fn finish_instance(&mut self) -> hyper_os::Result<()> {
        let Some(mut instance) = self.instance.take() else {
            return Ok(());
        };
        instance
            .runtime
            .as_process_supervisor()
            .wait_terminated(hyper_os::DEADLINE_INFINITE)?;
        instance.drain_runtime_statuses()?;
        let succeeded = matches!(
            instance.runtime.as_process_supervisor().info()?.terminal,
            Some(ProcessTermination::ProcessExited { status: 0 })
        );
        let event = instance.tracker.finish(succeeded);
        self.failed = matches!(event, vm_contract::InstanceEvent::Failed(_));
        self.publish_initial_event(event);
        drop(instance);
        Ok(())
    }

    fn publish_initial_event(&self, event: vm_contract::InstanceEvent) {
        if let Some(client) = self.clients[0].as_ref()
            && client.initial
        {
            let _ = client.control.as_byte_channel().try_send(&event.encode());
        }
    }

    fn complete_restart_if_ready(&mut self) {
        if self.restart_pending && self.instance.is_none() {
            self.restart_pending = false;
            if self.start_instance().is_err() {
                self.failed = true;
            }
        }
    }

    fn fleet_state(&self) -> vm_contract::FleetState {
        let Some(instance) = self.instance.as_ref() else {
            return if self.failed {
                vm_contract::FleetState::Failed
            } else {
                vm_contract::FleetState::Stopped
            };
        };
        if instance.stop != vm_contract::InstanceStopState::new() {
            vm_contract::FleetState::Stopping
        } else if instance.tracker.last_status() == Some(vm_contract::InstanceStatus::Running) {
            vm_contract::FleetState::Running
        } else {
            vm_contract::FleetState::Starting
        }
    }
}

struct VmDefinition {
    image: OwnedHandle<hyper_os::handle::FileObject>,
}

struct Provision {
    image: OwnedHandle<hyper_os::handle::FileObject>,
    control: OwnedHandle<ByteChannelObject>,
}

struct Client {
    control: OwnedHandle<ByteChannelObject>,
    capabilities: Option<CapabilityChannel>,
    initial: bool,
}

impl Client {
    fn initial(control: OwnedHandle<ByteChannelObject>) -> Self {
        Self {
            control,
            capabilities: None,
            initial: true,
        }
    }

    fn command(control: OwnedHandle<ByteChannelObject>, capabilities: CapabilityChannel) -> Self {
        Self {
            control,
            capabilities: Some(capabilities),
            initial: false,
        }
    }
}

#[derive(Clone, Copy)]
enum WaitSource {
    Connector,
    RuntimeProcess,
    RuntimeControl,
    Client(usize),
}

struct VmInstance {
    _resource_domain: OwnedHandle<ResourceDomainObject>,
    _task_group: OwnedHandle<TaskGroupObject>,
    runtime: OwnedHandle<ProcessObject>,
    runtime_control: Option<OwnedHandle<ByteChannelObject>>,
    serial: OwnedHandle<VirtualSerialObject>,
    console_client: Option<usize>,
    tracker: vm_contract::InstanceTracker,
    stop: vm_contract::InstanceStopState,
    exit_deadline: Option<hyper_os::time::FiniteDeadline>,
}

impl VmInstance {
    fn receive_runtime_status(&mut self) -> hyper_os::Result<()> {
        let mut message = [0u8; vm_contract::MESSAGE_BYTES];
        let length = self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .receive(&mut message)?;
        let status = message
            .get(..length)
            .and_then(vm_contract::InstanceStatus::decode)
            .ok_or(hyper_os::Error::InvalidResponse)?;
        self.tracker
            .observe(status)
            .map_err(|_| hyper_os::Error::InvalidResponse)
    }

    fn drain_runtime_statuses(&mut self) -> hyper_os::Result<()> {
        let Some(control) = self.runtime_control.as_ref() else {
            return Ok(());
        };
        loop {
            let mut message = [0u8; vm_contract::MESSAGE_BYTES];
            match control.as_byte_channel().try_receive(&mut message) {
                Ok(length) => match message
                    .get(..length)
                    .and_then(vm_contract::InstanceStatus::decode)
                {
                    Some(status) if self.tracker.observe(status).is_ok() => {}
                    Some(_) | None => self.tracker.reject_protocol(),
                },
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
                | Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn request_cooperative_stop(&mut self) -> hyper_os::Result<()> {
        if self.stop.request_cooperative() != vm_contract::StopAction::SendCooperative {
            return Ok(());
        }
        match self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .send(&vm_contract::InstanceCommand::Stop.encode())
        {
            Ok(()) => self.arm_exit_deadline(),
            Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {
                self.force_stop();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn force_stop(&mut self) {
        self.exit_deadline = None;
        if self.stop.request_forced() == vm_contract::StopAction::ForceProcess {
            let _ = self.runtime.as_process_supervisor().request_stop();
        }
    }

    fn arm_exit_deadline(&mut self) -> hyper_os::Result<()> {
        if self.exit_deadline.is_none() {
            self.exit_deadline = Some(hyper_os::time::deadline_after(INSTANCE_EXIT_GRACE)?);
        }
        Ok(())
    }

    fn grace_period_expired(&mut self) {
        self.exit_deadline = None;
        if self.stop.grace_period_expired() == vm_contract::StopAction::ForceProcess {
            let _ = self.runtime.as_process_supervisor().request_stop();
        }
    }
}

hyper_rt::entry!(application_main);
