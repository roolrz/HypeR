// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Board-scoped ownership of mailbox control and dynamic guest mappings.

#[path = "broker_listener.rs"]
mod listener;

use super::{Result, check_deadline, deadline, show};
use hyper_io_runtime::broker_exchange::{Pending, Step};
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::guest_io::{Mailbox, Notification, Operation};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, GuestMailboxObject, OwnedHandle, RightsOffer,
    VirtualMachineObject,
};
use hyper_os::vm;
use hyper_os::wait::{ObjectSignals, WaitItem};
use hyper_service::io;
use hyper_vm_image::guest_fdt::io::{DmaRange, IoClient, MmioDevice, SharedMemory};
use hyper_vm_runtime::io_guest::InstalledGuest;
use hyper_vm_runtime::io_protocol::{Command, MAX_RECORD, Reply, Request, Status};
use hyper_vm_runtime::virtio_scsi::BackendOperation;
use std::io::Read;
use std::num::NonZeroU64;

pub(super) struct Broker {
    listener: Option<listener::Listener>,
    slots: Vec<Slot>,
    observations: Vec<(CapabilityChannel, u64)>,
    #[cfg(feature = "broker-test")]
    fast_released: bool,
    #[cfg(feature = "broker-test")]
    fast_was_bound: bool,
}
struct Slot {
    policy: hyper_io_runtime::clients::Client,
    mailbox: Option<Mailbox>,
    binding: Option<Binding>,
    transaction: u64,
    generation: u64,
    queued: Option<listener::Admission>,
    #[cfg(feature = "broker-test")]
    hold_reply: bool,
    #[cfg(feature = "broker-test")]
    held_announced: bool,
}
struct Binding {
    _session: CapabilityChannel,
    identity: u64,
    channel: OwnedHandle<ByteChannelObject>,
    mapping: vm::GuestMapping,
    notification: Notification,
    epoch: u32,
    reply: Option<Vec<u8>>,
    prepare_sent: bool,
    phase: Phase,
    pending: Option<Pending>,
    retiring: bool,
    machine: Option<OwnedHandle<VirtualMachineObject>>,
    remote: Option<OwnedHandle<ByteChannelObject>>,
    base: u64,
    length: u64,
    limit: u64,
    retry_at: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Hello,
    Prepare,
    ReturnHandles,
    Active,
    Retire,
    TryRelease,
    Reset,
    Release,
    FinalRelease,
    Disconnect,
}

impl Broker {
    pub(super) fn load(endpoint: OwnedHandle<CapabilityChannelObject>) -> Result<Self> {
        let mut bytes = String::new();
        std::fs::File::open("/etc/hyper/io-clients.conf")
            .map_err(show)?
            .take(8193)
            .read_to_string(&mut bytes)
            .map_err(show)?;
        let clients = hyper_io_runtime::clients::parse(&bytes).map_err(show)?;
        if clients.len() + 1 > vm::IO_MAX_CLIENTS {
            return Err("too many configured I/O clients".into());
        }
        Ok(Self {
            observations: Vec::new(),
            #[cfg(feature = "broker-test")]
            fast_released: false,
            #[cfg(feature = "broker-test")]
            fast_was_bound: false,
            listener: Some(
                listener::Listener::start(CapabilityChannel::from_handle(endpoint))
                    .map_err(show)?,
            ),
            slots: clients
                .into_iter()
                .map(|policy| Slot {
                    policy,
                    mailbox: None,
                    binding: None,
                    transaction: 1,
                    generation: 0,
                    queued: None,
                    #[cfg(feature = "broker-test")]
                    hold_reply: false,
                    #[cfg(feature = "broker-test")]
                    held_announced: false,
                })
                .collect(),
        })
    }
    pub(super) fn descriptions(&self) -> Vec<IoClient> {
        self.slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let index = index as u32 + 1;
                IoClient {
                    id: slot.policy.id,
                    dynamic: true,
                    shared_memory: SharedMemory {
                        base: vm::DYNAMIC_ALIAS_OFFSET,
                        size: vm::DYNAMIC_PHYSICAL_LIMIT,
                        guest_base: 0,
                    },
                    mailbox: MmioDevice {
                        base: 0x0a01_0000 + u64::from(index) * 0x20000,
                        size: 4096,
                        irq: 41 + index * 2,
                    },
                    notification: MmioDevice {
                        base: 0x0a02_0000 + u64::from(index) * 0x20000,
                        size: 4096,
                        irq: 42 + index * 2,
                    },
                }
            })
            .collect()
    }
    pub(super) fn dma_ranges(&self, excluded: &[DmaRange]) -> Result<Vec<DmaRange>> {
        // No dynamic client means no guest-memory aperture to translate.
        if self.slots.is_empty() {
            return Ok(Vec::new());
        }
        hyper_io_runtime::clients::dynamic_dma_ranges(excluded)
    }
    pub(super) fn install(&mut self, guest: &InstalledGuest) -> Result<()> {
        let descriptions = self.descriptions();
        for (slot, description) in self.slots.iter_mut().zip(descriptions) {
            slot.mailbox = Some(
                Mailbox::create(
                    guest.machine.as_handle_ref(),
                    description.mailbox.base,
                    description.mailbox.irq,
                )
                .map_err(show)?,
            );
        }
        Ok(())
    }
    pub(super) fn wait_items(&self) -> Vec<WaitItem<'_>> {
        let mut items = Vec::new();
        for (endpoint, _) in &self.observations {
            items.push(WaitItem::new(
                endpoint.as_handle_ref(),
                ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
            ));
        }

        if let Some(listener) = &self.listener {
            items.push(WaitItem::new(
                listener.bell.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            ));
        }
        for slot in &self.slots {
            let Some(binding) = &slot.binding else {
                continue;
            };
            // Once closure is observed it must not remain permanently ready in
            // our wait set while the independent backend drains.
            if !binding.retiring {
                let mut signals = ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED;
                if binding.phase == Phase::ReturnHandles {
                    signals =
                        signals.union(ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING);
                }
                items.push(WaitItem::new(binding._session.as_handle_ref(), signals));
            }
            if let Some(pending) = &binding.pending {
                let signals = if pending.sent {
                    ObjectSignals::<GuestMailboxObject>::READABLE
                } else {
                    ObjectSignals::<GuestMailboxObject>::WRITABLE
                };
                #[cfg(feature = "broker-test")]
                if slot.hold_reply && pending.sent && binding.phase == Phase::Hello {
                    continue;
                }
                if let Some(mailbox) = &slot.mailbox {
                    items.push(WaitItem::new(
                        mailbox.as_handle_ref(),
                        signals.union(ObjectSignals::<GuestMailboxObject>::PEER_CLOSED),
                    ));
                }
            } else if binding.phase == Phase::Active {
                let signals = if binding.reply.is_some() {
                    ObjectSignals::<ByteChannelObject>::WRITABLE
                } else {
                    ObjectSignals::<ByteChannelObject>::READABLE
                };
                items.push(WaitItem::new(
                    binding.channel.as_handle_ref(),
                    signals.union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                ));
            }
        }
        items
    }
    pub(super) fn next_deadline(&self) -> u64 {
        self.slots
            .iter()
            .filter_map(|slot| slot.binding.as_ref())
            .map(|binding| {
                if let Some(pending) = &binding.pending {
                    pending.limit
                } else if binding.retry_at != 0 {
                    binding.retry_at.min(binding.limit)
                } else if matches!(binding.phase, Phase::ReturnHandles) || binding.reply.is_some() {
                    binding.limit
                } else {
                    hyper_os::DEADLINE_INFINITE
                }
            })
            .chain(self.observations.iter().map(|(_, limit)| *limit))
            .min()
            .unwrap_or(hyper_os::DEADLINE_INFINITE)
    }
    /// One bounded turn for each client; no client owns the supervisor thread.
    pub(super) fn service(&mut self, guest: &mut InstalledGuest) -> Result<bool> {
        use hyper_io_runtime::admission_policy::{self, Poll};
        let mut progress = false;
        match admission_policy::poll(&mut self.listener, listener::Listener::receive) {
            Poll::Received(admission) => {
                match admission {
                    listener::Request::Connect(admission) => self.admit(guest, admission)?,
                    listener::Request::Observe(endpoint) => {
                        if self.observations.len() < 4 {
                            self.observations.push((endpoint, deadline(1)?));
                        }
                    }
                }
                progress = true;
            }
            Poll::Idle => {}
            Poll::Disabled(error) => eprintln!(
                "HypeR io-runtime: new client admissions disabled: {}",
                show(error)
            ),
        }
        if !self.observations.is_empty() {
            match vm::machine_info(guest.machine.as_handle_ref()) {
                Ok(info) => {
                    let snapshot =
                        io::encode_observation(info, hyper_vm_runtime::io_guest::RAM_BYTES);
                    self.observations.retain(|(endpoint, limit)| {
                        let keep = check_deadline(*limit).is_ok()
                            && matches!(
                                endpoint.try_send(&snapshot, &mut []),
                                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
                            );
                        progress |= !keep;
                        keep
                    });
                }
                Err(error) => {
                    // Observation failure must never tear down the data plane.
                    eprintln!("HypeR io-runtime: status unavailable: {error}");
                    self.observations.clear();
                    progress = true;
                }
            }
        }
        for slot in &mut self.slots {
            #[cfg(feature = "broker-test")]
            {
                slot.hold_reply = slot.policy.id == 1 && !self.fast_released;
            }
            progress |= slot.service()?;
            #[cfg(feature = "broker-test")]
            if slot.policy.id == 2 {
                if slot
                    .binding
                    .as_ref()
                    .is_some_and(|binding| binding.phase == Phase::Active)
                {
                    self.fast_was_bound = true;
                }
                if self.fast_was_bound && !self.fast_released && slot.binding.is_none() {
                    self.fast_released = true;
                    println!(
                        "BROKER-TEST RELEASED client=2 generation={}",
                        slot.generation
                    );
                }
            }
        }
        for index in 0..self.slots.len() {
            if self.slots[index].binding.is_none()
                && let Some(admission) = self.slots[index].queued.take()
            {
                self.admit(guest, admission)?;
                progress = true;
            }
        }
        Ok(progress)
    }

    fn admit(&mut self, guest: &mut InstalledGuest, admission: listener::Admission) -> Result<()> {
        let descriptions = self.descriptions();
        let Some(index) = self.slots.iter().position(|slot| {
            slot.policy.id == admission.client && slot.policy.volume == admission.volume
        }) else {
            let _ = vm::request_stop(admission.machine.as_handle_ref());
            return Ok(());
        };
        let slot = &mut self.slots[index];
        if let Some(binding) = slot.binding.as_mut() {
            if closed(&binding._session) && slot.queued.is_none() {
                binding.retiring = true;
                slot.queued = Some(admission);
            } else {
                let _ = vm::request_stop(admission.machine.as_handle_ref());
            }
            return Ok(());
        }
        let Some(generation) = slot
            .generation
            .checked_add(1)
            .filter(|_| slot.binding.is_none())
        else {
            let _ = vm::request_stop(admission.machine.as_handle_ref());
            return Ok(());
        };
        let identity = NonZeroU64::new(generation).ok_or("invalid binding generation")?;
        let description = descriptions[index];
        // Allocate the reply channel before creating routes or exposing a token.
        let mut notification = None;
        let mut stage = "create reply channel";
        let resources = (|| -> hyper_os::Result<_> {
            let channels = hyper_os::channel::create_pair()?;
            stage = "register frontend MMIO";
            vm::register_mmio(
                admission.machine.as_handle_ref(),
                io::FRONTEND_MMIO,
                4096,
                identity,
            )?;
            stage = "create notification route";
            notification = Some(Notification::create(
                admission.machine.as_handle_ref(),
                guest.machine.as_handle_ref(),
                io::FRONTEND_MMIO,
                description.notification.base,
                io::FRONTEND_IRQ,
                description.notification.irq,
            )?);
            stage = "map backend guest memory";
            let mapping = vm::map_shared_guest_memory(
                guest.machine.as_handle_ref(),
                admission.memory.as_handle_ref(),
                admission.base,
            )?;
            Ok((channels, mapping))
        })();
        let ((channel, remote), mapping) = match resources {
            Ok(resources) => resources,
            Err(error) => {
                let _ = vm::request_stop(admission.machine.as_handle_ref());
                if let Some(notification) = notification {
                    notification.disconnect().map_err(show)?;
                }
                eprintln!(
                    "HypeR io-runtime: rejected client {} setup ({stage}): {error}",
                    admission.client
                );
                return Ok(());
            }
        };
        slot.generation = generation;
        slot.binding = Some(Binding {
            _session: admission.session,
            identity: identity.get(),
            channel,
            mapping,
            notification: notification.ok_or("missing notification")?,
            epoch: 1,
            reply: None,
            prepare_sent: false,
            phase: Phase::Hello,
            pending: None,
            retiring: false,
            machine: Some(admission.machine),
            remote: Some(remote),
            base: admission.base,
            length: admission.length,
            limit: deadline(30)?,
            retry_at: 0,
        });
        Ok(())
    }
}

fn closed(session: &CapabilityChannel) -> bool {
    hyper_os::wait::wait_many(
        &[WaitItem::new(
            session.as_handle_ref(),
            ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED,
        )],
        0,
    )
    .is_ok()
}
fn would_block(error: &hyper_os::Error) -> bool {
    *error == hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)
}
impl Slot {
    fn service(&mut self) -> Result<bool> {
        let Some(binding) = self.binding.as_mut() else {
            return Ok(false);
        };
        if !binding.retiring && closed(&binding._session) {
            binding.retiring = true;
        }
        // An already-sent request must finish before RESET may reuse this
        // mailbox, including cancelled admission and peer death during ACTIVATE.
        if let Some(pending) = binding.pending.as_mut() {
            if binding.retiring
                && pending.can_cancel()
                && !matches!(binding.phase, Phase::Reset | Phase::Release)
            {
                binding.pending = None;
                binding.phase = Phase::Retire;
                return Ok(true);
            }
            let mailbox = self.mailbox.as_ref().ok_or("missing mailbox")?;
            let now = hyper_os::time::monotonic_now()
                .map_err(show)?
                .as_nanoseconds();
            #[cfg(feature = "broker-test")]
            let step = {
                let hold = self.hold_reply && binding.phase == Phase::Hello;
                if hold && pending.sent && !self.held_announced {
                    self.held_announced = true;
                    println!(
                        "BROKER-TEST REPLY-HELD client=1 generation={}",
                        binding.identity
                    );
                }
                pending.poll(&HeldReply { mailbox, hold }, now)?
            };
            #[cfg(not(feature = "broker-test"))]
            let step = pending.poll(mailbox, now)?;
            match step {
                Step::Sent => {
                    #[cfg(feature = "broker-test")]
                    if self.policy.id == 1 && binding.phase == Phase::Hello {
                        println!(
                            "BROKER-TEST HELLO-SENT client=1 generation={}",
                            binding.identity
                        );
                    }
                    if matches!(pending.request.command, Command::Prepare { .. }) {
                        binding.prepare_sent = true;
                    }
                    return Ok(true);
                }
                Step::Waiting => return Ok(false),
                Step::Reply(mut record) => {
                    let length = record.len();
                    let reply = Reply::decode(&record[..length], pending.request).map_err(show)?;
                    if let Some(original) = pending.original {
                        if !binding.retiring {
                            record[32..40].copy_from_slice(&original.to_le_bytes());
                            binding.reply = Some(record[..length].to_vec());
                            binding.limit = deadline(30)?;
                        }
                    } else if reply.status != Status::Success {
                        if matches!(binding.phase, Phase::Reset | Phase::Release) {
                            return Err("backend refused retirement".into());
                        }
                        binding.retiring = true;
                    }
                    binding.pending = None;
                    binding.phase = match binding.phase {
                        Phase::Hello => Phase::Prepare,
                        Phase::Prepare => {
                            binding.limit = deadline(30)?;
                            Phase::ReturnHandles
                        }
                        Phase::Reset => Phase::Release,
                        Phase::Release => {
                            binding.limit = deadline(5)?;
                            Phase::FinalRelease
                        }
                        phase => phase,
                    };
                    return Ok(true);
                }
            }
        }
        if binding.retiring
            && matches!(
                binding.phase,
                Phase::Hello | Phase::Prepare | Phase::ReturnHandles | Phase::Active
            )
        {
            binding.phase = Phase::Retire;
        }
        let command = match binding.phase {
            Phase::Hello => Some(Command::Hello),
            Phase::Prepare => Some(Command::Prepare {
                alias: 0,
                guest_base: binding.base,
                length: binding.length,
                mapping_token: binding.mapping.token(),
            }),
            Phase::Reset => Some(Command::Device(BackendOperation::Reset)),
            Phase::Release => Some(Command::Release),
            _ => None,
        };
        if let Some(command) = command {
            self.queue(command, None)?;
            return Ok(true);
        }
        match binding.phase {
            Phase::ReturnHandles => {
                let mut message = io::BOUND_MESSAGE.to_vec();
                message.extend_from_slice(&binding.identity.to_le_bytes());
                let result = binding._session.try_send(
                    &message,
                    &mut [
                        CapabilityDisposition::move_handle(
                            &mut binding.machine,
                            RightsOffer::Exact(listener::MACHINE_RIGHTS),
                        )
                        .map_err(show)?,
                        CapabilityDisposition::move_handle(
                            &mut binding.remote,
                            RightsOffer::Exact(io::MAILBOX_RIGHTS),
                        )
                        .map_err(show)?,
                    ],
                );
                match result {
                    Ok(()) => {
                        binding.phase = Phase::Active;
                        #[cfg(feature = "broker-test")]
                        println!(
                            "BROKER-TEST BOUND client={} generation={}",
                            self.policy.id, binding.identity
                        );
                    }
                    Err(error) if would_block(&error) && check_deadline(binding.limit).is_ok() => {
                        return Ok(false);
                    }
                    Err(_) => binding.retiring = true,
                }
            }
            Phase::Retire => {
                if let Some(machine) = &binding.machine {
                    let _ = vm::request_stop(machine.as_handle_ref());
                }
                binding.reply = None;
                binding.epoch = binding
                    .notification
                    .control(Operation::Disable)
                    .map_err(show)?;
                binding.limit = deadline(5)?;
                binding.phase = Phase::TryRelease;
            }
            Phase::TryRelease | Phase::FinalRelease => {
                if binding.retry_at != 0 && check_deadline(binding.retry_at).is_ok() {
                    check_deadline(binding.limit)?;
                    return Ok(false);
                }
                match binding.mapping.release() {
                    Ok(()) => {
                        binding.retry_at = 0;
                        binding.phase = Phase::Disconnect;
                    }
                    Err(hyper_os::Error::Status(hyper_os::Status::BUSY))
                        if binding.phase == Phase::TryRelease && binding.prepare_sent =>
                    {
                        binding.retry_at = 0;
                        binding.phase = Phase::Reset;
                    }
                    Err(error) if would_block(&error) => {
                        check_deadline(binding.limit)?;
                        binding.retry_at =
                            hyper_os::time::deadline_after(std::time::Duration::from_millis(1))
                                .map_err(show)?
                                .as_raw();
                        return Ok(false);
                    }
                    Err(error) => return Err(show(error)),
                }
            }
            Phase::Disconnect => {
                binding.notification.disconnect().map_err(show)?;
                self.binding = None;
            }
            Phase::Active => {
                if let Some(reply) = &binding.reply {
                    match binding.channel.as_byte_channel().try_send(reply) {
                        Ok(()) => binding.reply = None,
                        Err(error)
                            if would_block(&error) && check_deadline(binding.limit).is_ok() =>
                        {
                            return Ok(false);
                        }
                        Err(_) => binding.retiring = true,
                    }
                    return Ok(true);
                }
                let mut bytes = [0; MAX_RECORD];
                match binding.channel.as_byte_channel().try_receive(&mut bytes) {
                    Ok(16) if &bytes[..8] == b"HIONOT01" && bytes[12..16] == [0; 4] => {
                        let operation =
                            match u32::from_le_bytes(bytes[8..12].try_into().map_err(show)?) {
                                0 => Some(Operation::Disable),
                                1 => Some(Operation::Enable),
                                2 => Some(Operation::RaiseConfigurationInterrupt),
                                _ => None,
                            };
                        if let Some(epoch) = operation
                            .and_then(|operation| binding.notification.control(operation).ok())
                        {
                            binding.epoch = epoch;
                            let mut reply = b"HIONOTR1".to_vec();
                            reply.extend_from_slice(&epoch.to_le_bytes());
                            reply.extend_from_slice(&[0; 4]);
                            binding.reply = Some(reply);
                            binding.limit = deadline(30)?;
                        } else {
                            binding.retiring = true;
                        }
                    }
                    Ok(length) => {
                        if let Ok(request) = Request::decode(&bytes[..length])
                            && hyper_io_runtime::clients::authorize_request(
                                request,
                                binding.identity,
                                binding.epoch,
                            )
                        {
                            self.queue(request.command, Some(request.transaction))?;
                        } else {
                            binding.retiring = true;
                        }
                    }
                    Err(error) if would_block(&error) => return Ok(false),
                    Err(_) => binding.retiring = true,
                }
            }
            _ => return Err("invalid broker phase".into()),
        }
        Ok(true)
    }
    fn queue(&mut self, command: Command, original: Option<u64>) -> Result<()> {
        let binding = self.binding.as_mut().ok_or("missing binding")?;
        let request = Request {
            binding: binding.identity,
            epoch: binding.epoch,
            transaction: self.transaction,
            command,
        };
        self.transaction = self
            .transaction
            .checked_add(1)
            .ok_or("backend sequence exhausted")?;
        binding.pending = Some(Pending {
            request,
            original,
            sent: false,
            limit: deadline(60)?,
        });
        Ok(())
    }
}

/// Fault injection below the production pending transaction: the request really
/// reaches Linux, but its response stays queued until another slot retires.
#[cfg(feature = "broker-test")]
struct HeldReply<'a> {
    mailbox: &'a Mailbox,
    hold: bool,
}
#[cfg(feature = "broker-test")]
impl hyper_vm_runtime::io_backend::ControlTransport for HeldReply<'_> {
    fn send(&self, bytes: &[u8]) -> hyper_os::Result<()> {
        self.mailbox.send(bytes)
    }
    fn receive(&self, bytes: &mut [u8]) -> hyper_os::Result<usize> {
        if self.hold {
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
        } else {
            self.mailbox.receive(bytes)
        }
    }
}
