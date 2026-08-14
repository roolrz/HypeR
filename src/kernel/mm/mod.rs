//! Kernel memory policy layered over reusable memory-management mechanisms.

pub mod allocator;
pub mod memory;

pub use memory::PreparedMemory;
