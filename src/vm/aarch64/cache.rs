// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` guest instruction-cache alias policy.

const CTR_EL0_L1IP_SHIFT: u64 = 14;
const CTR_EL0_L1IP_MASK: u64 = 0b11;
const CTR_EL0_L1IP_PIPT: u64 = 0b11;

/// Invalidation scope needed after writing guest instructions through a host
/// virtual alias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestInstructionCachePolicy {
    /// A physical-indexed cache permits precise invalidation by the host alias.
    Range,
    /// Aliasing or unknown indexing requires invalidating the complete
    /// inner-shareable instruction-cache domain.
    WholeInnerShareableDomain,
}

/// Selects the conservative guest instruction-cache policy from `CTR_EL0`.
///
/// Only the architecturally explicit PIPT encoding admits range invalidation.
/// VIPT, VPIPT, AIVIVT, and reserved/future behavior use whole-domain
/// invalidation because guest execution uses a different VA from the host
/// linear mapping used to populate the page.
pub const fn guest_instruction_cache_policy(ctr_el0: u64) -> GuestInstructionCachePolicy {
    if (ctr_el0 >> CTR_EL0_L1IP_SHIFT) & CTR_EL0_L1IP_MASK == CTR_EL0_L1IP_PIPT {
        GuestInstructionCachePolicy::Range
    } else {
        GuestInstructionCachePolicy::WholeInnerShareableDomain
    }
}
