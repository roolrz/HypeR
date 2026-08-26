// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Installed virtual-machine identity and aggregate ownership.
//!
//! The registry is the single publication point for guest address spaces and
//! interrupt models. Construction happens outside the registry lock through a
//! non-cloneable reservation; dropping an unpublished reservation rolls its
//! slot back. Installed entries are intentionally never removed in this first
//! lifecycle slice, which keeps references returned to active vCPUs stable.
//!
//! The registry lock protects slot identity only. It is released before an
//! address-space operation and must never be nested with a VM-internal lock.
//!
//! Adding removal is not a slot-state-only change. It first requires a
//! lifetime-bearing binding that can prove every scheduler thread and per-CPU
//! active-vCPU publication has retired, plus quiescence for address-space
//! operations and architecture-specific stage-2/TLB teardown. Until those
//! capabilities exist, an installed slot must retain its allocation forever.

use alloc::boxed::Box;

use hyper::cpu::CpuIndex;
use hyper::sync::InterruptSpinLock;
use hyper::vm::translation::{
    ExclusiveExecution, ExecutionClaim, ExecutionError, ExecutionReleaseFailure,
};

use super::VmInterruptController;
use super::device::VirtualDeviceSet;
use super::memory::GuestAddressSpace;
use crate::kernel::task::thread::ThreadId;

type RegistryLock = InterruptSpinLock<VmRegistry, crate::hal::irq::LocalMask>;
type AddressSpaceLock = InterruptSpinLock<GuestAddressSpace, crate::hal::irq::LocalMask>;

static REGISTRY: RegistryLock = InterruptSpinLock::new(VmRegistry::new());

/// Registry metadata is fixed-capacity so reservation never allocates while
/// holding the global identity lock.
const MAX_VIRTUAL_MACHINES: usize = 64;

/// Logical identity issued by the VM registry.
///
/// The slot and generation are deliberately private. Callers may retain and
/// compare an identity, but cannot manufacture one from a hardware VMID or a
/// registry index.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VmId {
    slot: u32,
    generation: u32,
}

impl VmId {
    const fn execution_owner(self) -> u64 {
        ((self.generation as u64) << 32) | self.slot as u64
    }
}

/// Non-cloneable vCPU capability for one fixed VM allocation.
///
/// The allocation is owned by `VmBuilder` while the vCPU is dormant and by an
/// `Installed` registry slot before that vCPU can become runnable. Installed
/// slots cannot currently be removed, so the address remains valid for the
/// token's entire usable lifetime. This is deliberately narrower than a true
/// shared-lifetime lease; registry removal must wait for allocator-safe shared
/// ownership.
pub(in crate::kernel) struct VmBinding {
    id: VmId,
    machine: usize,
}

impl VmBinding {
    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    pub(crate) fn interrupts(&self) -> &VmInterruptController {
        // SAFETY: VmBuilder mints this pointer from its boxed, fixed-address
        // aggregate. The vCPU stays dormant while the builder owns it, and a
        // runnable vCPU implies the same Box is in a non-removable Installed
        // slot. The returned reference cannot outlive this token borrow.
        unsafe { &(*core::ptr::with_exposed_provenance::<VirtualMachine>(self.machine)).interrupts }
    }

    #[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_X86_64))]
    pub(super) fn devices(&self) -> &VirtualDeviceSet {
        // SAFETY: The fixed-allocation and non-removal contract is identical
        // to `interrupts`; the reference is scoped to this capability borrow.
        unsafe { &(*core::ptr::with_exposed_provenance::<VirtualMachine>(self.machine)).devices }
    }

    pub(super) fn with_address_space<R>(
        &self,
        operation: impl FnOnce(&mut GuestAddressSpace) -> R,
    ) -> R {
        // SAFETY: The fixed-allocation contract is identical to `interrupts`.
        let machine =
            unsafe { &*core::ptr::with_exposed_provenance::<VirtualMachine>(self.machine) };
        machine.address_space.with(operation)
    }

    /// Claims this VM's currently single active execution interval.
    ///
    /// The capability is intentionally independent of vCPU identity: current
    /// construction installs one boot vCPU, and the invariant remains safe if
    /// additional vCPU objects are added before VM-wide shootdown support.
    pub(in crate::kernel) fn claim_execution(
        &self,
        cpu: CpuIndex,
    ) -> Result<ExecutionClaim, ExecutionError> {
        // SAFETY: The fixed-allocation and non-removal contract is identical
        // to `interrupts`; ExclusiveExecution contains only atomic state.
        let machine =
            unsafe { &*core::ptr::with_exposed_provenance::<VirtualMachine>(self.machine) };
        machine.execution.claim(cpu)
    }

    pub(in crate::kernel) fn release_execution(
        &self,
        claim: ExecutionClaim,
        current_cpu: CpuIndex,
    ) -> Result<(), ExecutionReleaseFailure> {
        // SAFETY: See `claim_execution`. Consuming the non-cloneable claim
        // makes one successful release the only valid transition.
        let machine =
            unsafe { &*core::ptr::with_exposed_provenance::<VirtualMachine>(self.machine) };
        machine.execution.release(claim, current_cpu)
    }
}

/// Architecture stage-2 identifier assigned independently of logical VM IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HardwareVmid(u16);

impl HardwareVmid {
    pub(crate) const fn get(self) -> u16 {
        self.0
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) const fn for_test(value: u16) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    IdentityExhausted,
    InvalidReservation,
    NotInstalled,
    RegistryFull,
    StaleIdentity,
}

/// Rollback capability for one unpublished registry slot.
///
/// This type is intentionally neither `Copy` nor `Clone`. The builder consumes
/// it on successful publication; every earlier return drops it and returns the
/// slot to `Vacant` with a new generation.
pub(crate) struct VmReservation {
    id: VmId,
    hardware_vmid: HardwareVmid,
    unpublished: bool,
}

impl VmReservation {
    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    pub(crate) const fn hardware_vmid(&self) -> HardwareVmid {
        self.hardware_vmid
    }
}

impl Drop for VmReservation {
    fn drop(&mut self) {
        if self.unpublished {
            REGISTRY.with(|registry| registry.cancel(self.id));
        }
    }
}

/// Locally complete VM state awaiting its single registry publication.
pub(crate) struct VmBuilder {
    machine: Box<VirtualMachine>,
    // Drop last so the logical and hardware identities cannot be reused while
    // unpublished VM-owned resources are still being destroyed.
    reservation: VmReservation,
}

impl VmBuilder {
    pub(crate) fn new(
        reservation: VmReservation,
        address_space: GuestAddressSpace,
        interrupts: VmInterruptController,
        devices: VirtualDeviceSet,
    ) -> Result<Self, Error> {
        let id = reservation.id;
        let machine = hyper::mm::try_box(VirtualMachine {
            id,
            address_space: InterruptSpinLock::new(address_space),
            execution: ExclusiveExecution::new(id.execution_owner()),
            interrupts,
            devices,
            boot_vcpu: None,
        })
        .map_err(|_| Error::Allocation)?;
        Ok(Self {
            machine,
            reservation,
        })
    }

    fn vcpu_binding(&self) -> VmBinding {
        VmBinding {
            id: self.reservation.id,
            machine: core::ptr::from_ref(self.machine.as_ref()).expose_provenance(),
        }
    }

    /// Prepares the non-runnable scheduler object and absorbs its rollback
    /// capability into the installation transaction.
    ///
    /// Only the returned typestate exposes installation. Neither the raw VM
    /// binding nor the reserved `ThreadId` can escape through a safe API.
    pub(crate) fn prepare_boot_vcpu(
        mut self,
        vcpu_id: u32,
        context: crate::hal::vm::VcpuContext,
    ) -> Result<PreparedVm, crate::kernel::task::scheduler::Error> {
        let dormant = super::vcpu::create_thread(self.vcpu_binding(), vcpu_id, context)?;
        // SAFETY: PreparedVm takes ownership of the rollback capability and
        // cannot expose this identity until registry installation succeeds.
        let thread = unsafe { dormant.id_for_vm_install() };
        self.machine.boot_vcpu = Some(thread);
        Ok(PreparedVm {
            dormant,
            machine: self.machine,
            reservation: self.reservation,
        })
    }
}

/// Fully allocated VM aggregate that can only be installed or rolled back.
pub(crate) struct PreparedVm {
    // Drop first so an unpublished vCPU loses its raw binding before the VM
    // allocation and identity can be released.
    dormant: crate::kernel::task::scheduler::DormantVcpuThread,
    machine: Box<VirtualMachine>,
    // Drop last for the same identity-reuse ordering as VmBuilder.
    reservation: VmReservation,
}

impl PreparedVm {
    /// Publishes the fully constructed aggregate in one registry transition.
    pub(crate) fn install(self) -> Result<InstalledVm, Error> {
        let Self {
            dormant,
            machine,
            mut reservation,
        } = self;
        let id = reservation.id;
        let mut machine = Some(machine);
        // Validation precedes ownership transfer under the registry lock. On
        // failure `machine` remains owned by this stack frame and is dropped
        // only after the lock has been released, avoiding registry -> allocator
        // lock nesting while its address space is destroyed.
        if let Err(error) = REGISTRY.with(|registry| registry.install(id, &mut machine)) {
            drop(dormant);
            drop(machine);
            drop(reservation);
            return Err(error);
        }
        reservation.unpublished = false;
        drop(reservation);
        // SAFETY: Installation transferred the binding's Box into a
        // non-removable registry slot before exposing the ThreadId.
        let boot_vcpu = unsafe { dormant.commit_after_vm_install() };
        Ok(InstalledVm { id, boot_vcpu })
    }
}

/// Capabilities exposed only after complete VM publication.
pub(crate) struct InstalledVm {
    id: VmId,
    boot_vcpu: ThreadId,
}

impl InstalledVm {
    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    pub(crate) const fn boot_vcpu(&self) -> ThreadId {
        self.boot_vcpu
    }
}

struct VirtualMachine {
    id: VmId,
    address_space: AddressSpaceLock,
    execution: ExclusiveExecution,
    interrupts: VmInterruptController,
    // RISC-V currently has no emulated devices, but the aggregate retains the
    // zero-sized set so adding one does not change VM lifecycle ownership.
    #[cfg_attr(CONFIG_ARCH_RISCV64, allow(dead_code))]
    devices: VirtualDeviceSet,
    boot_vcpu: Option<ThreadId>,
}

struct VmRegistry {
    slots: [VmSlot; MAX_VIRTUAL_MACHINES],
}

enum VmSlot {
    Vacant { generation: u32 },
    Reserved { generation: u32 },
    Installed(Box<VirtualMachine>),
    Exhausted,
}

impl VmRegistry {
    const fn new() -> Self {
        Self {
            slots: [const { VmSlot::Vacant { generation: 0 } }; MAX_VIRTUAL_MACHINES],
        }
    }

    fn reserve(&mut self) -> Result<VmReservation, Error> {
        let (slot, generation) = self
            .slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot {
                VmSlot::Vacant { generation } => Some((index, *generation)),
                VmSlot::Reserved { .. } | VmSlot::Installed(_) | VmSlot::Exhausted => None,
            })
            .ok_or(Error::RegistryFull)?;
        let slot_u32 = u32::try_from(slot).map_err(|_| Error::IdentityExhausted)?;
        let hardware = slot_u32
            .checked_add(1)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(Error::IdentityExhausted)?;
        self.slots[slot] = VmSlot::Reserved { generation };
        Ok(VmReservation {
            id: VmId {
                slot: slot_u32,
                generation,
            },
            hardware_vmid: HardwareVmid(hardware),
            unpublished: true,
        })
    }

    fn cancel(&mut self, id: VmId) {
        let Ok(slot) = usize::try_from(id.slot) else {
            return;
        };
        let Some(entry) = self.slots.get_mut(slot) else {
            return;
        };
        if matches!(entry, VmSlot::Reserved { generation } if *generation == id.generation) {
            *entry = match id.generation.checked_add(1) {
                Some(generation) => VmSlot::Vacant { generation },
                None => VmSlot::Exhausted,
            };
        }
    }

    fn install(
        &mut self,
        id: VmId,
        machine: &mut Option<Box<VirtualMachine>>,
    ) -> Result<(), Error> {
        let candidate = machine.as_ref().ok_or(Error::InvalidReservation)?;
        if candidate.id != id
            || candidate.boot_vcpu.is_none()
            || candidate.boot_vcpu == Some(ThreadId::BOOTSTRAP)
        {
            return Err(Error::InvalidReservation);
        }
        let slot = usize::try_from(id.slot).map_err(|_| Error::InvalidReservation)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::InvalidReservation)?;
        if !matches!(entry, VmSlot::Reserved { generation } if *generation == id.generation) {
            return Err(Error::InvalidReservation);
        }
        let machine = machine.take().ok_or(Error::InvalidReservation)?;
        *entry = VmSlot::Installed(machine);
        Ok(())
    }

    fn installed(&self, id: VmId) -> Result<&VirtualMachine, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        match self.slots.get(slot) {
            Some(VmSlot::Installed(machine)) if machine.id == id => Ok(machine),
            Some(
                VmSlot::Installed(_)
                | VmSlot::Reserved { .. }
                | VmSlot::Vacant { .. }
                | VmSlot::Exhausted,
            ) => Err(Error::StaleIdentity),
            None => Err(Error::NotInstalled),
        }
    }
}

pub(crate) fn reserve() -> Result<VmReservation, Error> {
    REGISTRY.with(VmRegistry::reserve)
}

/// Runs an operation under the installed VM's address-space lock.
///
/// The raw pointer is dereferenced only after the registry lock is released.
/// This is valid because installed entries cannot be removed in this lifecycle
/// slice, and each entry is heap allocated before publication.
pub(super) fn with_address_space<R>(
    id: VmId,
    operation: impl FnOnce(&mut GuestAddressSpace) -> R,
) -> Result<R, Error> {
    let address_space = REGISTRY.with(|registry| {
        registry
            .installed(id)
            .map(|machine| core::ptr::from_ref(&machine.address_space))
    })?;
    // SAFETY: Installation pins the boxed VirtualMachine permanently. Removal
    // is deliberately absent, so the address-space lock outlives this call.
    Ok(unsafe { &*address_space }.with(operation))
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn verify_reservation_rollback() -> Result<(), Error> {
    let first = reserve()?;
    let first_id = first.id();
    drop(first);
    let second = reserve()?;
    if second.id().slot != first_id.slot || second.id().generation == first_id.generation {
        return Err(Error::InvalidReservation);
    }
    drop(second);
    Ok(())
}
