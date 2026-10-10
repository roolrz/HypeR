// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Owned 39-bit IOVA address spaces using 4 KiB VMSAv8-64 stage-2 tables.

use super::{DmaBuffer, DmaMemory, Environment, Error, memory};
use alloc::vec::Vec;

const ADDRESS_MASK: u64 = 0x0000_ffff_ffff_f000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainId {
    pub(super) controller: u64,
    pub(super) serial: u64,
    pub(super) vmid: u16,
}

#[derive(Clone, Copy, Debug)]
pub enum Permissions {
    Read,
    Write,
    ReadWrite,
}

impl Permissions {
    fn descriptor(self) -> u64 {
        let access = match self {
            Self::Read => 1,
            Self::Write => 2,
            Self::ReadWrite => 3,
        };
        // Page, normal WB, S2AP, Inner Shareable, AF, execute-never.
        3 | (15 << 2) | (access << 6) | (3 << 8) | (1 << 10) | (1 << 54)
    }
}

pub(super) struct Mapping<M> {
    pub iova: u64,
    pub memory: M,
}
pub(super) struct Domain<M, B> {
    pub id: DomainId,
    pub tables: Vec<M>,
    pub mappings: Vec<Mapping<B>>,
}

impl<M: DmaMemory, B: DmaBuffer> Domain<M, B> {
    pub(super) fn new<E: Environment<Memory = M>>(id: DomainId, bits: u8) -> Result<Self, Error> {
        let mut tables = Vec::new();
        tables.try_reserve_exact(1).map_err(|_| Error::Allocation)?;
        tables.push(memory::allocate::<E>(0, bits)?);
        Ok(Self {
            id,
            tables,
            mappings: Vec::new(),
        })
    }

    /// Empty intermediate tables can survive allocation failure. They grant no
    /// DMA access and are reclaimed with the domain, never underneath a walker.
    fn leaf<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        create: bool,
        bits: u8,
    ) -> Result<(usize, usize), Error> {
        if iova >= (1 << 39) || !iova.is_multiple_of(4096) {
            return Err(Error::Address);
        }
        let mut table = 0;
        for shift in [30, 21] {
            let index = ((iova >> shift) & 511) as usize;
            let entry = memory::read(&self.tables[table], index);
            let next = if entry == 0 {
                if !create {
                    return Err(Error::NotMapped);
                }
                self.tables.try_reserve(1).map_err(|_| Error::Allocation)?;
                let page = memory::allocate::<E>(0, bits)?;
                let physical = page.physical();
                let next = self.tables.len();
                self.tables.push(page);
                E::synchronize();
                memory::write(&self.tables[table], index, physical | 3);
                next
            } else {
                self.tables
                    .iter()
                    .position(|page| page.physical() == entry & ADDRESS_MASK)
                    .ok_or(Error::Corrupt)?
            };
            table = next;
        }
        Ok((table, ((iova >> 12) & 511) as usize))
    }

    /// Validate and allocate all intermediate tables before publishing any leaf.
    /// Empty tables may remain after failure; they grant no access. One retained
    /// owner covers the complete mapping, avoiding one reference per RAM page.
    pub(super) fn map<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        buffer: B,
        permissions: Permissions,
        bits: u8,
    ) -> Result<(), Error> {
        let length = buffer.length();
        let end = iova.checked_add(length).ok_or(Error::Address)?;
        if length == 0
            || !length.is_multiple_of(4096)
            || !iova.is_multiple_of(4096)
            || end > 1 << 39
        {
            return Err(Error::Address);
        }
        let position = self.mappings.partition_point(|map| map.iova < iova);
        if position > 0
            && self.mappings[position - 1].iova + self.mappings[position - 1].memory.length() > iova
            || self
                .mappings
                .get(position)
                .is_some_and(|map| map.iova < end)
        {
            return Err(Error::AlreadyMapped);
        }
        self.mappings
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        for offset in (0..length).step_by(4096) {
            let physical = buffer.physical_page(offset);
            if !physical.is_multiple_of(4096)
                || physical
                    .checked_add(4096)
                    .is_none_or(|end| end > 1u64 << bits)
            {
                return Err(Error::Address);
            }
            self.leaf::<E>(iova + offset, true, bits)?;
        }
        // No fallible allocation or metadata growth follows this ownership cut.
        self.mappings.insert(
            position,
            Mapping {
                iova,
                memory: buffer,
            },
        );
        for offset in (0..length).step_by(4096) {
            let (table, index) = self.leaf::<E>(iova + offset, false, bits)?;
            let entry =
                self.mappings[position].memory.physical_page(offset) | permissions.descriptor();
            E::synchronize();
            memory::write(&self.tables[table], index, entry);
        }
        Ok(())
    }

    pub(super) fn invalidate<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        bits: u8,
    ) -> Result<usize, Error> {
        let mapping = self
            .mappings
            .binary_search_by_key(&iova, |map| map.iova)
            .map_err(|_| Error::NotMapped)?;
        for offset in (0..self.mappings[mapping].memory.length()).step_by(4096) {
            let (table, index) = self.leaf::<E>(iova + offset, false, bits)?;
            memory::write(&self.tables[table], index, 0);
        }
        Ok(mapping)
    }
}
