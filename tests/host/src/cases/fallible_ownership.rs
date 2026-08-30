// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ownership and destruction behavior of fallible allocation helpers.

use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};

static DROPS: AtomicUsize = AtomicUsize::new(0);

struct ZeroSizedDrop;

impl Drop for ZeroSizedDrop {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn try_box_moves_and_drops_zero_sized_values_once() {
    DROPS.store(0, Ordering::Relaxed);
    let value = crate::require_ok(hyper::mm::try_box(ZeroSizedDrop));
    assert_eq!(DROPS.load(Ordering::Relaxed), 0);
    drop(value);
    assert_eq!(DROPS.load(Ordering::Relaxed), 1);
}

#[test]
fn fallible_arc_clones_share_one_stable_value() {
    let owner = crate::require_ok(hyper::mm::FallibleArc::try_new(41_u64));
    let clone = owner.clone();

    assert_eq!(*owner, 41);
    assert_eq!(*clone, 41);
    assert_eq!(owner.strong_count(), 2);
}

#[test]
fn unique_fallible_arc_extracts_without_reallocation() {
    let owner = crate::require_ok(hyper::mm::FallibleArc::try_new(String::from("retired")));
    let value = match owner.try_unwrap() {
        Ok(value) => value,
        Err(_) => panic!("unique owner was not extracted"),
    };
    assert_eq!(value, "retired");
}

#[test]
fn failed_extraction_returns_the_original_live_owner() {
    let owner = crate::require_ok(hyper::mm::FallibleArc::try_new(41_u64));
    let peer = owner.clone();
    let owner = match owner.try_unwrap() {
        Ok(_) => panic!("shared owner unexpectedly extracted"),
        Err(owner) => owner,
    };
    assert_eq!(*owner, 41);
    assert_eq!(owner.strong_count(), 2);
    drop(peer);
    assert_eq!(owner.strong_count(), 1);
    let value = match owner.try_unwrap() {
        Ok(value) => value,
        Err(_) => panic!("unique owner was not extracted after peer release"),
    };
    assert_eq!(value, 41);
}

#[test]
fn unique_arc_retains_address_and_can_restore_sharing() {
    let owner = crate::require_ok(hyper::mm::FallibleArc::try_new(17_u64));
    let address = core::ptr::from_ref(&*owner);
    let mut unique = match owner.try_into_unique() {
        Ok(unique) => unique,
        Err(_) => panic!("sole shared owner did not become unique"),
    };
    assert_eq!(core::ptr::from_ref(&*unique), address);
    *unique = 23;
    let shared = unique.into_shared();
    assert_eq!(core::ptr::from_ref(&*shared), address);
    assert_eq!(*shared, 23);
}

#[test]
fn uninitialized_unique_arc_initializes_and_drops_once() {
    struct CountedDrop<'a>(&'a AtomicUsize);

    impl Drop for CountedDrop<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let drops = AtomicUsize::new(0);
    let slot = crate::require_ok(hyper::mm::UniqueFallibleArc::try_new_uninit());
    let owner = slot.write(CountedDrop(&drops));
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    drop(owner);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[test]
fn fallible_arc_destroys_the_value_after_the_last_concurrent_owner() {
    struct CountedDrop(std::sync::Arc<AtomicUsize>);

    impl Drop for CountedDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let drops = std::sync::Arc::new(AtomicUsize::new(0));
    let owner = crate::require_ok(hyper::mm::FallibleArc::try_new(CountedDrop(drops.clone())));
    let barrier = std::sync::Arc::new(Barrier::new(5));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let clone = owner.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            drop(clone);
        }));
    }

    barrier.wait();
    for worker in workers {
        assert!(worker.join().is_ok());
    }
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    drop(owner);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}
