// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture native-user machine contracts.
//!
//! This facade is intentionally unavailable on secondary architectures until
//! they define their own execution mechanism. It contains no process or ABI
//! policy and exposes no runnable entry before address-space residency exists.

#[cfg(target_arch = "aarch64")]
pub(crate) use super::imp::{
    UserExecutionCapabilities, UserMachineContractError, execution_capabilities,
};
