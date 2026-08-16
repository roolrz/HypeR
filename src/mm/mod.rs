//! Architecture-independent memory-management mechanisms.

mod address;
mod allocation;
pub mod allocator;
pub mod boot;
pub mod kaslr;

pub use address::{PAGE_SIZE, PhysicalAddress, VirtualAddress};
pub use allocation::{AllocationError, try_box};
pub use allocator::{BuddyAllocator, BuddyError, BuddyStats, MAX_ORDER, MemoryHandoff};
pub use boot::{BootAllocator, BootAllocatorError, BootMemoryStats};
