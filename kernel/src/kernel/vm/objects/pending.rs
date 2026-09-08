// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Unpublished VM configuration, realization, and sealing transaction.

use hyper::mm::{FallibleArc, PAGE_SIZE};
use hyper::sync::InterruptSpinLock;

use super::{Error, VirtualCpuBootstrap, reserve_object_charge};
use crate::kernel::accounting::{CommittedCharge, ResourceDomain};
use crate::kernel::authority::Rights;
use crate::kernel::mm::user_space::{GuestMemoryBacking, VmoObject};
use crate::kernel::object::{
    KernelObject, KernelRef, ObjectKind, TransferClass, VmDeviceBinding, private,
};
use crate::kernel::vm::installed::{InstalledMachine, VirtualMachineConfiguration};
use crate::kernel::vm::memory::GuestAddressSpace;
use crate::kernel::vm::registry::{PreparedVm, VmBuilder, VmLifecycleResources, VmReservation};

type PendingLock = InterruptSpinLock<PendingState, crate::hal::irq::LocalMask>;

enum PendingState {
    Configuring {
        reservation: VmReservation,
        lifecycle_resources: VmLifecycleResources,
        memory: Option<GuestMemoryBacking>,
        bootstrap: Option<VirtualCpuBootstrap>,
        virtual_serial:
            Option<KernelRef<super::super::virtual_serial::VirtualSerial, VmDeviceBinding>>,
    },
    Transition,
    Sealed(Option<PreparedVm>),
    Failed,
}

/// Linear, unpublished VM construction transaction.
pub(crate) struct PendingVirtualMachine {
    configuration: VirtualMachineConfiguration,
    state: PendingLock,
    domain: ResourceDomain,
    _object_charge: CommittedCharge,
}

impl PendingVirtualMachine {
    pub(crate) fn try_new(
        configuration: VirtualMachineConfiguration,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        validate_configuration(configuration)?;
        let lifecycle_resources =
            VmLifecycleResources::try_reserve(domain, configuration.vcpu_count)?;
        let reservation = crate::kernel::vm::registry::reserve()?;
        Ok(Self {
            configuration,
            state: PendingLock::new(PendingState::Configuring {
                reservation,
                lifecycle_resources,
                memory: None,
                bootstrap: None,
                virtual_serial: None,
            }),
            domain: domain.clone(),
            _object_charge: reserve_object_charge::<Self>(domain)?,
        })
    }

    pub(crate) fn resource_domain(&self) -> ResourceDomain {
        self.domain.clone()
    }

    pub(crate) fn set_memory(&self, vmo: &VmoObject) -> Result<(), Error> {
        if vmo.size() != self.configuration.memory_size {
            return Err(Error::InvalidConfiguration);
        }
        let backing = GuestMemoryBacking::try_from_vmo(vmo)?;
        self.state.with(|state| match state {
            PendingState::Configuring { memory, .. } if memory.is_none() => {
                *memory = Some(backing);
                Ok(())
            }
            PendingState::Configuring { .. }
            | PendingState::Transition
            | PendingState::Sealed(_)
            | PendingState::Failed => Err(Error::BadState),
        })
    }

    pub(crate) fn set_bootstrap(&self, bootstrap: VirtualCpuBootstrap) -> Result<(), Error> {
        validate_bootstrap(self.configuration, bootstrap)?;
        self.state.with(|state| match state {
            PendingState::Configuring {
                bootstrap: slot, ..
            } if slot.is_none() => {
                *slot = Some(bootstrap);
                Ok(())
            }
            PendingState::Configuring { .. }
            | PendingState::Transition
            | PendingState::Sealed(_)
            | PendingState::Failed => Err(Error::BadState),
        })
    }

    /// Commits one explicitly delegated host-console output route.
    ///
    /// The retained typed reference is independent of the caller's handle and
    /// moves into the installed device set at seal. No route exists unless a
    /// userspace VMM performs this operation before sealing.
    pub(crate) fn set_virtual_serial(
        &self,
        serial: KernelRef<super::super::virtual_serial::VirtualSerial, VmDeviceBinding>,
    ) -> Result<(), Error> {
        self.state.with(|state| match state {
            PendingState::Configuring { virtual_serial, .. } if virtual_serial.is_none() => {
                *virtual_serial = Some(serial);
                Ok(())
            }
            PendingState::Configuring { .. }
            | PendingState::Transition
            | PendingState::Sealed(_)
            | PendingState::Failed => Err(Error::BadState),
        })
    }

    /// Realizes every fallible VM resource without making it globally visible.
    pub(crate) fn seal(&self) -> Result<(), Error> {
        let state = self.state.with(|state| {
            let owned = core::mem::replace(state, PendingState::Transition);
            match owned {
                PendingState::Configuring {
                    reservation,
                    lifecycle_resources,
                    memory: Some(memory),
                    bootstrap: Some(bootstrap),
                    virtual_serial,
                } => Ok((
                    reservation,
                    lifecycle_resources,
                    memory,
                    bootstrap,
                    virtual_serial,
                )),
                other => {
                    *state = other;
                    Err(Error::BadState)
                }
            }
        })?;
        let result = self.prepare(state.0, state.1, state.2, state.3, state.4);
        self.state.with(|slot| match result {
            Ok(prepared) => {
                *slot = PendingState::Sealed(Some(prepared));
                Ok(())
            }
            Err(error) => {
                *slot = PendingState::Failed;
                Err(error)
            }
        })
    }

    fn prepare(
        &self,
        mut reservation: VmReservation,
        lifecycle_resources: VmLifecycleResources,
        memory: GuestMemoryBacking,
        bootstrap: VirtualCpuBootstrap,
        virtual_serial: Option<
            KernelRef<super::super::virtual_serial::VirtualSerial, VmDeviceBinding>,
        >,
    ) -> Result<PreparedVm, Error> {
        let mut address_space = GuestAddressSpace::from_vmo(
            reservation.take_hardware_vmid()?,
            self.configuration.guest_physical_base,
            self.configuration.memory_size,
            memory,
            &self.domain,
        )?;
        address_space.publish_resident_instructions()?;
        address_space.finish_boot_loading();
        let interrupt_plan = crate::hal::vm::prepare_interrupt_controller(
            self.configuration.vcpu_count,
            crate::kernel::vm::device::default_timer_interrupt(),
        )?;
        let interrupt_controller_charge = lifecycle_resources.reserve_interrupt_controller(
            crate::hal::vm::prepared_interrupt_controller_allocation_size(&interrupt_plan),
        )?;
        let interrupts = crate::hal::vm::create_prepared_interrupt_controller(interrupt_plan)?;
        let virtual_serial = virtual_serial
            .map(crate::kernel::vm::device::VirtualSerialBinding::from_virtual_serial);
        let devices = crate::kernel::vm::device::prepare(virtual_serial)?;
        let mut context = crate::hal::vm::prepare_native_bootstrap_context(
            bootstrap.entry,
            bootstrap.stack,
            bootstrap.arguments,
        )
        .map_err(|_| Error::InvalidConfiguration)?;
        crate::hal::vm::set_virtual_count(
            &mut context,
            crate::kernel::time::monotonic_ticks(),
            crate::kernel::time::monotonic_ticks(),
        );
        VmBuilder::new(
            reservation,
            lifecycle_resources,
            self.configuration,
            address_space,
            interrupts,
            interrupt_controller_charge,
            devices,
        )?
        .prepare_boot_vcpu(0, context)
        .map_err(Into::into)
    }

    pub(crate) fn take_prepared(&self) -> Result<PreparedVm, Error> {
        self.state.with(|state| match state {
            PendingState::Sealed(prepared) => prepared.take().ok_or(Error::BadState),
            PendingState::Configuring { .. } | PendingState::Transition | PendingState::Failed => {
                Err(Error::BadState)
            }
        })
    }

    pub(crate) fn installed_lifecycle(&self) -> Result<FallibleArc<InstalledMachine>, Error> {
        self.state.with(|state| match state {
            PendingState::Sealed(Some(prepared)) => Ok(prepared.lifecycle()),
            PendingState::Configuring { .. }
            | PendingState::Transition
            | PendingState::Sealed(None)
            | PendingState::Failed => Err(Error::BadState),
        })
    }
}

impl private::Sealed for PendingVirtualMachine {}
impl private::UserExportable for PendingVirtualMachine {}

impl KernelObject for PendingVirtualMachine {
    const KIND: ObjectKind = ObjectKind::PENDING_VIRTUAL_MACHINE;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::INSPECT)
        .union(Rights::WRITE)
        .union(Rights::START)
        .union(Rights::REQUEST_STOP);
}

fn validate_configuration(configuration: VirtualMachineConfiguration) -> Result<(), Error> {
    if !crate::hal::vm::userspace_vm_lifecycle_available() {
        return Err(Error::UnsupportedArchitecture);
    }
    let supported_architecture = crate::hal::vm::guest_architecture_abi();
    if configuration.architecture != supported_architecture {
        return Err(Error::UnsupportedArchitecture);
    }
    if !crate::kernel::vm::device::supports_configuration(
        configuration.platform_profile,
        configuration.guest_physical_base,
        configuration.memory_size,
    ) {
        return Err(Error::InvalidConfiguration);
    }
    if configuration.vcpu_count != 1
        || configuration.memory_size == 0
        || !configuration.memory_size.is_multiple_of(PAGE_SIZE)
        || !configuration.guest_physical_base.is_multiple_of(PAGE_SIZE)
        || configuration
            .guest_physical_base
            .checked_add(configuration.memory_size)
            .is_none()
    {
        return Err(Error::InvalidConfiguration);
    }
    Ok(())
}

fn validate_bootstrap(
    configuration: VirtualMachineConfiguration,
    bootstrap: VirtualCpuBootstrap,
) -> Result<(), Error> {
    let end = configuration
        .guest_physical_base
        .checked_add(configuration.memory_size)
        .ok_or(Error::InvalidConfiguration)?;
    if bootstrap.entry < configuration.guest_physical_base || bootstrap.entry >= end {
        return Err(Error::InvalidConfiguration);
    }
    if bootstrap.stack != 0
        && (bootstrap.stack <= configuration.guest_physical_base || bootstrap.stack > end)
    {
        return Err(Error::InvalidConfiguration);
    }
    Ok(())
}
