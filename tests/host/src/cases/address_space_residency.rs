// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::mm::{AddressSpaceResidency, ResidencyError};

#[test]
fn update_cut_blocks_late_entrants_and_targets_all_residents() {
    let mut state = AddressSpaceResidency::<4>::new(7);
    state
        .check_admission(0, 7)
        .unwrap_or_else(|error| panic!("admission failed: {error:?}"));
    state
        .publish_admission(0)
        .unwrap_or_else(|error| panic!("publish failed: {error:?}"));
    state
        .leave(0, 7)
        .unwrap_or_else(|error| panic!("leave failed: {error:?}"));
    state
        .check_admission(1, 7)
        .unwrap_or_else(|error| panic!("admission failed: {error:?}"));
    state
        .publish_admission(1)
        .unwrap_or_else(|error| panic!("publish failed: {error:?}"));
    assert_eq!(
        state.check_admission(1, 7),
        Err(ResidencyError::AlreadyActive)
    );
    let cut = state
        .begin_update(7)
        .unwrap_or_else(|error| panic!("cut failed: {error:?}"));
    assert_eq!(state.check_admission(2, 7), Err(ResidencyError::Busy));
    assert_eq!(cut.active(), &[false, true, false, false]);
    assert_eq!(cut.targets(), &[true, true, false, false]);
    state
        .finish_update(cut, 8)
        .unwrap_or_else(|error| panic!("finish failed: {error:?}"));
    let second = state
        .begin_update(8)
        .unwrap_or_else(|error| panic!("cut failed: {error:?}"));
    assert_eq!(second.targets(), &[false, true, false, false]);
}

#[test]
fn update_gate_dominates_the_transient_active_epoch() {
    let mut state = AddressSpaceResidency::<2>::new(4);
    state
        .check_admission(0, 4)
        .unwrap_or_else(|error| panic!("admission failed: {error:?}"));
    state
        .publish_admission(0)
        .unwrap_or_else(|error| panic!("publish failed: {error:?}"));
    let cut = state
        .begin_update(4)
        .unwrap_or_else(|error| panic!("cut failed: {error:?}"));

    // A remote root switch can already have published epoch 5 locally while
    // the coordinator still owns the epoch-4 cut. Leave must retry instead of
    // treating this valid interval as a stale activation.
    assert_eq!(state.leave(0, 5), Err(ResidencyError::Busy));
    state
        .finish_update(cut, 5)
        .unwrap_or_else(|error| panic!("finish failed: {error:?}"));
    state
        .leave(0, 5)
        .unwrap_or_else(|error| panic!("leave failed: {error:?}"));
}

#[test]
fn stale_epoch_cannot_admit_or_start_an_update() {
    let mut state = AddressSpaceResidency::<2>::new(11);
    assert_eq!(
        state.check_admission(0, 10),
        Err(ResidencyError::StaleEpoch)
    );
    assert!(matches!(
        state.begin_update(10),
        Err(ResidencyError::StaleEpoch)
    ));
}

#[test]
fn retirement_rejects_active_cpus_and_targets_inactive_residents() {
    let mut state = AddressSpaceResidency::<2>::new(3);
    crate::require_ok(state.check_admission(1, 3));
    crate::require_ok(state.publish_admission(1));
    assert!(matches!(
        state.begin_retirement(3),
        Err(ResidencyError::Busy)
    ));
    crate::require_ok(state.leave(1, 3));
    let cut = crate::require_ok(state.begin_retirement(3));
    assert_eq!(cut.active(), &[false, false]);
    assert_eq!(cut.targets(), &[false, true]);
    assert_eq!(state.check_admission(0, 3), Err(ResidencyError::Busy));
}
