// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Inert machine contracts for native `AArch64` userspace.
//!
//! This module validates register encodings and makes lower-EL return regimes
//! explicit. It deliberately owns no page-table memory, residency set, or
//! executable entry capability. A validated value therefore cannot activate
//! an address space; that operation must wait for a pinned process address
//! space and a concurrent-residency token supplied by kernel memory policy.

// These contracts are intentionally dormant until process memory can produce
// the residency capability required by a real entry path.
#![allow(dead_code)]

use super::registers;

const MINIMUM_ADDRESS_BITS: u8 = 32;
const MAXIMUM_ADDRESS_BITS: u8 = 48;

/// Translation mechanism selected for native EL0 on this host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UserTranslationRegime {
    /// VHE host EL2&0 stage-1 translation with `E2H=1` and `TGE=1`.
    VheHostStage1,
    /// nVHE direct EL0 with stage-1 disabled and per-process stage-2.
    NvheStage2Only,
}

/// Immutable machine limits selected during boot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserExecutionCapabilities {
    regime: UserTranslationRegime,
    address_bits: u8,
    physical_address_bits: u8,
    translation_identifier_bits: u8,
}

impl UserExecutionCapabilities {
    /// Validates one host-selected native-user translation contract.
    pub(super) const fn new(
        regime: UserTranslationRegime,
        address_bits: u8,
        physical_address_bits: u8,
        translation_identifier_bits: u8,
    ) -> Result<Self, UserMachineContractError> {
        if address_bits < MINIMUM_ADDRESS_BITS || address_bits > MAXIMUM_ADDRESS_BITS {
            return Err(UserMachineContractError::UnsupportedAddressWidth);
        }
        if physical_address_bits < MINIMUM_ADDRESS_BITS
            || physical_address_bits > MAXIMUM_ADDRESS_BITS
        {
            return Err(UserMachineContractError::UnsupportedPhysicalAddressWidth);
        }
        if translation_identifier_bits != 8 && translation_identifier_bits != 16 {
            return Err(UserMachineContractError::UnsupportedIdentifierWidth);
        }
        Ok(Self {
            regime,
            address_bits,
            physical_address_bits,
            translation_identifier_bits,
        })
    }

    pub(super) const fn regime(self) -> UserTranslationRegime {
        self.regime
    }

    pub(super) const fn address_bits(self) -> u8 {
        self.address_bits
    }

    pub(super) const fn physical_address_bits(self) -> u8 {
        self.physical_address_bits
    }

    pub(super) const fn translation_identifier_bits(self) -> u8 {
        self.translation_identifier_bits
    }
}

/// Inert register values for one process address space.
///
/// The generation prevents a future residency token from silently referring
/// to a recycled software address-space identity. It is not an architectural
/// ASID or VMID and is never encoded into a register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UserTranslationRegisters {
    regime: UserTranslationRegime,
    root_register: u64,
    generation: u64,
}

impl UserTranslationRegisters {
    /// Validates the root and identifier without claiming root ownership.
    ///
    /// The returned value is data only. It cannot be activated until a future
    /// address-space owner proves that the complete hierarchy is pinned and a
    /// concurrent residency protocol has admitted the calling CPU.
    pub(super) const fn new(
        capabilities: UserExecutionCapabilities,
        regime: UserTranslationRegime,
        root_address: u64,
        translation_identifier: u16,
        generation: u64,
    ) -> Result<Self, UserMachineContractError> {
        if !same_regime(regime, capabilities.regime) {
            return Err(UserMachineContractError::TranslationRegimeMismatch);
        }
        if root_address & (registers::TRANSLATION_GRANULE_4K - 1) != 0 {
            return Err(UserMachineContractError::InvalidRootAlignment);
        }
        let physical_limit = 1_u64 << capabilities.physical_address_bits;
        if root_address >= physical_limit
            || root_address & !registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT != 0
        {
            return Err(UserMachineContractError::RootOutsidePhysicalAddressSpace);
        }
        if translation_identifier == 0
            || translation_identifier as u32 >= (1_u32 << capabilities.translation_identifier_bits)
        {
            return Err(UserMachineContractError::InvalidTranslationIdentifier);
        }
        if generation == 0 {
            return Err(UserMachineContractError::InvalidGeneration);
        }
        let identifier_shift = match regime {
            UserTranslationRegime::VheHostStage1 => registers::TTBR_ASID_SHIFT,
            UserTranslationRegime::NvheStage2Only => registers::VTTBR_EL2_VMID_SHIFT as u64,
        };
        Ok(Self {
            regime,
            root_register: ((translation_identifier as u64) << identifier_shift) | root_address,
            generation,
        })
    }

    pub(super) const fn regime(self) -> UserTranslationRegime {
        self.regime
    }

    pub(super) const fn root_register(self) -> u64 {
        self.root_register
    }

    pub(super) const fn generation(self) -> u64 {
        self.generation
    }
}

/// Explicit lower-EL world selected by an owned return capability.
///
/// A lower-AArch64 exception vector does not identify this state. Native and
/// guest execution use the same vector slots but require different `HCR_EL2`
/// values, especially after nVHE native execution has enabled `DC`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LowerElReturnRegime {
    Native(UserTranslationRegime),
    Guest,
}

impl LowerElReturnRegime {
    /// Produces the `HCR_EL2` value required before `ERET`.
    ///
    /// `VM`, `TGE`, and `DC` are written deterministically because they select
    /// the lower-EL translation world. Unrelated host trap controls are
    /// preserved. The selected host mode must already match `E2H`; changing
    /// host mode is a boot-time operation, not a return-path side effect.
    pub(super) const fn transition_hcr(
        self,
        current_hcr: u64,
    ) -> Result<u64, UserMachineContractError> {
        let vhe_host = current_hcr & registers::HCR_EL2_E2H != 0;
        match self {
            Self::Native(UserTranslationRegime::VheHostStage1) => {
                if !vhe_host {
                    return Err(UserMachineContractError::HostModeMismatch);
                }
                Ok(
                    (current_hcr | registers::HCR_EL2_TGE | registers::HCR_EL2_RW)
                        & !(registers::HCR_EL2_DC | registers::HCR_EL2_VM),
                )
            }
            Self::Native(UserTranslationRegime::NvheStage2Only) => {
                if vhe_host {
                    return Err(UserMachineContractError::HostModeMismatch);
                }
                Ok(current_hcr
                    | registers::HCR_EL2_TGE
                    | registers::HCR_EL2_DC
                    | registers::HCR_EL2_VM
                    | registers::HCR_EL2_RW)
            }
            Self::Guest => Ok(
                (current_hcr | registers::HCR_EL2_VM | registers::HCR_EL2_RW)
                    & !(registers::HCR_EL2_TGE | registers::HCR_EL2_DC),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserMachineContractError {
    HostModeMismatch,
    InvalidGeneration,
    InvalidRootAlignment,
    InvalidTranslationIdentifier,
    RootOutsidePhysicalAddressSpace,
    TranslationRegimeMismatch,
    UnsupportedAddressWidth,
    UnsupportedIdentifierWidth,
    UnsupportedPhysicalAddressWidth,
}

const fn same_regime(left: UserTranslationRegime, right: UserTranslationRegime) -> bool {
    matches!(
        (left, right),
        (
            UserTranslationRegime::VheHostStage1,
            UserTranslationRegime::VheHostStage1
        ) | (
            UserTranslationRegime::NvheStage2Only,
            UserTranslationRegime::NvheStage2Only
        )
    )
}
