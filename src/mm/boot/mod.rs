//! Early allocation before the runtime heap is available.

mod allocator;

pub use allocator::{BootAllocator, BootAllocatorError, BootMemoryStats, MAX_BOOT_RESERVATIONS};
