// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host-mode-specific `AArch64` virtual-address geometry.
//!
//! VHE gives the EL2&0 regime independent lower and upper ranges. Native
//! process mappings use the lower range through `TTBR0_EL2`; permanent kernel
//! mappings use the canonical upper range through `TTBR1_EL2`. nVHE has only
//! the lower EL2 range, so the same PIE image selects the equivalent low
//! offsets at boot while native EL0 remains isolated by stage 2.

use hyper::hal::memory::{AddressTranslation, VirtualMemoryLayout};
use hyper::mm::{PhysicalAddress, VirtualAddress};

const ADDRESS_SPACE_SIZE: u64 = super::super::address::STAGE1_VA_LIMIT;
const UPPER_REGION_BASE: u64 = 0u64.wrapping_sub(ADDRESS_SPACE_SIZE);
const KERNEL_REGION_SIZE: u64 = 1 << 40;
const STAGE1_L0_SPAN: u64 = 1 << 39;
const MMIO_OFFSET: u64 = if ADDRESS_SPACE_SIZE >> 4 < STAGE1_L0_SPAN {
    STAGE1_L0_SPAN
} else {
    ADDRESS_SPACE_SIZE >> 4
};
const LINEAR_OFFSET: u64 = ADDRESS_SPACE_SIZE >> 2;
const KERNEL_OFFSET: u64 = ADDRESS_SPACE_SIZE - KERNEL_REGION_SIZE;
const KERNEL_STACK_OFFSET: u64 = KERNEL_OFFSET + super::super::kaslr::WINDOW_SIZE;
const KERNEL_STACK_ARENA_OFFSET: u64 = KERNEL_STACK_OFFSET + 2 * 1024 * 1024;
const BOOTSTRAP_ACCESSIBLE_LIMIT: u64 = 0x1_0000_0000;

const _: () = {
    assert!(MMIO_OFFSET < LINEAR_OFFSET);
    assert!(MMIO_OFFSET.is_multiple_of(STAGE1_L0_SPAN));
    assert!(LINEAR_OFFSET < KERNEL_OFFSET);
    assert!(KERNEL_STACK_ARENA_OFFSET < ADDRESS_SPACE_SIZE);
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RootRegion {
    Lower,
    Upper,
}

/// Complete host-owned geometry selected before permanent table construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HostLayout {
    region: RootRegion,
    pub(super) mmio_base: u64,
    pub(super) linear_base: u64,
    pub(super) kernel_base: u64,
    pub(super) kernel_stack_base: u64,
    pub(super) kernel_stack_arena_base: u64,
}

impl HostLayout {
    const fn new(region: RootRegion) -> Self {
        let base = match region {
            RootRegion::Lower => 0,
            RootRegion::Upper => UPPER_REGION_BASE,
        };
        Self {
            region,
            mmio_base: base + MMIO_OFFSET,
            linear_base: base + LINEAR_OFFSET,
            kernel_base: base + KERNEL_OFFSET,
            kernel_stack_base: base + KERNEL_STACK_OFFSET,
            kernel_stack_arena_base: base + KERNEL_STACK_ARENA_OFFSET,
        }
    }

    pub(super) const fn root_region(self) -> RootRegion {
        self.region
    }

    pub(super) const fn contains(self, address: u64) -> bool {
        match self.region {
            RootRegion::Lower => address < ADDRESS_SPACE_SIZE,
            RootRegion::Upper => address >= UPPER_REGION_BASE,
        }
    }
}

pub(super) fn selected() -> HostLayout {
    HostLayout::new(if super::super::host::is_vhe() {
        RootRegion::Upper
    } else {
        RootRegion::Lower
    })
}

pub struct Aarch64AddressTranslation;

impl AddressTranslation for Aarch64AddressTranslation {
    fn bootstrap_accessible_limit() -> u64 {
        BOOTSTRAP_ACCESSIBLE_LIMIT
    }

    fn layout() -> VirtualMemoryLayout {
        let layout = selected();
        VirtualMemoryLayout {
            mmio_base: layout.mmio_base,
            linear_base: layout.linear_base,
            kernel_base: layout.kernel_base,
        }
    }

    fn linear_address(physical: PhysicalAddress) -> Option<VirtualAddress> {
        let layout = selected();
        translated_address(layout.linear_base, layout.kernel_base, physical)
    }

    fn mmio_address(physical: PhysicalAddress) -> Option<VirtualAddress> {
        let layout = selected();
        translated_address(layout.mmio_base, layout.linear_base, physical)
    }
}

fn translated_address(base: u64, end: u64, physical: PhysicalAddress) -> Option<VirtualAddress> {
    base.checked_add(physical.get())
        .filter(|address| *address < end)
        .map(VirtualAddress::new)
}
