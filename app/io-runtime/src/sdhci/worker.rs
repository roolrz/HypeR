// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Independently scheduled MMIO/IRQ service: the main I/O runtime may block
//! waiting for Linux storage without blocking Linux's physical device exits.

use hyper_os::handle::{
    ByteChannelObject, OwnedHandle, PhysicalDeviceObject, VirtualCpuObject, VirtualMachineObject,
};
use hyper_os::wait::{ObjectSignals, RegistrationId, WaitSet};
use hyper_os::{Error, Result, Status, channel, device, vm};
use hyper_vm_support::io_guest::{InstalledGuest, PHYSICAL_MMIO};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub use super::registers::DEVICE_COOKIE;
use super::registers::{line_asserted, request_offset};

pub struct Worker {
    // Closing this endpoint is a durable stop notification, even if the worker
    // is between servicing a request and entering WaitSet::wait.
    stop: Option<OwnedHandle<ByteChannelObject>>,
    thread: Option<JoinHandle<Result<()>>>,
}
impl Worker {
    /// Stop accepting exits, request VM shutdown, and join the worker. No VM
    /// lifecycle lock may be held by the caller while joining this thread.
    pub fn stop(mut self) -> Result<()> {
        self.stop.take();
        match self.thread.take() {
            Some(thread) => thread.join().map_err(|_| Error::Status(Status::INTERNAL))?,
            None => Ok(()),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Never block a VM retirement path in Drop. The worker retains all
        // capabilities until it observes closure and requests shutdown.
        self.stop.take();
    }
}

struct MachineGuard(Arc<OwnedHandle<VirtualMachineObject>>);
impl Drop for MachineGuard {
    fn drop(&mut self) {
        let _ = vm::request_stop(self.0.as_handle_ref());
    }
}

struct Pump {
    physical: OwnedHandle<PhysicalDeviceObject>,
    machine: MachineGuard,
    cpus: Vec<Arc<OwnedHandle<VirtualCpuObject>>>,
    _stop: OwnedHandle<ByteChannelObject>,
    waits: WaitSet,
    stop_id: RegistrationId,
    machine_id: RegistrationId,
    irq_id: RegistrationId,
    cpu_ids: Vec<RegistrationId>,
    irq_armed: bool,
    mode: Mode,
}

/// The caller must first register `PHYSICAL_MMIO..+65536` with `DEVICE_COOKIE`,
/// and must not run the guest before this function succeeds.
pub fn start(
    physical: &OwnedHandle<PhysicalDeviceObject>,
    guest: &InstalledGuest,
) -> Result<Worker> {
    start_mode(physical, guest, Mode::Sdhci)
}

#[cfg(feature = "userspace-device-test")]
pub fn start_virtio_test(
    physical: &OwnedHandle<PhysicalDeviceObject>,
    guest: &InstalledGuest,
) -> Result<Worker> {
    start_mode(physical, guest, Mode::VirtioTest)
}

#[derive(Clone, Copy)]
enum Mode {
    Sdhci,
    #[cfg(feature = "userspace-device-test")]
    VirtioTest,
}
impl Mode {
    fn validate(self, physical: &OwnedHandle<PhysicalDeviceObject>) -> Result<()> {
        let valid = match self {
            Self::Sdhci => device::mmio_read(physical.as_handle_ref(), 0x40, 4)? & (1 << 28) != 0,
            #[cfg(feature = "userspace-device-test")]
            Self::VirtioTest => {
                device::mmio_read(physical.as_handle_ref(), 0, 4)? == 0x74726976
                    && device::mmio_read(physical.as_handle_ref(), 4, 4)? == 2
                    && device::mmio_read(physical.as_handle_ref(), 8, 4)? == 8
                    && device::mmio_read(physical.as_handle_ref(), 0x70, 4)? == 0
            }
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Status(Status::NOT_SUPPORTED))
        }
    }
    fn asserted(self, physical: &OwnedHandle<PhysicalDeviceObject>) -> Result<bool> {
        match self {
            Self::Sdhci => {
                let status = device::mmio_read(physical.as_handle_ref(), 0x30, 4)? as u32;
                let enable = device::mmio_read(physical.as_handle_ref(), 0x38, 4)? as u32;
                Ok(line_asserted(status, enable))
            }
            #[cfg(feature = "userspace-device-test")]
            Self::VirtioTest => Ok(device::mmio_read(physical.as_handle_ref(), 0x60, 4)? & 3 != 0),
        }
    }
}

fn start_mode(
    physical: &OwnedHandle<PhysicalDeviceObject>,
    guest: &InstalledGuest,
    mode: Mode,
) -> Result<Worker> {
    if guest.cpus.is_empty() || guest.cpus.len() > 8 {
        return Err(Error::Status(Status::NOT_SUPPORTED));
    }
    let physical = physical.duplicate(physical.info()?.rights)?;
    // Share the same process-local capabilities. Installed VM/vCPU handles
    // intentionally cannot be duplicated through the Native capability API.
    let machine = MachineGuard(Arc::clone(&guest.machine));
    // Even a read may have device side effects. Only inspect capabilities after
    // installation has retained the VM backing and activated the DMA lease.
    mode.validate(&physical)?;
    let mut cpus = Vec::new();
    cpus.try_reserve_exact(guest.cpus.len())
        .map_err(|_| Error::Status(Status::NO_MEMORY))?;
    for cpu in &guest.cpus {
        cpus.push(Arc::clone(cpu));
    }
    let (stop, receiver) = channel::create_pair()?;
    let waits = WaitSet::new(cpus.len() + 3)?;
    let stop_id = waits.add(
        receiver.as_handle_ref(),
        ObjectSignals::<ByteChannelObject>::PEER_CLOSED,
    )?;
    let machine_id = waits.add(
        machine.0.as_handle_ref(),
        ObjectSignals::<VirtualMachineObject>::TERMINATED,
    )?;
    let irq_id = waits.add(
        physical.as_handle_ref(),
        ObjectSignals::<PhysicalDeviceObject>::READABLE,
    )?;
    let mut cpu_ids = Vec::new();
    cpu_ids
        .try_reserve_exact(cpus.len())
        .map_err(|_| Error::Status(Status::NO_MEMORY))?;
    for cpu in &cpus {
        cpu_ids.push(waits.add(
            cpu.as_handle_ref(),
            ObjectSignals::<VirtualCpuObject>::MMIO_REQUEST,
        )?);
    }
    let pump = Pump {
        physical,
        machine,
        cpus,
        _stop: receiver,
        waits,
        stop_id,
        machine_id,
        irq_id,
        cpu_ids,
        irq_armed: true,
        mode,
    };
    let thread = thread::Builder::new()
        .name("sdhci".into())
        .stack_size(64 * 1024)
        .spawn(move || pump.run())
        .map_err(|_| Error::Status(Status::NO_MEMORY))?;
    Ok(Worker {
        stop: Some(stop),
        thread: Some(thread),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Line {
    Asserted,
    Idle,
    Changed,
}

impl Pump {
    fn reconcile(&self) -> Result<Line> {
        for _ in 0..2 {
            let sequence = device::irq_pending(self.physical.as_handle_ref())?;
            let asserted = self.mode.asserted(&self.physical)?;
            match device::irq_complete(self.physical.as_handle_ref(), sequence, asserted) {
                Ok(()) => return Ok(if asserted { Line::Asserted } else { Line::Idle }),
                Err(Error::Status(Status::BUSY)) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(Line::Changed)
    }

    fn update_irq(&mut self) -> Result<()> {
        let line = self.reconcile()?;
        if !self.irq_armed && line != Line::Asserted {
            self.waits.rearm(self.irq_id)?;
            self.irq_armed = true;
        }
        Ok(())
    }

    fn mmio(&mut self, index: usize) -> Result<()> {
        if let Some(request) = vm::pending_mmio(self.cpus[index].as_handle_ref())? {
            let offset = request_offset(request, PHYSICAL_MMIO)?;
            let completion = match request.operation {
                vm::MmioOperation::Read => vm::MmioCompletion::Read(device::mmio_read(
                    self.physical.as_handle_ref(),
                    offset,
                    request.width,
                )?),
                vm::MmioOperation::Write(value) => {
                    device::mmio_write(
                        self.physical.as_handle_ref(),
                        offset,
                        request.width,
                        value,
                    )?;
                    vm::MmioCompletion::Write
                }
            };
            // Resample after every access, including partial-width W1C writes,
            // before allowing the trapped instruction to resume.
            self.update_irq()?;
            vm::complete_mmio(self.cpus[index].as_handle_ref(), request.id, completion)?;
        }
        self.waits.rearm(self.cpu_ids[index])
    }

    fn run(mut self) -> Result<()> {
        let result = self.service();
        // A transport failure must terminate Linux, waking any Native client
        // waiting for its response. MachineGuard also covers unwinding.
        let stop = vm::request_stop(self.machine.0.as_handle_ref());
        result.and(stop)
    }

    fn service(&mut self) -> Result<()> {
        loop {
            let event = self.waits.wait(u64::MAX)?;
            if event.registration == self.stop_id || event.registration == self.machine_id {
                return Ok(());
            }
            if event.registration == self.irq_id {
                self.irq_armed = false;
                self.update_irq()?;
            } else if let Some(index) = self.cpu_ids.iter().position(|id| *id == event.registration)
            {
                self.mmio(index)?;
            } else {
                return Err(Error::InvalidResponse);
            }
        }
    }
}
