// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Pure validation and semantic decoding of guest `GICv3` accesses.

use crate::vm::exit::{AccessWidth, GuestPhysicalAddress};

use super::{DISTRIBUTOR_BASE, DISTRIBUTOR_SIZE, REDISTRIBUTOR_BASE, REDISTRIBUTOR_SIZE};

const FRAME_SIZE: u64 = 0x0001_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frame {
    Distributor,
    RedistributorControl,
    RedistributorSgi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitmapRegister {
    Group,
    SetEnable,
    ClearEnable,
    SetPending,
    ClearPending,
    SetActive,
    ClearActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SingleVcpuRoute {
    interrupt: u32,
}

impl SingleVcpuRoute {
    pub const fn interrupt(self) -> u32 {
        self.interrupt
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceRegister {
    DistributorControl,
    DistributorType,
    DistributorType2,
    DistributorImplementer,
    DistributorStatus,
    RedistributorControl,
    RedistributorImplementer,
    RedistributorType,
    RedistributorStatus,
    RedistributorWake,
    PeripheralId2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelRegisterDescriptor {
    Bitmap {
        register: BitmapRegister,
        first_interrupt: u32,
    },
    Priority {
        first_interrupt: u32,
        count: u8,
    },
    Configuration {
        first_interrupt: u32,
    },
    Route(SingleVcpuRoute),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelRegister {
    pub(super) kind: ModelRegisterKind,
}

impl ModelRegister {
    /// Returns a non-authoritative description suitable for diagnostics/tests.
    ///
    /// A descriptor cannot be converted back into the opaque validated token.
    pub const fn descriptor(self) -> ModelRegisterDescriptor {
        match self.kind {
            ModelRegisterKind::Bitmap {
                register,
                first_interrupt,
            } => ModelRegisterDescriptor::Bitmap {
                register,
                first_interrupt,
            },
            ModelRegisterKind::Priority {
                first_interrupt,
                count,
            } => ModelRegisterDescriptor::Priority {
                first_interrupt,
                count,
            },
            ModelRegisterKind::Configuration { first_interrupt } => {
                ModelRegisterDescriptor::Configuration { first_interrupt }
            }
            ModelRegisterKind::Route(route) => ModelRegisterDescriptor::Route(route),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModelRegisterKind {
    Bitmap {
        register: BitmapRegister,
        first_interrupt: u32,
    },
    Priority {
        first_interrupt: u32,
        count: u8,
    },
    Configuration {
        first_interrupt: u32,
    },
    Route(SingleVcpuRoute),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedRegister {
    Service(ServiceRegister),
    Model(ModelRegister),
    Reserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    CrossesFrame,
    InvalidRegisterAccess,
}

/// One complete access contained by a single guest `GICv3` frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedAccess {
    frame: Frame,
    register: DecodedRegister,
}

impl DecodedAccess {
    /// Identifies the owning frame for diagnostics and frame-local policy.
    pub const fn frame(self) -> Frame {
        self.frame
    }

    pub const fn register(self) -> DecodedRegister {
        self.register
    }
}

/// Decodes an access whose first byte might belong to the reference `GICv3`.
///
/// `Ok(None)` means the first byte is outside every GIC frame. Once the first
/// byte is owned, overflow, a frame-boundary crossing, or an invalid access to
/// a modeled register is reported instead of being forwarded to another
/// device. Reserved space remains a decoded RAZ/WI register.
pub fn decode_access(
    address: GuestPhysicalAddress,
    width: AccessWidth,
) -> Result<Option<DecodedAccess>, DecodeError> {
    let address = address.get();
    let bytes = width.bytes() as u64;
    let distributor_base = u64::from(DISTRIBUTOR_BASE);
    let distributor_end = distributor_base
        .checked_add(u64::from(DISTRIBUTOR_SIZE))
        .ok_or(DecodeError::CrossesFrame)?;
    if (distributor_base..distributor_end).contains(&address) {
        let end = address
            .checked_add(bytes)
            .ok_or(DecodeError::CrossesFrame)?;
        if end > distributor_end {
            return Err(DecodeError::CrossesFrame);
        }
        return decode_frame(Frame::Distributor, address - distributor_base, width).map(Some);
    }

    let redistributor_base = u64::from(REDISTRIBUTOR_BASE);
    let redistributor_end = redistributor_base
        .checked_add(u64::from(REDISTRIBUTOR_SIZE))
        .ok_or(DecodeError::CrossesFrame)?;
    if !(redistributor_base..redistributor_end).contains(&address) {
        return Ok(None);
    }
    let end = address
        .checked_add(bytes)
        .ok_or(DecodeError::CrossesFrame)?;
    let sgi_base = redistributor_base
        .checked_add(FRAME_SIZE)
        .ok_or(DecodeError::CrossesFrame)?;
    let (frame, base, frame_end) = if address < sgi_base {
        (Frame::RedistributorControl, redistributor_base, sgi_base)
    } else {
        (Frame::RedistributorSgi, sgi_base, redistributor_end)
    };
    if end > frame_end {
        return Err(DecodeError::CrossesFrame);
    }
    decode_frame(frame, address - base, width).map(Some)
}

fn decode_frame(
    frame: Frame,
    offset: u64,
    width: AccessWidth,
) -> Result<DecodedAccess, DecodeError> {
    let offset = u32::try_from(offset).map_err(|_| DecodeError::CrossesFrame)?;
    let register = match frame {
        Frame::Distributor => decode_distributor(offset, width)?,
        Frame::RedistributorControl => decode_redistributor_control(offset, width)?,
        Frame::RedistributorSgi => decode_redistributor_sgi(offset, width)?,
    };
    Ok(DecodedAccess { frame, register })
}

fn decode_distributor(offset: u32, width: AccessWidth) -> Result<DecodedRegister, DecodeError> {
    for (base, register) in [
        (0x0000, ServiceRegister::DistributorControl),
        (0x0004, ServiceRegister::DistributorType),
        (0x0008, ServiceRegister::DistributorImplementer),
        (0x000c, ServiceRegister::DistributorType2),
        (0x0010, ServiceRegister::DistributorStatus),
        (0xffe8, ServiceRegister::PeripheralId2),
    ] {
        if let Some(result) = fixed(offset, width, base, AccessWidth::Word, register) {
            return result;
        }
    }
    for (base, register) in [
        (0x0084, BitmapRegister::Group),
        (0x0104, BitmapRegister::SetEnable),
        (0x0184, BitmapRegister::ClearEnable),
        (0x0204, BitmapRegister::SetPending),
        (0x0284, BitmapRegister::ClearPending),
        (0x0304, BitmapRegister::SetActive),
        (0x0384, BitmapRegister::ClearActive),
    ] {
        if let Some(result) = bitmap(offset, width, base, 32, register) {
            return result;
        }
    }
    if let Some(result) = priority(offset, width, 0x0420, 32) {
        return result;
    }
    if let Some(result) = configuration(offset, width, 0x0c08, 32) {
        return result;
    }
    if let Some(result) = route(offset, width, 0x6100, 32) {
        return result;
    }
    Ok(DecodedRegister::Reserved)
}

fn decode_redistributor_control(
    offset: u32,
    width: AccessWidth,
) -> Result<DecodedRegister, DecodeError> {
    for (base, required, register) in [
        (
            0x0000,
            AccessWidth::Word,
            ServiceRegister::RedistributorControl,
        ),
        (
            0x0004,
            AccessWidth::Word,
            ServiceRegister::RedistributorImplementer,
        ),
        (
            0x0008,
            AccessWidth::DoubleWord,
            ServiceRegister::RedistributorType,
        ),
        (
            0x0010,
            AccessWidth::Word,
            ServiceRegister::RedistributorStatus,
        ),
        (
            0x0014,
            AccessWidth::Word,
            ServiceRegister::RedistributorWake,
        ),
        (0xffe8, AccessWidth::Word, ServiceRegister::PeripheralId2),
    ] {
        if let Some(result) = fixed(offset, width, base, required, register) {
            return result;
        }
    }
    Ok(DecodedRegister::Reserved)
}

fn decode_redistributor_sgi(
    offset: u32,
    width: AccessWidth,
) -> Result<DecodedRegister, DecodeError> {
    for (base, register) in [
        (0x0080, BitmapRegister::Group),
        (0x0100, BitmapRegister::SetEnable),
        (0x0180, BitmapRegister::ClearEnable),
        (0x0200, BitmapRegister::SetPending),
        (0x0280, BitmapRegister::ClearPending),
        (0x0300, BitmapRegister::SetActive),
        (0x0380, BitmapRegister::ClearActive),
    ] {
        if let Some(result) = bitmap(offset, width, base, 0, register) {
            return result;
        }
    }
    if let Some(result) = priority(offset, width, 0x0400, 0) {
        return result;
    }
    if let Some(result) = configuration(offset, width, 0x0c00, 0) {
        return result;
    }
    Ok(DecodedRegister::Reserved)
}

fn fixed(
    offset: u32,
    width: AccessWidth,
    base: u32,
    required: AccessWidth,
    register: ServiceRegister,
) -> Option<Result<DecodedRegister, DecodeError>> {
    if !overlaps(offset, width, base, required.bytes() as u32) {
        return None;
    }
    Some(if offset == base && width == required {
        Ok(DecodedRegister::Service(register))
    } else {
        Err(DecodeError::InvalidRegisterAccess)
    })
}

fn bitmap(
    offset: u32,
    width: AccessWidth,
    base: u32,
    first_interrupt: u32,
    register: BitmapRegister,
) -> Option<Result<DecodedRegister, DecodeError>> {
    model_word(
        offset,
        width,
        base,
        ModelRegister {
            kind: ModelRegisterKind::Bitmap {
                register,
                first_interrupt,
            },
        },
    )
}

fn model_word(
    offset: u32,
    width: AccessWidth,
    base: u32,
    register: ModelRegister,
) -> Option<Result<DecodedRegister, DecodeError>> {
    if !overlaps(offset, width, base, AccessWidth::Word.bytes() as u32) {
        return None;
    }
    Some(if offset == base && width == AccessWidth::Word {
        Ok(DecodedRegister::Model(register))
    } else {
        Err(DecodeError::InvalidRegisterAccess)
    })
}

fn priority(
    offset: u32,
    width: AccessWidth,
    base: u32,
    first_interrupt: u32,
) -> Option<Result<DecodedRegister, DecodeError>> {
    const PRIORITY_BYTES: u32 = 32;
    if !overlaps(offset, width, base, PRIORITY_BYTES) {
        return None;
    }
    let bytes = width.bytes() as u32;
    let Some(relative) = offset.checked_sub(base) else {
        return Some(Err(DecodeError::InvalidRegisterAccess));
    };
    let complete = relative
        .checked_add(bytes)
        .is_some_and(|end| end <= PRIORITY_BYTES);
    let valid = match width {
        AccessWidth::Byte => complete,
        AccessWidth::Word => complete && relative % 4 == 0,
        AccessWidth::HalfWord | AccessWidth::DoubleWord => false,
    };
    let first_lane = first_interrupt.checked_add(relative);
    Some(if valid && first_lane.is_some() {
        let Some(first_interrupt) = first_lane else {
            return Some(Err(DecodeError::InvalidRegisterAccess));
        };
        Ok(DecodedRegister::Model(ModelRegister {
            kind: ModelRegisterKind::Priority {
                first_interrupt,
                count: bytes as u8,
            },
        }))
    } else {
        Err(DecodeError::InvalidRegisterAccess)
    })
}

fn configuration(
    offset: u32,
    width: AccessWidth,
    base: u32,
    first_interrupt: u32,
) -> Option<Result<DecodedRegister, DecodeError>> {
    const CONFIGURATION_BYTES: u32 = 8;
    if !overlaps(offset, width, base, CONFIGURATION_BYTES) {
        return None;
    }
    let Some(relative) = offset.checked_sub(base) else {
        return Some(Err(DecodeError::InvalidRegisterAccess));
    };
    let valid = width == AccessWidth::Word && relative % 4 == 0 && relative < CONFIGURATION_BYTES;
    let lane_offset = (relative / 4).checked_mul(16);
    let first_lane = lane_offset.and_then(|offset| first_interrupt.checked_add(offset));
    Some(if valid && first_lane.is_some() {
        let Some(first_interrupt) = first_lane else {
            return Some(Err(DecodeError::InvalidRegisterAccess));
        };
        Ok(DecodedRegister::Model(ModelRegister {
            kind: ModelRegisterKind::Configuration { first_interrupt },
        }))
    } else {
        Err(DecodeError::InvalidRegisterAccess)
    })
}

fn route(
    offset: u32,
    width: AccessWidth,
    base: u32,
    first_interrupt: u32,
) -> Option<Result<DecodedRegister, DecodeError>> {
    const ROUTE_BYTES: u32 = 32 * 8;
    if !overlaps(offset, width, base, ROUTE_BYTES) {
        return None;
    }
    let Some(relative) = offset.checked_sub(base) else {
        return Some(Err(DecodeError::InvalidRegisterAccess));
    };
    // Partial writes require retained 64-bit route state. The current device
    // contract instead exposes one complete route token targeting vCPU 0.
    let valid = width == AccessWidth::DoubleWord && relative % 8 == 0;
    let interrupt = first_interrupt.checked_add(relative / 8);
    Some(if valid && interrupt.is_some() {
        let Some(interrupt) = interrupt else {
            return Some(Err(DecodeError::InvalidRegisterAccess));
        };
        Ok(DecodedRegister::Model(ModelRegister {
            kind: ModelRegisterKind::Route(SingleVcpuRoute { interrupt }),
        }))
    } else {
        Err(DecodeError::InvalidRegisterAccess)
    })
}

fn overlaps(offset: u32, width: AccessWidth, base: u32, bytes: u32) -> bool {
    let end = match offset.checked_add(width.bytes() as u32) {
        Some(end) => end,
        None => return true,
    };
    let register_end = match base.checked_add(bytes) {
        Some(end) => end,
        None => return true,
    };
    offset < register_end && end > base
}
