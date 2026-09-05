// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::mm::{TranslationIdError, TranslationIdPool};

enum TestNamespace {}

#[test]
fn unpublished_identifier_reuse_advances_generation() {
    // SAFETY: This test creates the only pool for its private marker.
    let mut pool = unsafe { TranslationIdPool::<TestNamespace, 3>::new() };
    let first = pool
        .reserve()
        .unwrap_or_else(|error| panic!("reserve failed: {error:?}"));
    let first_value = first.value();
    let first_generation = first.generation();
    pool.cancel(first)
        .unwrap_or_else(|error| panic!("cancel failed: {error:?}"));
    let second = pool
        .reserve()
        .unwrap_or_else(|error| panic!("reserve failed: {error:?}"));
    assert_eq!(first_value, second.value());
    assert_ne!(first_generation, second.generation());
}

#[test]
fn active_identifier_cannot_reenter_pool_before_acknowledged_retirement() {
    // SAFETY: This test creates the only pool for its private marker.
    let mut pool = unsafe { TranslationIdPool::<TestNamespace, 2>::new() };
    let reserved = pool
        .reserve()
        .unwrap_or_else(|error| panic!("reserve failed: {error:?}"));
    let active = pool
        .activate(reserved)
        .unwrap_or_else(|error| panic!("activation failed: {error:?}"));
    let active_value = active.value();
    let active_generation = active.generation();
    assert!(matches!(pool.reserve(), Err(TranslationIdError::Exhausted)));
    let retiring = pool
        .begin_retirement(active)
        .unwrap_or_else(|error| panic!("retirement failed: {error:?}"));
    assert!(matches!(pool.reserve(), Err(TranslationIdError::Exhausted)));
    // SAFETY: This model test represents the acknowledged invalidation edge.
    unsafe { pool.complete_retirement(retiring) }
        .unwrap_or_else(|error| panic!("completion failed: {error:?}"));
    let reused = pool
        .reserve()
        .unwrap_or_else(|error| panic!("reserve failed: {error:?}"));
    assert_eq!(reused.value(), active_value);
    assert_ne!(reused.generation(), active_generation);
}
