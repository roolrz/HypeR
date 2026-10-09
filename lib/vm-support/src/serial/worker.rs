// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Independently scheduled UART service. The caller may block on storage while
//! the I/O VM continues issuing console MMIO; no kernel lock spans this worker.

use super::{ns16550::Ns16550, pl011::VirtualPl011};
use hyper_os::handle::{ByteChannelObject, OwnedHandle, VirtualCpuObject, VirtualMachineObject};
use hyper_os::wait::{self, ObjectSignals, WaitItem};
use hyper_os::{Error, Result, Status, channel, vm};
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

const CAPACITY: usize = 64 * 1024;
const BATCH: usize = super::MESSAGE_BYTES;
// Cookies are scoped to a VM and disjoint from broker-assigned small IDs.
const DEVICE: u64 = 0x5541_5254;

/// Runtime end of one bounded duplex UART byte stream. Closing it stops the
/// worker and requests VM shutdown; a failed console must not strand a vCPU.
pub struct Port {
    channel: OwnedHandle<ByteChannelObject>,
    thread: JoinHandle<Result<()>>,
}
impl Port {
    pub fn start(
        machine: Arc<OwnedHandle<VirtualMachineObject>>,
        cpus: Vec<Arc<OwnedHandle<VirtualCpuObject>>>,
        profile: vm::PlatformProfile,
    ) -> Result<Self> {
        let (model, base, length, interrupt) = match profile {
            vm::PlatformProfile::Aarch64Reference => {
                (Model::Pl011(VirtualPl011::new()), 0x0900_0000, 0x1000, 33)
            }
            vm::PlatformProfile::Riscv64Reference => (
                Model::Ns16550(Ns16550::with_clock(1_000_000_000, 3_686_400)),
                0x1000_0000,
                0x1000,
                10,
            ),
        };
        let device = NonZeroU64::new(DEVICE).ok_or(Error::InvalidResponse)?;
        let route = vm::DeviceDoorbell::register(machine.as_handle_ref(), base, length, device)?;
        if profile == vm::PlatformProfile::Riscv64Reference {
            route.bind_firmware_console(machine.as_handle_ref())?;
        }
        let (client, server) = channel::create_pair()?;
        let worker = Worker {
            machine,
            cpus,
            route,
            channel: server,
            model,
            base,
            interrupt,
            asserted: false,
            input: VecDeque::with_capacity(BATCH),
            output: VecDeque::with_capacity(CAPACITY),
        };
        let thread = thread::Builder::new()
            .name("vm-uart".into())
            .stack_size(64 * 1024)
            .spawn(move || worker.run())
            .map_err(|_| Error::Status(Status::NO_MEMORY))?;
        Ok(Self {
            channel: client,
            thread,
        })
    }
    pub fn as_handle_ref(&self) -> hyper_os::HandleRef<'_, ByteChannelObject> {
        self.channel.as_handle_ref()
    }
    /// Receives one complete output message. Supply at least `serial::MESSAGE_BYTES` capacity.
    pub fn try_read(&self, bytes: &mut [u8]) -> Result<usize> {
        match self.channel.as_byte_channel().try_receive(bytes) {
            Err(Error::Status(Status::WOULD_BLOCK | Status::PEER_CLOSED)) => Ok(0),
            result => result,
        }
    }
    pub fn try_write(&self, bytes: &[u8]) -> Result<usize> {
        let count = bytes.len().min(BATCH);
        if count == 0 {
            return Ok(0);
        }
        self.channel.as_byte_channel().try_send(&bytes[..count])?;
        Ok(count)
    }
    pub fn wait_item(&self) -> WaitItem<'_> {
        WaitItem::new(
            self.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )
    }
    /// Call after requesting VM stop. Joins independently of guest progress;
    /// channel closure wakes backpressure/input waits without polling.
    pub fn join(self) -> Result<()> {
        let Self { channel, thread } = self;
        drop(channel);
        thread.join().map_err(|_| Error::Status(Status::INTERNAL))?
    }
}

enum Model {
    Pl011(VirtualPl011),
    Ns16550(Ns16550),
}
impl Model {
    fn can_receive(&self) -> bool {
        match self {
            Self::Pl011(uart) => uart.can_receive(),
            Self::Ns16550(uart) => uart.can_receive_external(),
        }
    }
    fn receive(&mut self, byte: u8, now: u64) {
        match self {
            Self::Pl011(uart) => {
                uart.receive(byte);
            }
            Self::Ns16550(uart) => {
                uart.receive_external(byte, now);
            }
        }
    }
    fn advance(&mut self, now: u64) {
        if let Self::Ns16550(uart) = self {
            uart.advance(now);
        }
    }
    fn deadline(&self) -> u64 {
        match self {
            Self::Pl011(_) => u64::MAX,
            Self::Ns16550(uart) => uart.next_timeout().unwrap_or(u64::MAX),
        }
    }
    fn asserted(&self) -> bool {
        match self {
            Self::Pl011(uart) => uart.interrupt_asserted(),
            Self::Ns16550(uart) => uart.interrupt_asserted(),
        }
    }
    fn access(
        &mut self,
        offset: u64,
        request: vm::MmioRequest,
        now: u64,
    ) -> Result<(vm::MmioCompletion, Option<u8>)> {
        match self {
            Self::Pl011(uart) => {
                let result = match request.operation {
                    vm::MmioOperation::Read => uart.read(offset, request.width as usize),
                    vm::MmioOperation::Write(value) => {
                        uart.write(offset, request.width as usize, value)
                    }
                }
                .map_err(|_| Error::InvalidResponse)?;
                let completion = match result.value {
                    Some(value) => vm::MmioCompletion::Read(value),
                    None => vm::MmioCompletion::Write,
                };
                Ok((completion, result.transmitted))
            }
            Self::Ns16550(uart) => {
                if request.width != 1 || offset >= 8 {
                    return Err(Error::InvalidResponse);
                }
                Ok(match request.operation {
                    vm::MmioOperation::Read => (
                        vm::MmioCompletion::Read(u64::from(uart.read_at(offset as usize, now))),
                        None,
                    ),
                    vm::MmioOperation::Write(value) => (
                        vm::MmioCompletion::Write,
                        uart.write_at(offset as usize, value as u8, now),
                    ),
                })
            }
        }
    }
}

struct Worker {
    machine: Arc<OwnedHandle<VirtualMachineObject>>,
    cpus: Vec<Arc<OwnedHandle<VirtualCpuObject>>>,
    route: vm::DeviceDoorbell,
    channel: OwnedHandle<ByteChannelObject>,
    model: Model,
    base: u64,
    interrupt: u32,
    asserted: bool,
    input: VecDeque<u8>,
    output: VecDeque<u8>,
}
impl Worker {
    fn now() -> Result<u64> {
        Ok(hyper_os::time::monotonic_now()?.as_nanoseconds())
    }
    fn input(&mut self, now: u64) {
        self.model.advance(now);
        while self.model.can_receive() {
            let Some(byte) = self.input.pop_front() else {
                break;
            };
            self.model.receive(byte, now);
        }
    }
    fn interrupt(&mut self) -> Result<()> {
        let asserted = self.model.asserted();
        if asserted != self.asserted {
            vm::set_device_interrupt(self.machine.as_handle_ref(), self.interrupt, asserted)?;
            self.asserted = asserted;
        }
        Ok(())
    }
    fn run(mut self) -> Result<()> {
        let result = match self.service() {
            // Lifecycle admission closes before the termination signal publishes.
            Err(Error::Status(Status::BAD_STATE)) => Ok(()),
            result => result,
        };
        // Stop also cancels outstanding detached MMIO. The worker owns VM/vCPU
        // capabilities until it leaves, independently of caller teardown.
        let _ = vm::request_stop(self.machine.as_handle_ref());
        if let Err(error) = &result {
            eprintln!("HypeR virtual UART: service failed: {error:?}");
        }
        result
    }
    fn service(&mut self) -> Result<()> {
        let mut bytes = [0; BATCH];
        loop {
            self.route.prepare_scan()?;
            if self.input.is_empty() {
                match self.channel.as_byte_channel().try_receive(&mut bytes) {
                    Ok(count) => self.input.extend(&bytes[..count]),
                    Err(Error::Status(Status::WOULD_BLOCK)) => {}
                    Err(Error::Status(Status::PEER_CLOSED)) => return Ok(()),
                    Err(error) => return Err(error),
                }
            }
            let now = Self::now()?;
            self.input(now);
            self.interrupt()?;
            // Bound service to keep input, timers and administrative stop live.
            for index in 0..self.cpus.len() {
                if self.output.len() == CAPACITY {
                    break;
                }
                let request = match self.route.pending(self.cpus[index].as_handle_ref()) {
                    Ok(Some(request)) => request,
                    Ok(None) => continue,
                    Err(Error::Status(Status::BAD_STATE)) => return Ok(()),
                    Err(error) => return Err(error),
                };
                let (completion, transmitted) = match request {
                    vm::DeviceRequest::Mmio(mmio) => {
                        let offset = mmio
                            .address
                            .checked_sub(self.base)
                            .ok_or(Error::InvalidResponse)?;
                        self.model.access(offset, mmio, now)?
                    }
                    vm::DeviceRequest::FirmwareConsoleWrite { byte, .. } => {
                        (vm::MmioCompletion::Write, Some(byte))
                    }
                };
                if let Some(byte) = transmitted {
                    self.output.push_back(byte);
                }
                self.input(now);
                self.interrupt()?;
                match self
                    .route
                    .complete(self.cpus[index].as_handle_ref(), request, completion)
                {
                    Ok(()) => {}
                    Err(Error::Status(Status::BAD_STATE)) => return Ok(()),
                    Err(error) => return Err(error),
                }
            }
            let blocked = self.output.len() == CAPACITY;
            while !self.output.is_empty() {
                let count = self.output.len().min(BATCH);
                for (out, byte) in bytes[..count].iter_mut().zip(self.output.iter()) {
                    *out = *byte;
                }
                match self.channel.as_byte_channel().try_send(&bytes[..count]) {
                    Ok(()) => {
                        self.output.drain(..count);
                    }
                    Err(Error::Status(Status::WOULD_BLOCK)) => break,
                    Err(Error::Status(Status::PEER_CLOSED)) => return Ok(()),
                    Err(error) => return Err(error),
                }
            }
            if blocked && self.output.len() < CAPACITY {
                continue;
            }
            let mut signals = ObjectSignals::<ByteChannelObject>::PEER_CLOSED;
            if self.input.is_empty() {
                signals = signals.union(ObjectSignals::<ByteChannelObject>::READABLE);
            }
            if !self.output.is_empty() {
                signals = signals.union(ObjectSignals::<ByteChannelObject>::WRITABLE);
            }
            let items = [
                WaitItem::new(self.channel.as_handle_ref(), signals),
                WaitItem::new(
                    self.machine.as_handle_ref(),
                    ObjectSignals::<VirtualMachineObject>::TERMINATED,
                ),
                self.route.wait_item(),
            ];
            let count = if self.output.len() < CAPACITY { 3 } else { 2 };
            match wait::wait_many(&items[..count], self.model.deadline()) {
                Ok(event) if event.index == 1 => return Ok(()),
                Ok(_) | Err(Error::Status(Status::TIMED_OUT)) => {}
                Err(error) => return Err(error),
            }
        }
    }
}
