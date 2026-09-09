// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

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
