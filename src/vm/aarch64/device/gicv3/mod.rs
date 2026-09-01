// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest-visible `GICv3` placement, decoding, and reusable register state.

mod decode;
mod model;

pub use decode::{
    BitmapRegister, DecodeError, DecodedAccess, DecodedRegister, Frame, ModelRegister,
    ModelRegisterDescriptor, ServiceRegister, SingleVcpuRoute, decode_access,
};
pub use model::{ModelError, RegisterState, read_model_register, write_model_register};

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
