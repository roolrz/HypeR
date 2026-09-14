// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Installed userspace-device routes and hardware-detached MMIO requests.

use super::{Error, InstalledMachine, RuntimeState};
use hyper::vm::device::mmio::Request;
use hyper::vm::exit::{MmioAccess, MmioAction};

#[derive(Clone, Copy)]
pub(super) struct Region {
    base: u64,
    end: u64,
    device: u64,
}

impl InstalledMachine {
    pub(in crate::kernel::vm) fn register_mmio(
        &self,
        base: u64,
        length: u64,
        device: u64,
    ) -> Result<(), Error> {
        if !crate::kernel::vm::device::supports_userspace_mmio(
            self.configuration.platform_profile,
            base,
            length,
        ) || device == 0
        {
            return Err(Error::BadState);
        }
        let end = base.checked_add(length).ok_or(Error::BadState)?;
        self.state.with(|state| {
            if !matches!(state, RuntimeState::Installed { .. }) {
                return Err(Error::BadState);
            }
            let id = match state {
                RuntimeState::Installed { id, .. } => *id,
                _ => return Err(Error::BadState),
            };
            if crate::kernel::vm::registry::with_binding(id, |binding| {
                binding.io_range_conflicts(base, length)
            })
            .unwrap_or(true)
            {
                return Err(Error::BadState);
            }
            self.mmio_regions.with(|regions| {
                if regions.iter().flatten().any(|region| {
                    region.device == device || (base < region.end && region.base < end)
                }) {
                    return Err(Error::BadState);
                }
                let slot = regions
                    .iter_mut()
                    .find(|slot| slot.is_none())
                    .ok_or(Error::BadState)?;
                *slot = Some(Region { base, end, device });
                Ok(())
            })
        })
    }

    #[allow(
        dead_code,
        reason = "selected guest backends opt into deferred userspace MMIO"
    )]
    pub(in crate::kernel) fn route_mmio(
        &self,
        vcpu: u32,
        access: MmioAccess,
    ) -> Option<MmioAction> {
        let base = access.address().get();
        let end = base.checked_add(access.size() as u64)?;
        let device = self.mmio_regions.with(|regions| {
            regions
                .iter()
                .flatten()
                .find(|region| base >= region.base && end <= region.end)
                .map(|region| region.device)
        })?;
        match self.endpoint(vcpu).and_then(|endpoint| {
            endpoint
                .stage_mmio(device, access)
                .map_err(|_| Error::BadState)
        }) {
            Ok(()) => Some(MmioAction::Deferred),
            Err(_) => Some(MmioAction::Stop),
        }
    }

    pub(in crate::kernel::vm) fn publish_mmio(&self, vcpu: u32) -> Result<(), Error> {
        self.endpoint(vcpu)?
            .publish_mmio()
            .map_err(|_| Error::BadState)
    }

    pub(in crate::kernel::vm) fn pending_mmio(&self, vcpu: u32) -> Result<Option<Request>, Error> {
        self.state.with(|state| {
            if !matches!(state, RuntimeState::Running { .. }) {
                return Err(Error::BadState);
            }
            Ok(self.endpoint(vcpu)?.pending_mmio())
        })
    }

    pub(in crate::kernel::vm) fn complete_mmio(
        &self,
        vcpu: u32,
        id: u64,
        action: MmioAction,
    ) -> Result<(), Error> {
        // Completion and stop admission have one lifecycle lock. The runner
        // alone updates architectural state after consuming this result.
        self.state.with(|state| {
            if !matches!(state, RuntimeState::Running { .. }) {
                return Err(Error::BadState);
            }
            self.endpoint(vcpu)?
                .complete_mmio(id, action)
                .map_err(|_| Error::BadState)
        })
    }

    pub(in crate::kernel::vm) fn take_mmio_completion(
        &self,
        vcpu: u32,
    ) -> Result<Option<MmioAction>, Error> {
        self.endpoint(vcpu)?
            .take_mmio_completion()
            .map_err(|_| Error::BadState)
    }
}

impl InstalledMachine {
    pub(in crate::kernel::vm) fn validate_io_route(
        &self,
        route: &crate::kernel::vm::io::Route,
    ) -> Result<(), crate::kernel::vm::io::Error> {
        let base = route.base();
        self.mmio_regions.with(|regions| {
            if regions.iter().flatten().any(|region| {
                base < region.end
                    && region.base < base + 4096
                    && !(route.deferred_overlay()
                        && region.base == base
                        && region.end == base + 4096)
            }) {
                Err(crate::kernel::vm::io::Error::Busy)
            } else {
                Ok(())
            }
        })
    }
}
