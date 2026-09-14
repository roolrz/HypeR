// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{FirmwareNode, MmioResource};
use hyper_os::device::{self, FirmwareField, IrqTrigger};
use hyper_os::handle::{DeviceAssignmentAuthorityObject, HandleRef};
use hyper_os::{Error, Result, Status};

// Bound untrusted firmware-derived allocation and work; this is discovery,
// never the I/O hot path. Snapshot reads do not claim any resource.
const MAX_NODES: u32 = 4096;
const MAX_FIELD: usize = 65536;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const PROPERTIES: &[&str] = &[
    "#clock-cells",
    "#gpio-cells",
    "bias-pull-up",
    "brcm,gpio-bank-widths",
    "bus-width",
    "cd-gpios",
    "clock-frequency",
    "clock-names",
    "clocks",
    "dma-coherent",
    "enable-active-high",
    "function",
    "gpio-controller",
    "gpios",
    "interrupt-controller",
    "interrupts",
    "interrupts-extended",
    "iommus",
    "mmc-ddr-3_3v",
    "phandle",
    "phys",
    "pinctrl-0",
    "pinctrl-names",
    "pins",
    "power-domains",
    "reg-names",
    "regulator-always-on",
    "regulator-boot-on",
    "regulator-max-microvolt",
    "regulator-min-microvolt",
    "regulator-settling-time-us",
    "resets",
    "sd-uhs-ddr50",
    "sd-uhs-sdr104",
    "sd-uhs-sdr50",
    "states",
    "vmmc-supply",
    "vqmmc-supply",
];

fn field(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
    node: u32,
    kind: FirmwareField,
    name: &str,
    bytes: &mut usize,
) -> Result<Vec<u8>> {
    let size = device::firmware_read(authority, node, kind, name, &mut [])?;
    if size > MAX_FIELD || size > MAX_BYTES.saturating_sub(*bytes) {
        return Err(Error::Status(Status::RESOURCE_LIMIT));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(size)
        .map_err(|_| Error::Status(Status::NO_MEMORY))?;
    output.resize(size, 0);
    if size != 0 && device::firmware_read(authority, node, kind, name, &mut output)? != size {
        return Err(Error::InvalidResponse);
    }
    *bytes += size;
    Ok(output)
}

pub(super) fn read(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
) -> Result<Vec<FirmwareNode>> {
    let mut nodes = Vec::new();
    let mut bytes = 0;
    for id in 0..=MAX_NODES {
        let info = match device::firmware_info(authority, id) {
            Ok(info) => info,
            Err(Error::Status(Status::NOT_FOUND)) => return Ok(nodes),
            Err(error) => return Err(error),
        };
        if id == MAX_NODES {
            return Err(Error::Status(Status::RESOURCE_LIMIT));
        }
        let path = field(authority, id, FirmwareField::Path, "", &mut bytes)?;
        let path = String::from_utf8(path).map_err(|_| Error::InvalidResponse)?;
        let compatible = field(authority, id, FirmwareField::Compatible, "", &mut bytes)?;
        let compatible = if compatible.is_empty() {
            Vec::new()
        } else {
            let text = core::str::from_utf8(&compatible).map_err(|_| Error::InvalidResponse)?;
            let text = text.strip_suffix('\0').ok_or(Error::InvalidResponse)?;
            text.split('\0').map(String::from).collect()
        };
        let registers = field(authority, id, FirmwareField::Registers, "", &mut bytes)?;
        if registers.len() != info.register_count as usize * 16 {
            return Err(Error::InvalidResponse);
        }
        let registers = registers
            .chunks_exact(16)
            .map(|record| {
                Ok(MmioResource {
                    base: u64::from_le_bytes(
                        record[..8].try_into().map_err(|_| Error::InvalidResponse)?,
                    ),
                    length: u64::from_le_bytes(
                        record[8..].try_into().map_err(|_| Error::InvalidResponse)?,
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut properties = Vec::new();
        properties
            .try_reserve_exact(PROPERTIES.len())
            .map_err(|_| Error::Status(Status::NO_MEMORY))?;
        for name in PROPERTIES {
            match field(authority, id, FirmwareField::Property, name, &mut bytes) {
                Ok(value) => properties.push((String::from(*name), value)),
                Err(Error::Status(Status::NOT_FOUND)) => {}
                Err(error) => return Err(error),
            }
        }
        nodes
            .try_reserve(1)
            .map_err(|_| Error::Status(Status::NO_MEMORY))?;
        nodes.push(FirmwareNode {
            id,
            path,
            compatible,
            registers,
            properties,
            kernel_owned: info.kernel_owned,
            interrupt: info
                .interrupt
                .map(|irq| (irq.number, irq.trigger == IrqTrigger::Level)),
        });
    }
    Err(Error::Status(Status::RESOURCE_LIMIT))
}
