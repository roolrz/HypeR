// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{Claim, Error};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::authority::Rights;
use crate::kernel::irq::interrupt::{self, HandlerResult, Registration, VirtualInterrupt};
use crate::kernel::object::{
    KernelObject, KernelRef, ObjectKind, TransferClass, VmDeviceBinding, private,
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
            _charge: charge,
        })
    }
    pub(crate) fn info(&self) -> super::service::Info {
        super::service::Info {
            device_id: 8,
            transport_version: 2,
            mmio_size: self.claim.hardware.mapping.resource().size(),
        }
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
    pub(crate) fn access_at(&self, offset: usize, access: MmioAccess) -> MmioAction {
        if !matches!(access.size(), 1 | 2 | 4)
            || (offset < 0x100 && access.size() != 4)
            || !offset.is_multiple_of(access.size())
            || offset + access.size() > self.claim.hardware.mapping.resource().size() as usize
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
                    MmioOperation::Read => MmioAction::CompleteRead(
                        self.claim.hardware.read_access(offset, access.size()),
                    ),
                    MmioOperation::Write(value) => {
                        if !active.negotiation.write(offset, value as u32) {
                            return MmioAction::Stop;
                        }
                        self.claim
                            .hardware
                            .write_access(offset, access.size(), value);
                        if offset == 0x64 || offset == 0x70 {
                            crate::kernel::vm::io::set_line(
                                binding,
                                route.irq,
                                self.claim.hardware.read(0x60) != 0,
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
            && (offset == 0x64 || offset == 0x70)
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
    let route = object.state.with(|state| match state {
        State::Active(route) => Some(*route),
        _ => None,
    });
    let mask = if let Some(route) = route {
        crate::kernel::vm::io::with_irq_binding(route.vm, |binding| {
            let mask = object.state.with(|state| {
                let active = matches!(state, State::Active(_));
                let status = object.claim.hardware.read(0x60);
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
                    object.claim.hardware.read(0x60),
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
        .union(Rights::WRITE);
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
        if !base.is_multiple_of(4096)
            || !(0x0b00_0000..0x0c00_0000).contains(&base)
            || !(40..64).contains(&irq)
        {
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
    pub(crate) fn offset(&self, access: MmioAccess) -> Option<usize> {
        let offset = access.address().get().checked_sub(self.base)?;
        if offset >= 4096 {
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
