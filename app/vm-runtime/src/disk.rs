// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One disk's slow configuration channel and direct notification route.

use super::Error;
use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::handle::{
    ByteChannelObject, GuestMemoryObject, OwnedHandle, RightsOffer, VirtualCpuObject,
    VirtualMachineObject,
};
use hyper_os::wait::{ObjectSignals, RegistrationId, WaitSet};
use hyper_service::io;
use hyper_vm_support::io_backend::{Backend, OperationDeadline, RemoteNotification};
use std::mem::MaybeUninit;
use std::num::NonZeroU64;
use std::sync::Arc;

pub(super) struct Disk {
    _session: CapabilityChannel,
    backend: Backend<
        Arc<OwnedHandle<ByteChannelObject>>,
        RemoteNotification<Arc<OwnedHandle<ByteChannelObject>>>,
    >,
    registrations: Vec<RegistrationId>,
    deadline: OperationDeadline,
}

pub(super) fn bind(
    session: CapabilityChannel,
    machine: OwnedHandle<VirtualMachineObject>,
    memory: &OwnedHandle<GuestMemoryObject>,
    base: u64,
    length: u64,
) -> Result<(OwnedHandle<VirtualMachineObject>, Disk), Error> {
    let deadline = hyper_os::time::deadline_after(std::time::Duration::from_secs(60))
        .map_err(Error::OperatingSystem)?
        .as_raw();
    let rights = machine
        .as_handle_ref()
        .info()
        .map_err(Error::OperatingSystem)?
        .rights;
    let mut machine = Some(machine);
    let record = io::encode_memory(base, length).ok_or(Error::InvalidControl)?;
    let dispositions = &mut [
        CapabilityDisposition::move_handle(&mut machine, RightsOffer::Exact(rights))
            .map_err(Error::OperatingSystem)?,
        CapabilityDisposition::duplicate(
            memory.as_handle_ref(),
            RightsOffer::Exact(io::MEMORY_RIGHTS),
        )
        .map_err(Error::OperatingSystem)?,
    ];
    io::send_capabilities(&session, &record, dispositions, deadline)
        .map_err(Error::OperatingSystem)?;
    let mut bytes = [MaybeUninit::uninit(); 32];
    let mut slots = [
        CapabilityReceiveSlot::new::<VirtualMachineObject>(rights),
        CapabilityReceiveSlot::new::<ByteChannelObject>(io::MAILBOX_RIGHTS),
    ];
    let message = session
        .receive(deadline, &mut bytes, &mut slots)
        .map_err(Error::OperatingSystem)?;
    if message.capability_count() != 2
        || message.bytes().len() != io::BOUND_MESSAGE.len() + 8
        || !message.bytes().starts_with(io::BOUND_MESSAGE)
    {
        return Err(Error::InvalidControl);
    }
    let identity = NonZeroU64::new(u64::from_le_bytes(
        message.bytes()[io::BOUND_MESSAGE.len()..]
            .try_into()
            .map_err(|_| Error::InvalidControl)?,
    ))
    .ok_or(Error::InvalidControl)?;
    let machine = slots[0]
        .take::<VirtualMachineObject>()
        .map_err(Error::OperatingSystem)?
        .ok_or(Error::InvalidControl)?;
    let channel = slots[1]
        .take::<ByteChannelObject>()
        .map_err(Error::OperatingSystem)?
        .ok_or(Error::InvalidControl)?;
    let channel = Arc::new(channel);
    let mut backend = Backend::new(
        channel.clone(),
        RemoteNotification::new(channel),
        (base, length),
        io::FRONTEND_MMIO,
        identity,
    )
    .map_err(|_| Error::InvalidControl)?;
    while !backend.ready() {
        backend.progress().map_err(|_| Error::InvalidControl)?;
        if backend.ready() {
            break;
        }
        if hyper_os::time::monotonic_now()
            .map_err(Error::OperatingSystem)?
            .as_nanoseconds()
            >= deadline
        {
            return Err(Error::InvalidControl);
        }
        let signals = ObjectSignals::<ByteChannelObject>::READABLE
            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED);
        let signals = if backend.wants_write() {
            signals.union(ObjectSignals::<ByteChannelObject>::WRITABLE)
        } else {
            signals
        };
        hyper_os::wait::wait_many(
            &[hyper_os::wait::WaitItem::new(
                backend.mailbox().as_handle_ref(),
                signals,
            )],
            deadline,
        )
        .map_err(Error::OperatingSystem)?;
    }
    Ok((
        machine,
        Disk {
            _session: session,
            backend,
            registrations: Vec::new(),
            deadline: OperationDeadline::default(),
        },
    ))
}

impl Disk {
    pub(super) fn service(&mut self, cpus: &[OwnedHandle<VirtualCpuObject>]) -> Result<(), Error> {
        if let Some(completion) = self.backend.progress().map_err(|_| Error::InvalidControl)? {
            hyper_os::vm::complete_mmio(
                cpus[completion.vcpu].as_handle_ref(),
                completion.id,
                completion.result,
            )
            .map_err(Error::OperatingSystem)?;
        }
        // Observe completion before enforcing the original operation budget.
        // A readable console or unrelated vCPU cannot extend this deadline.
        self.update_deadline()?;
        for (index, cpu) in cpus.iter().enumerate() {
            if self.backend.busy() {
                break;
            }
            if let Some(request) =
                hyper_os::vm::pending_mmio(cpu.as_handle_ref()).map_err(Error::OperatingSystem)?
                && let Some(completion) = self
                    .backend
                    .mmio(index, request)
                    .map_err(|_| Error::InvalidControl)?
            {
                hyper_os::vm::complete_mmio(cpu.as_handle_ref(), completion.id, completion.result)
                    .map_err(Error::OperatingSystem)?;
            }
        }
        self.update_deadline()?;
        Ok(())
    }

    fn update_deadline(&mut self) -> Result<(), Error> {
        if !self.backend.busy() {
            return self
                .deadline
                .observe(false, 0)
                .map_err(|_| Error::InvalidControl);
        }
        let now = hyper_os::time::monotonic_now()
            .map_err(Error::OperatingSystem)?
            .as_nanoseconds();
        self.deadline
            .observe(self.backend.busy(), now)
            .map_err(|_| Error::InvalidControl)
    }

    pub(super) fn deadline(&self) -> u64 {
        self.deadline.raw()
    }

    pub(super) fn prepare_wait(
        &mut self,
        waits: &WaitSet,
        cpus: &[OwnedHandle<VirtualCpuObject>],
    ) -> Result<(), Error> {
        for id in self.registrations.drain(..) {
            waits.remove(id).map_err(Error::OperatingSystem)?;
        }
        let signals = ObjectSignals::<ByteChannelObject>::READABLE
            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED);
        let signals = if self.backend.wants_write() {
            signals.union(ObjectSignals::<ByteChannelObject>::WRITABLE)
        } else {
            signals
        };
        self.registrations.push(
            waits
                .add(self.backend.mailbox().as_handle_ref(), signals)
                .map_err(Error::OperatingSystem)?,
        );
        if !self.backend.busy() {
            for cpu in cpus {
                self.registrations.push(
                    waits
                        .add(
                            cpu.as_handle_ref(),
                            ObjectSignals::<VirtualCpuObject>::MMIO_REQUEST,
                        )
                        .map_err(Error::OperatingSystem)?,
                );
            }
        }
        Ok(())
    }
    pub(super) fn owns(&self, registration: RegistrationId) -> bool {
        self.registrations.contains(&registration)
    }
}
