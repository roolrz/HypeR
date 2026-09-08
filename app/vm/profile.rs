// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest platform profiles implemented by the per-VM runtime.

use hyper_vm_image::{Architecture, PlatformProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectedProfile {
    Aarch64Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionError {
    Architecture,
    PlatformProfile,
    VirtualCpuCount,
}

/// Selects an implemented loader only after the generic image parser has
/// established a well-formed architecture and platform identity.
pub(crate) const fn select(
    architecture: Architecture,
    platform_profile: PlatformProfile,
    vcpu_count: u32,
) -> Result<SelectedProfile, SelectionError> {
    if !matches!(architecture, Architecture::Aarch64) {
        return Err(SelectionError::Architecture);
    }
    if !matches!(platform_profile, PlatformProfile::Aarch64Reference) {
        return Err(SelectionError::PlatformProfile);
    }
    if vcpu_count != 1 {
        return Err(SelectionError::VirtualCpuCount);
    }
    Ok(SelectedProfile::Aarch64Reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_distinguishes_implemented_and_unsupported_profiles() {
        assert_eq!(
            select(Architecture::Aarch64, PlatformProfile::Aarch64Reference, 1),
            Ok(SelectedProfile::Aarch64Reference)
        );
        assert_eq!(
            select(Architecture::Riscv64, PlatformProfile::Riscv64Reference, 1),
            Err(SelectionError::Architecture)
        );
        assert_eq!(
            select(Architecture::Aarch64, PlatformProfile::X86_64Reference, 1),
            Err(SelectionError::PlatformProfile)
        );
        assert_eq!(
            select(Architecture::Aarch64, PlatformProfile::Aarch64Reference, 2),
            Err(SelectionError::VirtualCpuCount)
        );
    }
}
