// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! VM-owned virtual-device instances and host bindings.
//!
//! Reusable register models live under [`hyper::vm`]. This service owns one
//! model set per [`super::registry::VirtualMachine`] and applies kernel policy
//! after dropping each device lock: host-console output and architecture
//! interrupt delivery never execute while a model is borrowed.

#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_X86_64))]
use super::registry::VmBinding;

#[cfg(CONFIG_ARCH_AARCH64)]
mod gicv3;

#[cfg(CONFIG_ARCH_AARCH64)]
mod imp {
    use hyper::sync::InterruptSpinLock;
    use hyper::vm::aarch64::device::pl011::{
        REFERENCE_BASE, REFERENCE_INTERRUPT, REFERENCE_SIZE, VirtualPl011, VirtualPl011Error,
    };
    use hyper::vm::exit::{MmioAccess, MmioOperation};
    use hyper::vm::interrupt::VirtualInterruptId;

    use super::VmBinding;

    type ConsoleLock = InterruptSpinLock<VirtualPl011, crate::hal::irq::LocalMask>;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Error {
        InvalidInterrupt,
        Model(VirtualPl011Error),
    }

    pub(crate) struct VirtualDeviceSet {
        console: ConsoleLock,
        console_interrupt: VirtualInterruptId,
    }

    pub(super) struct MmioOutcome {
        pub(super) value: Option<u64>,
        pub(super) interrupt: Option<(VirtualInterruptId, bool)>,
    }

    impl VirtualDeviceSet {
        pub(super) fn prepare() -> Result<Self, Error> {
            let console_interrupt =
                VirtualInterruptId::new(REFERENCE_INTERRUPT).ok_or(Error::InvalidInterrupt)?;
            Ok(Self {
                console: InterruptSpinLock::new(VirtualPl011::new()),
                console_interrupt,
            })
        }

        fn access(&self, access: MmioAccess) -> Result<Option<MmioOutcome>, Error> {
            let address = access.address().get();
            let Some(offset) = address.checked_sub(REFERENCE_BASE) else {
                return Ok(None);
            };
            if offset >= REFERENCE_SIZE {
                return Ok(None);
            }
            let outcome = self.console.with(|console| match access.operation() {
                MmioOperation::Read => console.read(offset, access.size()),
                MmioOperation::Write(value) => console.write(offset, access.size(), value),
            });
            let outcome = outcome.map_err(Error::Model)?;
            if let Some(byte) = outcome.transmitted {
                crate::kernel::log::console::write_raw_byte(byte);
            }
            Ok(Some(MmioOutcome {
                value: outcome.value,
                interrupt: Some((self.console_interrupt, outcome.interrupt_asserted)),
            }))
        }

        fn receive(&self, byte: u8) -> MmioOutcome {
            let interrupt_asserted = self.console.with(|console| console.receive(byte));
            MmioOutcome {
                value: None,
                interrupt: Some((self.console_interrupt, interrupt_asserted)),
            }
        }
    }

    pub(super) fn handle_mmio(
        binding: &VmBinding,
        access: MmioAccess,
    ) -> Result<Option<MmioOutcome>, Error> {
        binding.devices().access(access)
    }

    pub(super) fn receive(binding: &VmBinding, byte: u8) -> MmioOutcome {
        binding.devices().receive(byte)
    }

    pub(super) fn handle_gic(
        interrupts: &super::super::VmInterruptController,
        vcpu: hyper::vm::interrupt::VirtualCpuId,
        access: MmioAccess,
    ) -> Result<Option<MmioOutcome>, super::gicv3::Error> {
        if !super::gicv3::handles(access.address().get()) {
            return Ok(None);
        }
        let value = match access.operation() {
            MmioOperation::Read => Some(super::gicv3::read(
                interrupts,
                vcpu,
                access.address().get(),
                access.size(),
            )?),
            MmioOperation::Write(value) => {
                super::gicv3::write(
                    interrupts,
                    vcpu,
                    access.address().get(),
                    access.size(),
                    value,
                )?;
                None
            }
        };
        Ok(Some(MmioOutcome {
            value,
            interrupt: None,
        }))
    }
}

#[cfg(CONFIG_ARCH_RISCV64)]
mod imp {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Error {}

    pub(crate) struct VirtualDeviceSet;

    impl VirtualDeviceSet {
        pub(super) const fn prepare() -> Result<Self, Error> {
            Ok(Self)
        }
    }
}

#[cfg(CONFIG_ARCH_X86_64)]
mod imp {
    use hyper::sync::InterruptSpinLock;
    use hyper::vm::x86::device::legacy_pc::{
        Error as ModelError, LegacyPcDevices, PendingInterrupt,
    };

    use super::VmBinding;

    type LegacyLock = InterruptSpinLock<LegacyPcDevices, crate::hal::irq::LocalMask>;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Error {
        Active(super::super::active_vcpu::Error),
        Model(ModelError),
        NoActiveVcpu,
    }

    pub(crate) struct VirtualDeviceSet {
        legacy_pc: LegacyLock,
    }

    impl VirtualDeviceSet {
        pub(super) const fn prepare() -> Result<Self, Error> {
            Ok(Self {
                legacy_pc: InterruptSpinLock::new(LegacyPcDevices::new()),
            })
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
            if let Some(byte) = outcome.transmitted {
                crate::kernel::log::console::write_raw_byte(byte);
            }
            Ok(outcome.value)
        }

        fn pending_interrupt(&self, timer_pending: bool) -> Option<PendingInterrupt> {
            self.legacy_pc
                .with(|devices| devices.pending_interrupt(timer_pending))
        }
    }

    pub(super) fn access_port(
        binding: &VmBinding,
        port: u16,
        size: usize,
        write: bool,
        value: u32,
    ) -> Result<Option<u32>, Error> {
        binding.devices().access(port, size, write, value)
    }

    pub(super) fn pending_interrupt(
        binding: &VmBinding,
        timer_pending: bool,
    ) -> Option<PendingInterrupt> {
        binding.devices().pending_interrupt(timer_pending)
    }
}

pub use imp::Error;
pub(crate) use imp::VirtualDeviceSet;

pub(crate) fn prepare() -> Result<VirtualDeviceSet, Error> {
    VirtualDeviceSet::prepare()
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(in crate::kernel) fn dispatch_mmio(
    execution: &mut crate::kernel::task::thread::VcpuExecution,
    interrupts: &super::VmInterruptController,
    access: hyper::vm::exit::MmioAccess,
) -> hyper::vm::exit::MmioAction {
    use hyper::vm::exit::{MmioAction, MmioOperation};

    let Some(binding) = execution.vm_binding() else {
        return MmioAction::Stop;
    };
    match imp::handle_mmio(binding, access) {
        Ok(Some(outcome)) => {
            if let Some((interrupt, asserted)) = outcome.interrupt
                && let Err(error) = crate::hal::vm::update_guest_device_interrupt(
                    &mut execution.hardware,
                    execution.vcpu_id,
                    interrupts,
                    interrupt,
                    asserted,
                )
            {
                crate::pr_err!("HypeR: failed to update guest device interrupt: {error:?}");
                return MmioAction::Stop;
            }
            match (access.operation(), outcome.value) {
                (MmioOperation::Read, Some(value)) => MmioAction::CompleteRead(value),
                (MmioOperation::Write(_), _) => MmioAction::CompleteWrite,
                (MmioOperation::Read, None) => MmioAction::Stop,
            }
        }
        Ok(None) => match imp::handle_gic(
            interrupts,
            hyper::vm::interrupt::VirtualCpuId::new(execution.vcpu_id),
            access,
        ) {
            Ok(Some(outcome)) => match (access.operation(), outcome.value) {
                (MmioOperation::Read, Some(value)) => MmioAction::CompleteRead(value),
                (MmioOperation::Write(_), _) => MmioAction::CompleteWrite,
                (MmioOperation::Read, None) => MmioAction::Stop,
            },
            Ok(None) => MmioAction::Unhandled,
            Err(error) => {
                crate::pr_err!(
                    "HypeR: unsupported guest GIC access at {:#x}: {error:?}",
                    access.address().get()
                );
                MmioAction::Stop
            }
        },
        Err(error) => {
            crate::pr_err!(
                "HypeR: unsupported guest console access at {:#x}: {error:?}",
                access.address().get()
            );
            MmioAction::Stop
        }
    }
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(super) fn receive_console_input(byte: u8) -> bool {
    match super::active_vcpu::with(|execution, interrupts| {
        let binding = execution.vm_binding().ok_or(imp::Error::InvalidInterrupt)?;
        let outcome = imp::receive(binding, byte);
        let Some((interrupt, asserted)) = outcome.interrupt else {
            return Ok(());
        };
        crate::hal::vm::update_guest_device_interrupt(
            &mut execution.hardware,
            execution.vcpu_id,
            interrupts,
            interrupt,
            asserted,
        )
        .map_err(|_| imp::Error::InvalidInterrupt)
    }) {
        Ok(Some(Ok(()))) => true,
        Ok(Some(Err(_)) | None) | Err(_) => false,
    }
}

#[cfg(not(CONFIG_ARCH_AARCH64))]
pub(super) const fn receive_console_input(_byte: u8) -> bool {
    false
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn access_port(access: hyper::vm::x86::exit::PortIoExit) -> Result<Option<u32>, Error> {
    use hyper::vm::x86::exit::PortIoOperation;

    let (write, value) = match access.operation() {
        PortIoOperation::Input => (false, 0),
        PortIoOperation::Output(value) => (true, value),
    };
    match super::active_vcpu::with(|execution, _| {
        let binding = execution.vm_binding().ok_or(Error::NoActiveVcpu)?;
        imp::access_port(binding, access.port(), access.width().bytes(), write, value)
    })
    .map_err(Error::Active)?
    {
        Some(result) => result,
        None => Err(Error::NoActiveVcpu),
    }
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn pending_interrupt(
    timer_pending: bool,
) -> Result<Option<hyper::vm::x86::device::legacy_pc::PendingInterrupt>, Error> {
    match super::active_vcpu::with(|execution, _| {
        let binding = execution.vm_binding().ok_or(Error::NoActiveVcpu)?;
        Ok(imp::pending_interrupt(binding, timer_pending))
    })
    .map_err(Error::Active)?
    {
        Some(result) => result,
        None => Err(Error::NoActiveVcpu),
    }
}
