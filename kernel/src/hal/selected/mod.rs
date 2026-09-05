// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Statically selected machine capabilities for the kernel binary.
//!
//! Reusable architecture-neutral contracts live in [`hyper::hal`]. This
//! binary-only layer binds those contracts to the selected architecture and
//! is the sole machine-operation dependency exposed to kernel policy.

pub(crate) mod atomic;
pub(crate) mod cache;
pub(crate) mod context;
pub(crate) mod cpu;
pub(crate) mod exception;
pub(crate) mod irq;
pub(crate) mod memory;
pub(crate) mod platform;
pub(crate) mod time;
pub(crate) mod user;
pub(crate) mod vm;
