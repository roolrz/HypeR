// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Volume attachment and lifecycle contracts for Native filesystem services.

use crate::StartupContract;
use hyper_os::handle::{ByteChannelObject, CapabilityChannelObject, NativeBlockObject, Rights};
use hyper_os::startup::StartupPurpose;

/// Init connects the block provider to the filesystem manager exactly once.
pub const ATTACH: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8007_0001);
pub const PROVIDER: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8007_0002);
pub const ATTACH_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.attach",
    ATTACH,
    Rights::WAIT.union(Rights::READ),
);
pub const PROVIDER_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.provider",
    PROVIDER,
    Rights::WAIT.union(Rights::WRITE),
);
pub const MANAGER_STARTUP_CONTRACTS: &[StartupContract] = &[ATTACH_CONTRACT];

/// Only the selected worker receives volume authority. The owner pipe makes
/// manager termination observable without a syscall on each filesystem read.
pub const BLOCK: StartupPurpose<NativeBlockObject> = StartupPurpose::new(0x8007_0003);
pub const READY: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8007_0004);
pub const OWNER: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8007_0005);

pub const ATTACH_BYTES: usize = 24;
pub const READY_MESSAGE: &[u8] = b"HYPER-FS-READY/1\n";

/// The sector count is informational; block syscalls validate kernel geometry.
/// Handle slots are, in order, the sole block owner and readiness writer.
pub fn encode_attach(volume: hyper_os::block::VolumeInfo) -> Option<[u8; ATTACH_BYTES]> {
    if volume.sectors == 0 {
        return None;
    }
    let mut bytes = [0; ATTACH_BYTES];
    bytes[..8].copy_from_slice(b"HFSVOL01");
    bytes[8..16].copy_from_slice(&volume.sectors.to_le_bytes());
    bytes[16..24].copy_from_slice(&u64::from(volume.read_only).to_le_bytes());
    Some(bytes)
}

pub fn decode_attach(bytes: &[u8]) -> Option<hyper_os::block::VolumeInfo> {
    if bytes.len() != ATTACH_BYTES || &bytes[..8] != b"HFSVOL01" {
        return None;
    }
    let sectors = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
    let readonly = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
    (sectors != 0 && readonly <= 1).then_some(hyper_os::block::VolumeInfo {
        sectors,
        read_only: readonly != 0,
    })
}

#[cfg(test)]
mod tests {
    use hyper_os::block::VolumeInfo;
    #[test]
    fn attach_rejects_invalid_geometry_and_record_shape() -> Result<(), &'static str> {
        assert_eq!(
            super::encode_attach(VolumeInfo {
                sectors: 0,
                read_only: false
            }),
            None
        );
        let mut bytes = super::encode_attach(VolumeInfo {
            sectors: 65536,
            read_only: true,
        })
        .ok_or("nonzero volume must encode")?;
        assert_eq!(
            super::decode_attach(&bytes),
            Some(VolumeInfo {
                sectors: 65536,
                read_only: true
            })
        );
        assert_eq!(super::decode_attach(&bytes[..15]), None);
        bytes[0] = 0;
        assert_eq!(super::decode_attach(&bytes), None);
        Ok(())
    }
}
