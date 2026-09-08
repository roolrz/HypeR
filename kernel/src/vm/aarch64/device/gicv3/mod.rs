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

pub const DISTRIBUTOR_BASE: u32 =
    hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_BASE as u32;
pub const DISTRIBUTOR_SIZE: u32 =
    hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_SIZE as u32;
pub const REDISTRIBUTOR_BASE: u32 =
    hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_BASE as u32;
pub const REDISTRIBUTOR_SIZE: u32 =
    hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_SIZE as u32;

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
