// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

extern crate std;
use super::{DmaRange, IoDevices, MmioDevice, SharedMemory};
use crate::guest_fdt::{Aarch64LinuxBoot, Error, build_aarch64_linux_with_io};
use std::{collections::BTreeMap, string::String, vec::Vec};

const MIB: u64 = 1024 * 1024;
const DMA: [DmaRange; 2] = [
    DmaRange {
        dma_base: 0x1_8000_0000,
        cpu_base: 0x4000_0000,
        size: 64 * MIB,
    },
    DmaRange {
        dma_base: 0x2_9000_0000,
        cpu_base: 0x4400_0000,
        size: 64 * MIB,
    },
];
fn boot() -> Aarch64LinuxBoot<'static> {
    Aarch64LinuxBoot {
        memory_base: 0x4000_0000,
        memory_size: 64 * MIB,
        vcpu_count: 2,
        gic_version: 3,
        initramfs: Some((0x4100_0000, 0x4120_0000)),
        boot_arguments: "console=ttyAMA0 hyper.role=io",
    }
}
fn devices() -> IoDevices<'static> {
    IoDevices {
        virtio: Some(MmioDevice {
            base: 0x0b00_0000,
            size: 0x200,
            irq: 40,
        }),
        dma_ranges: &DMA,
        shared_memory: Some(SharedMemory {
            base: 0x4400_0000,
            size: 64 * MIB,
            guest_base: 0x4000_0000,
        }),
        mailbox: Some(MmioDevice {
            base: 0x0a00_0000,
            size: 4096,
            irq: 41,
        }),
        notification: Some(MmioDevice {
            base: 0x0a00_1000,
            size: 4096,
            irq: 42,
        }),
    }
}
fn build(devices: IoDevices<'_>) -> Result<Vec<u8>, Error> {
    let mut structure = [0; 8192];
    let mut strings = [0; 2048];
    let mut output = [0; 12288];
    let length =
        build_aarch64_linux_with_io(boot(), devices, &mut structure, &mut strings, &mut output)?;
    Ok(output[..length].to_vec())
}
fn word(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("word extent")?
            .try_into()
            .map_err(|_| "word")?,
    ))
}
fn text(bytes: &[u8], offset: usize) -> Result<(&str, usize), &'static str> {
    let tail = bytes.get(offset..).ok_or("string extent")?;
    let end = tail
        .iter()
        .position(|value| *value == 0)
        .ok_or("string terminator")?;
    Ok((
        core::str::from_utf8(&tail[..end]).map_err(|_| "string encoding")?,
        end + 1,
    ))
}
// Parse paths and cell bytes independently of the production encoder.
fn properties(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, &'static str> {
    if word(bytes, 0)? != 0xd00d_feed || word(bytes, 4)? as usize != bytes.len() {
        return Err("header");
    }
    let mut at = word(bytes, 8)? as usize;
    let end = at + word(bytes, 36)? as usize;
    let strings = word(bytes, 12)? as usize;
    let mut path = Vec::<String>::new();
    let mut result = BTreeMap::new();
    while at < end {
        let token = word(bytes, at)?;
        at += 4;
        match token {
            1 => {
                let (name, length) = text(bytes, at)?;
                path.push(String::from(name));
                at = (at + length + 3) & !3;
            }
            2 => {
                path.pop().ok_or("unbalanced node")?;
            }
            3 => {
                let length = word(bytes, at)? as usize;
                let name = word(bytes, at + 4)? as usize;
                at += 8;
                let (name, _) = text(bytes, strings + name)?;
                let key = std::format!("{}/{name}", path.join("/"));
                if result
                    .insert(
                        key,
                        bytes
                            .get(at..at + length)
                            .ok_or("property extent")?
                            .to_vec(),
                    )
                    .is_some()
                {
                    return Err("duplicate");
                }
                at = (at + length + 3) & !3;
            }
            9 if path.is_empty() && at == end => return Ok(result),
            _ => return Err("token"),
        }
    }
    Err("missing end")
}

#[test]
fn physical_tree_maps_host_dma_addresses_and_retains_imported_struct_pages()
-> Result<(), &'static str> {
    let values = properties(&build(devices()).map_err(|_| "build")?)?;
    let value = |path| {
        values
            .get(path)
            .map(Vec::as_slice)
            .ok_or("missing property")
    };
    assert_eq!(
        value("/memory@40000000/reg")?,
        [0x4000_0000u64.to_be_bytes(), (64 * MIB).to_be_bytes()].concat()
    );
    assert_eq!(
        value("/memory@44000000/reg")?,
        value("/reserved-memory/guest-memory@44000000/reg")?
    );
    assert!(!values.contains_key("/reserved-memory/guest-memory@44000000/no-map"));
    assert!(!values.contains_key("/reserved-memory/guest-memory@44000000/reusable"));
    assert_eq!(
        value("/hyper-guest-memory/memory-region")?,
        value("/reserved-memory/guest-memory@44000000/phandle")?
    );
    assert_eq!(
        value("/hyper-guest-memory/hyper,guest-base")?,
        0x4000_0000u64.to_be_bytes()
    );
    let expected: Vec<_> = DMA
        .iter()
        .flat_map(|range| [range.dma_base, range.cpu_base, range.size])
        .flat_map(u64::to_be_bytes)
        .collect();
    assert_eq!(value("/io-bus/dma-ranges")?, expected);
    assert!(values.contains_key("/io-bus/dma-coherent"));
    assert_eq!(
        value("/io-bus/virtio_mmio@b000000/interrupts")?,
        [0u32.to_be_bytes(), 8u32.to_be_bytes(), 4u32.to_be_bytes()].concat()
    );
    assert_eq!(
        value("/guest-mailbox@a000000/compatible")?,
        b"hyper,guest-mailbox-v1\0"
    );
    assert_eq!(
        value("/guest-notification@a001000/compatible")?,
        b"hyper,guest-notification-v1\0"
    );
    Ok(())
}

#[test]
fn frontend_tree_uses_standard_virtio_without_physical_dma_authority() -> Result<(), &'static str> {
    let devices = IoDevices {
        virtio: Some(MmioDevice {
            base: 0x0a00_0000,
            size: 4096,
            irq: 40,
        }),
        ..IoDevices::empty()
    };
    let values = properties(&build(devices).map_err(|_| "build")?)?;
    assert_eq!(
        values
            .get("/virtio_mmio@a000000/compatible")
            .ok_or("virtio")?,
        b"virtio,mmio\0"
    );
    assert!(!values.contains_key("/io-bus/dma-ranges"));
    assert!(!values.contains_key("/reserved-memory/#address-cells"));
    Ok(())
}

#[test]
fn rejects_dma_holes_aliasing_overflow_and_irq_collisions() {
    let mut invalid = devices();
    invalid.dma_ranges = &DMA[..1];
    assert_eq!(build(invalid), Err(Error::InvalidInput));
    let mut overlap = DMA;
    overlap[1].dma_base = overlap[0].dma_base;
    invalid.dma_ranges = &overlap;
    assert_eq!(build(invalid), Err(Error::InvalidInput));
    let mut overlap = DMA;
    overlap[1].cpu_base = overlap[0].cpu_base;
    invalid.dma_ranges = &overlap;
    assert_eq!(build(invalid), Err(Error::InvalidInput));
    let mut overlap = DMA;
    overlap[1].dma_base = !4095u64;
    invalid.dma_ranges = &overlap;
    assert_eq!(build(invalid), Err(Error::AddressOverflow));
    invalid = devices();
    invalid.notification = invalid.mailbox;
    assert_eq!(build(invalid), Err(Error::InvalidInput));
    invalid = devices();
    invalid.mailbox = Some(MmioDevice {
        base: 0x0a00_0000,
        size: 4096,
        irq: 33,
    });
    assert_eq!(build(invalid), Err(Error::InvalidInput));
}

#[test]
fn reserved_alias_cannot_overlap_linux_allocatable_ram() {
    let mut invalid = devices();
    invalid.shared_memory = Some(SharedMemory {
        base: 0x4200_0000,
        size: 64 * MIB,
        guest_base: 0x4000_0000,
    });
    assert_eq!(build(invalid), Err(Error::InvalidInput));
}
