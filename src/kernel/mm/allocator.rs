// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel ownership and fault policy for the Rust global allocator.

use hyper::mm::allocator::heap::KernelGlobalAllocator;

pub type GlobalAllocator = KernelGlobalAllocator<crate::hal::irq::LocalMask>;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();
