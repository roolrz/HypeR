//! Architecture-independent synchronization primitives.

pub mod atomic;
pub mod lock;

pub use lock::{InterruptSpinLock, SpinLock};
