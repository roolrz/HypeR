// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! x86-64 legacy guest-platform device service.

use hyper::sync::InterruptSpinLock;
use hyper::vm::x86::device::legacy_pc::{Error as ModelError, LegacyPcDevices, PendingInterrupt};

type LegacyLock = InterruptSpinLock<LegacyPcDevices, crate::hal::irq::LocalMask>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Active(super::super::super::active_vcpu::Error),
    Model(ModelError),
    NoActiveVcpu,
}

pub(crate) struct VirtualDeviceSet {
    legacy_pc: LegacyLock,
    virtual_serial: Option<super::super::VirtualSerialBinding>,
}

impl VirtualDeviceSet {
    const fn new(virtual_serial: Option<super::super::VirtualSerialBinding>) -> Self {
        Self {
            legacy_pc: InterruptSpinLock::new(LegacyPcDevices::new()),
            virtual_serial,
        }
    }

    fn access(
        &self,
        port: u16,
        size: usize,
        write: bool,
        value: u32,
    ) -> Result<Option<u32>, Error> {
        let outcome = self
            .legacy_pc
            .with(|devices| devices.access(port, size, write, value))
            .map_err(Error::Model)?;
        if let Some(byte) = outcome.transmitted
            && let Some(output) = &self.virtual_serial
        {
            output.write_byte(byte);
        }
        Ok(outcome.value)
    }

    fn pending_interrupt(&self, timer_pending: bool) -> Option<PendingInterrupt> {
        self.legacy_pc
            .with(|devices| devices.pending_interrupt(timer_pending))
    }

    pub(in crate::kernel::vm) fn bind_virtual_serial(
        &self,
        vm: super::super::super::registry::VmId,
        vcpu: u32,
        thread: crate::kernel::task::thread::ThreadId,
    ) {
        if let Some(output) = &self.virtual_serial {
            output.bind(crate::kernel::vm::virtual_serial::Route { vm, vcpu, thread });
        }
    }

    pub(in crate::kernel::vm) fn disconnect_virtual_serial(
        &self,
        vm: super::super::super::registry::VmId,
    ) {
        if let Some(output) = &self.virtual_serial {
            output.disconnect(vm);
        }
    }
}

pub(super) const fn prepare(
    virtual_serial: Option<super::super::VirtualSerialBinding>,
) -> Result<VirtualDeviceSet, Error> {
    Ok(VirtualDeviceSet::new(virtual_serial))
}

pub(super) const fn supports_configuration(
    _profile: u32,
    _memory_base: u64,
    _memory_size: u64,
) -> bool {
    false
}

pub(super) const fn default_timer_interrupt() -> hyper::vm::interrupt::VirtualInterruptId {
    hyper::vm::interrupt::VirtualInterruptId::new(0xef)
}

pub(super) fn kick_virtual_serial(route: crate::kernel::vm::virtual_serial::Route) {
    let _ = (route.vcpu, route.thread);
    let _ = super::super::super::registry::with_binding(route.vm, |binding| {
        binding.devices().disconnect_virtual_serial(route.vm);
    });
}

pub(super) fn access_port(access: hyper::vm::x86::exit::PortIoExit) -> Result<Option<u32>, Error> {
    use hyper::vm::x86::exit::PortIoOperation;

    let (write, value) = match access.operation() {
        PortIoOperation::Input => (false, 0),
        PortIoOperation::Output(value) => (true, value),
    };
    match super::super::super::active_vcpu::with(|execution| {
        let binding = execution.vm_binding().ok_or(Error::NoActiveVcpu)?;
        binding
            .devices()
            .access(access.port(), access.width().bytes(), write, value)
    })
    .map_err(Error::Active)?
    {
        Some(result) => result,
        None => Err(Error::NoActiveVcpu),
    }
}

pub(super) fn pending_interrupt(timer_pending: bool) -> Result<Option<PendingInterrupt>, Error> {
    match super::super::super::active_vcpu::with(|execution| {
        let binding = execution.vm_binding().ok_or(Error::NoActiveVcpu)?;
        Ok(binding.devices().pending_interrupt(timer_pending))
    })
    .map_err(Error::Active)?
    {
        Some(result) => result,
        None => Err(Error::NoActiveVcpu),
    }
}
