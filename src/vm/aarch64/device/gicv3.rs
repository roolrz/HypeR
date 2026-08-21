// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest-visible placement of the `GICv3` on the reference virtual board.

pub const DISTRIBUTOR_BASE: u32 = 0x0800_0000;
pub const DISTRIBUTOR_SIZE: u32 = 0x0001_0000;
pub const REDISTRIBUTOR_BASE: u32 = 0x080a_0000;
pub const REDISTRIBUTOR_SIZE: u32 = 0x0002_0000;

/// Linux `reg` cells for the distributor and one Redistributor region.
pub const REFERENCE_REG_CELLS: [u32; 8] = [
    0,
    DISTRIBUTOR_BASE,
    0,
    DISTRIBUTOR_SIZE,
    0,
    REDISTRIBUTOR_BASE,
    0,
    REDISTRIBUTOR_SIZE,
];
