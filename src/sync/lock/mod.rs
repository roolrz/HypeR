// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Locking primitives and interrupt-masking composition.

mod interrupt;
mod spin;

pub use interrupt::InterruptSpinLock;
pub use spin::SpinLock;
