// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` `PL011` and `GICv3` guest-platform device service.

use hyper::sync::InterruptSpinLock;
use hyper::vm::aarch64::device::pl011::{
    REFERENCE_BASE, REFERENCE_INTERRUPT, REFERENCE_SIZE, VirtualPl011, VirtualPl011Error,
};
use hyper::vm::arm::gic::GicInterruptId;
use hyper::vm::exit::{MmioAccess, MmioAction, MmioOperation};

use super::super::super::registry::{VmBinding, VmId};

type ConsoleLock = InterruptSpinLock<VirtualPl011, crate::hal::irq::LocalMask>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInterrupt,
    Model(VirtualPl011Error),
}

pub(crate) struct VirtualDeviceSet {
    console: ConsoleLock,
    console_interrupt: GicInterruptId,
    virtual_serial: Option<super::super::VirtualSerialBinding>,
}

struct MmioOutcome {
    value: Option<u64>,
}

impl VirtualDeviceSet {
    fn new(virtual_serial: Option<super::super::VirtualSerialBinding>) -> Result<Self, Error> {
        let console_interrupt =
            GicInterruptId::new(REFERENCE_INTERRUPT).ok_or(Error::InvalidInterrupt)?;
        Ok(Self {
            console: InterruptSpinLock::new(VirtualPl011::new()),
            console_interrupt,
            virtual_serial,
        })
    }

    fn access(
        &self,
        access: MmioAccess,
        update: impl FnOnce(GicInterruptId, bool) -> Result<(), Error>,
    ) -> Result<Option<MmioOutcome>, Error> {
        let address = access.address().get();
        let Some(offset) = address.checked_sub(REFERENCE_BASE) else {
            return Ok(None);
        };
        if offset >= REFERENCE_SIZE {
            return Ok(None);
        }
        let outcome = self.console.with(|console| {
            pump_serial_input(console, self.virtual_serial.as_ref());
            let outcome = match access.operation() {
                MmioOperation::Read => console.read(offset, access.size()),
                MmioOperation::Write(value) => console.write(offset, access.size(), value),
            }
            .map_err(Error::Model)?;
            pump_serial_input(console, self.virtual_serial.as_ref());
            // The console lock precedes the guest interrupt-controller lock.
            // This preserves FIFO mutation -> line publication ordering.
            update(self.console_interrupt, outcome.interrupt_asserted)?;
            Ok::<_, Error>(outcome)
        })?;
        // Host output occurs after both device and controller locks release.
        if let Some(byte) = outcome.transmitted
            && let Some(output) = &self.virtual_serial
        {
            output.write_byte(byte);
        }
        Ok(Some(MmioOutcome {
            value: outcome.value,
        }))
    }

    pub(in crate::kernel::vm) fn bind_virtual_serial(
        &self,
        vm: VmId,
        vcpu: u32,
        thread: crate::kernel::task::thread::ThreadId,
    ) {
        if let Some(output) = &self.virtual_serial {
            output.bind(crate::kernel::vm::virtual_serial::Route { vm, vcpu, thread });
        }
    }

    pub(in crate::kernel::vm) fn disconnect_virtual_serial(&self, vm: VmId) {
        if let Some(output) = &self.virtual_serial {
            output.disconnect(vm);
        }
    }

    fn receive_from_virtual_serial(
        &self,
        update: impl FnOnce(GicInterruptId, bool) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.console.with(|console| {
            pump_serial_input(console, self.virtual_serial.as_ref());
            update(self.console_interrupt, console.interrupt_asserted())
        })
    }
}

fn pump_serial_input(
    console: &mut VirtualPl011,
    binding: Option<&super::super::VirtualSerialBinding>,
) {
    let Some(binding) = binding else {
        return;
    };
    while console.can_receive() {
        let Some(byte) = binding.pop_guest_input() else {
            break;
        };
        let _ = console.receive(byte);
    }
}

pub(super) fn prepare(
    virtual_serial: Option<super::super::VirtualSerialBinding>,
) -> Result<VirtualDeviceSet, Error> {
    VirtualDeviceSet::new(virtual_serial)
}

pub(super) fn supports_configuration(profile: u32, memory_base: u64, memory_size: u64) -> bool {
    profile == hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE as u32
        && memory_base
            == hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GUEST_RAM_BASE
        && memory_size != 0
        && memory_base.checked_add(memory_size).is_some()
}

pub(super) const fn default_timer_interrupt() -> hyper::vm::interrupt::VirtualInterruptId {
    hyper::vm::interrupt::VirtualInterruptId::new(
        hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_TIMER_INTERRUPT as u32,
    )
}

#[must_use]
pub(in crate::kernel) struct MmioDispatch {
    action: MmioAction,
}

impl MmioDispatch {
    const fn new(action: MmioAction) -> Self {
        Self { action }
    }

    pub(in crate::kernel) const fn into_action(self) -> MmioAction {
        self.action
    }
}

pub(super) fn dispatch_mmio(
    execution: &mut crate::kernel::vm::vcpu::VcpuExecution,
    access: MmioAccess,
) -> MmioDispatch {
    enum Resolution {
        Action(MmioAction),
        Unhandled(Option<super::super::super::UnhandledMmioReport>),
    }

    fn resolve_access(operation: MmioOperation, outcome: MmioOutcome) -> Resolution {
        match (operation, outcome.value) {
            (MmioOperation::Read, Some(value)) => {
                Resolution::Action(MmioAction::CompleteRead(value))
            }
            (MmioOperation::Write(_), _) => Resolution::Action(MmioAction::CompleteWrite),
            (MmioOperation::Read, None) => Resolution::Action(MmioAction::Stop),
        }
    }

    let Some((vcpu_id, resolution)) = (|| {
        let (binding, hardware, vcpu_id, interrupts) = execution.device_context()?;
        let resolution = match handle_mmio(binding, hardware, interrupts, vcpu_id, access) {
            Ok(Some(outcome)) => resolve_access(access.operation(), outcome),
            Ok(None) => match handle_gic(hardware, interrupts, vcpu_id, access) {
                Ok(Some(outcome)) => resolve_access(access.operation(), outcome),
                Ok(None) => Resolution::Unhandled(binding.admit_unhandled_mmio(vcpu_id, access)),
                Err(_) => Resolution::Action(MmioAction::Stop),
            },
            Err(_) => Resolution::Action(MmioAction::Stop),
        };
        Some((vcpu_id, resolution))
    })() else {
        return MmioDispatch::new(MmioAction::Stop);
    };
    let action = match resolution {
        Resolution::Action(action) => action,
        Resolution::Unhandled(report) => {
            publish_terminal_mmio_report(execution, vcpu_id, report);
            MmioAction::Unhandled
        }
    };
    MmioDispatch::new(action)
}

fn handle_mmio(
    binding: &VmBinding,
    hardware: &mut crate::hal::vm::VcpuHardwareState,
    interrupts: &super::super::super::VmInterruptController,
    vcpu_id: u32,
    access: MmioAccess,
) -> Result<Option<MmioOutcome>, Error> {
    binding.devices().access(access, |interrupt, asserted| {
        crate::hal::vm::update_guest_device_interrupt(
            hardware, vcpu_id, interrupts, interrupt, asserted,
        )
        .map_err(|_| Error::InvalidInterrupt)
    })
}

fn handle_gic(
    hardware: &mut crate::hal::vm::VcpuHardwareState,
    interrupts: &super::super::super::VmInterruptController,
    vcpu_id: u32,
    access: MmioAccess,
) -> Result<Option<MmioOutcome>, super::gicv3::Error> {
    let Some(decoded) = super::gicv3::decode(access)? else {
        return Ok(None);
    };
    let value = super::gicv3::access(hardware, interrupts, vcpu_id, decoded, access.operation())?;
    Ok(Some(MmioOutcome { value }))
}

fn publish_terminal_mmio_report(
    execution: &mut crate::kernel::vm::vcpu::VcpuExecution,
    vcpu_id: u32,
    report: Option<super::super::super::UnhandledMmioReport>,
) {
    if let Some(report) = report
        && execution.publish_terminal_mmio_report(report).is_err()
    {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: duplicate terminal MMIO report for active vCPU {vcpu_id}"
        ));
    }
}

pub(super) fn kick_virtual_serial(route: crate::kernel::vm::virtual_serial::Route) {
    let delivery = super::super::super::registry::with_binding(route.vm, |binding| {
        binding
            .devices()
            .receive_from_virtual_serial(|interrupt, asserted| {
                crate::hal::vm::update_saved_guest_device_interrupt(
                    binding.interrupts(),
                    route.vcpu,
                    interrupt,
                    asserted,
                )
                .map_err(|_| Error::InvalidInterrupt)
            })
            .map_err(|_| ())?;
        binding
            .publish_interrupt_reconcile(route.vcpu, route.thread)
            .map_err(|_| ())
    });
    if !matches!(delivery, Ok(Ok(()))) {
        let _ = super::super::super::registry::with_binding(route.vm, |binding| {
            binding.devices().disconnect_virtual_serial(route.vm);
        });
    }
}
