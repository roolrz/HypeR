// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent memory-management mechanisms.

mod access;
mod address;
mod allocation;
pub mod allocator;
pub mod boot;
pub mod kaslr;

pub use access::{ForeignCopyError, ForeignMemory, copy_from_foreign, copy_to_foreign};
pub use address::{PAGE_SIZE, PhysicalAddress, VirtualAddress};
pub use allocation::{AllocationError, FallibleArc, try_box};
pub use allocator::{BuddyAllocator, BuddyError, BuddyStats, MAX_ORDER, MemoryHandoff};
pub use boot::{BootAllocator, BootAllocatorError, BootMemoryStats};
