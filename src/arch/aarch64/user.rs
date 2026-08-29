// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native EL0 machine capability selection for `AArch64`.
//!
//! This layer selects VHE host stage-1 or nVHE stage-2-only execution. It does
//! not own process mappings, syscall policy, or a runnable entry operation.

pub use super::user_contract::{UserExecutionCapabilities, UserMachineContractError};

use super::user_contract::UserTranslationRegime;

/// Reports the native-user machine contract selected by the boot CPU.
///
/// nVHE is a supported result with a stage-2-only contract, not an absence of
/// native-user capability. Eight-bit ASIDs/VMIDs are selected conservatively
/// until boot capability discovery publishes the optional wider fields.
#[allow(dead_code)] // The first process-address-space owner will consume this discovery API.
pub fn execution_capabilities() -> Result<UserExecutionCapabilities, UserMachineContractError> {
    let address = super::address::capabilities();
    if super::host::is_vhe() {
        UserExecutionCapabilities::new(
            UserTranslationRegime::VheHostStage1,
            address.virtual_address_bits,
            address.physical_address_bits,
            8,
        )
    } else {
        UserExecutionCapabilities::new(
            UserTranslationRegime::NvheStage2Only,
            address.intermediate_physical_address_bits,
            address.physical_address_bits,
            8,
        )
    }
}
