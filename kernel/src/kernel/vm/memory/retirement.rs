// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linear VMID and stage-2 retirement transactions.

use hyper::mm::RetirementCut;

use super::{Error, GuestAddressSpace, Stage2Identifier};
use crate::kernel::vm::residency_state::Stage2AllocationIdentity;

impl GuestAddressSpace {
    pub(in crate::kernel) fn begin_retirement(
        &mut self,
        capability: &crate::hal::vm::GuestStage2RetirementCapability,
        topology_count: usize,
    ) -> Result<GuestStage2Retirement, Error> {
        self.ensure_healthy()?;
        if topology_count == 0 || topology_count > hyper::cpu::MAX_CPUS {
            return Err(Error::InvalidCpu);
        }
        let incarnation = self.incarnation()?;
        // The registry acquired this selected-mechanism proof before its own
        // irreversible cut. Request preparation is therefore infallible and
        // remains ahead of residency and identifier retirement.
        let request = crate::hal::vm::prepare_guest_stage2_retirement(capability, &self.stage2);
        let cut = self
            .residency
            .begin_retirement(incarnation.translation_epoch())
            .map_err(Error::Residency)?;
        if cut
            .targets()
            .iter()
            .copied()
            .enumerate()
            .any(|(cpu, targeted)| targeted && cpu >= topology_count)
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement targets escape the frozen CPU topology"
            ));
        }

        let previous = core::mem::replace(&mut self.identifier, Stage2Identifier::Poisoned);
        let Stage2Identifier::Active(identifier) = previous else {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement lost its active VMID"
            ));
        };
        let retiring = match identifier.begin_retirement() {
            Ok(retiring) => retiring,
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: guest VMID retirement could not begin after the residency cut: {error:?}"
            )),
        };
        self.identifier = Stage2Identifier::Retiring(retiring);
        Ok(GuestStage2Retirement {
            cut,
            allocation: incarnation.allocation(),
            request,
        })
    }

    pub(in crate::kernel) fn finish_retirement(&mut self, retirement: GuestStage2Retirement) {
        let GuestStage2Retirement {
            cut,
            allocation,
            request: _,
        } = retirement;
        let identity_matches = match &self.identifier {
            Stage2Identifier::Retiring(identifier) => {
                self.stage2.root_address() == allocation.root()
                    && u64::from(identifier.value()) == allocation.vmid()
                    && identifier.generation() == allocation.generation()
            }
            _ => false,
        };
        if !identity_matches {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement completion changed translation identity"
            ));
        }
        if self.residency.finish_retirement(cut).is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest residency retirement completion is inconsistent"
            ));
        }
        let previous = core::mem::replace(&mut self.identifier, Stage2Identifier::Poisoned);
        let Stage2Identifier::Retiring(identifier) = previous else {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement lost its retiring VMID"
            ));
        };
        // SAFETY: The residency state is irreversibly retired and the caller
        // obtained exact-generation acknowledgement from every sticky target.
        if unsafe { identifier.complete() }.is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest VMID completion is inconsistent"
            ));
        }
        self.identifier = Stage2Identifier::Retired;
    }
}

#[must_use = "guest retirement must obtain every target acknowledgement"]
pub(in crate::kernel) struct GuestStage2Retirement {
    cut: RetirementCut<{ hyper::cpu::MAX_CPUS }>,
    allocation: Stage2AllocationIdentity,
    request: crate::hal::vm::GuestStage2RetirementRequest,
}

impl GuestStage2Retirement {
    pub(in crate::kernel) fn targets(&self) -> &[bool; hyper::cpu::MAX_CPUS] {
        self.cut.targets()
    }

    pub(in crate::kernel) const fn local_request(&self) -> GuestStage2LocalRequest {
        GuestStage2LocalRequest {
            allocation: self.allocation,
            hardware: self.request,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::kernel) struct GuestStage2LocalRequest {
    allocation: Stage2AllocationIdentity,
    hardware: crate::hal::vm::GuestStage2RetirementRequest,
}

pub(in crate::kernel) fn service_local_retirement(request: GuestStage2LocalRequest) {
    crate::hal::vm::service_guest_stage2_retirement(request.hardware);
    if super::residency::clear_local_observations(request.allocation).is_err() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: guest retirement could not clear local observations"
        ));
    }
}
