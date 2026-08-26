// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! AArch64 guest instruction-cache alias policy.

use hyper::vm::aarch64::cache::{GuestInstructionCachePolicy, guest_instruction_cache_policy};

#[test]
fn pipt_admits_precise_guest_instruction_invalidation() {
    let ctr = 0b11_u64 << 14;
    assert_eq!(
        guest_instruction_cache_policy(ctr),
        GuestInstructionCachePolicy::Range
    );
}

#[test]
fn every_non_pipt_encoding_uses_conservative_domain_invalidation() {
    for encoding in 0_u64..0b11 {
        let ctr = encoding << 14;
        assert_eq!(
            guest_instruction_cache_policy(ctr),
            GuestInstructionCachePolicy::WholeInnerShareableDomain
        );
    }
}
