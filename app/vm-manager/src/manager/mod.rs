// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet ownership and client dispatch, with isolated runtime transactions.

mod instance;
mod launch;
mod listener;

use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::channel;
use hyper_os::fs::{Directory, File};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, ResourceDomainObject,
    RightsOffer,
};
use hyper_os::startup::{self, Startup};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::process as process_contract;
use hyper_service::vm as vm_contract;
use hyper_vm_manager::MachinePolicy;
use hyper_vm_policy::fleet::{self, Action, Request, Response};
use instance::VmInstance;
use std::io::Read;
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};

const MAX_CLIENTS: usize = 8;
const CAPABILITY_REPLY_DEADLINE: Duration = Duration::from_millis(100);

/// Long-lived fleet authority separated from disposable VM instances.
pub(super) struct FleetManager {
    runtime_image: File,
    libraries: Directory,
    factory: OwnedHandle<hyper_os::handle::TaskFactoryObject>,
    fleet_domain: OwnedHandle<ResourceDomainObject>,
    authority: OwnedHandle<hyper_os::handle::VirtualMachineCreationAuthorityObject>,
    provisioning: CapabilityChannel,
    io_broker: Option<CapabilityChannel>,
    connections: listener::Listener,
    root: Directory,
    machines: Vec<Machine>,
    initial_vm: Option<usize>,
    clients: [Option<Client>; MAX_CLIENTS],
    next_wait: usize,
}

impl FleetManager {
    pub(super) fn from_startup(startup: &mut Startup<'_>) -> hyper_os::Result<Self> {
        Ok(Self {
            runtime_image: File::from_handle(startup.take(vm_contract::RUNTIME_IMAGE)?),
            libraries: Directory::from_handle(
                startup.take(process_contract::CHILD_LIBRARY_DIRECTORY)?,
            ),
            factory: startup.take(startup::TASK_FACTORY)?,
            fleet_domain: startup.take(startup::RESOURCE_DOMAIN)?,
            authority: startup.take(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)?,
            provisioning: CapabilityChannel::from_handle(startup.take(vm_contract::PROVISIONING)?),
            io_broker: startup
                .take_optional(hyper_service::io::BROKER_CLIENT)?
                .map(CapabilityChannel::from_handle),
            connections: listener::Listener::start(CapabilityChannel::from_handle(
                startup.take(vm_contract::MANAGER_CONNECTION)?,
            ))?,
            root: Directory::from_handle(startup.take(startup::ROOT_DIRECTORY)?),
            machines: Vec::new(),
            initial_vm: None,
            clients: std::array::from_fn(|_| None),
            next_wait: 0,
        })
    }

    pub(super) fn run(&mut self) -> hyper_os::Result<()> {
        eprintln!("HypeR vm-manager: ready");
        let provision = self.receive_provision()?;
        let config = File::from_handle(provision.config).into_std();
        // Bound allocation even if the file grows after provisioning.
        let mut bytes = Vec::new();
        config
            .take(fleet::MAX_CONFIG_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
        if bytes.len() as u64 > fleet::MAX_CONFIG_BYTES {
            return Err(hyper_os::Error::InvalidResponse);
        }
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
                self.machines[vm].policy.start_failed();
                self.publish_initial_event(
                    vm,
                    vm_contract::InstanceEvent::Failed(vm_contract::InstanceFailure::Runtime),
                );
            }
        }
        if !has_boot_vm {
            self.publish_boot_event(vm_contract::BootEvent::NoAutostart);
        }
        loop {
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

    fn accept_client(&mut self) -> hyper_os::Result<()> {
        let Some(connection) = self.connections.accept()? else {
            return Ok(());
        };
        if let Some(index) = self.clients.iter().position(Option::is_none) {
            self.clients[index] =
                Some(Client::command(connection.control, connection.capabilities));
        } else {
            let bytes = fleet::encode(&Response::Error {
                message: format!("client limit reached (maximum {MAX_CLIENTS} connections); retry after another client exits"),
            })
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
            // A fresh connection has no earlier server response. Never block
            // the supervisor on a client that is not reading or has exited.
            if let Err(error) = connection.control.as_byte_channel().try_send(&bytes) {
                eprintln!(
                    "HypeR vm-manager: client limit reached; rejection delivery failed: {error}"
                );
            }
            // Dropping the rejected connection closes both received handles.
        }
        Ok(())
    }

    fn observe_one_event(&mut self) -> hyper_os::Result<()> {
        const WAIT_CAPACITY: usize = 2 + MAX_CLIENTS + 2 * fleet::MAX_DEFINITIONS;
        let mut waits = Vec::with_capacity(WAIT_CAPACITY);
        let mut sources = Vec::with_capacity(WAIT_CAPACITY);
        waits.push(self.connections.wait_item());
        sources.push(WaitSource::Connection);
        if self.machines.iter().any(|machine| {
            machine
                .instance
                .as_ref()
                .is_some_and(VmInstance::wants_disk_admission)
        }) && let Some(broker) = self.io_broker.as_ref()
        {
            waits.push(WaitItem::new(
                broker.as_handle_ref(),
                ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
            ));
            sources.push(WaitSource::DiskAdmission);
        }
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
        let deadline = match self
            .machines
            .iter()
            .filter_map(|machine| {
                machine
                    .instance
                    .as_ref()
                    .and_then(|instance| instance.policy.exit_deadline())
            })
            .min()
        {
            Some(deadline) => {
                hyper_os::time::deadline_after(deadline.saturating_duration_since(Instant::now()))?
                    .as_raw()
            }
            None => hyper_os::DEADLINE_INFINITE,
        };
        // Ready connection traffic must not starve lifecycle/control events.
        let first = self.next_wait % waits.len();
        waits.rotate_left(first);
        sources.rotate_left(first);
        let observation = match wait_many(&waits, deadline) {
            Ok(observation) => observation,
            Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) => {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        self.next_wait = (first + observation.index + 1) % sources.len();
        match sources[observation.index] {
            WaitSource::Connection => self.accept_client(),
            WaitSource::DiskAdmission => self.admit_disk(),
            WaitSource::RuntimeProcess(vm) => self.finish_instance(vm),
            WaitSource::RuntimeControl(vm) => self.handle_runtime_control(vm),
            WaitSource::Client(index) => self.handle_client(index, observation.observed),
        }
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
                    self.machines[vm].policy.request_stop(false);
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
            if definition.name == "io" {
                return Err("VM name 'io' is reserved for the read-only I/O VM".into());
            }
            if self
                .machines
                .iter()
                .any(|machine| machine.definition.name == definition.name)
            {
                return Err(format!("VM '{}' already exists", definition.name));
            }
            if let Some(disk) = &definition.disk
                && self
                    .machines
                    .iter()
                    .filter_map(|machine| machine.definition.disk.as_ref())
                    .any(|other| other.client == disk.client || other.volume == disk.volume)
            {
                return Err(format!("disk volume '{}' is already assigned", disk.volume));
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
                policy: MachinePolicy::default(),
            });
        }
        // No definition becomes visible until the whole batch is validated.
        self.machines.extend(prepared);
        Ok(())
    }

    fn execute_command(&mut self, client: usize, command: Request) -> hyper_os::Result<()> {
        let (name, action) = match command {
            Request::List => {
                let deadline = hyper_os::time::deadline_after(Duration::from_millis(250))?.as_raw();
                let mut machines: Vec<_> = (0..self.machines.len())
                    .map(|vm| self.summary(vm, deadline))
                    .collect();
                if self.io_broker.is_some() {
                    machines.push(self.io_summary(deadline));
                }
                return self.reply(client, Response::Entries { machines });
            }
            Request::Create { definitions } => {
                let first = self.machines.len();
                if let Err(message) = self.install_definitions(definitions) {
                    return self.reply_error(client, &message);
                }
                let mut failures = Vec::new();
                for vm in first..self.machines.len() {
                    if self.machines[vm].definition.autostart && self.start_instance(vm).is_err() {
                        self.machines[vm].policy.start_failed();
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
            Request::Affinity {
                name,
                vcpu,
                affinity_words,
            } => {
                return self.set_vcpu_affinity(client, &name, vcpu, affinity_words);
            }
            Request::Control { name, action } => (name, action),
        };
        if name == "io" && self.io_broker.is_some() {
            return if action == Action::Status {
                self.reply(
                    client,
                    Response::Entries {
                        machines: vec![self.io_summary(
                            hyper_os::time::deadline_after(Duration::from_millis(250))?.as_raw(),
                        )],
                    },
                )
            } else {
                self.reply_error(
                    client,
                    "I/O VM is read-only; its lifecycle belongs to io-runtime",
                )
            };
        }
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
            Action::Status => {
                let deadline = hyper_os::time::deadline_after(Duration::from_millis(250))?.as_raw();
                let summary = self.summary(vm, deadline);
                self.reply(
                    client,
                    Response::Entries {
                        machines: vec![summary],
                    },
                )
            }
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
                self.machines[vm].policy.request_stop(false);
                self.request_stop(vm)?;
                self.reply(client, Response::Accepted)
            }
            Action::Start | Action::Restart => {
                if self.machines[vm].instance.is_some() {
                    if action == Action::Start {
                        return self.reply_error(client, "VM is already active");
                    }
                    self.machines[vm].policy.request_stop(true);
                    self.request_stop(vm)?;
                } else if let Err(error) = self.start_instance(vm) {
                    self.machines[vm].policy.start_failed();
                    return self.reply_error(client, &format!("cannot start VM '{name}': {error}"));
                }
                self.reply(client, Response::Accepted)
            }
        }
    }

    fn set_vcpu_affinity(
        &mut self,
        client: usize,
        name: &str,
        vcpu: u32,
        cpus: Vec<u64>,
    ) -> hyper_os::Result<()> {
        if name == "io" {
            return self.reply_error(
                client,
                "I/O VM is read-only; its placement belongs to io-runtime",
            );
        }
        let Some(machine) = self
            .machines
            .iter_mut()
            .find(|machine| machine.definition.name == name)
        else {
            return self.reply_error(client, "VM does not exist; use 'vmm list'");
        };
        if let Err(message) = hyper_vm_manager::affinity_allowed(
            name,
            machine
                .instance
                .as_ref()
                .map(|instance| instance.policy.state()),
        ) {
            return self.reply_error(client, message);
        }
        let Some(instance) = machine.instance.as_mut() else {
            return self.reply_error(client, "VM must be running");
        };
        let mut affinity = [0; vm_contract::VCPU_AFFINITY_WORDS];
        if cpus.is_empty() || cpus.len() > affinity.len() || cpus.iter().all(|word| *word == 0) {
            return self.reply_error(
                client,
                "affinity must contain at least one CPU within the supported bitmap",
            );
        }
        affinity[..cpus.len()].copy_from_slice(&cpus);
        let deadline = hyper_os::time::deadline_after(Duration::from_millis(250))?.as_raw();
        match instance.control_vcpu(vcpu, Some(affinity), deadline) {
            Ok(reply) if reply.status == hyper_os::Status::OK => {
                self.reply(client, Response::AffinityAccepted { vcpu })
            }
            Ok(reply) => {
                self.reply_error(client, &format!("affinity rejected: {:?}", reply.status))
            }
            Err(error) => self.reply_error(
                client,
                &format!(
                    "affinity reply unavailable ({error}); outcome unknown, inspect vmm status"
                ),
            ),
        }
    }

    fn summary(&mut self, vm: usize, deadline: u64) -> fleet::Summary {
        let observation = self.machines[vm]
            .instance
            .as_mut()
            .and_then(|instance| instance.observe_memory(deadline));
        let mut placement = Vec::new();
        if let Some(observation) = observation {
            for vcpu in 0..observation.vcpus {
                let Some(instance) = self.machines[vm].instance.as_mut() else {
                    break;
                };
                let Ok(reply) = instance.control_vcpu(vcpu, None, deadline) else {
                    break;
                };
                if reply.status == hyper_os::Status::OK {
                    placement.push(fleet::VcpuPlacement {
                        vcpu,
                        host_cpu: reply.host_cpu,
                        pending_host_cpu: reply.pending_host_cpu,
                    });
                }
            }
        }
        let definition = &self.machines[vm].definition;
        let state = self.fleet_state(vm);
        fleet::Summary {
            placement,
            read_only: false,
            vcpus: observation.map(|value| value.vcpus),
            memory_bytes: observation.map(|value| value.capacity_bytes),
            resident_memory_bytes: observation.and_then(|value| value.resident_bytes),
            name: definition.name.clone(),
            image: definition.image.clone(),
            autostart: definition.autostart,
            disk: definition.disk.clone(),
            state,
        }
    }

    fn io_summary(&self, deadline: u64) -> fleet::Summary {
        use hyper_service::io;
        let observation = (|| -> hyper_os::Result<_> {
            let broker = self
                .io_broker
                .as_ref()
                .ok_or(hyper_os::Error::MissingHandle)?;
            let (local, remote) = CapabilityChannel::create()?;
            let mut remote = Some(remote.into_handle());
            if hyper_os::time::monotonic_now()?.as_nanoseconds() >= deadline {
                return Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT));
            }
            let limit = deadline;
            io::send_capabilities(
                broker,
                io::OBSERVE_MESSAGE,
                &mut [CapabilityDisposition::move_handle(
                    &mut remote,
                    RightsOffer::Exact(io::SESSION_RIGHTS),
                )?],
                limit,
            )?;
            let mut bytes = [MaybeUninit::uninit(); io::OBSERVATION_BYTES];
            let reply = local.receive(limit, &mut bytes, &mut [])?;
            if reply.capability_count() != 0 {
                return Err(hyper_os::Error::InvalidResponse);
            }
            io::decode_observation(reply.bytes()).ok_or(hyper_os::Error::InvalidResponse)
        })()
        .ok();
        use hyper_os::vm::VirtualMachinePhase as Phase;
        fleet::Summary {
            name: "io".into(),
            image: "/vm/io.itb".into(),
            autostart: true,
            disk: None,
            read_only: true,
            placement: observation
                .filter(|info| info.vcpus != 0)
                .map(|info| {
                    vec![fleet::VcpuPlacement {
                        vcpu: 0,
                        host_cpu: info.boot_host_cpu,
                        pending_host_cpu: None,
                    }]
                })
                .unwrap_or_default(),
            vcpus: observation.map(|info| info.vcpus),
            memory_bytes: observation.map(|info| info.ram_bytes),
            resident_memory_bytes: observation.and_then(|info| info.resident_bytes),
            state: match observation.map(|info| info.phase) {
                Some(Phase::Installed) => fleet::State::Starting,
                Some(Phase::Running) => fleet::State::Running,
                Some(Phase::Stopping) => fleet::State::Stopping,
                Some(Phase::Stopped) => fleet::State::Stopped,
                None => fleet::State::Unavailable,
            },
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
        if !instance.policy.can_attach_console() {
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
            instance.policy.attach_console(client);
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
            if let Some(instance) = machine.instance.as_mut() {
                instance.policy.disconnect_client(index);
            }
        }
    }

    fn fleet_state(&self, vm: usize) -> fleet::State {
        let machine = &self.machines[vm];
        machine
            .policy
            .state(machine.instance.as_ref().map(|instance| &instance.policy))
    }
}

struct Machine {
    definition: fleet::Definition,
    image: OwnedHandle<hyper_os::handle::FileObject>,
    instance: Option<VmInstance>,
    policy: MachinePolicy,
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
    Connection,
    DiskAdmission,
    RuntimeProcess(usize),
    RuntimeControl(usize),
    Client(usize),
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
