//! Allocation-free early physical-page allocator.

use crate::platform::{MAX_MEMORY_REGIONS, MAX_RESERVED_REGIONS, PhysicalRange, RegionList};

use crate::mm::{PAGE_SIZE, PhysicalAddress};

pub const MAX_BOOT_RESERVATIONS: usize = MAX_RESERVED_REGIONS + 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootAllocatorError {
    AddressOverflow,
    InvalidAlignment,
    InvalidRequest,
    OutOfMemory,
    TooManyReservations,
    Unaddressable,
}

/// Page-granular early physical memory allocator.
///
/// This allocator is intentionally allocation-free. Every returned range is
/// inserted into the reservation set, giving later allocators a complete view
/// of memory consumed during boot.
#[derive(Clone, Copy)]
pub struct BootAllocator {
    memory: RegionList<MAX_MEMORY_REGIONS>,
    reserved: RegionList<MAX_BOOT_RESERVATIONS>,
    accessible_limit: u64,
}

impl BootAllocator {
    pub fn new(
        memory: &RegionList<MAX_MEMORY_REGIONS>,
        firmware_reserved: &RegionList<MAX_RESERVED_REGIONS>,
        accessible_limit: u64,
    ) -> Result<Self, BootAllocatorError> {
        let mut reserved = RegionList::new();
        for &range in firmware_reserved.as_slice() {
            reserved
                .insert(page_covering_range(range)?)
                .map_err(|_| BootAllocatorError::TooManyReservations)?;
        }
        Ok(Self {
            memory: *memory,
            reserved,
            accessible_limit,
        })
    }

    pub fn reserve(&mut self, range: PhysicalRange) -> Result<(), BootAllocatorError> {
        self.reserved
            .insert(page_covering_range(range)?)
            .map_err(|_| BootAllocatorError::TooManyReservations)
    }

    pub fn allocate_pages(
        &mut self,
        page_count: usize,
        alignment_pages: usize,
    ) -> Result<PhysicalAddress, BootAllocatorError> {
        if page_count == 0 {
            return Err(BootAllocatorError::InvalidRequest);
        }
        if alignment_pages == 0 || !alignment_pages.is_power_of_two() {
            return Err(BootAllocatorError::InvalidAlignment);
        }

        let size = u64::try_from(page_count)
            .ok()
            .and_then(|count| count.checked_mul(PAGE_SIZE))
            .ok_or(BootAllocatorError::AddressOverflow)?;
        let alignment = u64::try_from(alignment_pages)
            .ok()
            .and_then(|count| count.checked_mul(PAGE_SIZE))
            .ok_or(BootAllocatorError::AddressOverflow)?;

        for &region in self.memory.as_slice() {
            let region_end = region.end().min(self.accessible_limit);
            let mut candidate = align_up(region.start(), alignment)?;
            while candidate
                .checked_add(size)
                .filter(|end| *end <= region_end)
                .is_some()
            {
                let allocation = PhysicalRange::new(candidate, size)
                    .ok_or(BootAllocatorError::AddressOverflow)?;
                let overlap_end = self
                    .reserved
                    .as_slice()
                    .iter()
                    .filter(|reserved| reserved.overlaps(allocation))
                    .map(|reserved| reserved.end())
                    .max();
                if let Some(overlap_end) = overlap_end {
                    candidate = align_up(overlap_end, alignment)?;
                    continue;
                }

                self.reserved
                    .insert(allocation)
                    .map_err(|_| BootAllocatorError::TooManyReservations)?;
                return Ok(PhysicalAddress::new(candidate));
            }
        }
        Err(BootAllocatorError::OutOfMemory)
    }

    /// Allocates and zeroes pages through the bootstrap identity map.
    ///
    /// # Safety
    ///
    /// Every allocatable physical address below `accessible_limit` must be
    /// writable through an identity mapping in the current address space.
    pub unsafe fn allocate_zeroed_pages(
        &mut self,
        page_count: usize,
        alignment_pages: usize,
    ) -> Result<PhysicalAddress, BootAllocatorError> {
        let address = self.allocate_pages(page_count, alignment_pages)?;
        let pointer = address
            .as_usize()
            .ok_or(BootAllocatorError::Unaddressable)? as *mut u8;
        let byte_count = page_count
            .checked_mul(PAGE_SIZE as usize)
            .ok_or(BootAllocatorError::AddressOverflow)?;
        // SAFETY: The caller guarantees identity-mapped writable RAM and this
        // allocation is exclusively owned by the boot allocator.
        unsafe { core::ptr::write_bytes(pointer, 0, byte_count) };
        Ok(address)
    }

    pub fn reservations(&self) -> &[PhysicalRange] {
        self.reserved.as_slice()
    }

    pub fn memory(&self) -> &RegionList<MAX_MEMORY_REGIONS> {
        &self.memory
    }

    pub fn handoff(&self) -> crate::mm::MemoryHandoff {
        crate::mm::MemoryHandoff::new(self.memory, self.reserved)
    }
}

fn page_covering_range(range: PhysicalRange) -> Result<PhysicalRange, BootAllocatorError> {
    let start = range.start() & !(PAGE_SIZE - 1);
    let end = align_up(range.end(), PAGE_SIZE)?;
    PhysicalRange::new(start, end - start).ok_or(BootAllocatorError::InvalidRequest)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, BootAllocatorError> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or(BootAllocatorError::AddressOverflow)
}
