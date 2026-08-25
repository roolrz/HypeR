// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Interrupt masking and explicitly ordered atomic-operation contracts.

use core::sync::atomic::{AtomicUsize, Ordering};

use hyper::hal::interrupt::InterruptMask;
use hyper::sync::atomic::{AtomicFlag, AtomicU64, Ordering as AtomicOrdering, fence};
use hyper::sync::{InterruptMaskGuard, InterruptSpinLock};
use std::sync::Arc;
use std::thread;

static MASK_DEPTH: AtomicUsize = AtomicUsize::new(0);
static CONTENTION_WAITS: AtomicUsize = AtomicUsize::new(0);

struct TestInterruptMask;

impl InterruptMask for TestInterruptMask {
    type State = usize;

    fn save_and_disable() -> Self::State {
        MASK_DEPTH.fetch_add(1, Ordering::SeqCst)
    }

    fn restore(state: Self::State) {
        MASK_DEPTH.store(state, Ordering::SeqCst);
    }
}

struct ProgressInterruptMask;

impl InterruptMask for ProgressInterruptMask {
    type State = ();

    fn save_and_disable() -> Self::State {}

    fn restore(_: Self::State) {}

    fn wait_for_lock_owner() {
        CONTENTION_WAITS.fetch_add(1, Ordering::SeqCst);
        thread::yield_now();
    }
}

// Inference is unique only while InterruptMaskGuard implements neither auto
// trait. If it becomes Send or Sync, the corresponding conditional impl makes
// this marker selection ambiguous and compilation fails.
const _: fn() = || {
    trait AmbiguousIfImpl<Marker: ?Sized> {
        fn marker() {}
    }

    impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfImpl<dyn Send> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfImpl<dyn Sync> for T {}

    let _ = <InterruptMaskGuard<TestInterruptMask> as AmbiguousIfImpl<_>>::marker;
};

#[test]
fn interrupt_lock_restores_the_previous_mask_state() {
    let lock = InterruptSpinLock::<_, TestInterruptMask>::new(41usize);
    lock.with(|value| {
        assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 1);
        *value += 1;
        assert!(lock.try_with(|_| ()).is_none());
        assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 1);
    });
    assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 0);
    assert_eq!(lock.try_with(|value| *value), Some(42));
    assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 0);
}

#[test]
fn both_interrupt_lock_acquisition_paths_run_architecture_progress() {
    exercise_contended_lock(false);
    exercise_contended_lock(true);
}

fn exercise_contended_lock(retain_mask: bool) {
    CONTENTION_WAITS.store(0, Ordering::SeqCst);
    let lock = Arc::new(InterruptSpinLock::<_, ProgressInterruptMask>::new(0usize));
    let held = Arc::new(core::sync::atomic::AtomicBool::new(false));

    let owner_lock = Arc::clone(&lock);
    let owner_held = Arc::clone(&held);
    let owner = thread::spawn(move || {
        owner_lock.with(|value| {
            owner_held.store(true, Ordering::Release);
            while CONTENTION_WAITS.load(Ordering::Acquire) == 0 {
                thread::yield_now();
            }
            *value = 41;
        });
    });

    while !held.load(Ordering::Acquire) {
        thread::yield_now();
    }
    let contender_lock = Arc::clone(&lock);
    let contender = thread::spawn(move || {
        if retain_mask {
            // SAFETY: The retained guard is dropped on this same host thread
            // immediately after acquisition, preserving CPU affinity and order.
            let ((), guard) = unsafe {
                contender_lock.with_mask_retained(|value| {
                    *value += 1;
                })
            };
            drop(guard);
        } else {
            contender_lock.with(|value| *value += 1);
        }
    });

    assert!(owner.join().is_ok());
    assert!(contender.join().is_ok());
    assert!(CONTENTION_WAITS.load(Ordering::Acquire) > 0);
    assert_eq!(lock.with(|value| *value), 42);
}

#[test]
fn interrupt_mask_default_lock_wait_is_available() {
    // TestInterruptMask intentionally does not override the progress hook. This
    // call is also a compile-time contract for the architecture-neutral default.
    TestInterruptMask::wait_for_lock_owner();
}

#[test]
fn atomic_flag_and_counter_use_explicit_ordering() {
    let flag = AtomicFlag::default();
    assert!(flag.try_acquire());
    assert!(!flag.try_acquire());
    assert!(flag.is_acquired());
    flag.release();
    assert!(!flag.is_acquired());

    let counter = AtomicU64::new(40);
    assert_eq!(counter.fetch_add(2, AtomicOrdering::AcqRel), 40);
    fence(AtomicOrdering::SeqCst);
    assert_eq!(counter.load(AtomicOrdering::Acquire), 42);
}
