//! Kernel ownership and fault policy for the Rust global allocator.

use hyper::mm::allocator::heap::KernelGlobalAllocator;

pub type GlobalAllocator = KernelGlobalAllocator<crate::arch::irq::LocalMask>;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();
