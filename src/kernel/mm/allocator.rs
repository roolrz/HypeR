//! Kernel ownership and fault policy for the Rust global allocator.

use hyper::mm::heap::KernelGlobalAllocator;

pub type GlobalAllocator = KernelGlobalAllocator<crate::arch::LocalInterruptMask>;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();
