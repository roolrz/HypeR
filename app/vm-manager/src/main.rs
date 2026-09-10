// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet policy and supervision for Native virtual-machine runtimes.

use hyper_vm_policy::fleet::{self, Action, Request, Response};
use std::mem::MaybeUninit;
use std::time::Duration;

use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::channel;
use hyper_os::fs::{Directory, File};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, ResourceDomainObject,
    Rights, RightsOffer, TaskGroupObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::{
    ProcessBuilder, ProcessTermination, create_resource_domain, create_task_group,
};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::process as process_contract;
use hyper_service::vm as vm_contract;
use std::process::ExitCode;

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
    root: Directory,
    machines: Vec<Machine>,
    initial_vm: Option<usize>,
    clients: [Option<Client>; MAX_CLIENTS],
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
            root: Directory::from_handle(startup.take(startup::ROOT_DIRECTORY)?),
            machines: Vec::new(),
            initial_vm: None,
            clients: std::array::from_fn(|_| None),
        })
    }

    fn run(&mut self) -> hyper_os::Result<()> {
        let provision = self.receive_provision()?;
        let config = File::from_handle(provision.config);
        let size = config.size()?;
        if size > fleet::MAX_CONFIG_BYTES {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let mut bytes = vec![0; size as usize];
        config.read_exact_at(0, &mut bytes)?;
        let config = fleet::Config::parse(&bytes).map_err(|_| hyper_os::Error::InvalidResponse)?;
        self.clients[0] = Some(Client::initial(provision.control));
        self.install_definitions(config.machines)
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
        self.initial_vm = self
            .machines
            .iter()
            .position(|machine| machine.definition.autostart);
        let has_boot_vm = self.initial_vm.is_some();
        for vm in 0..self.machines.len() {
            if self.machines[vm].definition.autostart && self.start_instance(vm).is_err() {
                self.machines[vm].failed = true;
                self.publish_initial_event(
                    vm,
                    vm_contract::InstanceEvent::Failed(vm_contract::InstanceFailure::Runtime),
                );
            }
        }
        if !has_boot_vm {
            self.publish_boot_event(vm_contract::InstanceEvent::Stopped);
        }
        loop {
            self.accept_client()?;
            self.observe_one_event()?;
            self.complete_restarts()?;
        }
    }

    fn receive_provision(&self) -> hyper_os::Result<Provision> {
        let mut bytes = [MaybeUninit::<u8>::uninit(); vm_contract::MESSAGE_BYTES];
        let mut slots = [
            CapabilityReceiveSlot::new::<hyper_os::handle::FileObject>(
                vm_contract::PROVISIONED_CONFIG_RIGHTS,
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
            config: slots[0]
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
        const WAIT_CAPACITY: usize = MAX_CLIENTS + 2 * fleet::MAX_DEFINITIONS;
        let mut waits = Vec::with_capacity(WAIT_CAPACITY);
        let mut sources = Vec::with_capacity(WAIT_CAPACITY);
        for (vm, machine) in self.machines.iter().enumerate() {
            if let Some(instance) = machine.instance.as_ref() {
                waits.push(WaitItem::new(
                    instance.runtime.as_handle_ref(),
                    ObjectSignals::<ProcessObject>::TERMINATED,
                ));
                sources.push(WaitSource::RuntimeProcess(vm));
                if let Some(control) = instance.runtime_control.as_ref() {
                    waits.push(WaitItem::new(
                        control.as_handle_ref(),
                        ObjectSignals::<ByteChannelObject>::READABLE
                            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                    ));
                    sources.push(WaitSource::RuntimeControl(vm));
                }
            }
        }
        for (index, client) in self.clients.iter().enumerate() {
            let Some(client) = client else {
                continue;
            };
            waits.push(WaitItem::new(
                client.control.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            ));
            sources.push(WaitSource::Client(index));
        }
        // accept_client already waits on the listener each iteration. Once
        // the last instance/client retires there is no secondary wait set.
        if waits.is_empty() {
            return Ok(());
        }
        let deadline = hyper_os::time::deadline_after(EVENT_POLL_INTERVAL)?.as_raw();
        let observation = match wait_many(&waits, deadline) {
            Ok(observation) => observation,
            Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) => {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        match sources[observation.index] {
            WaitSource::RuntimeProcess(vm) => self.finish_instance(vm),
            WaitSource::RuntimeControl(vm) => self.handle_runtime_control(vm, observation.observed),
            WaitSource::Client(index) => self.handle_client(index, observation.observed),
        }
    }

    fn handle_runtime_control(&mut self, vm: usize, observed: u64) -> hyper_os::Result<()> {
        let Some(instance) = self.machines[vm].instance.as_mut() else {
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
        let mut bytes = vec![0u8; fleet::MAX_MESSAGE_BYTES];
        let received = self.clients[index]
            .as_ref()
            .ok_or(hyper_os::Error::InvalidResponse)?
            .control
            .as_byte_channel()
            .try_receive(&mut bytes);
        let length = match received {
            Ok(length) => length,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => return Ok(()),
            Err(_) => {
                self.disconnect_client(index);
                return Ok(());
            }
        };
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
                if let Some(vm) = self.initial_vm {
                    self.request_stop(vm)?;
                }
            } else {
                self.disconnect_client(index);
            }
            return Ok(());
        }
        match fleet::request(message) {
            Ok(command) => match self.execute_command(index, command) {
                Ok(()) => Ok(()),
                Err(error) => self.reply_error(index, &format!("operation failed: {error}")),
            },
            Err(_) => self.reply_error(index, "invalid fleet request"),
        }
    }

    fn install_definitions(&mut self, definitions: Vec<fleet::Definition>) -> Result<(), String> {
        fleet::validate_definitions(&definitions)?;
        if self.machines.len() + definitions.len() > fleet::MAX_DEFINITIONS {
            return Err(format!(
                "at most {} VM definitions are supported",
                fleet::MAX_DEFINITIONS
            ));
        }
        let mut prepared = Vec::new();
        for definition in definitions {
            if self
                .machines
                .iter()
                .any(|machine| machine.definition.name == definition.name)
            {
                return Err(format!("VM '{}' already exists", definition.name));
            }
            let rights = hyper_os::fs::FileRights::from_rights(vm_contract::MANAGED_IMAGE_RIGHTS)
                .ok_or("invalid image rights")?;
            let image = self
                .root
                .open(&definition.image, rights)
                .map_err(|error| format!("cannot open image '{}': {error}", definition.image))?;
            prepared.push(Machine {
                definition,
                image: image.into_handle(),
                instance: None,
                restart_pending: false,
                failed: false,
            });
        }
        // No definition becomes visible until the whole batch is validated.
        self.machines.extend(prepared);
        Ok(())
    }

    fn execute_command(&mut self, client: usize, command: Request) -> hyper_os::Result<()> {
        let (name, action) = match command {
            Request::List => {
                return self.reply(
                    client,
                    Response::Entries {
                        machines: (0..self.machines.len())
                            .map(|vm| self.summary(vm))
                            .collect(),
                    },
                );
            }
            Request::Create { definitions } => {
                let first = self.machines.len();
                if let Err(message) = self.install_definitions(definitions) {
                    return self.reply_error(client, &message);
                }
                let mut failures = Vec::new();
                for vm in first..self.machines.len() {
                    if self.machines[vm].definition.autostart && self.start_instance(vm).is_err() {
                        self.machines[vm].failed = true;
                        failures.push(self.machines[vm].definition.name.clone());
                    }
                }
                if !failures.is_empty() {
                    return self.reply_error(
                        client,
                        &format!(
                            "definitions created, but failed to start: {}",
                            failures.join(", ")
                        ),
                    );
                }
                return self.reply(client, Response::Accepted);
            }
            Request::Control { name, action } => (name, action),
        };
        let Some(vm) = self
            .machines
            .iter()
            .position(|machine| machine.definition.name == name)
        else {
            return self.reply_error(
                client,
                &format!("VM '{name}' does not exist; use 'vmm list'"),
            );
        };
        match action {
            Action::Status => self.reply(
                client,
                Response::Entries {
                    machines: vec![self.summary(vm)],
                },
            ),
            Action::Console => self.attach_console(vm, client),
            Action::Delete => {
                if self.machines[vm].instance.is_some() {
                    return self.reply_error(client, "stop the VM before deleting its definition");
                }
                self.machines.remove(vm);
                if let Some(initial) = self.initial_vm {
                    self.initial_vm = if initial == vm {
                        None
                    } else {
                        Some(initial - usize::from(initial > vm))
                    };
                }
                self.reply(client, Response::Accepted)
            }
            Action::Stop => {
                self.machines[vm].restart_pending = false;
                self.request_stop(vm)?;
                self.reply(client, Response::Accepted)
            }
            Action::Start | Action::Restart => {
                if self.machines[vm].instance.is_some() {
                    if action == Action::Start {
                        return self.reply_error(client, "VM is already active");
                    }
                    self.machines[vm].restart_pending = true;
                    self.request_stop(vm)?;
                } else if let Err(error) = self.start_instance(vm) {
                    self.machines[vm].failed = true;
                    return self.reply_error(client, &format!("cannot start VM '{name}': {error}"));
                }
                self.reply(client, Response::Accepted)
            }
        }
    }

    fn summary(&self, vm: usize) -> fleet::Summary {
        let definition = &self.machines[vm].definition;
        let state = self.fleet_state(vm);
        fleet::Summary {
            name: definition.name.clone(),
            image: definition.image.clone(),
            autostart: definition.autostart,
            state,
        }
    }

    fn reply_error(&mut self, client: usize, message: &str) -> hyper_os::Result<()> {
        self.reply(
            client,
            Response::Error {
                message: message.into(),
            },
        )
    }

    fn reply(&mut self, client: usize, response: Response) -> hyper_os::Result<()> {
        let bytes = fleet::encode(&response).map_err(|_| hyper_os::Error::InvalidResponse)?;
        if self.clients[client]
            .as_ref()
            .ok_or(hyper_os::Error::InvalidResponse)?
            .control
            .as_byte_channel()
            .try_send(&bytes)
            .is_err()
        {
            self.disconnect_client(client);
        }
        Ok(())
    }

    fn attach_console(&mut self, vm: usize, client: usize) -> hyper_os::Result<()> {
        let Some(instance) = self.machines[vm].instance.as_ref() else {
            return self.reply_error(
                client,
                "VM must be running and its console must be unattached",
            );
        };
        if instance.console_client.is_some() || self.fleet_state(vm) != fleet::State::Running {
            return self.reply_error(
                client,
                "VM must be running and its console must be unattached",
            );
        }
        let (runtime_end, client_end) = channel::create_pair()?;
        instance
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .try_send(&vm_contract::InstanceCommand::AttachConsole.encode())?;
        let mut runtime_end = Some(runtime_end);
        if send_console_endpoint(&instance.console_connection, &mut runtime_end).is_err() {
            return self.reply_error(client, "console connection failed");
        }
        let mut console_channel = Some(client_end);
        if let Some(instance) = self.machines[vm].instance.as_mut() {
            instance.console_client = Some(client);
        }
        self.reply(client, Response::Accepted)?;
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
                &mut console_channel,
                RightsOffer::Exact(vm_contract::CONSOLE_SESSION_RIGHTS),
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
        for machine in &mut self.machines {
            if let Some(instance) = machine.instance.as_mut()
                && instance.console_client == Some(index)
            {
                instance.console_client = None;
            }
        }
    }

    fn request_stop(&mut self, vm: usize) -> hyper_os::Result<()> {
        if let Some(instance) = self.machines[vm].instance.as_mut() {
            instance.request_cooperative_stop()?;
        }
        Ok(())
    }

    fn start_instance(&mut self, vm: usize) -> hyper_os::Result<()> {
        if self.machines[vm].instance.is_some() {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let definition = &self.machines[vm];
        let domain = create_resource_domain(
            self.fleet_domain.as_handle_ref(),
            hyper_vm_policy::INITIAL_VM_LIMITS,
        )?;
        let group = create_task_group(self.factory.as_handle_ref(), domain.as_handle_ref())?;
        let lease = hyper_os::vm::derive_creation_lease(
            self.authority.as_handle_ref(),
            domain.as_handle_ref(),
        )?;
        let (console_connection, runtime_connection) = CapabilityChannel::create()?;
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
                runtime_connection.into_handle(),
                vm_contract::RUNTIME_CONSOLE_CONNECTION_CONTRACT.purpose(),
                RightsOffer::Exact(
                    vm_contract::RUNTIME_CONSOLE_CONNECTION_CONTRACT.required_rights(),
                ),
            )
            .map_err(|failure| failure.error())?;
        builder.seal()?;
        let runtime = builder.start().map_err(|failure| failure.error())?;
        self.machines[vm].instance = Some(VmInstance {
            _resource_domain: domain,
            _task_group: group,
            runtime,
            runtime_control: Some(manager_runtime),
            console_connection,
            console_client: None,
            tracker: vm_contract::InstanceTracker::new(),
            stop: vm_contract::InstanceStopState::new(),
            exit_deadline: None,
        });
        self.machines[vm].failed = false;
        Ok(())
    }

    fn finish_instance(&mut self, vm: usize) -> hyper_os::Result<()> {
        let Some(mut instance) = self.machines[vm].instance.take() else {
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
        self.machines[vm].failed = matches!(event, vm_contract::InstanceEvent::Failed(_));
        self.publish_initial_event(vm, event);
        drop(instance);
        Ok(())
    }

    fn publish_boot_event(&self, event: vm_contract::InstanceEvent) {
        if let Some(client) = self.clients[0].as_ref()
            && client.initial
        {
            let _ = client.control.as_byte_channel().try_send(&event.encode());
        }
    }

    fn publish_initial_event(&mut self, vm: usize, event: vm_contract::InstanceEvent) {
        if self.initial_vm == Some(vm) {
            self.initial_vm = None;
            self.publish_boot_event(event);
        }
    }

    fn complete_restarts(&mut self) -> hyper_os::Result<()> {
        let now = hyper_os::time::monotonic_now()?.as_nanoseconds();
        for vm in 0..self.machines.len() {
            if let Some(instance) = self.machines[vm].instance.as_mut()
                && instance
                    .exit_deadline
                    .is_some_and(|deadline| deadline.as_raw() <= now)
            {
                instance.grace_period_expired();
            }
            if self.machines[vm].restart_pending && self.machines[vm].instance.is_none() {
                self.machines[vm].restart_pending = false;
                if self.start_instance(vm).is_err() {
                    self.machines[vm].failed = true;
                }
            }
        }
        Ok(())
    }

    fn fleet_state(&self, vm: usize) -> fleet::State {
        let Some(instance) = self.machines[vm].instance.as_ref() else {
            return if self.machines[vm].failed {
                fleet::State::Failed
            } else {
                fleet::State::Stopped
            };
        };
        if instance.stop != vm_contract::InstanceStopState::new() {
            fleet::State::Stopping
        } else if instance.tracker.last_status() == Some(vm_contract::InstanceStatus::Running) {
            fleet::State::Running
        } else {
            fleet::State::Starting
        }
    }
}

struct Machine {
    definition: fleet::Definition,
    image: OwnedHandle<hyper_os::handle::FileObject>,
    instance: Option<VmInstance>,
    restart_pending: bool,
    failed: bool,
}

struct Provision {
    config: OwnedHandle<hyper_os::handle::FileObject>,
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
    RuntimeProcess(usize),
    RuntimeControl(usize),
    Client(usize),
}

struct VmInstance {
    _resource_domain: OwnedHandle<ResourceDomainObject>,
    _task_group: OwnedHandle<TaskGroupObject>,
    runtime: OwnedHandle<ProcessObject>,
    runtime_control: Option<OwnedHandle<ByteChannelObject>>,
    console_connection: CapabilityChannel,
    console_client: Option<usize>,
    tracker: vm_contract::InstanceTracker,
    stop: vm_contract::InstanceStopState,
    exit_deadline: Option<hyper_os::time::FiniteDeadline>,
}

impl VmInstance {
    fn receive_runtime_status(&mut self) -> hyper_os::Result<()> {
        let mut message = [0u8; vm_contract::MESSAGE_BYTES];
        let received = self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .try_receive(&mut message);
        let length = match received {
            Ok(length) => length,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => return Ok(()),
            Err(error) => return Err(error),
        };
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
            .try_send(&vm_contract::InstanceCommand::Stop.encode())
        {
            Ok(()) => self.arm_exit_deadline(),
            Err(hyper_os::Error::Status(
                hyper_os::Status::PEER_CLOSED | hyper_os::Status::WOULD_BLOCK,
            )) => {
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

fn send_console_endpoint(
    endpoint: &CapabilityChannel,
    handle: &mut Option<OwnedHandle<ByteChannelObject>>,
) -> hyper_os::Result<()> {
    let deadline = hyper_os::time::deadline_after(CAPABILITY_REPLY_DEADLINE)?.as_raw();
    loop {
        let waits = [WaitItem::new(
            endpoint.as_handle_ref(),
            ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
        )];
        let observed = wait_many(&waits, deadline)?;
        if !ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
            .is_present_in(observed.observed)
        {
            return Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED));
        }
        let disposition = CapabilityDisposition::move_handle(
            handle,
            RightsOffer::Exact(vm_contract::CONSOLE_SESSION_RIGHTS),
        )?;
        match endpoint.try_send(&vm_contract::ConsoleCapability.encode(), &mut [disposition]) {
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            result => return result,
        }
    }
}

fn main() -> ExitCode {
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup),
        Err(_) => ExitCode::FAILURE,
    }
}
