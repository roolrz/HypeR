// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{Claim, Error};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::authority::Rights;
use crate::kernel::irq::interrupt::{self, HandlerResult, Registration, VirtualInterrupt};
use crate::kernel::object::{
    KernelObject, KernelRef, ObjectKind, SignalMask, SignalSource, SignalState, TransferClass,
    VmDeviceBinding, private,
};
use crate::kernel::vm::registry::VmId;
use hyper::sync::InterruptSpinLock;
use hyper::vm::exit::{MmioAccess, MmioAction, MmioOperation};

type Lock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

fn charge<T: KernelObject>(domain: &ResourceDomain) -> Result<CommittedCharge, Error> {
    let bytes = crate::kernel::object::object_allocation_size::<T>().ok_or(Error::Resource)?;
    domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes as u64),
        )
        .map(|charge| charge.commit())
        .map_err(|_| Error::Resource)
}

pub(crate) struct DeviceAssignmentAuthority {
    _charge: CommittedCharge,
}
impl DeviceAssignmentAuthority {
    #[cfg_attr(feature = "kernel-self-test", allow(dead_code))]
    pub(crate) fn try_new(domain: &ResourceDomain) -> Result<Self, Error> {
        Ok(Self {
            _charge: charge::<Self>(domain)?,
        })
    }
}
impl private::Sealed for DeviceAssignmentAuthority {}
impl private::UserExportable for DeviceAssignmentAuthority {}
impl KernelObject for DeviceAssignmentAuthority {
    const KIND: ObjectKind = ObjectKind::DEVICE_ASSIGNMENT_AUTHORITY;
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::DUPLICATE)
        .union(Rights::INSPECT);
}

#[derive(Clone, Copy)]
struct Route {
    vm: VmId,
    irq: u32,
    negotiation: super::model::Negotiation,
    interrupt: super::model::LevelInterrupt,
}
enum State {
    Claimed,
    Attached,
    Active(Route),
    Retired,
    Quarantined,
}

pub(crate) struct PhysicalDevice {
    claim: Claim,
    state: Lock<State>,
    registration: Lock<Option<Registration>>,
    readable: SignalState,
    _charge: CommittedCharge,
}
impl PhysicalDevice {
    pub(crate) fn claim(index: usize, domain: &ResourceDomain) -> Result<Self, Error> {
        if !crate::hal::vm::supports_guest_device_assignment() {
            return Err(Error::Unsupported);
        }
        let charge = charge::<Self>(domain)?;
        let claim = super::super::platform_bus::claim(index).ok_or(Error::BadState)?;
        Ok(Self {
            claim,
            state: Lock::new(State::Claimed),
            registration: Lock::new(None),
            readable: SignalState::new(),
            _charge: charge,
        })
    }
    pub(crate) fn claim_matching(
        profile: u32,
        identity_kind: u32,
        identity: &str,
        domain: &ResourceDomain,
    ) -> Result<Self, super::service::MatchError> {
        let charge = charge::<Self>(domain).map_err(super::service::classify)?;
        let claim = super::super::platform_bus::claim_matching(profile, identity_kind, identity)?;
        Ok(Self {
            claim,
            state: Lock::new(State::Claimed),
            registration: Lock::new(None),
            readable: SignalState::new(),
            _charge: charge,
        })
    }
    pub(crate) fn claim_bundle(
        entries: &[(u32, u32, u64)],
        irq_node: u32,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        if !crate::hal::vm::supports_guest_device_assignment() {
            return Err(Error::Unsupported);
        }
        let charge = charge::<Self>(domain)?;
        let claim = super::super::platform_bus::claim_bundle(entries, irq_node)?;
        Ok(Self {
            claim,
            state: Lock::new(State::Claimed),
            registration: Lock::new(None),
            readable: SignalState::new(),
            _charge: charge,
        })
    }
    fn set_readable(&self, ready: bool) {
        if self
            .readable
            .update(
                SignalMask::from_trusted_bits(1),
                SignalMask::from_trusted_bits(u64::from(ready)),
            )
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!("physical IRQ signal sequence exhausted"));
        }
    }
    pub(crate) fn mmio(
        &self,
        offset: u64,
        width: u32,
        write: bool,
        value: u64,
    ) -> Result<u64, Error> {
        let offset = usize::try_from(offset).map_err(|_| Error::InvalidArgument)?;
        let width = width as usize;
        if !matches!(self.claim.hardware.profile, super::Profile::Userspace) {
            return Err(Error::Unsupported);
        }
        if !matches!(width, 1 | 2 | 4)
            || !offset.is_multiple_of(width)
            || (write && value > (u64::MAX >> (64 - width * 8)))
            || (!write && value != 0)
        {
            return Err(Error::InvalidArgument);
        }
        // Even a device read may have side effects. Access begins only after
        // the installed VM owns all DMA backing; CPU access and retirement are
        // serialized here. Userspace selects register semantics, never extents.
        let route = self.state.with(|state| match state {
            State::Active(route) => Ok(*route),
            _ => Err(Error::BadState),
        })?;
        // Activation prepares the IRQ before registry publication. An active
        // object alone is not yet permission to touch hardware: reject access
        // until the fully installed VM and its retained backing are visible.
        crate::kernel::vm::io::with_irq_binding(route.vm, |_| {
            self.state.with(|state| {
                if !matches!(state, State::Active(_)) {
                    return Err(Error::BadState);
                }
                if write {
                    self.claim
                        .hardware
                        .write_access(offset, width, value)
                        .then_some(0)
                        .ok_or(Error::InvalidArgument)
                } else {
                    self.claim
                        .hardware
                        .read_access(offset, width)
                        .ok_or(Error::InvalidArgument)
                }
            })
        })
        .map_err(|_| Error::BadState)?
    }
    pub(crate) fn irq_pending(&self) -> Result<u64, Error> {
        if !matches!(self.claim.hardware.profile, super::Profile::Userspace) {
            return Err(Error::Unsupported);
        }
        self.state.with(|state| match state {
            State::Active(route) => Ok(route.interrupt.pending()),
            _ => Err(Error::BadState),
        })
    }
    pub(crate) fn irq_complete(&self, sequence: u64, asserted: bool) -> Result<(), Error> {
        if !matches!(self.claim.hardware.profile, super::Profile::Userspace) {
            return Err(Error::Unsupported);
        }
        let route = self.state.with(|state| match state {
            State::Active(route) => Ok(*route),
            _ => Err(Error::BadState),
        })?;
        crate::kernel::vm::io::with_irq_binding(route.vm, |binding| {
            let result = self.state.with(|state| {
                let State::Active(current) = state else {
                    return Err(Error::BadState);
                };
                if current.interrupt.pending() != sequence {
                    return Err(Error::Busy);
                }
                crate::kernel::vm::io::set_line(binding, route.irq, asserted);
                current
                    .interrupt
                    .complete(sequence, asserted)
                    .map_err(|_| Error::Busy)?;
                // Acknowledging observation is distinct from hardware rearm.
                // A still-asserted level keeps its token but is no longer a
                // continuously-ready userspace event.
                self.set_readable(current.interrupt.readable());
                Ok(())
            });
            binding.publish_changed_interrupts();
            result
        })
        .map_err(|_| Error::BadState)??;
        if !asserted {
            self.registration.with(|registration| {
                let registration = registration.as_ref().ok_or(Error::BadState)?;
                // IRQ dispatch applies its mask before releasing the registry
                // lock. Rearm takes that same lock, then rechecks the token:
                // neither a late mask nor a newer pending IRQ can be lost.
                interrupt::enable_registered_shared_if(registration, || {
                    self.state
                        .with(|state| matches!(state, State::Active(route) if route.interrupt.can_rearm()))
                })
                .map_err(|_| Error::Interrupt)
            })?;
        }
        Ok(())
    }

    pub(crate) fn info(&self) -> super::service::Info {
        super::service::Info {
            device_id: if matches!(self.claim.hardware.profile, super::Profile::Virtio) {
                8
            } else {
                0
            },
            transport_version: if matches!(self.claim.hardware.profile, super::Profile::Virtio) {
                2
            } else {
                0
            },
            mmio_size: self.claim.hardware.mapping.resource().size(),
        }
    }
    pub(crate) fn profile_info(&self) -> [u8; 32] {
        let count = 1 + self.claim.hardware.extra.iter().flatten().count() as u32;
        let mut output = [0; 32];
        output[0..4].copy_from_slice(&self.claim.hardware.profile.id().to_le_bytes());
        output[8..12].copy_from_slice(&count.to_le_bytes());
        output
    }
    pub(crate) fn resource_info(&self, index: u32) -> Result<[u8; 32], Error> {
        let window = if index == 0 {
            super::Window {
                mapping: self.claim.hardware.mapping,
                offset: 0,
            }
        } else {
            self.claim
                .hardware
                .extra
                .get(index as usize - 1)
                .copied()
                .flatten()
                .ok_or(Error::InvalidArgument)?
        };
        let mut output = [0; 32];
        output[0..4].copy_from_slice(&(index + 1).to_le_bytes());
        output[8..16].copy_from_slice(&(window.offset as u64).to_le_bytes());
        output[16..24].copy_from_slice(&window.mapping.resource().size().to_le_bytes());
        Ok(output)
    }
    fn attach(&self) -> Result<(), Error> {
        self.state.with(|state| match state {
            State::Claimed => {
                *state = State::Attached;
                Ok(())
            }
            _ => Err(Error::BadState),
        })
    }
    pub(crate) fn activate_for(&self, vm: VmId, irq: u32) -> Result<(), Error> {
        let route = Route {
            vm,
            irq,
            negotiation: super::model::Negotiation::new(),
            interrupt: super::model::LevelInterrupt::new(),
        };
        self.state.with(|state| {
            if matches!(state, State::Attached) {
                Ok(())
            } else {
                Err(Error::BadState)
            }
        })?;
        let hw = self.claim.hardware;
        let prepared = hw
            .domain
            .prepare_shared_mapping(
                hw.interrupt,
                hyper::hal::interrupt::InterruptPriority::Normal,
                hw.trigger,
                core::ptr::from_ref(self).expose_provenance(),
                interrupt_handler,
            )
            .map_err(|_| Error::Interrupt)?;
        self.state.with(|state| *state = State::Active(route));
        let registration = match interrupt::activate(prepared) {
            Ok(registration) => registration,
            Err(failure) => {
                let (_, prepared) = failure.into_parts();
                self.state.with(|state| *state = State::Attached);
                if let Err(failure) = interrupt::discard_prepared(prepared) {
                    let _retained = core::mem::ManuallyDrop::new(failure);
                    crate::kernel::crash::fatal(format_args!(
                        "assigned-device IRQ activation rollback failed"
                    ));
                }
                return Err(Error::Interrupt);
            }
        };
        self.registration.with(|slot| *slot = Some(registration));
        Ok(())
    }
    pub(crate) fn quiesce(&self) -> Result<(), Error> {
        let result = self.state.with(|state| {
            match state {
                State::Retired => return Ok(()),
                State::Quarantined => return Err(Error::Quarantined),
                State::Claimed | State::Attached => {
                    *state = State::Retired;
                    return Ok(());
                }
                State::Active(_) => {}
            }
            // All owning VM vCPUs have detached before this entry. A modern
            // virtio reset acknowledges only once the device has stopped DMA.
            // Never free backing merely because a poll budget elapsed.
            if matches!(self.claim.hardware.profile, super::Profile::Userspace) {
                *state = State::Quarantined;
                return Err(Error::Quarantined);
            }
            self.claim.hardware.write(0x70, 0);
            for _ in 0..1024 {
                if self.claim.hardware.read(0x70) == 0 {
                    *state = State::Retired;
                    return Ok(());
                }
                core::hint::spin_loop();
            }
            *state = State::Quarantined;
            Err(Error::Quarantined)
        });
        result?;
        // IRQ callbacks take state, so unregister must run outside that lock.
        if let Some(registration) = self.registration.with(Option::take) {
            let irq = registration.interrupt();
            if let Err(failure) = interrupt::unregister(registration) {
                let (_, registration) = failure.into_parts();
                self.registration.with(|slot| *slot = Some(registration));
                self.state.with(|state| *state = State::Quarantined);
                return Err(Error::Quarantined);
            }
            if interrupt::unmap(irq).is_err() {
                self.state.with(|state| *state = State::Quarantined);
                return Err(Error::Quarantined);
            }
        }
        Ok(())
    }
    fn irq_write(&self, offset: usize) -> bool {
        match self.claim.hardware.profile {
            super::Profile::Virtio => offset == 0x64 || offset == 0x70,
            super::Profile::Userspace => false,
        }
    }
    pub(crate) fn access_at(&self, offset: usize, access: MmioAccess) -> MmioAction {
        if !matches!(access.size(), 1 | 2 | 4)
            || (matches!(self.claim.hardware.profile, super::Profile::Virtio)
                && offset < 0x100
                && access.size() != 4)
            || !offset.is_multiple_of(access.size())
            || self
                .claim
                .hardware
                .register(offset, access.size())
                .is_none()
        {
            return MmioAction::Stop;
        }
        let route = self.state.with(|state| match state {
            State::Active(route) => Some(*route),
            _ => None,
        });
        let Some(route) = route else {
            return MmioAction::Stop;
        };
        let action = crate::kernel::vm::io::with_irq_binding(route.vm, |binding| {
            let action = self.state.with(|state| {
                let State::Active(active) = state else {
                    return MmioAction::Stop;
                };
                match access.operation() {
                    MmioOperation::Read => self
                        .claim
                        .hardware
                        .read_access(offset, access.size())
                        .map(MmioAction::CompleteRead)
                        .unwrap_or(MmioAction::Stop),
                    MmioOperation::Write(value) => {
                        if matches!(self.claim.hardware.profile, super::Profile::Virtio)
                            && !active.negotiation.write(offset, value as u32)
                        {
                            return MmioAction::Stop;
                        }
                        if !self
                            .claim
                            .hardware
                            .write_access(offset, access.size(), value)
                        {
                            return MmioAction::Stop;
                        }
                        if self.irq_write(offset) {
                            crate::kernel::vm::io::set_line(
                                binding,
                                route.irq,
                                self.claim.hardware.line_asserted(),
                            );
                        }
                        MmioAction::CompleteWrite
                    }
                }
            });
            binding.publish_changed_interrupts();
            action
        })
        .unwrap_or(MmioAction::Stop);
        if matches!(access.operation(), MmioOperation::Write(_))
            && self.irq_write(offset)
            && matches!(
                self.claim.hardware.trigger,
                hyper::hal::interrupt::InterruptTrigger::Level
            )
        {
            let enabled = self.registration.with(|registration| {
                registration.as_ref().is_some_and(|registration| {
                    interrupt::enable_registered_shared(registration).is_ok()
                })
            });
            if !enabled {
                return MmioAction::Stop;
            }
        }
        action
    }
}

fn interrupt_handler(_: VirtualInterrupt, context: usize) -> HandlerResult {
    // SAFETY: Assignment owns a stable KernelRef until synchronized unregister;
    // the callback never takes registration or mutates the IRQ registry.
    let object = unsafe { &*core::ptr::with_exposed_provenance::<PhysicalDevice>(context) };
    if matches!(object.claim.hardware.profile, super::Profile::Userspace) {
        object.state.with(|state| {
            if let State::Active(route) = state {
                if route.interrupt.deliver().is_err() {
                    crate::kernel::crash::fatal(format_args!("physical IRQ token exhausted"));
                }
                object.set_readable(route.interrupt.readable());
            }
        });
        // The IRQ registry lock spans this callback and the hardware mask.
        // The only rearm API acquires that lock after validating registration.
        return HandlerResult::HandledAndMaskLocal;
    }
    let route = object.state.with(|state| match state {
        State::Active(route) => Some(*route),
        _ => None,
    });
    let mask = if let Some(route) = route {
        crate::kernel::vm::io::with_irq_binding(route.vm, |binding| {
            let mask = object.state.with(|state| {
                let active = matches!(state, State::Active(_));
                let status = u32::from(object.claim.hardware.line_asserted());
                if active {
                    crate::kernel::vm::io::set_line(binding, route.irq, status != 0);
                }
                super::model::mask_interrupt(active, status, object.claim.hardware.trigger)
            });
            binding.publish_changed_interrupts();
            mask
        })
        .unwrap_or_else(|_| {
            // Activation enables the source before the VM is published. A
            // latched edge from its previous owner may arrive in that window;
            // failed lookup is not proof that this assignment is stopped.
            // Masking it would strand future edges without any guest ACK to
            // rearm them. Re-read assignment state under the same lock used by
            // reset, and preserve the ordinary trigger/status decision.
            // Before publication the admitted modern transport is still reset
            // (no guest DRIVER_OK); its cleared status also keeps a stale
            // controller-latched level IRQ enabled.
            object.state.with(|state| {
                super::model::mask_interrupt(
                    matches!(state, State::Active(_)),
                    u32::from(object.claim.hardware.line_asserted()),
                    object.claim.hardware.trigger,
                )
            })
        })
    } else {
        true
    };
    if mask {
        HandlerResult::HandledAndMaskLocal
    } else {
        HandlerResult::Handled
    }
}

impl private::Sealed for PhysicalDevice {}
impl private::UserExportable for PhysicalDevice {}
impl KernelObject for PhysicalDevice {
    const KIND: ObjectKind = ObjectKind::PHYSICAL_DEVICE;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::DUPLICATE)
        .union(Rights::INSPECT)
        .union(Rights::WRITE)
        .union(Rights::WAIT);
    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(SignalSource::new(
            &self.readable,
            SignalMask::from_trusted_bits(1),
        ))
    }
}

pub(crate) struct Assignment {
    object: KernelRef<PhysicalDevice, VmDeviceBinding>,
    base: u64,
    irq: u32,
}
impl Assignment {
    pub(crate) fn new(
        object: KernelRef<PhysicalDevice, VmDeviceBinding>,
        base: u64,
        irq: u32,
    ) -> Result<Self, Error> {
        if !crate::hal::vm::supports_guest_device_assignment() {
            return Err(Error::Unsupported);
        }
        if !super::model::assignment_aperture(base) || !(40..64).contains(&irq) {
            return Err(Error::InvalidArgument);
        }
        object.object().attach()?;
        Ok(Self { object, base, irq })
    }
    pub(crate) fn object(&self) -> KernelRef<PhysicalDevice, VmDeviceBinding> {
        self.object.clone()
    }
    pub(crate) const fn irq(&self) -> u32 {
        self.irq
    }
    /// Only the exact aperture owned by this userspace assignment can be
    /// delegated to an installed Native MMIO handler.
    pub(crate) fn owns_userspace_aperture(&self, base: u64, length: u64) -> bool {
        super::model::owns_userspace_aperture(
            matches!(
                self.object.object().claim.hardware.profile,
                super::Profile::Userspace
            ),
            self.base,
            base,
            length,
        )
    }
    pub(crate) fn offset(&self, access: MmioAccess) -> Option<usize> {
        if matches!(
            self.object.object().claim.hardware.profile,
            super::Profile::Userspace
        ) {
            return None;
        }
        let offset = access.address().get().checked_sub(self.base)?;
        if offset >= 65536 {
            return None;
        }
        Some(offset as usize)
    }
}
impl Drop for Assignment {
    fn drop(&mut self) {
        self.object.object().state.with(|state| match state {
            State::Attached => *state = State::Claimed,
            State::Claimed | State::Retired => {}
            State::Active(_) | State::Quarantined => crate::kernel::crash::fatal(format_args!(
                "active physical assignment dropped without quiescence"
            )),
        });
    }
}
