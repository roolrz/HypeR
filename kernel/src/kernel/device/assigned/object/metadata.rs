// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::super::{Profile, Window, service};
use super::{Error, PhysicalDevice};
use hyper::drivers::pci;

impl PhysicalDevice {
    pub(crate) fn info(&self) -> service::Info {
        service::Info {
            device_id: match self.claim.hardware.profile {
                Profile::Virtio(kind) => kind.device_id(),
                _ => 0,
            },
            transport_version: if matches!(self.claim.hardware.profile, Profile::Virtio(_)) {
                2
            } else {
                0
            },
            mmio_size: self.claim.hardware.mapping.resource().size(),
        }
    }

    pub(crate) fn profile_info(&self) -> [u8; 32] {
        let hw = &self.claim.hardware;
        let (count, identity, dma) = match &hw.profile {
            Profile::Pci(transport) => (
                2 + transport.bars().iter().flatten().count() as u32,
                transport.identity(),
                transport.dma_offset(),
            ),
            _ => (1 + hw.extra.iter().flatten().count() as u32, 0, 0),
        };
        let mut output = [0; 32];
        output[0..4].copy_from_slice(&hw.profile.id().to_le_bytes());
        output[4..8].copy_from_slice(&hw.interrupt_count.to_le_bytes());
        output[8..12].copy_from_slice(&count.to_le_bytes());
        output[12..16].copy_from_slice(&identity.to_le_bytes());
        output[16..24].copy_from_slice(&dma.to_le_bytes());
        output[24..32].copy_from_slice(&hw.profile.aperture().to_le_bytes());
        output
    }

    pub(crate) fn resource_info(&self, index: u32) -> Result<[u8; 32], Error> {
        if let Profile::Pci(transport) = &self.claim.hardware.profile {
            let (kind, flags, offset, length, bus) = match index {
                0 => (0x100, 0, 0, pci::ECAM_SIZE, 0),
                1 => (0x101, 0, pci::MSI_FRAME_OFFSET, 4096, 0),
                _ => {
                    let bar = transport
                        .bars()
                        .iter()
                        .flatten()
                        .nth(index as usize - 2)
                        .ok_or(Error::InvalidArgument)?;
                    (
                        0x200 + bar.index,
                        bar.flags,
                        bar.offset,
                        bar.mapping.resource().size(),
                        bar.offset,
                    )
                }
            };
            return Ok(resource(kind, flags, offset, length, bus));
        }
        let window = if index == 0 {
            Window {
                mapping: self.claim.hardware.mapping,
                offset: 0,
            }
        } else {
            self.claim
                .hardware
                .extra
                .get(index as usize - 1)
                .copied()
                .flatten()
                .ok_or(Error::InvalidArgument)?
        };
        Ok(resource(
            index + 1,
            0,
            window.offset as u64,
            window.mapping.resource().size(),
            0,
        ))
    }
}

fn resource(kind: u32, flags: u32, offset: u64, length: u64, bus: u64) -> [u8; 32] {
    let mut output = [0; 32];
    output[0..4].copy_from_slice(&kind.to_le_bytes());
    output[4..8].copy_from_slice(&flags.to_le_bytes());
    output[8..16].copy_from_slice(&offset.to_le_bytes());
    output[16..24].copy_from_slice(&length.to_le_bytes());
    output[24..32].copy_from_slice(&bus.to_le_bytes());
    output
}
