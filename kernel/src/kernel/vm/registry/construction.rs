// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! VM construction, boot-vCPU preparation, and atomic publication typestates.

use hyper::mm::FallibleArc;

use super::execution::{VirtualMachine, VmBinding};
use super::resources::VmLifecycleResources;
use super::{Error, REGISTRY, VmId, VmReservation};
use crate::kernel::accounting::CommittedCharge;
use crate::kernel::task::thread::ThreadId;
use crate::kernel::vm::VmInterruptController;
use crate::kernel::vm::device::VirtualDeviceSet;
use crate::kernel::vm::memory::GuestAddressSpace;

/// Locally complete VM state awaiting its single registry publication.
pub(crate) struct VmBuilder {
    machine: FallibleArc<VirtualMachine>,
    // Drop last so the logical and hardware identities cannot be reused while
    // unpublished VM-owned resources are still being destroyed.
    reservation: VmReservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VcpuPreparationError {
    Registry(Error),
    Scheduler(crate::kernel::task::scheduler::Error),
}

impl VmBuilder {
    pub(crate) fn new(
        reservation: VmReservation,
        lifecycle_resources: VmLifecycleResources,
        configuration: crate::kernel::vm::installed::VirtualMachineConfiguration,
        address_space: GuestAddressSpace,
        interrupts: VmInterruptController,
        interrupt_controller_charge: Option<CommittedCharge>,
        devices: VirtualDeviceSet,
    ) -> Result<Self, Error> {
        let id = reservation.id;
        let lifecycle = crate::kernel::vm::installed::InstalledMachine::try_new(
            configuration,
            lifecycle_resources,
        )
        .map_err(|_| Error::Allocation)?;
        let machine = VirtualMachine::try_new(
            id,
            address_space,
            interrupts,
            devices,
            lifecycle,
            interrupt_controller_charge,
        )?;
        Ok(Self {
            machine,
            reservation,
        })
    }

    fn vcpu_binding(&self) -> VmBinding {
        VmBinding::new(self.reservation.id, self.machine.clone())
    }

    /// Prepares the non-runnable scheduler object and absorbs its rollback
    /// capability into the installation transaction.
    ///
    /// Only the returned typestate exposes installation. Neither the VM
    /// binding nor the reserved `ThreadId` can escape through a safe API.
    pub(crate) fn prepare_boot_vcpu(
        self,
        vcpu_id: u32,
        context: crate::hal::vm::VcpuContext,
    ) -> Result<PreparedVm, VcpuPreparationError> {
        let resources = self
            .machine
            .lifecycle()
            .reserve_vcpu_runtime()
            .map_err(VcpuPreparationError::Registry)?;
        let dormant = crate::kernel::vm::vcpu::create_thread(
            self.vcpu_binding(),
            vcpu_id,
            context,
            resources,
        )
        .map_err(VcpuPreparationError::Scheduler)?;
        // SAFETY: PreparedVm takes ownership of the rollback capability and
        // cannot expose this identity until registry installation succeeds.
        let thread = unsafe { dormant.id_for_vm_install() };
        let endpoint = self
            .machine
            .endpoint(vcpu_id)
            .map_err(VcpuPreparationError::Registry)?;
        if endpoint.bind_thread(thread).is_err() {
            crate::hal::cpu::halt()
        }
        Ok(PreparedVm {
            dormant,
            machine: self.machine,
            reservation: self.reservation,
            boot_vcpu: thread,
        })
    }
}

/// Fully allocated VM aggregate that can only be installed or rolled back.
pub(crate) struct PreparedVm {
    // Drop first so an unpublished vCPU releases its strong VM binding before
    // the builder owner and identity are released.
    dormant: crate::kernel::task::scheduler::DormantVcpuThread,
    machine: FallibleArc<VirtualMachine>,
    // Drop last for the same identity-reuse ordering as VmBuilder.
    reservation: VmReservation,
    boot_vcpu: ThreadId,
}

impl PreparedVm {
    pub(in crate::kernel::vm) fn lifecycle(
        &self,
    ) -> FallibleArc<crate::kernel::vm::installed::InstalledMachine> {
        self.machine.lifecycle()
    }

    /// Completes every fallible prerequisite for registry publication.
    pub(crate) fn install(self) -> Result<InstalledVm, Error> {
        let id = self.reservation.id;
        REGISTRY.with(|registry| registry.validate_install(id, &self.machine))?;
        self.machine.activate_identifier_for_install()?;
        self.machine.bind_virtual_serial(0, self.boot_vcpu);
        let lifecycle = self.machine.lifecycle();
        let Self {
            dormant,
            machine,
            mut reservation,
            boot_vcpu: _,
        } = self;

        // Preserve PreparedVm's declared rollback drop order through every
        // fallible operation. This infallible publication tail may now split
        // its fields into independent local owners.
        let mut machine = Some(machine);
        // The first locked validation proved this uniquely reserved slot and
        // complete machine. A live reservation prevents any intervening slot
        // transition, so final publication is infallible after VMID activation.
        REGISTRY.with(|registry| registry.install_prevalidated(id, &mut machine));
        reservation.unpublished = false;
        drop(reservation);
        // SAFETY: Installation transferred one strong owner into the registry
        // before exposing the ThreadId retained by the VM and vCPU binding.
        let _boot_vcpu = unsafe { dormant.commit_after_vm_install() };
        Ok(InstalledVm {
            id,
            #[cfg(feature = "kernel-self-test")]
            boot_vcpu: _boot_vcpu,
            control: VmControl::mint_for_install(id),
            lifecycle,
        })
    }
}

/// Sole authority to start retirement of one installed VM incarnation.
///
/// Only `PreparedVm::install` can mint this non-Clone token. In particular,
/// no module can reconstruct it from the intentionally Copy `VmId`.
#[must_use = "the installed VM lifecycle authority must be retired explicitly"]
pub(in crate::kernel::vm) struct VmControl {
    id: VmId,
}

impl VmControl {
    const fn mint_for_install(id: VmId) -> Self {
        Self { id }
    }

    pub(super) const fn id(&self) -> VmId {
        self.id
    }
}

/// Capabilities exposed only after complete VM publication.
#[must_use = "bind the installed VM identity, vCPU, and lifecycle authority"]
pub(crate) struct InstalledVm {
    id: VmId,
    #[cfg(feature = "kernel-self-test")]
    boot_vcpu: ThreadId,
    control: VmControl,
    lifecycle: FallibleArc<crate::kernel::vm::installed::InstalledMachine>,
}

impl InstalledVm {
    #[cfg(feature = "kernel-self-test")]
    pub(in crate::kernel::vm) fn into_boot_parts(self) -> (VmId, ThreadId, VmControl) {
        (self.id, self.boot_vcpu, self.control)
    }

    pub(in crate::kernel::vm) fn publish_handle_lifecycle(
        self,
    ) -> FallibleArc<crate::kernel::vm::installed::InstalledMachine> {
        self.lifecycle.publish_installed(self.id, self.control);
        self.lifecycle
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) const fn boot_vcpu_for_test(&self) -> ThreadId {
        self.boot_vcpu
    }
}
