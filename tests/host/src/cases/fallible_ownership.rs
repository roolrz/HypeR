//! Ownership and destruction behavior of fallible allocation helpers.

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
