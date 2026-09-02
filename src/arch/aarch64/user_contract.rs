// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Machine contracts for native `AArch64` userspace.
//!
//! This module validates register encodings and makes lower-EL return regimes
//! explicit. It deliberately owns no page-table memory or residency policy;
//! those owners remain in the kernel and reach the architecture only through
//! opaque HAL capabilities.

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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) const fn regime(self) -> UserTranslationRegime {
        self.regime
    }

    pub(super) const fn address_bits(self) -> u8 {
        self.address_bits
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) const fn physical_address_bits(self) -> u8 {
        self.physical_address_bits
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) const fn translation_identifier_bits(self) -> u8 {
        self.translation_identifier_bits
    }

    /// Selects the exclusive native-user address limit for this regime.
    ///
    /// `vhe_privileged_base` is the first host-owned address in the shared VHE
    /// stage-1 root. nVHE uses a private stage-2 IPA space and therefore does
    /// not inherit that host-layout reservation.
    pub(super) const fn user_address_limit(self, vhe_privileged_base: u64) -> u64 {
        let translation_limit = 1u64 << self.address_bits;
        match self.regime {
            UserTranslationRegime::VheHostStage1 => {
                if translation_limit < vhe_privileged_base {
                    translation_limit
                } else {
                    vhe_privileged_base
                }
            }
            UserTranslationRegime::NvheStage2Only => translation_limit,
        }
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
    /// The returned value is data only. Activation remains a separate HAL
    /// operation which requires the kernel owner to retain the hierarchy and
    /// identifier while its residency protocol admits the calling CPU.
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
    #[cfg_attr(not(test), allow(dead_code))]
    Guest,
}

impl LowerElReturnRegime {
    /// Produces the `HCR_EL2` value required before `ERET`.
    ///
    /// Native values are built from kernel policy rather than inherited from
    /// a preceding guest. Guest values preserve unrelated controls while
    /// selecting the lower-EL guest regime. The selected host mode must
    /// already match `E2H`.
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
                Ok(native_hcr_base() | registers::HCR_EL2_E2H | registers::HCR_EL2_TGE)
            }
            Self::Native(UserTranslationRegime::NvheStage2Only) => {
                if vhe_host {
                    return Err(UserMachineContractError::HostModeMismatch);
                }
                Ok(native_hcr_base()
                    | registers::HCR_EL2_TGE
                    | registers::HCR_EL2_DC
                    | registers::HCR_EL2_VM)
            }
            Self::Guest => Ok(Self::guest_hcr(current_hcr)),
        }
    }

    pub(super) const fn guest_hcr(current_hcr: u64) -> u64 {
        (current_hcr
            | registers::HCR_EL2_VM
            | registers::HCR_EL2_RW
            | registers::HCR_EL2_TWI
            | registers::HCR_EL2_TWE)
            & !(registers::HCR_EL2_TGE | registers::HCR_EL2_DC)
    }
}

/// Deterministic trap policy for untrusted native EL0 execution.
///
/// Unsupported wait, cache-maintenance, translation-maintenance, and feature
/// discovery operations trap instead of inheriting controls from whichever
/// guest or native thread previously occupied the CPU.
const fn native_hcr_base() -> u64 {
    registers::HCR_EL2_BOOT_VALUE
        | registers::HCR_EL2_TWI
        | registers::HCR_EL2_TWE
        | registers::HCR_EL2_TID0
        | registers::HCR_EL2_TID1
        | registers::HCR_EL2_TID2
        | registers::HCR_EL2_TID3
        | registers::HCR_EL2_TSC
        | registers::HCR_EL2_TIDCP
        | registers::HCR_EL2_TSW
        | registers::HCR_EL2_TPCP
        | registers::HCR_EL2_TPU
        | registers::HCR_EL2_TTLB
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserMachineContractError {
    HostModeMismatch,
    InvalidGeneration,
    InvalidPermissions,
    InvalidRootAlignment,
    InvalidTranslationIdentifier,
    RootOutsidePhysicalAddressSpace,
    TranslationRegimeMismatch,
    UnsupportedAddressWidth,
    UnsupportedIdentifierWidth,
    UnsupportedPhysicalAddressWidth,
    UnsupportedPrivilegedAccessProtection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UserPagePermissions {
    readable: bool,
    writable: bool,
    executable: bool,
}

impl UserPagePermissions {
    pub(super) const fn new(
        readable: bool,
        writable: bool,
        executable: bool,
    ) -> Result<Self, UserMachineContractError> {
        if (writable || executable) && !readable || (writable && executable) {
            return Err(UserMachineContractError::InvalidPermissions);
        }
        Ok(Self {
            readable,
            writable,
            executable,
        })
    }

    pub(super) const fn vhe_stage1_descriptor(self, physical: u64) -> u64 {
        if !self.readable {
            return registers::STAGE1_DESC_INVALID;
        }
        let mut descriptor = (physical & registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT)
            | registers::STAGE1_DESC_TABLE_OR_PAGE
            | registers::STAGE1_DESC_ATTR_NORMAL
            | registers::STAGE1_DESC_INNER_SHAREABLE
            | registers::STAGE1_DESC_AP_EL0
            | registers::STAGE1_DESC_ACCESS_FLAG
            | registers::STAGE1_DESC_NOT_GLOBAL
            | registers::STAGE1_DESC_PXN;
        if !self.writable {
            descriptor |= registers::STAGE1_DESC_AP_READ_ONLY;
        }
        if !self.executable {
            descriptor |= registers::STAGE1_DESC_UXN;
        }
        descriptor
    }

    pub(super) const fn nvhe_stage2_descriptor(self, physical: u64) -> u64 {
        if !self.readable {
            return registers::STAGE2_DESC_INVALID;
        }
        let mut descriptor = (physical & registers::TRANSLATION_DESC_ADDRESS_MASK_48BIT)
            | registers::STAGE2_DESC_TABLE_OR_PAGE
            | registers::STAGE2_DESC_MEMATTR_NORMAL_WB
            | registers::STAGE2_DESC_INNER_SHAREABLE
            | registers::STAGE2_DESC_ACCESS_FLAG
            | if self.writable {
                registers::STAGE2_DESC_READ_WRITE
            } else {
                registers::STAGE2_DESC_READ_ONLY
            };
        if !self.executable {
            descriptor |= registers::STAGE2_DESC_XN;
        }
        descriptor
    }
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
