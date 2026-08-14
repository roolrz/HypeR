//! Runtime physical-page and heap allocators.

mod buddy;
pub mod heap;

pub use buddy::{BuddyAllocator, BuddyError, MAX_ORDER, MemoryHandoff};
