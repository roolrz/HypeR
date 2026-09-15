// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent kernel policy, grouped by subsystem.

// Native Process execution is the normal production workload. Keep its
// complete ownership graph compiled on every architecture so accounting,
// capability, memory, and lifecycle contracts cannot decay behind AArch64-only
// runtime coverage.
pub(crate) mod abi;
pub(crate) mod accounting;
pub(crate) mod authority;
pub(crate) mod block;
pub(crate) mod boot;
pub(crate) mod capability;
pub mod cpu;
pub mod crash;
pub mod debug;
pub mod device;
pub(crate) mod entry;
#[cfg(not(feature = "kernel-self-test"))]
pub(crate) mod init;
pub(crate) mod inspect;
pub(crate) mod io_cache;
pub(crate) mod ipc;
pub mod irq;
pub mod log;
pub mod mm;
pub(crate) mod object;
pub(crate) mod process;
pub(crate) mod reaper;
pub mod sync;
pub mod task;
pub mod time;
pub(crate) mod vfs;
pub mod vm;
