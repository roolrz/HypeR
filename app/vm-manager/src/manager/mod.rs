// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet ownership, startup provisioning, and fair event dispatch.

mod commands;
mod console;
mod instance;
mod launch;
mod listener;

use self::instance::VmInstance;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityReceiveSlot};
use hyper_os::fs::{Directory, File};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, ResourceDomainObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::process as process_contract;
use hyper_service::vm as vm_contract;
use hyper_vm_manager::MachinePolicy;
use hyper_vm_policy::fleet;
use std::io::Read;
use std::mem::MaybeUninit;
use std::time::Instant;

const MAX_CLIENTS: usize = 8;

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
        self.install_definitions(config.machines).map_err(|error| {
            eprintln!("HypeR vm-manager: {error}");
            hyper_os::Error::InvalidResponse
        })?;
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
            hyper_vm_policy::image::validate_file(&image, &definition.configuration)
                .map_err(|error| format!("cannot load image '{}': {error}", definition.image))?;
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
