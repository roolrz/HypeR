// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet policy and supervision for Native virtual-machine runtimes.

mod listener;

use hyper_vm_manager::{InstancePolicy, MachinePolicy, RuntimeControlState, complete_admission};
use hyper_vm_policy::fleet::{self, Action, Request, Response};
use std::io::Read;
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};

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
use hyper_service::stdio as stdio_contract;
use hyper_service::vm as vm_contract;
use std::process::ExitCode;

const RUNTIME_ARGUMENT: &str = "/svc/vm-runtime";
const MAX_CLIENTS: usize = 8;
const CAPABILITY_REPLY_DEADLINE: Duration = Duration::from_millis(100);

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let mut manager = match FleetManager::from_startup(&mut startup) {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("vm-manager: startup failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    match manager.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vm-manager: supervisor failed: {error}");
            ExitCode::FAILURE
        }
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
    io_broker: Option<CapabilityChannel>,
    connections: listener::Listener,
    root: Directory,
    machines: Vec<Machine>,
    initial_vm: Option<usize>,
    clients: [Option<Client>; MAX_CLIENTS],
    next_wait: usize,
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

    fn run(&mut self) -> hyper_os::Result<()> {
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

    fn admit_disk(&mut self) -> hyper_os::Result<()> {
        let Some(instance) = self.machines.iter_mut().find_map(|machine| {
            machine
                .instance
                .as_mut()
                .filter(|instance| instance.wants_disk_admission())
        }) else {
            return Ok(());
        };
        let admission = instance
            .disk_admission
            .as_mut()
            .ok_or(hyper_os::Error::InvalidResponse)?;
        let broker = self
            .io_broker
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?;
        let disposition = CapabilityDisposition::move_handle(
            &mut admission.endpoint,
            RightsOffer::Exact(hyper_service::io::SESSION_RIGHTS),
        )?;
        let result = broker.try_send(&admission.record, &mut [disposition]);
        complete_admission(&mut instance.disk_admission, result);
        if instance.disk_admission.is_some() {
            return Ok(());
        }
        #[cfg(feature = "broker-test")]
        if self
            .machines
            .iter()
            .filter_map(|machine| machine.instance.as_ref())
            .filter(|instance| instance.disk_admission.is_none())
            .count()
            == 2
        {
            self.io_broker = None;
            println!("BROKER-TEST MANAGER-ENDPOINT-CLOSED");
        }
        Ok(())
    }

    fn handle_runtime_control(&mut self, vm: usize) -> hyper_os::Result<()> {
        let Some(instance) = self.machines[vm].instance.as_mut() else {
            return Ok(());
        };
        // Signal publication can lag the channel queue. Always read first:
        // READABLE may already be drained, and EOF must not discard records.
        match instance.receive_runtime_status() {
            Ok(RuntimeControlState::Closed) => {
                drop(instance.runtime_control.take());
            }
            Ok(RuntimeControlState::Open) => {}
            Err(error) => {
                eprintln!("HypeR vm-manager: VM {vm} runtime control failed: {error}");
                instance.policy.reject_protocol();
                instance.force_stop();
            }
        }
        if instance.policy.is_terminal() {
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

    fn request_stop(&mut self, vm: usize) -> hyper_os::Result<()> {
        if let Some(instance) = self.machines[vm].instance.as_mut() {
            instance.request_cooperative_stop()?;
            drop(instance.disk_admission.take());
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
        for (output, contract) in [
            (
                hyper_rt::process::stdout()?,
                stdio_contract::STANDARD_OUTPUT_CONTRACT,
            ),
            (
                hyper_rt::process::stderr()?,
                stdio_contract::STANDARD_ERROR_CONTRACT,
            ),
        ] {
            builder.add_handle_duplicate(
                output.as_handle_ref(),
                contract.purpose(),
                RightsOffer::Exact(contract.required_rights()),
            )?;
        }
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
        let disk_admission = if let Some(disk) = &definition.definition.disk {
            self.io_broker
                .as_ref()
                .ok_or(hyper_os::Error::MissingHandle)?;
            let (owner, runtime) = CapabilityChannel::create()?;
            builder
                .add_handle_move(
                    runtime.into_handle(),
                    hyper_service::io::SESSION.as_raw(),
                    RightsOffer::Exact(hyper_service::io::SESSION_RIGHTS),
                )
                .map_err(|failure| failure.error())?;
            let record = hyper_service::io::encode_connect(disk.client, &disk.volume)
                .ok_or(hyper_os::Error::InvalidResponse)?;
            Some(DiskAdmission {
                endpoint: Some(owner.into_handle()),
                record,
            })
        } else {
            None
        };
        builder.seal()?;
        let runtime = builder.start().map_err(|failure| failure.error())?;
        self.machines[vm].instance = Some(VmInstance {
            _resource_domain: domain,
            _task_group: group,
            runtime,
            runtime_control: Some(manager_runtime),
            console_connection,
            policy: InstancePolicy::default(),
            observation_sequence: 0,
            disk_admission,
        });
        self.machines[vm].policy.started();
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
        let outcome =
            std::mem::take(&mut instance.policy).finish(succeeded, &mut instance.disk_admission);
        self.machines[vm].policy.finished(&outcome);
        if !outcome.reboot {
            self.publish_initial_event(vm, outcome.event);
        }
        drop(instance);
        Ok(())
    }

    fn publish_boot_event(&self, event: vm_contract::BootEvent) {
        if let Some(client) = self.clients[0].as_ref()
            && client.initial
        {
            let _ = client.control.as_byte_channel().try_send(&event.encode());
        }
    }

    fn publish_initial_event(&mut self, vm: usize, event: vm_contract::InstanceEvent) {
        if self.initial_vm == Some(vm) {
            self.initial_vm = None;
            self.publish_boot_event(vm_contract::BootEvent::InstanceTerminated(event));
        }
    }

    fn complete_restarts(&mut self) -> hyper_os::Result<()> {
        let now = Instant::now();
        for vm in 0..self.machines.len() {
            if let Some(instance) = self.machines[vm].instance.as_mut()
                && instance.policy.expire_deadline(now) == vm_contract::StopAction::ForceProcess
            {
                let _ = instance.runtime.as_process_supervisor().request_stop();
            }
            let machine = &mut self.machines[vm];
            if machine.policy.take_restart(machine.instance.is_some())
                && self.start_instance(vm).is_err()
            {
                self.machines[vm].policy.start_failed();
                self.publish_initial_event(
                    vm,
                    vm_contract::InstanceEvent::Failed(vm_contract::InstanceFailure::Runtime),
                );
            }
        }
        Ok(())
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

struct DiskAdmission {
    endpoint: Option<OwnedHandle<CapabilityChannelObject>>,
    record: [u8; hyper_service::io::CONNECT_BYTES],
}

struct VmInstance {
    _resource_domain: OwnedHandle<ResourceDomainObject>,
    _task_group: OwnedHandle<TaskGroupObject>,
    runtime: OwnedHandle<ProcessObject>,
    runtime_control: Option<OwnedHandle<ByteChannelObject>>,
    console_connection: CapabilityChannel,
    policy: InstancePolicy,
    disk_admission: Option<DiskAdmission>,
    observation_sequence: u64,
}

impl VmInstance {
    fn next_request_sequence(&mut self) -> hyper_os::Result<u64> {
        self.observation_sequence = self
            .observation_sequence
            .checked_add(1)
            .ok_or(hyper_os::Error::InvalidResponse)?;
        Ok(self.observation_sequence)
    }

    fn control_vcpu(
        &mut self,
        vcpu: u32,
        affinity: Option<[u64; vm_contract::VCPU_AFFINITY_WORDS]>,
        deadline: u64,
    ) -> hyper_os::Result<vm_contract::VcpuControlReply> {
        let request = vm_contract::VcpuControlRequest {
            sequence: self.next_request_sequence()?,
            vcpu,
            affinity,
        };
        self.exchange(&request.encode(), deadline, |bytes| {
            vm_contract::VcpuControlReply::decode(bytes)
                .filter(|reply| reply.sequence == request.sequence && reply.vcpu == request.vcpu)
        })
    }

    fn observe_memory(&mut self, deadline: u64) -> Option<vm_contract::Observation> {
        if self.policy.state() != fleet::State::Running {
            return None;
        }
        let request = vm_contract::ObservationRequest(self.next_request_sequence().ok()?);
        self.exchange(&request.encode(), deadline, |bytes| {
            vm_contract::Observation::decode(bytes).filter(|reply| reply.request == request)
        })
        .ok()
    }

    /// One bounded transport for observations and control acknowledgements.
    /// Lifecycle messages keep advancing, and late replies remain harmless.
    fn exchange<T>(
        &mut self,
        request: &[u8],
        deadline: u64,
        decode: impl Fn(&[u8]) -> Option<T>,
    ) -> hyper_os::Result<T> {
        if hyper_os::time::monotonic_now()?.as_nanoseconds() >= deadline {
            return Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT));
        }
        let control = self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel();
        control.try_send(request)?;
        loop {
            if hyper_os::time::monotonic_now()?.as_nanoseconds() >= deadline {
                return Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT));
            }
            let mut bytes = [0u8; vm_contract::OBSERVATION_BYTES];
            match control.try_receive(&mut bytes) {
                Ok(length) => {
                    if let Some(reply) = decode(&bytes[..length]) {
                        return Ok(reply);
                    }
                    if self.policy.observe_message(&bytes[..length]).is_err() {
                        self.policy.reject_protocol();
                        self.force_stop();
                        return Err(hyper_os::Error::InvalidResponse);
                    }
                    if self.policy.is_terminal() {
                        let _ = self.arm_exit_deadline();
                        return Err(hyper_os::Error::Status(hyper_os::Status::BAD_STATE));
                    }
                }
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {
                    let waits = [WaitItem::new(
                        control.as_handle_ref(),
                        ObjectSignals::<ByteChannelObject>::READABLE
                            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                    )];
                    let ready = wait_many(&waits, deadline)?;
                    if !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(ready.observed) {
                        return Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED));
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn wants_disk_admission(&self) -> bool {
        self.policy
            .wants_disk_admission(self.disk_admission.is_some())
    }

    fn receive_runtime_status(&mut self) -> hyper_os::Result<RuntimeControlState> {
        let mut message = [0u8; vm_contract::OBSERVATION_BYTES];
        let received = self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .try_receive(&mut message);
        self.policy
            .observe_receive(received.map(|length| &message[..length]))
    }

    fn drain_runtime_statuses(&mut self) -> hyper_os::Result<()> {
        let Some(control) = self.runtime_control.as_ref() else {
            return Ok(());
        };
        loop {
            let mut message = [0u8; vm_contract::OBSERVATION_BYTES];
            match control.as_byte_channel().try_receive(&mut message) {
                Ok(length) => {
                    if self.policy.observe_message(&message[..length]).is_err() {
                        self.policy.reject_protocol();
                    }
                }
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
                | Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn request_cooperative_stop(&mut self) -> hyper_os::Result<()> {
        if self.policy.request_cooperative_stop() != vm_contract::StopAction::SendCooperative {
            return Ok(());
        }
        let sent = self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)
            .and_then(|control| {
                control
                    .as_byte_channel()
                    .try_send(&vm_contract::InstanceCommand::Stop.encode())
            });
        if self.policy.cooperative_stop_sent(sent, Instant::now())
            == vm_contract::StopAction::ForceProcess
        {
            let _ = self.runtime.as_process_supervisor().request_stop();
        }
        Ok(())
    }

    fn force_stop(&mut self) {
        if self.policy.force_stop() == vm_contract::StopAction::ForceProcess {
            let _ = self.runtime.as_process_supervisor().request_stop();
        }
    }

    fn arm_exit_deadline(&mut self) -> hyper_os::Result<()> {
        if self.policy.arm_exit_deadline(Instant::now()).is_none() {
            self.force_stop();
        }
        Ok(())
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
