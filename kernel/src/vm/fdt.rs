// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linux-format device tree for FDT-booted virtual platforms.

use alloc::vec::Vec;
use core::fmt::{self, Write};

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_END: u32 = 9;
const HEADER_SIZE: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AddressOverflow,
    NameTooLong,
    NameOffsetOverflow,
}

pub fn begin_hex_node(builder: &mut Builder, prefix: &str, value: u64) -> Result<(), Error> {
    let mut name = NodeName::<32>::new();
    write!(&mut name, "{prefix}{value:x}").map_err(|_| Error::NameTooLong)?;
    builder.begin_node(name.as_str())
}

struct NodeName<const CAPACITY: usize> {
    bytes: [u8; CAPACITY],
    length: usize,
}

impl<const CAPACITY: usize> NodeName<CAPACITY> {
    const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            length: 0,
        }
    }

    fn as_str(&self) -> &str {
        // fmt::Write only accepts UTF-8 strings and copies them unchanged.
        core::str::from_utf8(&self.bytes[..self.length]).unwrap_or("")
    }
}

impl<const CAPACITY: usize> fmt::Write for NodeName<CAPACITY> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

pub struct Builder {
    structure: Vec<u8>,
    strings: Vec<u8>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub const fn new() -> Self {
        Self {
            structure: Vec::new(),
            strings: Vec::new(),
        }
    }

    pub fn begin_node(&mut self, name: &str) -> Result<(), Error> {
        push_u32(&mut self.structure, FDT_BEGIN_NODE)?;
        push_bytes(&mut self.structure, name.as_bytes())?;
        push_byte(&mut self.structure, 0)?;
        pad(&mut self.structure)
    }

    pub fn end_node(&mut self) -> Result<(), Error> {
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

    pub fn property_empty(&mut self, name: &str) -> Result<(), Error> {
        self.property(name, &[])
    }

    pub fn property_u32(&mut self, name: &str, value: u32) -> Result<(), Error> {
        self.property(name, &value.to_be_bytes())
    }

    pub fn property_u64_cells(&mut self, name: &str, value: u64) -> Result<(), Error> {
        self.property_cells(name, &[(value >> 32) as u32, value as u32])
    }

    pub fn property_u64_pair(&mut self, name: &str, first: u64, second: u64) -> Result<(), Error> {
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

    pub fn property_cells(&mut self, name: &str, values: &[u32]) -> Result<(), Error> {
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

    pub fn property_string(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(value.len() + 1)
            .map_err(|_| Error::Allocation)?;
        encoded.extend_from_slice(value.as_bytes());
        encoded.push(0);
        self.property(name, &encoded)
    }

    pub fn property_string_list(&mut self, name: &str, values: &[&str]) -> Result<(), Error> {
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

    pub fn finish(mut self) -> Result<Vec<u8>, Error> {
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
