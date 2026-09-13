// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable, bounded guest RAM composition over explicitly retained VMO leases.

use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::mm::user_space::{GuestMemoryBacking, MemoryObjectError};
use hyper::mm::{PAGE_SIZE, PhysicalAddress};

pub(crate) struct Region {
    offset: u64,
    source_offset: u64,
    length: u64,
    backing: GuestMemoryBacking,
}

impl Region {
    pub(crate) fn new(
        offset: u64,
        source_offset: u64,
        length: u64,
        backing: GuestMemoryBacking,
    ) -> Result<Self, MemoryObjectError> {
        if length == 0
            || !offset.is_multiple_of(PAGE_SIZE)
            || !source_offset.is_multiple_of(PAGE_SIZE)
            || !length.is_multiple_of(PAGE_SIZE)
            || offset.checked_add(length).is_none()
            || source_offset
                .checked_add(length)
                .is_none_or(|end| end > backing.size())
        {
            return Err(MemoryObjectError::AllocationSize);
        }
        Ok(Self {
            offset,
            source_offset,
            length,
            backing,
        })
    }
}

/// Coverage is checked before sealing; an unmapped gap never becomes anonymous RAM.
pub(crate) struct Layout {
    size: u64,
    regions: [Option<Region>; 8],
    _charge: CommittedCharge,
}

impl Layout {
    pub(crate) fn try_new(
        size: u64,
        domain: &ResourceDomain,
    ) -> Result<alloc::boxed::Box<Self>, super::Error> {
        let charge = domain
            .reserve(ResourceAmount::ZERO.with(
                ResourceKind::KernelMemoryBytes,
                core::mem::size_of::<Self>() as u64,
            ))?
            .commit();
        hyper::mm::try_box(Self {
            size,
            regions: [const { None }; 8],
            _charge: charge,
        })
        .map_err(|_| super::Error::MetadataAllocation)
    }

    /// A rejected region stays caller-owned so its lease is dropped outside locks.
    pub(crate) fn insert(&mut self, region: &mut Option<Region>) -> Result<(), MemoryObjectError> {
        let candidate = region.as_ref().ok_or(MemoryObjectError::AllocationSize)?;
        let end = candidate.offset + candidate.length;
        if end > self.size
            || self
                .regions
                .iter()
                .flatten()
                .any(|old| candidate.offset < old.offset + old.length && old.offset < end)
        {
            return Err(MemoryObjectError::AllocationSize);
        }
        let slot = self
            .regions
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(MemoryObjectError::AllocationSize)?;
        *slot = region.take();
        Ok(())
    }

    pub(crate) fn complete(&self) -> bool {
        // All entries are non-overlapping and contained in [0,size).
        self.regions.iter().flatten().map(|r| r.length).sum::<u64>() == self.size
    }

    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    fn resolve(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<(&GuestMemoryBacking, u64), MemoryObjectError> {
        let end = offset
            .checked_add(length)
            .ok_or(MemoryObjectError::AllocationSize)?;
        self.regions
            .iter()
            .flatten()
            .find(|region| offset >= region.offset && end <= region.offset + region.length)
            .map(|region| {
                (
                    &region.backing,
                    region.source_offset + offset - region.offset,
                )
            })
            .ok_or(MemoryObjectError::AllocationSize)
    }

    /// DMA-owning VMs must retain resident backing for every advertised range.
    pub(crate) fn validate_dma(&self) -> Result<(), MemoryObjectError> {
        for region in self.regions.iter().flatten() {
            let first = region.backing.physical_page(region.source_offset)?.get();
            first
                .checked_add(region.length)
                .ok_or(MemoryObjectError::AllocationSize)?;
            for offset in (0..region.length).step_by(PAGE_SIZE as usize) {
                if region
                    .backing
                    .physical_page(region.source_offset + offset)?
                    .get()
                    != first + offset
                {
                    return Err(MemoryObjectError::AllocationSize);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn populate_page(&self, offset: u64) -> Result<(), MemoryObjectError> {
        let (backing, offset) = self.resolve(offset, PAGE_SIZE)?;
        backing.populate_page(offset)
    }
    pub(crate) fn physical_page(&self, offset: u64) -> Result<PhysicalAddress, MemoryObjectError> {
        let (backing, offset) = self.resolve(offset, PAGE_SIZE)?;
        backing.physical_page(offset)
    }
    pub(crate) fn page_is_resident(&self, offset: u64) -> Result<bool, MemoryObjectError> {
        let (backing, offset) = self.resolve(offset, PAGE_SIZE)?;
        backing.page_is_resident(offset)
    }
    pub(crate) fn read_exposed(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), MemoryObjectError> {
        let (backing, offset) = self.resolve(offset, destination.len() as u64)?;
        backing.read_exposed(offset, destination)
    }
    pub(crate) fn write_exposed(
        &self,
        offset: u64,
        source: &[u8],
    ) -> Result<(), MemoryObjectError> {
        let (backing, offset) = self.resolve(offset, source.len() as u64)?;
        backing.write_exposed(offset, source)
    }
}
