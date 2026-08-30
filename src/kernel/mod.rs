// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent kernel policy, grouped by subsystem.

// Resource accounting is compiled before Process and user-address-space
// owners exist so its portable lifecycle contracts remain continuously tested.
pub(crate) mod abi;
#[allow(dead_code, unused_imports)]
pub(crate) mod accounting;
pub(crate) mod boot;
// The compiled capability core deliberately precedes its Process owner. Keep
// this narrow exception until strong Process/address-space publication lands;
// cfg-gating the module would leave its cross-architecture safety uncompiled.
#[allow(dead_code, unused_imports)]
pub(crate) mod capability;
pub mod cpu;
pub mod crash;
pub mod debug;
pub mod device;
pub(crate) mod entry;
pub mod irq;
pub mod log;
pub mod mm;
#[allow(dead_code, unused_imports)]
pub(crate) mod process;
pub mod sync;
pub mod task;
pub mod time;
pub mod vm;
