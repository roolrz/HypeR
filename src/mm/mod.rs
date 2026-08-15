//! Architecture-independent memory-management mechanisms.

mod address;
pub mod allocator;
pub mod boot;
pub mod kaslr;

pub use address::{PAGE_SIZE, PhysicalAddress, VirtualAddress};
pub use allocator::{BuddyAllocator, BuddyError, BuddyStats, MAX_ORDER, MemoryHandoff};
pub use boot::{BootAllocator, BootAllocatorError, BootMemoryStats};

// Compatibility facade for the existing global-allocator integration.
pub use allocator::heap;
