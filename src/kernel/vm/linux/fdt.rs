//! Linux-format device tree for the initial AArch64 virtual platform.

use alloc::{format, vec::Vec};

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_END: u32 = 9;
const HEADER_SIZE: usize = 40;

const GIC_PHANDLE: u32 = 1;
const UART_CLOCK_PHANDLE: u32 = 2;
const APB_CLOCK_PHANDLE: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AddressOverflow,
    NameOffsetOverflow,
}

pub fn build(
    memory_base: u64,
    memory_size: u64,
    initramfs: Option<(u64, u64)>,
    command_line: &str,
    vcpu_count: u32,
) -> Result<Vec<u8>, Error> {
    let mut builder = Builder::new();
    builder.begin_node("")?;
    builder.property_u32("#address-cells", 2)?;
    builder.property_u32("#size-cells", 2)?;
    builder.property_u32("interrupt-parent", GIC_PHANDLE)?;
    builder.property_string("compatible", "hyper,virtual-machine")?;
    builder.property_string("model", "HypeR AArch64 virtual machine")?;

    builder.begin_node("chosen")?;
    builder.property_string("bootargs", command_line)?;
    builder.property_string("stdout-path", "/pl011@9000000")?;
    if let Some((start, end)) = initramfs {
        builder.property_u64_cells("linux,initrd-start", start)?;
        builder.property_u64_cells("linux,initrd-end", end)?;
    }
    builder.end_node()?;

    builder.begin_node("aliases")?;
    builder.property_string("serial0", "/pl011@9000000")?;
    builder.end_node()?;

    builder.begin_node("memory@40000000")?;
    builder.property_string("device_type", "memory")?;
    builder.property_u64_pair("reg", memory_base, memory_size)?;
    builder.end_node()?;

    builder.begin_node("cpus")?;
    builder.property_u32("#address-cells", 2)?;
    builder.property_u32("#size-cells", 0)?;
    for index in 0..vcpu_count {
        builder.begin_node(&format!("cpu@{index:x}"))?;
        builder.property_string("device_type", "cpu")?;
        builder.property_string("compatible", "arm,armv8")?;
        builder.property_string("enable-method", "psci")?;
        builder.property_u64_cells("reg", u64::from(index))?;
        builder.end_node()?;
    }
    builder.end_node()?;

    builder.begin_node("psci")?;
    builder.property_string_list("compatible", &["arm,psci-1.0", "arm,psci-0.2"])?;
    builder.property_string("method", "hvc")?;
    builder.end_node()?;

    builder.begin_node("intc@8000000")?;
    builder.property_empty("interrupt-controller")?;
    builder.property_u32("#interrupt-cells", 3)?;
    builder.property_string("compatible", "arm,gic-v3")?;
    builder.property_u32("phandle", GIC_PHANDLE)?;
    builder.property_cells(
        "reg",
        &[
            0,
            0x0800_0000,
            0,
            0x0001_0000,
            0,
            0x080a_0000,
            0,
            0x0002_0000,
        ],
    )?;
    builder.end_node()?;

    builder.begin_node("timer")?;
    builder.property_string("compatible", "arm,armv8-timer")?;
    builder.property_empty("always-on")?;
    builder.property_cells("interrupts", &[1, 13, 4, 1, 14, 4, 1, 11, 4, 1, 10, 4])?;
    builder.end_node()?;

    fixed_clock(&mut builder, "clock-uart", UART_CLOCK_PHANDLE, 24_000_000)?;
    fixed_clock(&mut builder, "clock-apb", APB_CLOCK_PHANDLE, 24_000_000)?;

    builder.begin_node("pl011@9000000")?;
    builder.property_string_list("compatible", &["arm,pl011", "arm,primecell"])?;
    builder.property_u64_pair("reg", 0x0900_0000, 0x1000)?;
    builder.property_cells("interrupts", &[0, 1, 4])?;
    builder.property_cells("clocks", &[UART_CLOCK_PHANDLE, APB_CLOCK_PHANDLE])?;
    builder.property_string_list("clock-names", &["uartclk", "apb_pclk"])?;
    builder.end_node()?;

    builder.end_node()?;
    builder.finish()
}

fn fixed_clock(
    builder: &mut Builder,
    name: &str,
    phandle: u32,
    frequency: u32,
) -> Result<(), Error> {
    builder.begin_node(name)?;
    builder.property_string("compatible", "fixed-clock")?;
    builder.property_u32("#clock-cells", 0)?;
    builder.property_u32("clock-frequency", frequency)?;
    builder.property_u32("phandle", phandle)?;
    builder.end_node()
}

struct Builder {
    structure: Vec<u8>,
    strings: Vec<u8>,
}

impl Builder {
    const fn new() -> Self {
        Self {
            structure: Vec::new(),
            strings: Vec::new(),
        }
    }

    fn begin_node(&mut self, name: &str) -> Result<(), Error> {
        push_u32(&mut self.structure, FDT_BEGIN_NODE)?;
        push_bytes(&mut self.structure, name.as_bytes())?;
        push_byte(&mut self.structure, 0)?;
        pad(&mut self.structure)
    }

    fn end_node(&mut self) -> Result<(), Error> {
        push_u32(&mut self.structure, FDT_END_NODE)
    }

    fn property(&mut self, name: &str, value: &[u8]) -> Result<(), Error> {
        let name_offset = self.name_offset(name)?;
        push_u32(&mut self.structure, FDT_PROP)?;
        push_u32(
            &mut self.structure,
            u32::try_from(value.len()).map_err(|_| Error::AddressOverflow)?,
        )?;
        push_u32(&mut self.structure, name_offset)?;
        push_bytes(&mut self.structure, value)?;
        pad(&mut self.structure)
    }

    fn property_empty(&mut self, name: &str) -> Result<(), Error> {
        self.property(name, &[])
    }

    fn property_u32(&mut self, name: &str, value: u32) -> Result<(), Error> {
        self.property(name, &value.to_be_bytes())
    }

    fn property_u64_cells(&mut self, name: &str, value: u64) -> Result<(), Error> {
        self.property_cells(name, &[(value >> 32) as u32, value as u32])
    }

    fn property_u64_pair(&mut self, name: &str, first: u64, second: u64) -> Result<(), Error> {
        self.property_cells(
            name,
            &[
                (first >> 32) as u32,
                first as u32,
                (second >> 32) as u32,
                second as u32,
            ],
        )
    }

    fn property_cells(&mut self, name: &str, values: &[u32]) -> Result<(), Error> {
        let bytes = values.len().checked_mul(4).ok_or(Error::AddressOverflow)?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(bytes)
            .map_err(|_| Error::Allocation)?;
        for value in values {
            encoded.extend_from_slice(&value.to_be_bytes());
        }
        self.property(name, &encoded)
    }

    fn property_string(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(value.len() + 1)
            .map_err(|_| Error::Allocation)?;
        encoded.extend_from_slice(value.as_bytes());
        encoded.push(0);
        self.property(name, &encoded)
    }

    fn property_string_list(&mut self, name: &str, values: &[&str]) -> Result<(), Error> {
        let length = values
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len() + 1))
            .ok_or(Error::AddressOverflow)?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(length)
            .map_err(|_| Error::Allocation)?;
        for value in values {
            encoded.extend_from_slice(value.as_bytes());
            encoded.push(0);
        }
        self.property(name, &encoded)
    }

    fn name_offset(&mut self, name: &str) -> Result<u32, Error> {
        let mut offset = 0;
        while offset < self.strings.len() {
            let tail = &self.strings[offset..];
            let length = match tail.iter().position(|byte| *byte == 0) {
                Some(length) => length,
                None => tail.len(),
            };
            if &tail[..length] == name.as_bytes() {
                return u32::try_from(offset).map_err(|_| Error::NameOffsetOverflow);
            }
            offset += length + 1;
        }
        let result = u32::try_from(self.strings.len()).map_err(|_| Error::NameOffsetOverflow)?;
        self.strings
            .try_reserve_exact(name.len() + 1)
            .map_err(|_| Error::Allocation)?;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        Ok(result)
    }

    fn finish(mut self) -> Result<Vec<u8>, Error> {
        push_u32(&mut self.structure, FDT_END)?;
        let reservation_offset = HEADER_SIZE;
        let structure_offset = reservation_offset + 16;
        let strings_offset = structure_offset
            .checked_add(self.structure.len())
            .ok_or(Error::AddressOverflow)?;
        let total_size = strings_offset
            .checked_add(self.strings.len())
            .ok_or(Error::AddressOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(total_size)
            .map_err(|_| Error::Allocation)?;
        for value in [
            FDT_MAGIC,
            u32::try_from(total_size).map_err(|_| Error::AddressOverflow)?,
            u32::try_from(structure_offset).map_err(|_| Error::AddressOverflow)?,
            u32::try_from(strings_offset).map_err(|_| Error::AddressOverflow)?,
            u32::try_from(reservation_offset).map_err(|_| Error::AddressOverflow)?,
            17,
            16,
            0,
            u32::try_from(self.strings.len()).map_err(|_| Error::AddressOverflow)?,
            u32::try_from(self.structure.len()).map_err(|_| Error::AddressOverflow)?,
        ] {
            output.extend_from_slice(&value.to_be_bytes());
        }
        output.extend_from_slice(&[0; 16]);
        output.extend_from_slice(&self.structure);
        output.extend_from_slice(&self.strings);
        Ok(output)
    }
}

fn push_u32(output: &mut Vec<u8>, value: u32) -> Result<(), Error> {
    push_bytes(output, &value.to_be_bytes())
}

fn push_byte(output: &mut Vec<u8>, value: u8) -> Result<(), Error> {
    output.try_reserve(1).map_err(|_| Error::Allocation)?;
    output.push(value);
    Ok(())
}

fn push_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    output
        .try_reserve(value.len())
        .map_err(|_| Error::Allocation)?;
    output.extend_from_slice(value);
    Ok(())
}

fn pad(output: &mut Vec<u8>) -> Result<(), Error> {
    while output.len() & 3 != 0 {
        push_byte(output, 0)?;
    }
    Ok(())
}
