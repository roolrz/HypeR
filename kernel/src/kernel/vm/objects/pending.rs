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
use crate::kernel::vm::memory::backing::{Layout, Region};
use crate::kernel::vm::registry::{PreparedVm, VmBuilder, VmLifecycleResources, VmReservation};

type PendingLock = InterruptSpinLock<PendingState, crate::hal::irq::LocalMask>;

enum PendingState {
    Configuring {
        reservation: VmReservation,
        lifecycle_resources: VmLifecycleResources,
        memory: alloc::boxed::Box<Layout>,
        bootstrap: Option<VirtualCpuBootstrap>,
        physical: Option<crate::kernel::device::assigned::Assignment>,
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
        // Reject unavailable host backends before reserving VM slots/resources.
        // The service transaction restores the one-shot creation lease on error.
        if crate::kernel::vm::entry_ready().is_none() {
            return Err(Error::UnsupportedArchitecture);
        }
        validate_configuration(configuration)?;
        let lifecycle_resources =
            VmLifecycleResources::try_reserve(domain, configuration.vcpu_count)?;
        let reservation = crate::kernel::vm::registry::reserve()?;
        Ok(Self {
            configuration,
            state: PendingLock::new(PendingState::Configuring {
                reservation,
                lifecycle_resources,
                memory: Layout::try_new(configuration.memory_size, domain)?,
                bootstrap: None,
                physical: None,
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
        self.map_memory(Region::new(0, 0, self.configuration.memory_size, backing)?)
    }

    pub(crate) fn map_memory(&self, region: Region) -> Result<(), Error> {
        let mut region = Some(region);
        let result = self.state.with(|state| match state {
            PendingState::Configuring { memory, .. } => {
                memory.insert(&mut region).map_err(Into::into)
            }
            _ => Err(Error::BadState),
        });
        // Final lease release may free pages; keep it outside PendingLock.
        drop(region);
        result
    }

    pub(crate) fn assign_physical(
        &self,
        assignment: crate::kernel::device::assigned::Assignment,
    ) -> Result<(), Error> {
        let mut assignment = Some(assignment);
        let result = self.state.with(|state| match state {
            PendingState::Configuring { physical, .. } if physical.is_none() => {
                *physical = assignment.take();
                Ok(())
            }
            _ => Err(Error::BadState),
        });
        drop(assignment);
        result
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
        let mut serial = Some(serial);
        let result = self.state.with(|state| match state {
            PendingState::Configuring { virtual_serial, .. } if virtual_serial.is_none() => {
                let candidate = serial.as_ref().ok_or(Error::BadState)?;
                if !candidate.object().claim_assignment() {
                    return Err(Error::BadState);
                }
                *virtual_serial = serial.take();
                Ok(())
            }
            PendingState::Configuring { .. }
            | PendingState::Transition
            | PendingState::Sealed(_)
            | PendingState::Failed => Err(Error::BadState),
        });
        // A last rejected reference may release retained pages/accounting.
        drop(serial);
        result
    }

    /// Realizes every fallible VM resource without making it globally visible.
    pub(crate) fn seal(&self) -> Result<(), Error> {
        let state = self.state.with(|state| {
            let owned = core::mem::replace(state, PendingState::Transition);
            match owned {
                PendingState::Configuring {
                    reservation,
                    lifecycle_resources,
                    memory,
                    bootstrap: Some(bootstrap),
                    virtual_serial,
                    physical,
                } if memory.complete() => Ok((
                    reservation,
                    lifecycle_resources,
                    memory,
                    bootstrap,
                    virtual_serial,
                    physical,
                )),
                other => {
                    *state = other;
                    Err(Error::BadState)
                }
            }
        })?;
        let result = self.prepare(state.0, state.1, state.2, state.3, state.4, state.5);
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

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &self,
        mut reservation: VmReservation,
        lifecycle_resources: VmLifecycleResources,
        memory: alloc::boxed::Box<Layout>,
        bootstrap: VirtualCpuBootstrap,
        virtual_serial: Option<
            KernelRef<super::super::virtual_serial::VirtualSerial, VmDeviceBinding>,
        >,
        physical: Option<crate::kernel::device::assigned::Assignment>,
    ) -> Result<PreparedVm, Error> {
        if physical.is_some() {
            memory.validate_dma()?;
        }
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
        let prepared = VmBuilder::new(
            reservation,
            lifecycle_resources,
            self.configuration,
            address_space,
            interrupts,
            interrupt_controller_charge,
            devices,
        )?
        .prepare_boot_vcpu(0, bootstrap)
        .map_err(Error::from)?;
        prepared.set_physical(physical);
        Ok(prepared)
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
    if !(1..=crate::hal::vm::maximum_guest_vcpus()).contains(&configuration.vcpu_count)
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
