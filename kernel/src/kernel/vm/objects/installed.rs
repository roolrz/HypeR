// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Handle-visible installed VM and vCPU lifecycle objects.

use hyper::mm::FallibleArc;

use super::{Error, reserve_object_charge};
use crate::kernel::accounting::{CommittedCharge, ResourceDomain};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectKind, SignalMask, SignalSource, TransferClass, private,
};
use crate::kernel::vm::installed::{
    InstalledMachine, VirtualCpuSnapshot, VirtualMachineConfiguration, VirtualMachineSnapshot,
};

pub(crate) struct VirtualMachineObject {
    owner: FallibleArc<InstalledMachine>,
    _object_charge: CommittedCharge,
}

impl VirtualMachineObject {
    pub(crate) fn try_new(
        owner: FallibleArc<InstalledMachine>,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        Ok(Self {
            owner,
            _object_charge: reserve_object_charge::<Self>(domain)?,
        })
    }

    pub(crate) fn owner(&self) -> FallibleArc<InstalledMachine> {
        self.owner.clone()
    }

    pub(crate) fn request_stop(&self) {
        InstalledMachine::request_stop(&self.owner);
    }

    pub(crate) fn snapshot(&self) -> VirtualMachineSnapshot {
        self.owner.snapshot()
    }

    pub(crate) fn configuration(&self) -> VirtualMachineConfiguration {
        self.owner.configuration()
    }
}

impl private::Sealed for VirtualMachineObject {}
impl private::UserExportable for VirtualMachineObject {}

impl KernelObject for VirtualMachineObject {
    const KIND: ObjectKind = ObjectKind::VIRTUAL_MACHINE;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::WRITE)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::REQUEST_STOP);

    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(SignalSource::new(
            self.owner.signal_state(),
            SignalMask::from_trusted_bits(
                hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_TERMINATED
                    | hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_POWER_REQUEST
                    | hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_VCPU_TERMINATED,
            ),
        ))
    }

    fn on_zero_active_handles(&self, _retirement: &mut crate::kernel::object::ObjectRetirement) {
        self.request_stop();
    }
}

pub(crate) struct VirtualCpuObject {
    owner: FallibleArc<InstalledMachine>,
    id: u32,
    _object_charge: CommittedCharge,
}

impl VirtualCpuObject {
    pub(crate) fn try_new(
        owner: FallibleArc<InstalledMachine>,
        id: u32,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        owner.endpoint(id)?;
        Ok(Self {
            owner,
            id,
            _object_charge: reserve_object_charge::<Self>(domain)?,
        })
    }

    pub(crate) fn snapshot(&self) -> VirtualCpuSnapshot {
        self.owner.snapshot_vcpu(self.id)
    }

    pub(crate) fn pending_mmio(&self) -> Result<Option<hyper::vm::device::mmio::Request>, Error> {
        Ok(self.owner.pending_mmio(self.id)?)
    }

    pub(crate) fn complete_mmio(
        &self,
        id: u64,
        action: hyper::vm::exit::MmioAction,
    ) -> Result<(), Error> {
        self.owner.complete_mmio(self.id, id, action)?;
        Ok(())
    }

    /// Commits this installed vCPU from dormant to scheduler-runnable.
    pub(crate) fn start(&self) -> Result<(), Error> {
        self.owner.start_vcpu(self.id)?;
        Ok(())
    }
}

impl private::Sealed for VirtualCpuObject {}
impl private::UserExportable for VirtualCpuObject {}

impl KernelObject for VirtualCpuObject {
    const KIND: ObjectKind = ObjectKind::VIRTUAL_CPU;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::WRITE)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::START);

    fn signal_source(&self) -> Option<SignalSource<'_>> {
        let signals = match self.owner.vcpu_signal_state(self.id) {
            Ok(signals) => signals,
            Err(_) => crate::hal::cpu::halt(),
        };
        Some(SignalSource::new(
            signals,
            SignalMask::from_trusted_bits(
                hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_TERMINATED
                    | hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_MMIO_REQUEST,
            ),
        ))
    }
}
