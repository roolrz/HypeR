// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Hierarchical ownership and quota policy for kernel-managed resources.

mod resource_domain;

pub(crate) use resource_domain::{
    ChargeReservation, CommittedCharge, ResourceAmount, ResourceDomain, ResourceDomainId,
    ResourceError, ResourceKind, ResourceLimits, ResourceUsage, RetirementSnapshot,
};
