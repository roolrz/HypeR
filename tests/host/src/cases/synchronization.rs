//! Interrupt masking and explicitly ordered atomic-operation contracts.

use core::sync::atomic::{AtomicUsize, Ordering};

use hyper::hal::interrupt::InterruptMask;
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicFlag, AtomicU64, Ordering as AtomicOrdering, fence};

static MASK_DEPTH: AtomicUsize = AtomicUsize::new(0);

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
fn atomic_flag_and_counter_use_explicit_ordering() {
    let flag = AtomicFlag::default();
    assert!(flag.try_acquire());
    assert!(!flag.try_acquire());
    assert!(flag.is_acquired(AtomicOrdering::Relaxed));
    flag.release();
    assert!(!flag.is_acquired(AtomicOrdering::Acquire));

    let counter = AtomicU64::new(40);
    assert_eq!(counter.fetch_add(2, AtomicOrdering::AcqRel), 40);
    fence(AtomicOrdering::SeqCst);
    assert_eq!(counter.load(AtomicOrdering::Acquire), 42);
}
