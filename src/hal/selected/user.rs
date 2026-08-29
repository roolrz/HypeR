// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected native-user machine capability.
//!
//! Process, syscall, rights, and mapping ownership remain kernel policy. This
//! facade currently publishes only inert `AArch64` contracts; activation is
//! intentionally absent until memory policy can provide a concurrent
//! residency token.

#[cfg(CONFIG_ARCH_AARCH64)]
#[allow(unused_imports)] // Reserved until process residency can produce an activation token.
pub(crate) use crate::arch::user::{
    UserExecutionCapabilities, UserMachineContractError, execution_capabilities,
};
