//! Locking primitives and interrupt-masking composition.

mod interrupt;
mod spin;

pub use interrupt::InterruptSpinLock;
pub use spin::SpinLock;
