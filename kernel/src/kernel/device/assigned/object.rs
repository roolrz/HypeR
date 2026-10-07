// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{Claim, Error};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::authority::Rights;
use crate::kernel::irq::interrupt::{self, Registration};
use crate::kernel::object::{
    KernelObject, ObjectKind, SignalMask, SignalSource, SignalState, TransferClass, private,
};
use crate::kernel::vm::registry::VmId;
use hyper::drivers::pci::{FunctionState, MAX_VECTORS};
use hyper::sync::InterruptSpinLock;

mod access;
mod assignment;
mod interrupts;
mod metadata;
pub(crate) use assignment::Assignment;

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
    registrations: Lock<[Option<Registration>; MAX_VECTORS]>,
    contexts: [interrupts::InterruptContext; MAX_VECTORS],
    pci: Lock<Option<FunctionState>>,
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
            registrations: Lock::new([const { None }; MAX_VECTORS]),
            contexts: core::array::from_fn(interrupts::InterruptContext::new),
            pci: Lock::new(None),
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
            registrations: Lock::new([const { None }; MAX_VECTORS]),
            contexts: core::array::from_fn(interrupts::InterruptContext::new),
            pci: Lock::new(None),
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
            registrations: Lock::new([const { None }; MAX_VECTORS]),
            contexts: core::array::from_fn(interrupts::InterruptContext::new),
            pci: Lock::new(None),
            readable: SignalState::new(),
            _charge: charge,
        })
    }
    fn guest_interrupt_count(&self) -> u32 {
        self.claim.hardware.interrupt_count.max(1)
    }
    fn attach(&self, base: u64, irq: u32) -> Result<(), Error> {
        self.state.with(|state| match state {
            State::Claimed => {
                if let super::Profile::Pci(transport) = &self.claim.hardware.profile {
                    self.pci
                        .with(|slot| *slot = Some(transport.state(base, irq)));
                }
                *state = State::Attached;
                Ok(())
            }
            _ => Err(Error::BadState),
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
            self.registrations.with(|registrations| {
                let registration = registrations[0].as_ref().ok_or(Error::BadState)?;
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
}

impl private::Sealed for PhysicalDevice {}
impl private::UserExportable for PhysicalDevice {}
impl KernelObject for PhysicalDevice {
    fn diagnostic_details(
        &self,
        cursor: u64,
    ) -> Result<
        crate::kernel::object::diagnostics::Details,
        crate::kernel::object::diagnostics::DetailError,
    > {
        self.diagnostic(cursor)
    }

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
