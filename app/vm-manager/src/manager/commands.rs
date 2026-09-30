// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Client requests, fleet policy, and bounded status observations.

use super::{Client, FleetManager, MAX_CLIENTS};
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::handle::{ByteChannelObject, RightsOffer};
use hyper_os::wait::ObjectSignals;
use hyper_service::vm as vm_contract;
use hyper_vm_policy::fleet::{self, Action, Request, Response};
use std::mem::MaybeUninit;
use std::time::Duration;

impl FleetManager {
    pub(super) fn accept_client(&mut self) -> hyper_os::Result<()> {
        let Some(connection) = self.connections.accept()? else {
            return Ok(());
        };
        if let Some(index) = self.clients.iter().position(Option::is_none) {
            self.clients[index] = Some(Client {
                control: connection.control,
                capabilities: connection.capabilities,
            });
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

    pub(super) fn handle_client(&mut self, index: usize, observed: u64) -> hyper_os::Result<()> {
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
        match fleet::request(message) {
            Ok(command) => match self.execute_command(index, command) {
                Ok(()) => Ok(()),
                Err(error) => self.reply_error(index, &format!("operation failed: {error}")),
            },
            Err(_) => self.reply_error(index, "invalid fleet request"),
        }
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

    pub(super) fn reply_error(&mut self, client: usize, message: &str) -> hyper_os::Result<()> {
        self.reply(
            client,
            Response::Error {
                message: message.into(),
            },
        )
    }

    pub(super) fn reply(&mut self, client: usize, response: Response) -> hyper_os::Result<()> {
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

    pub(super) fn disconnect_client(&mut self, index: usize) {
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
