// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Early allocation before the runtime heap is available.

mod allocator;

pub use allocator::{BootAllocator, BootAllocatorError, BootMemoryStats, MAX_BOOT_RESERVATIONS};
