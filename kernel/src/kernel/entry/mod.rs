// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Narrow upward adapters for architecture exception and guest-exit entry.
//!
//! Raw frames and machine decoding remain below `arch`. These adapters are the
//! only paths by which architecture entry code invokes kernel crash, IRQ, and
//! VM policy, keeping the unavoidable dependency inversion explicit and small.

pub(crate) mod exception;
pub(crate) mod irq;
mod services;
pub(crate) mod user;
pub(crate) mod vmexit;

#[cfg(all(
    feature = "kernel-self-test",
    any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64)
))]
pub(crate) use services::verify_thread_affinity_creation_for_test;
