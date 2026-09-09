// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest platform profiles implemented by the per-VM runtime.

use hyper_vm_image::{Architecture, PlatformProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedProfile {
    Aarch64Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionError {
    Architecture,
    PlatformProfile,
    VirtualCpuCount,
}

/// Selects an implemented loader only after the generic image parser has
/// established a well-formed architecture and platform identity.
pub const fn select(
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
#[path = "../tests/profile.rs"]
mod tests;
