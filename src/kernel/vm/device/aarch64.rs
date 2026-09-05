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
type ConsoleRouteLock = InterruptSpinLock<Option<ConsoleRoute>, crate::hal::irq::LocalMask>;

#[derive(Clone, Copy)]
struct ConsoleRoute {
    vm: VmId,
    vcpu: u32,
    thread: crate::kernel::task::thread::ThreadId,
}

static CONSOLE_ROUTE: ConsoleRouteLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInterrupt,
    Model(VirtualPl011Error),
}

pub(crate) struct VirtualDeviceSet {
    console: ConsoleLock,
    console_interrupt: GicInterruptId,
}

struct MmioOutcome {
    value: Option<u64>,
}

impl VirtualDeviceSet {
    fn new() -> Result<Self, Error> {
        let console_interrupt =
            GicInterruptId::new(REFERENCE_INTERRUPT).ok_or(Error::InvalidInterrupt)?;
        Ok(Self {
            console: InterruptSpinLock::new(VirtualPl011::new()),
            console_interrupt,
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
            let outcome = match access.operation() {
                MmioOperation::Read => console.read(offset, access.size()),
                MmioOperation::Write(value) => console.write(offset, access.size(), value),
            }
            .map_err(Error::Model)?;
            // The console lock precedes the guest interrupt-controller lock.
            // This preserves FIFO mutation -> line publication ordering.
            update(self.console_interrupt, outcome.interrupt_asserted)?;
            Ok::<_, Error>(outcome)
        })?;
        // Host output occurs after both device and controller locks release.
        if let Some(byte) = outcome.transmitted {
            crate::kernel::log::console::write_raw_byte(byte);
        }
        Ok(Some(MmioOutcome {
            value: outcome.value,
        }))
    }

    fn receive(
        &self,
        byte: u8,
        update: impl FnOnce(GicInterruptId, bool) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.console.with(|console| {
            let asserted = console.receive(byte);
            update(self.console_interrupt, asserted)
        })
    }
}

pub(super) fn prepare() -> Result<VirtualDeviceSet, Error> {
    VirtualDeviceSet::new()
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
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    interrupts: &super::super::super::VmInterruptController,
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
        let (binding, hardware, vcpu_id) = execution.device_context()?;
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
    execution: &mut crate::kernel::task::thread::VcpuExecution,
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

pub(super) fn receive_console_input(byte: u8) -> super::super::ConsoleInputDisposition {
    let Some(route) = CONSOLE_ROUTE.with(|route| *route) else {
        return super::super::ConsoleInputDisposition::from_guest_claim(false);
    };
    let delivery = super::super::super::registry::with_binding(route.vm, |binding| {
        binding
            .devices()
            .receive(byte, |interrupt, asserted| {
                crate::hal::vm::update_saved_guest_device_interrupt(
                    binding.interrupts(),
                    route.vcpu,
                    interrupt,
                    asserted,
                )
                .map_err(|_| Error::InvalidInterrupt)
            })
            .map_err(|_| ConsoleDeliveryError::Device)?;
        // Device and controller locks are released before endpoint publication
        // can wake a Thread or issue a targeted reschedule notification.
        binding
            .publish_interrupt_reconcile(route.vcpu, route.thread)
            .map_err(ConsoleDeliveryError::Registry)
    });
    match delivery {
        Ok(Ok(())) => {}
        Ok(Err(ConsoleDeliveryError::Registry(
            super::super::super::registry::Error::EndpointClosed,
        ))) => {
            let _ = clear_console_route_exact(route.vm, route.thread);
        }
        Ok(Err(_)) => {}
        Err(
            super::super::super::registry::Error::NotInstalled
            | super::super::super::registry::Error::StaleIdentity,
        ) => {
            let _ = clear_console_route_exact(route.vm, route.thread);
        }
        Err(_) => {}
    }
    // Once a route was observed, failure cannot transfer ownership to Native
    // userspace: doing so would leak a guest-owned input byte across domains.
    super::super::ConsoleInputDisposition::from_guest_claim(true)
}

enum ConsoleDeliveryError {
    Device,
    Registry(super::super::super::registry::Error),
}

/// Selects the first Linux VM as the host-console input recipient.
pub(super) fn try_publish_console_route(
    vm: VmId,
    vcpu: u32,
    thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    if thread == crate::kernel::task::thread::ThreadId::BOOTSTRAP
        || !super::super::super::registry::is_installed(vm)
    {
        return false;
    }
    // Never nest registry and route locks. Validation on each side closes the
    // race with Installed -> Quiescing without reversing lock order.
    let published = CONSOLE_ROUTE.with(|route| {
        if route.is_some() {
            return false;
        }
        *route = Some(ConsoleRoute { vm, vcpu, thread });
        true
    });
    if !published {
        return false;
    }
    if super::super::super::registry::is_installed(vm) {
        true
    } else {
        let _ = clear_console_route_exact(vm, thread);
        false
    }
}

pub(super) fn clear_console_route_for_vm(expected_vm: VmId) {
    CONSOLE_ROUTE.with(|route| {
        if route.is_some_and(|current| current.vm == expected_vm) {
            *route = None;
        }
    });
}

fn clear_console_route_exact(
    expected_vm: VmId,
    expected_thread: crate::kernel::task::thread::ThreadId,
) -> bool {
    CONSOLE_ROUTE.with(|route| match *route {
        Some(current) if current.vm == expected_vm && current.thread == expected_thread => {
            *route = None;
            true
        }
        Some(_) | None => false,
    })
}
