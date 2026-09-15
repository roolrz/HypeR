// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Static configuration-volume client. The shared grant is a dedicated Native
//! initiator pool; no business VM memory or per-request userspace relay is used.

use super::{MAILBOX_MMIO, Result, check_deadline, deadline, pump_guest, show};
use hyper_os::block::{self, NativeBlock};
use hyper_os::device::{self, DmaExtent};
use hyper_os::guest_io::Mailbox;
use hyper_os::handle::{
    ByteChannelObject, DeviceAssignmentAuthorityObject, GuestMailboxObject, GuestMemoryObject,
    HandleRef, OwnedHandle, VirtualMachineObject,
};
use hyper_os::memory::WritableVmo;
use hyper_os::startup::{self, Startup};
use hyper_os::vm;
use hyper_os::wait::{self, ObjectSignals, WaitItem};
use hyper_vm_image::guest_fdt::io::{DmaRange, IoClient, MmioDevice, SharedMemory};
use hyper_vm_runtime::io_guest::{InstalledGuest, RAM_BASE, RAM_BYTES, SharedGrant};
use hyper_vm_runtime::io_protocol::{Command, MAX_RECORD, Reply, Request, Status};
use hyper_vm_runtime::virtio_scsi::{BackendOperation, QUEUES, Queue, VERSION_1};

const SHARED_BASE: u64 = RAM_BASE + RAM_BYTES;
const NOTIFICATION_MMIO: u64 = 0x0a02_0000;

pub(super) struct Client {
    _memory: WritableVmo,
    grant: OwnedHandle<GuestMemoryObject>,
    dma: DmaExtent,
    ready: Option<OwnedHandle<ByteChannelObject>>,
    block: Option<NativeBlock>,
}
impl Client {
    pub(super) fn prepare(
        authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
        ready: OwnedHandle<ByteChannelObject>,
    ) -> Result<Self> {
        let memory = WritableVmo::create_contiguous(block::MEMORY_BYTES).map_err(show)?;
        let dma = device::dma_extent(authority, memory.as_handle_ref(), 0, block::MEMORY_BYTES)
            .map_err(show)?;
        let grant = vm::create_guest_memory(memory.as_handle_ref()).map_err(show)?;
        Ok(Self {
            _memory: memory,
            grant,
            dma,
            ready: Some(ready),
            block: None,
        })
    }
    pub(super) fn description(&self) -> IoClient {
        IoClient {
            id: 0,
            shared_memory: SharedMemory {
                base: SHARED_BASE,
                size: block::MEMORY_BYTES,
                guest_base: SHARED_BASE,
            },
            mailbox: MmioDevice {
                base: MAILBOX_MMIO,
                size: 4096,
                irq: 41,
            },
            notification: MmioDevice {
                base: NOTIFICATION_MMIO,
                size: 4096,
                irq: 42,
            },
            dynamic: false,
        }
    }
    pub(super) fn dma_range(&self) -> DmaRange {
        DmaRange {
            dma_base: self.dma.physical_base,
            cpu_base: SHARED_BASE,
            size: block::MEMORY_BYTES,
        }
    }
    pub(super) fn mapping(&self) -> SharedGrant<'_> {
        SharedGrant {
            memory: self.grant.as_handle_ref(),
            guest_offset: RAM_BYTES,
            memory_offset: 0,
            size: block::MEMORY_BYTES,
        }
    }
    pub(super) fn install(&mut self, guest: &InstalledGuest) -> Result<()> {
        self.block = Some(
            NativeBlock::create(
                self.grant.as_handle_ref(),
                guest.machine.as_handle_ref(),
                SHARED_BASE,
                NOTIFICATION_MMIO,
                42,
            )
            .map_err(show)?,
        );
        Ok(())
    }
    pub(super) fn mount(
        &mut self,
        startup: &Startup<'_>,
        guest: &mut InstalledGuest,
        mailbox: &Mailbox,
        features: u64,
    ) -> Result<()> {
        if features & VERSION_1 == 0 {
            return Err("backend lacks virtio version 1".into());
        }
        println!("HypeR io-runtime: preparing configuration volume");
        exchange(
            guest,
            mailbox,
            Request {
                binding: 1,
                epoch: 1,
                transaction: 2,
                command: Command::Prepare {
                    alias: SHARED_BASE,
                    guest_base: SHARED_BASE,
                    length: block::MEMORY_BYTES,
                    mapping_token: 0,
                },
            },
        )?;
        let queues: [Queue; QUEUES] = std::array::from_fn(|index| {
            let base = SHARED_BASE + index as u64 * block::QUEUE_STRIDE;
            Queue {
                size: block::QUEUE_SIZE,
                descriptor: base,
                available: base + block::AVAILABLE_OFFSET,
                used: base + block::USED_OFFSET,
                ready: true,
            }
        });
        exchange(
            guest,
            mailbox,
            Request {
                binding: 1,
                epoch: 1,
                transaction: 3,
                command: Command::Device(BackendOperation::Activate {
                    features: VERSION_1,
                    queues,
                }),
            },
        )?;
        let block = self.block.take().ok_or("Native initiator missing")?;
        println!("HypeR io-runtime: configuration queues active");
        let root = startup.borrow(startup::ROOT_DIRECTORY).map_err(show)?;
        let (sender, receiver) = hyper_os::channel::create_pair().map_err(show)?;
        // Only the worker enters potentially blocking filesystem syscalls.
        // The owner keeps processing console and guest power events. Scope
        // exit joins before backing/grants can be dropped on any error path.
        let mounted = std::thread::scope(|scope| -> Result<NativeBlock> {
            let worker = std::thread::Builder::new()
                .spawn_scoped(scope, move || {
                    let result = (|| {
                        println!("HypeR io-runtime: discovering configuration device");
                        let sectors = block.activate(false).map_err(show)?;
                        println!("HypeR io-runtime: configuration device: {sectors} sectors");
                        std::fs::create_dir_all("/data").map_err(show)?;
                        block.mount(root, "data").map_err(show)?;
                        println!("HypeR io-runtime: configuration volume: {sectors} sectors");
                        Ok(block)
                    })();
                    // The result remains owned by the join handle. This channel is
                    // only a wakeup, so closure also prompts the owner on failure.
                    let _ = sender.as_byte_channel().try_send(b"done");
                    result
                })
                .map_err(show)?;
            let pending = wait_mount(guest, mailbox, &receiver);
            if pending.is_err() {
                let _ = vm::request_stop(guest.machine.as_handle_ref());
            }
            let result = worker
                .join()
                .map_err(|_| "configuration mount worker panicked".to_string())?;
            pending?;
            result
        })?;
        self.block = Some(mounted);
        self.ready
            .take()
            .ok_or("readiness capability already consumed")?
            .as_byte_channel()
            .send(hyper_service::io::READY_MESSAGE)
            .map_err(show)?;
        Ok(())
    }
}

fn exchange(guest: &mut InstalledGuest, mailbox: &Mailbox, request: Request) -> Result<()> {
    let mut record = [0; MAX_RECORD];
    let length = request.encode(&mut record).map_err(show)?;
    mailbox.send(&record[..length]).map_err(show)?;
    let limit = deadline(60)?;
    loop {
        if !pump_guest(guest)? {
            return Err("backend stopped during configuration".into());
        }
        match mailbox.receive(&mut record) {
            Ok(length) => {
                let reply = Reply::decode(&record[..length], request).map_err(show)?;
                return if reply.status == Status::Success {
                    Ok(())
                } else {
                    Err(format!(
                        "backend refused {:?}: {:?}",
                        request.command, reply.status
                    ))
                };
            }
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            Err(error) => return Err(show(error)),
        }
        check_deadline(limit)?;
        let items = [
            guest.output.wait_item(),
            WaitItem::new(
                guest.machine.as_handle_ref(),
                ObjectSignals::<VirtualMachineObject>::POWER_REQUEST
                    .union(ObjectSignals::<VirtualMachineObject>::VCPU_TERMINATED),
            ),
            WaitItem::new(
                mailbox.as_handle_ref(),
                ObjectSignals::<GuestMailboxObject>::READABLE
                    .union(ObjectSignals::<GuestMailboxObject>::PEER_CLOSED),
            ),
        ];
        let event = wait::wait_many(&items, limit).map_err(show)?;
        if event.index == 2
            && ObjectSignals::<GuestMailboxObject>::PEER_CLOSED.is_present_in(event.observed)
        {
            return Err("backend control channel closed during configuration".into());
        }
    }
}

fn wait_mount(
    guest: &mut InstalledGuest,
    mailbox: &Mailbox,
    receiver: &OwnedHandle<ByteChannelObject>,
) -> Result<()> {
    let limit = deadline(hyper_service::io::READY_TIMEOUT_SECONDS)?;
    loop {
        if !pump_guest(guest)? {
            return Err("backend stopped during mount".into());
        }
        // Observe the durable worker result before serial readiness/deadline.
        match receiver.as_byte_channel().try_receive(&mut [0; 4]) {
            Ok(_) | Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => return Ok(()),
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            Err(error) => return Err(show(error)),
        }
        check_deadline(limit)?;
        let items = [
            guest.output.wait_item(),
            WaitItem::new(
                guest.machine.as_handle_ref(),
                ObjectSignals::<VirtualMachineObject>::POWER_REQUEST
                    .union(ObjectSignals::<VirtualMachineObject>::VCPU_TERMINATED),
            ),
            WaitItem::new(
                mailbox.as_handle_ref(),
                ObjectSignals::<GuestMailboxObject>::READABLE
                    .union(ObjectSignals::<GuestMailboxObject>::PEER_CLOSED),
            ),
            WaitItem::new(
                receiver.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            ),
        ];
        match wait::wait_many(&items, limit).map_err(show)?.index {
            2 => return Err("unexpected backend control event during mount".into()),
            3 => return Ok(()),
            _ => {}
        }
    }
}
