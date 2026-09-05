// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Statically selected Linux guest boot ABI.
//!
//! This is the sole host-build selection point for Linux guest-visible types
//! and conventions. It is intentionally VM product policy, not host HAL.

#[cfg(CONFIG_ARCH_AARCH64)]
#[path = "selected/aarch64.rs"]
mod imp;
#[cfg(CONFIG_ARCH_RISCV64)]
#[path = "selected/riscv64.rs"]
mod imp;
#[cfg(CONFIG_ARCH_X86_64)]
#[path = "selected/x86_64.rs"]
mod imp;

pub(super) use imp::*;
