// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent synchronization primitives.

pub mod atomic;
pub mod lock;

pub use lock::{InterruptMaskGuard, InterruptSpinLock, SpinLock};
