// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Generation-qualified VM identities, registry slots, and strong lookup.
//!
//! The registry lock protects slot identity only. Construction and retirement
//! own their linear capabilities in sibling modules and never perform guest
//! address-space work while this lock is held.

mod construction;
mod control;
mod execution;
mod resources;

pub(super) use construction::VmControl;
pub(crate) use construction::{PreparedVm, VcpuPreparationError, VmBuilder};
#[cfg(feature = "kernel-self-test")]
pub(crate) use control::{DormantVcpuQuiesceError, verify_dormant_vcpu_quiesce};
pub(super) use control::{QuiescePoll, QuiescentControl, QuiescingVm};
pub(in crate::kernel) use execution::VmBinding;
pub(crate) use execution::{VmExecutionClaim, VmExecutionError};
pub(crate) use resources::VmLifecycleResources;

use hyper::mm::{FallibleArc, UniqueFallibleArc};
use hyper::sync::InterruptSpinLock;

use self::execution::VirtualMachine;
use super::memory::Stage2IdentifierReservation;
use crate::kernel::accounting::ResourceError;

type RegistryLock = InterruptSpinLock<VmRegistry, crate::hal::irq::LocalMask>;

static REGISTRY: RegistryLock = InterruptSpinLock::new(VmRegistry::new());

/// Registry metadata is fixed-capacity so reservation never allocates while
/// holding the global identity lock.
pub(super) const MAX_VIRTUAL_MACHINES: usize = 64;

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

    #[allow(dead_code)]
    const fn diagnostic_id(self) -> super::diagnostics::VmDiagnosticId {
        super::diagnostics::VmDiagnosticId::new(self.slot, self.generation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    #[cfg(feature = "kernel-self-test")]
    AdministrativeStopUnsupported,
    Allocation,
    EndpointClosed,
    IdentityExhausted,
    InvalidReservation,
    NotInstalled,
    Quiescing,
    RegistryFull,
    Resource(ResourceError),
    Scheduler,
    StaleIdentity,
    UnknownVcpu,
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

/// Rollback capability for one unpublished registry slot.
///
/// This type is intentionally neither `Copy` nor `Clone`. The builder consumes
/// it on successful publication; every earlier return drops it and returns the
/// slot to `Vacant` with a new generation.
pub(crate) struct VmReservation {
    id: VmId,
    hardware_vmid: Option<
        crate::kernel::mm::translation_id::IdentifierReservation<
            crate::kernel::mm::translation_id::Stage2Vmid,
        >,
    >,
    unpublished: bool,
}

impl VmReservation {
    #[cfg(feature = "kernel-self-test")]
    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    pub(crate) fn take_hardware_vmid(&mut self) -> Result<Stage2IdentifierReservation, Error> {
        self.hardware_vmid.take().ok_or(Error::InvalidReservation)
    }
}

impl Drop for VmReservation {
    fn drop(&mut self) {
        if self.unpublished {
            REGISTRY.with(|registry| registry.cancel(self.id));
        }
    }
}

struct VmRegistry {
    slots: [VmSlot; MAX_VIRTUAL_MACHINES],
}

enum VmSlot {
    Vacant {
        generation: u32,
    },
    Reserved {
        generation: u32,
    },
    Installed(FallibleArc<VirtualMachine>),
    Quiescing(FallibleArc<VirtualMachine>),
    QuiescentHeld {
        owner: UniqueFallibleArc<VirtualMachine>,
    },
    RetiringHeld(FallibleArc<VirtualMachine>),
    RetiredHeld {
        owner: UniqueFallibleArc<VirtualMachine>,
    },
    Destroying {
        generation: u32,
    },
    Exhausted,
}

impl VmRegistry {
    const fn new() -> Self {
        Self {
            slots: [const { VmSlot::Vacant { generation: 0 } }; MAX_VIRTUAL_MACHINES],
        }
    }

    fn reserve(&mut self) -> Result<VmId, Error> {
        let (slot, generation) = self
            .slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot {
                VmSlot::Vacant { generation } => Some((index, *generation)),
                VmSlot::Reserved { .. } | VmSlot::Installed(_) | VmSlot::Exhausted => None,
                VmSlot::Quiescing(_)
                | VmSlot::QuiescentHeld { .. }
                | VmSlot::RetiringHeld(_)
                | VmSlot::RetiredHeld { .. }
                | VmSlot::Destroying { .. } => None,
            })
            .ok_or(Error::RegistryFull)?;
        let slot_u32 = u32::try_from(slot).map_err(|_| Error::IdentityExhausted)?;
        self.slots[slot] = VmSlot::Reserved { generation };
        Ok(VmId {
            slot: slot_u32,
            generation,
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

    fn validate_install(&self, id: VmId, candidate: &VirtualMachine) -> Result<(), Error> {
        if candidate.id() != id
            || candidate
                .endpoint(0)
                .ok()
                .and_then(super::endpoint::VcpuEndpoint::thread)
                .is_none()
        {
            return Err(Error::InvalidReservation);
        }
        let slot = usize::try_from(id.slot).map_err(|_| Error::InvalidReservation)?;
        let entry = self.slots.get(slot).ok_or(Error::InvalidReservation)?;
        if !matches!(entry, VmSlot::Reserved { generation } if *generation == id.generation) {
            return Err(Error::InvalidReservation);
        }
        Ok(())
    }

    fn install_prevalidated(
        &mut self,
        id: VmId,
        machine: &mut Option<FallibleArc<VirtualMachine>>,
    ) {
        let valid = machine
            .as_deref()
            .is_some_and(|candidate| self.validate_install(id, candidate).is_ok());
        if !valid {
            crate::hal::cpu::halt();
        }
        let Some(machine) = machine.take() else {
            crate::hal::cpu::halt();
        };
        let Some(entry) = self.slots.get_mut(id.slot as usize) else {
            crate::hal::cpu::halt();
        };
        *entry = VmSlot::Installed(machine);
    }

    fn installed(&self, id: VmId) -> Result<&FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        match self.slots.get(slot) {
            Some(VmSlot::Installed(machine)) if machine.id() == id => Ok(machine),
            Some(
                VmSlot::Installed(_)
                | VmSlot::Reserved { .. }
                | VmSlot::Vacant { .. }
                | VmSlot::Exhausted,
            ) => Err(Error::StaleIdentity),
            Some(
                VmSlot::Quiescing(_)
                | VmSlot::QuiescentHeld { .. }
                | VmSlot::RetiringHeld(_)
                | VmSlot::RetiredHeld { .. }
                | VmSlot::Destroying { .. },
            ) => Err(Error::StaleIdentity),
            None => Err(Error::NotInstalled),
        }
    }

    fn lease(&self, id: VmId) -> Result<VmLease, Error> {
        self.installed(id).map(|machine| VmLease {
            machine: machine.clone(),
        })
    }

    fn begin_quiesce(&mut self, id: VmId) -> Result<FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::Installed(machine) if machine.id() == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::Installed(machine) = old else {
            crate::hal::cpu::halt()
        };
        let lease = machine.clone();
        *entry = VmSlot::Quiescing(machine);
        Ok(lease)
    }

    fn try_hold_quiescent(&mut self, id: VmId) -> Result<bool, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        let ready = match entry {
            VmSlot::Quiescing(machine) if machine.id() == id => machine.is_quiescent(),
            VmSlot::Quiescing(_) => return Err(Error::StaleIdentity),
            _ => return Err(Error::Quiescing),
        };
        if !ready {
            return Ok(false);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::Quiescing(machine) = old else {
            crate::hal::cpu::halt()
        };
        match machine.try_into_unique() {
            Ok(owner) => {
                *entry = VmSlot::QuiescentHeld { owner };
                Ok(true)
            }
            Err(machine) => {
                *entry = VmSlot::Quiescing(machine);
                Ok(false)
            }
        }
    }

    fn begin_retirement(&mut self, id: VmId) -> Result<FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::QuiescentHeld { owner } if owner.id() == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::QuiescentHeld { owner } = old else {
            crate::hal::cpu::halt()
        };
        let machine = owner.into_shared();
        let operation = machine.clone();
        *entry = VmSlot::RetiringHeld(machine);
        Ok(operation)
    }

    fn promote_retired(&mut self, id: VmId) -> Result<(), Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::RetiringHeld(machine) if machine.id() == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::RetiringHeld(machine) = old else {
            crate::hal::cpu::halt()
        };
        match machine.try_into_unique() {
            Ok(owner) => {
                *entry = VmSlot::RetiredHeld { owner };
                Ok(())
            }
            Err(machine) => {
                *entry = VmSlot::RetiringHeld(machine);
                Err(Error::Quiescing)
            }
        }
    }

    fn begin_destroy(&mut self, id: VmId) -> Result<UniqueFallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::RetiredHeld { owner } if owner.id() == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(
            entry,
            VmSlot::Destroying {
                generation: id.generation,
            },
        );
        let VmSlot::RetiredHeld { owner } = old else {
            crate::hal::cpu::halt()
        };
        Ok(owner)
    }

    fn finish_destroy(&mut self, id: VmId) -> Result<(), Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::Destroying { generation } if *generation == id.generation) {
            return Err(Error::StaleIdentity);
        }
        *entry = match id.generation.checked_add(1) {
            Some(generation) => VmSlot::Vacant { generation },
            None => VmSlot::Exhausted,
        };
        Ok(())
    }
}

/// Scoped strong ownership returned by registry lookup.
///
/// This wrapper intentionally does not implement `Deref`; operations must use
/// a narrow method so registry internals cannot escape as untracked borrows.
struct VmLease {
    machine: FallibleArc<VirtualMachine>,
}

/// Runs a VM operation through a generation-qualified strong lease.
///
/// The registry lock is released before `operation`; the temporary binding
/// keeps every VM-owned device, endpoint, and interrupt model alive. Callers
/// must release their subsystem locks before invoking scheduler notification.
#[allow(dead_code)]
pub(super) fn with_binding<R>(
    id: VmId,
    operation: impl FnOnce(&VmBinding) -> R,
) -> Result<R, Error> {
    let lease = REGISTRY.with(|registry| registry.lease(id))?;
    let binding = VmBinding::new(id, lease.machine.clone());
    Ok(operation(&binding))
}

pub(crate) fn reserve() -> Result<VmReservation, Error> {
    let id = REGISTRY.with(VmRegistry::reserve)?;
    let hardware_vmid = match crate::kernel::mm::translation_id::reserve::<
        crate::kernel::mm::translation_id::Stage2Vmid,
    >(8)
    {
        Ok(identifier) => identifier,
        Err(_) => {
            REGISTRY.with(|registry| registry.cancel(id));
            return Err(Error::IdentityExhausted);
        }
    };
    Ok(VmReservation {
        id,
        hardware_vmid: Some(hardware_vmid),
        unpublished: true,
    })
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
