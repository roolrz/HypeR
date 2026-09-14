// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! VM-owned control mailboxes and prevalidated cross-VM notification routes.

mod mailbox;
mod model;
mod notification;
pub(crate) mod service;
pub(crate) use mailbox::Mailbox;
pub(crate) use notification::Notification;

use super::registry::{VmBinding, VmId};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::object::{KernelObject, object_allocation_size};
use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;
use hyper::vm::exit::{MmioAccess, MmioAction};

pub(crate) use super::service::Error;

impl From<model::Error> for Error {
    fn from(value: model::Error) -> Self {
        match value {
            model::Error::Invalid => Self::InvalidArgument,
            model::Error::Busy => Self::WouldBlock,
            model::Error::Closed => Self::BadState,
            model::Error::Exhausted => Self::ResourceLimit,
        }
    }
}

fn charge<T: KernelObject, S>(domain: &ResourceDomain) -> Result<CommittedCharge, Error> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| value.checked_add(FallibleArc::<S>::allocation_size()))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(Error::NoMemory)?;
    domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelMemoryBytes, bytes)
                .with(ResourceKind::KernelObjects, 1),
        )
        .map(|value| value.commit())
        .map_err(|_| Error::ResourceLimit)
}

pub(super) fn valid_location(base: u64, irq: u32) -> bool {
    crate::hal::vm::supports_io_notifications()
        && (0x0a00_0000..0x0b00_0000).contains(&base)
        && base.is_multiple_of(4096)
        && (40..64).contains(&irq)
}

/// Mutation-only saved-model update. The caller serializes its source level
/// and calls `publish_changed_interrupts` after releasing the source lock.
pub(in crate::kernel) fn set_line(binding: &VmBinding, irq: u32, asserted: bool) {
    if crate::hal::vm::update_saved_device_line(binding.interrupts(), irq, asserted).is_err() {
        crate::kernel::crash::fatal(format_args!(
            "validated guest notification IRQ became invalid"
        ));
    }
}

/// Temporary generation-qualified binding; no route owns a registry VM lease.
pub(in crate::kernel) fn with_irq_binding<R>(
    id: VmId,
    operation: impl FnOnce(&VmBinding) -> R,
) -> Result<R, Error> {
    super::registry::with_binding(id, operation).map_err(|_| Error::BadState)
}

#[derive(Clone)]
pub(crate) enum Route {
    Mailbox(FallibleArc<mailbox::Shared>),
    NotificationFront(FallibleArc<notification::Shared>),
    NotificationBack(FallibleArc<notification::Shared>),
}
impl Route {
    pub(super) fn base(&self) -> u64 {
        match self {
            Self::Mailbox(value) => value.base,
            Self::NotificationFront(value) => value.front_base,
            Self::NotificationBack(value) => value.back_base,
        }
    }
    pub(crate) fn irq(&self) -> u32 {
        match self {
            Self::Mailbox(value) => value.irq,
            Self::NotificationFront(value) => value.front_irq,
            Self::NotificationBack(value) => value.back_irq,
        }
    }
    pub(super) fn deferred_overlay(&self) -> bool {
        matches!(self, Self::NotificationFront(_))
    }
    fn route(&self, access: MmioAccess) -> Option<MmioAction> {
        match self {
            Self::Mailbox(value) => Some(value.mmio(access)),
            Self::NotificationFront(value) => value.front_mmio(access),
            Self::NotificationBack(value) => Some(value.back_mmio(access)),
        }
    }
    fn close(&self) {
        match self {
            Self::Mailbox(value) => value.close(),
            Self::NotificationFront(value) | Self::NotificationBack(value) => value.close(),
        }
    }
}

pub(crate) struct Routes {
    entries: InterruptSpinLock<
        [Option<Route>; hyper::abi::native::HYPER_NATIVE_IO_MAX_CLIENTS as usize * 2],
        crate::hal::irq::LocalMask,
    >,
}
impl Routes {
    pub(crate) fn new() -> Self {
        Self {
            entries: InterruptSpinLock::new(core::array::from_fn(|_| None)),
        }
    }
    pub(crate) fn can_insert(&self, route: &Route) -> Result<(), Error> {
        self.entries.with(|entries| {
            if entries
                .iter()
                .flatten()
                .any(|other| other.base() == route.base() || other.irq() == route.irq())
                || entries.iter().all(Option::is_some)
            {
                Err(Error::Busy)
            } else {
                Ok(())
            }
        })
    }
    /// VM lifecycle admission holds across preflight and this infallible commit.
    pub(crate) fn insert(&self, route: Route) -> Result<(), Error> {
        self.entries.with(|entries| {
            if entries
                .iter()
                .flatten()
                .any(|other| other.base() == route.base() || other.irq() == route.irq())
            {
                return Err(Error::Busy);
            }
            let slot = entries
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(Error::Busy)?;
            *slot = Some(route);
            Ok(())
        })
    }
    pub(crate) fn remove_notification(&self, route_id: u64) -> Option<Route> {
        self.entries.with(|entries| {
            entries
                .iter_mut()
                .find(|entry| match entry {
                    Some(Route::NotificationFront(shared) | Route::NotificationBack(shared)) => {
                        shared.route_id == route_id
                    }
                    _ => false,
                })
                .and_then(Option::take)
        })
    }

    pub(crate) fn conflicts(&self, base: u64, length: u64) -> bool {
        self.entries.with(|entries| {
            entries.iter().flatten().any(|route| {
                base < route.base() + 4096
                    && route.base() < base.saturating_add(length)
                    && !(route.deferred_overlay() && base == route.base() && length == 4096)
            })
        })
    }
    pub(crate) fn route(&self, access: MmioAccess) -> Option<MmioAction> {
        let address = access.address().get();
        let route = self.entries.with(|entries| {
            entries
                .iter()
                .flatten()
                .find(|route| address >= route.base() && address < route.base() + 4096)
                .cloned()
        })?;
        route.route(access)
    }
    pub(crate) fn close(&self) {
        // Release the registry's retained routes even when diagnostic Native
        // handles remain. No callback or destructor runs under the route lock.
        let detached = self
            .entries
            .with(|entries| core::mem::replace(entries, core::array::from_fn(|_| None)));
        for route in detached.into_iter().flatten() {
            route.close();
        }
    }
}
