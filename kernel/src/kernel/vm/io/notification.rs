// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Direct queue kicks and completion IRQs, independent of userspace scheduling.

use super::super::{installed::InstalledMachine, registry::VmId};
use super::{Error, Route, model};
use crate::kernel::accounting::{CommittedCharge, ResourceDomain};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectKind, ObjectRetirement, SignalMask, SignalSource, SignalState,
    TransferClass, private,
};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static NEXT_ROUTE: AtomicU64 = AtomicU64::new(1);
fn new_route_id() -> Result<u64, Error> {
    NEXT_ROUTE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| Error::ResourceLimit)
}
use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;
use hyper::vm::exit::{MmioAccess, MmioAction, MmioOperation};

pub(crate) struct Shared {
    pub(super) route_id: u64,
    front: Option<VmId>,
    back: VmId,
    pub(super) front_base: u64,
    pub(super) back_base: u64,
    pub(super) front_irq: u32,
    pub(super) back_irq: u32,
    state: InterruptSpinLock<model::NotificationState, crate::hal::irq::LocalMask>,
    admission:
        InterruptSpinLock<super::super::memory::admission::Reply, crate::hal::irq::LocalMask>,
    installed: AtomicBool,
    detached: AtomicBool,
    signals: SignalState,
    _charge: CommittedCharge,
}
impl Shared {
    fn mutate<R>(&self, operation: impl FnOnce(&mut model::NotificationState) -> R) -> R {
        // Either peer can leave the registry independently. Keep the surviving
        // MMIO endpoint operational and let Native policy report device failure.
        let front = self
            .front
            .and_then(|id| super::super::registry::acquire_binding(id).ok());
        let back = super::super::registry::acquire_binding(self.back).ok();
        let value = self.state.with(|state| {
            if self.detached.load(Ordering::Acquire) {
                return operation(state);
            }
            if (self.front.is_some() && front.is_none()) || back.is_none() {
                state.close();
            }
            let value = operation(state);
            if let Some(front) = &front {
                super::set_line(front, self.front_irq, state.front_irq());
            }
            if let Some(back) = &back {
                super::set_line(back, self.back_irq, state.back_irq());
            }
            let signals = u64::from(state.closed)
                | if self.front.is_none() {
                    u64::from(state.status) << 1
                } else {
                    0
                };
            if self
                .signals
                .update(
                    SignalMask::from_trusted_bits(7),
                    SignalMask::from_trusted_bits(signals),
                )
                .is_err()
            {
                crate::hal::cpu::halt();
            }
            value
        });
        if let Some(front) = front {
            front.publish_changed_interrupts();
        }
        if let Some(back) = back {
            back.publish_changed_interrupts();
        }
        value
    }
    pub(super) fn close(&self) {
        if !self.installed.load(Ordering::Acquire) {
            self.state.with(model::NotificationState::close);
            return;
        }
        self.mutate(model::NotificationState::close);
    }
    pub(super) fn front_mmio(&self, access: MmioAccess) -> Option<MmioAction> {
        let offset = access.address().get() - self.front_base;
        if !matches!(offset, 0x50 | 0x60 | 0x64) {
            return None;
        }
        if access.size() != 4 {
            return Some(MmioAction::Stop);
        }
        Some(self.mutate(|state| match (offset, access.operation()) {
            (0x50, MmioOperation::Write(queue)) => {
                state.kick(queue);
                MmioAction::CompleteWrite
            }
            (0x60, MmioOperation::Read) => MmioAction::CompleteRead(state.status as u64),
            (0x64, MmioOperation::Write(mask)) => {
                state.ack(mask as u32);
                MmioAction::CompleteWrite
            }
            _ => MmioAction::Stop,
        }))
    }
    pub(super) fn back_mmio(&self, access: MmioAccess) -> MmioAction {
        let offset = access.address().get() - self.back_base;
        if (0x20..=0x70).contains(&offset) {
            if access.size() != 8 {
                return MmioAction::Stop;
            }
            return self.admission.with(|reply| {
                if self.detached.load(Ordering::Acquire) {
                    return MmioAction::Stop;
                }
                match (offset, access.operation()) {
                    (0x20, MmioOperation::Write(token)) => {
                        reply.quiesce(self.back, self.route_id, token);
                        MmioAction::CompleteWrite
                    }
                    (0x28, MmioOperation::Write(token)) => {
                        reply.admit(self.back, self.route_id, token);
                        MmioAction::CompleteWrite
                    }
                    (0x50, MmioOperation::Write(index)) => {
                        reply.select(self.back, self.route_id, index);
                        MmioAction::CompleteWrite
                    }
                    (offset, MmioOperation::Read) => reply
                        .read(offset)
                        .map_or(MmioAction::Stop, MmioAction::CompleteRead),
                    _ => MmioAction::Stop,
                }
            });
        }

        self.mutate(|state| match (offset, access.size(), access.operation()) {
            (0x00, 4, MmioOperation::Read) => MmioAction::CompleteRead(0x4859_4e42),
            (0x04, 4, MmioOperation::Read) => MmioAction::CompleteRead(1),
            (0x08, 4, MmioOperation::Read) => MmioAction::CompleteRead(state.epoch as u64),
            (0x0c, 4, MmioOperation::Read) => MmioAction::CompleteRead(u64::from(state.enabled)),
            (0x10, 4, MmioOperation::Read) => MmioAction::CompleteRead(state.take_kicks() as u64),
            (0x18, 8, MmioOperation::Write(value)) => {
                state.call(value);
                MmioAction::CompleteWrite
            }
            _ => MmioAction::Stop,
        })
    }
}

#[derive(Clone)]
pub(crate) struct Notification {
    shared: FallibleArc<Shared>,
}
impl Notification {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        front: &FallibleArc<InstalledMachine>,
        back: &FallibleArc<InstalledMachine>,
        front_base: u64,
        back_base: u64,
        front_irq: u32,
        back_irq: u32,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        if !super::valid_location(front_base, front_irq)
            || !super::valid_location(back_base, back_irq)
        {
            return Err(Error::InvalidArgument);
        }
        let front = front.io_install_id()?;
        let back = back.io_update_id()?;
        if front == back {
            return Err(Error::InvalidArgument);
        }
        let charge = super::charge::<Self, Shared>(domain)?;
        let shared = FallibleArc::try_new(Shared {
            route_id: new_route_id()?,
            front: Some(front),
            back,
            front_base,
            back_base,
            front_irq,
            back_irq,
            state: InterruptSpinLock::new(model::NotificationState::new()),
            admission: InterruptSpinLock::new(super::super::memory::admission::Reply::new()),
            installed: AtomicBool::new(false),
            detached: AtomicBool::new(false),
            signals: SignalState::new(),
            _charge: charge,
        })
        .map_err(|_| Error::NoMemory)?;
        Ok(Self { shared })
    }
    pub(crate) fn install(
        &self,
        front: &FallibleArc<InstalledMachine>,
        back: &FallibleArc<InstalledMachine>,
    ) -> Result<(), Error> {
        let commit = || {
            let front_route = Route::NotificationFront(self.shared.clone());
            let back_route = Route::NotificationBack(self.shared.clone());
            front.validate_io_route(&front_route)?;
            back.validate_io_route(&back_route)?;
            super::with_irq_binding(self.shared.front.ok_or(Error::BadState)?, |front_binding| {
                super::with_irq_binding(self.shared.back, |back_binding| {
                    front_binding.preflight_io_route(&front_route)?;
                    back_binding.preflight_io_route(&back_route)?;
                    // Both VM lifecycle locks remain held in identity order.
                    // All fallible validation preceded this two-owner commit.
                    if front_binding.install_io_route(front_route).is_err()
                        || back_binding.install_io_route(back_route).is_err()
                    {
                        crate::kernel::crash::fatal(format_args!(
                            "guest notification reservation lost during commit"
                        ));
                    }
                    self.shared.installed.store(true, Ordering::Release);
                    Ok(())
                })?
            })?
        };
        if self.shared.front.ok_or(Error::BadState)? < self.shared.back {
            front.with_io_install(|id| {
                if Some(id) != self.shared.front {
                    return Err(Error::BadState);
                }
                back.with_io_update(|id| {
                    if id != self.shared.back {
                        return Err(Error::BadState);
                    }
                    commit()
                })
            })
        } else {
            back.with_io_update(|id| {
                if id != self.shared.back {
                    return Err(Error::BadState);
                }
                front.with_io_install(|id| {
                    if Some(id) != self.shared.front {
                        return Err(Error::BadState);
                    }
                    commit()
                })
            })
        }
    }
    /// A Native client uses the same backend register contract, with durable
    /// scheduler signals replacing the frontend guest interrupt line.
    pub(crate) fn prepare_native(
        back: &FallibleArc<InstalledMachine>,
        back_base: u64,
        back_irq: u32,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        if !super::valid_location(back_base, back_irq) {
            return Err(Error::InvalidArgument);
        }
        let shared = FallibleArc::try_new(Shared {
            route_id: new_route_id()?,
            front: None,
            back: back.io_install_id()?,
            front_base: 0,
            back_base,
            front_irq: 0,
            back_irq,
            state: InterruptSpinLock::new(model::NotificationState::new()),
            admission: InterruptSpinLock::new(super::super::memory::admission::Reply::new()),
            installed: AtomicBool::new(false),
            detached: AtomicBool::new(false),
            signals: SignalState::new(),
            _charge: super::charge::<Self, Shared>(domain)?,
        })
        .map_err(|_| Error::NoMemory)?;
        Ok(Self { shared })
    }

    pub(crate) fn install_native(&self, back: &FallibleArc<InstalledMachine>) -> Result<(), Error> {
        if self.shared.front.is_some() {
            return Err(Error::BadState);
        }
        back.with_io_install(|id| {
            if id != self.shared.back {
                return Err(Error::BadState);
            }
            let route = Route::NotificationBack(self.shared.clone());
            back.validate_io_route(&route)?;
            super::with_irq_binding(id, |binding| {
                binding.preflight_io_route(&route)?;
                binding.install_io_route(route)?;
                self.shared.installed.store(true, Ordering::Release);
                Ok(())
            })?
        })
    }

    pub(crate) fn kick_native_mask(&self, queues: u32) -> Result<(), Error> {
        if queues == 0 || queues & !((1 << model::QUEUE_COUNT) - 1) != 0 {
            return Err(Error::BadState);
        }
        self.shared.mutate(|state| {
            if state.closed || !state.enabled {
                return Err(Error::BadState);
            }
            for queue in 0..model::QUEUE_COUNT {
                if queues & (1 << queue) != 0 {
                    state.kick(queue);
                }
            }
            Ok(())
        })
    }

    pub(crate) fn acknowledge_native(&self) {
        self.shared.mutate(|state| state.ack(1));
    }

    pub(crate) fn native_signal_source(&self) -> SignalSource<'_> {
        SignalSource::new(&self.shared.signals, SignalMask::from_trusted_bits(7))
    }

    pub(crate) fn closed_signal_source(&self) -> SignalSource<'_> {
        SignalSource::new(&self.shared.signals, SignalMask::from_trusted_bits(1))
    }

    pub(crate) fn close_native(&self) {
        self.shared.close();
    }

    fn disconnect(&self) -> Result<u32, Error> {
        let front = self
            .shared
            .front
            .and_then(|id| super::super::registry::acquire_binding(id).ok());
        let back = super::super::registry::acquire_binding(self.shared.back).ok();
        // Admission is serialized with permanent withdrawal. Once detached,
        // even an already cloned old route can never affect a new binding.
        let detached = self.shared.admission.with(|_| {
            if self.shared.detached.load(Ordering::Acquire) {
                return Err(Error::BadState);
            }
            if back.as_ref().is_some_and(|binding| {
                binding.with_address_space(|space| {
                    space
                        .live
                        .slots
                        .iter()
                        .flatten()
                        .any(|record| record.state.owns(self.shared.route_id))
                })
            }) {
                return Err(Error::Busy);
            }
            self.shared.state.with(|state| {
                state.close();
                if self
                    .shared
                    .signals
                    .update(
                        SignalMask::from_trusted_bits(7),
                        SignalMask::from_trusted_bits(1),
                    )
                    .is_err()
                {
                    crate::hal::cpu::halt();
                }
                if let Some(front) = &front {
                    super::set_line(front, self.shared.front_irq, false);
                }
                if let Some(back) = &back {
                    super::set_line(back, self.shared.back_irq, false);
                }
                self.shared.detached.store(true, Ordering::Release);
                let removed_front = front
                    .as_ref()
                    .and_then(|binding| binding.remove_io_notification(self.shared.route_id));
                let removed_back = back
                    .as_ref()
                    .and_then(|binding| binding.remove_io_notification(self.shared.route_id));
                Ok((state.epoch, removed_front, removed_back))
            })
        })?;
        if let Some(front) = front {
            front.publish_changed_interrupts();
        }
        if let Some(back) = back {
            back.publish_changed_interrupts();
        }
        let (epoch, front_route, back_route) = detached;
        drop(front_route);
        drop(back_route);
        Ok(epoch)
    }

    pub(crate) fn control(&self, operation: u32) -> Result<u32, Error> {
        if operation == hyper::abi::native::HYPER_NATIVE_GUEST_NOTIFICATION_DISCONNECT as u32 {
            return self.disconnect();
        }
        self.shared
            .mutate(|state| state.control(operation))
            .map_err(Into::into)
    }
}
impl private::Sealed for Notification {}
impl private::UserExportable for Notification {}
impl KernelObject for Notification {
    const KIND: ObjectKind = ObjectKind::GUEST_NOTIFICATION;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::WRITE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::WAIT);
    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(SignalSource::new(
            &self.shared.signals,
            SignalMask::from_trusted_bits(1),
        ))
    }
    fn on_zero_active_handles(&self, _: &mut ObjectRetirement) {
        self.shared.close();
    }
}
