// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent kernel policy, grouped by subsystem.

// Native Process execution is the normal production workload. Keep its
// complete ownership graph compiled on every architecture so accounting,
// capability, memory, and lifecycle contracts cannot decay behind AArch64-only
// runtime coverage.
pub(crate) mod abi;
#[allow(dead_code, unused_imports)]
pub(crate) mod accounting;
#[allow(dead_code)]
pub(crate) mod authority;
pub(crate) mod boot;
#[allow(dead_code, unused_imports)]
pub(crate) mod capability;
pub mod cpu;
pub mod crash;
pub mod debug;
pub mod device;
pub(crate) mod entry;
pub(crate) mod fs;
#[cfg(not(feature = "kernel-self-test"))]
pub(crate) mod init;
#[allow(dead_code, unused_imports)]
pub(crate) mod ipc;
pub mod irq;
pub mod log;
pub mod mm;
pub(crate) mod object;
#[allow(dead_code, unused_imports)]
pub(crate) mod process;
pub(crate) mod reaper;
pub mod sync;
pub mod task;
pub mod time;
pub mod vm;
