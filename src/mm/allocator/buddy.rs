//! Intrusive physical-page buddy allocator.

use core::ptr::{read_volatile, write_volatile};

use crate::platform::{MAX_MEMORY_REGIONS, PhysicalRange, RegionList};

use crate::mm::boot::MAX_BOOT_RESERVATIONS;
use crate::mm::{PAGE_SIZE, PhysicalAddress};

pub const MAX_ORDER: usize = 18;
const NONE: u64 = u64::MAX;

#[derive(Clone, Copy)]
pub struct MemoryHandoff {
    memory: RegionList<MAX_MEMORY_REGIONS>,
    reserved: RegionList<MAX_BOOT_RESERVATIONS>,
}

impl MemoryHandoff {
    pub(crate) const fn new(
        memory: RegionList<MAX_MEMORY_REGIONS>,
        reserved: RegionList<MAX_BOOT_RESERVATIONS>,
    ) -> Self {
        Self { memory, reserved }
    }

    pub fn memory(&self) -> &[PhysicalRange] {
        self.memory.as_slice()
    }

    pub fn reserved(&self) -> &[PhysicalRange] {
        self.reserved.as_slice()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuddyError {
    AddressOverflow,
    InvalidOrder,
    OutOfMemory,
    Unaddressable,
}

/// Intrusive physical page allocator using power-of-two buddy blocks.
///
/// Free-list links are stored in the first word of each free block. The
/// allocator therefore needs only a direct-map base and no separately allocated
/// metadata. All methods require external synchronization.
pub struct BuddyAllocator {
    free_lists: [u64; MAX_ORDER + 1],
    direct_map_base: u64,
    free_pages: usize,
}

// SAFETY: The allocator contains only physical addresses and is always used
// behind the global heap lock.
unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    /// Builds free lists from RAM minus every boot reservation.
    ///
    /// # Safety
    ///
    /// `direct_map_base + PA` must provide writable Normal memory for every
    /// page supplied by `handoff`.
    pub unsafe fn from_handoff(
        handoff: &MemoryHandoff,
        direct_map_base: u64,
    ) -> Result<Self, BuddyError> {
        let mut allocator = Self {
            free_lists: [NONE; MAX_ORDER + 1],
            direct_map_base,
            free_pages: 0,
        };

        for &memory in handoff.memory() {
            let mut cursor = align_up(memory.start(), PAGE_SIZE)?;
            let end = align_down(memory.end(), PAGE_SIZE);
            while cursor < end {
                let next_reserved = handoff
                    .reserved()
                    .iter()
                    .filter(|range| range.end() > cursor && range.start() < end)
                    .min_by_key(|range| range.start());
                let free_end = next_reserved.map_or(end, |range| range.start().min(end));
                if cursor < free_end {
                    unsafe { allocator.add_interval(cursor, free_end)? };
                }
                cursor = match next_reserved {
                    Some(range) => align_up(range.end(), PAGE_SIZE)?,
                    None => end,
                };
            }
        }
        Ok(allocator)
    }

    pub fn allocate(&mut self, order: usize) -> Result<PhysicalAddress, BuddyError> {
        if order > MAX_ORDER {
            return Err(BuddyError::InvalidOrder);
        }
        let source_order = (order..=MAX_ORDER)
            .find(|&candidate| self.free_lists[candidate] != NONE)
            .ok_or(BuddyError::OutOfMemory)?;
        let block = unsafe { self.pop(source_order)? };

        for split_order in (order..source_order).rev() {
            let buddy = block
                .checked_add(block_size(split_order))
                .ok_or(BuddyError::AddressOverflow)?;
            unsafe { self.push(split_order, buddy)? };
        }
        self.free_pages -= 1usize << order;
        Ok(PhysicalAddress::new(block))
    }

    /// Returns a block previously allocated with the same order.
    ///
    /// # Safety
    ///
    /// `address` and `order` must describe one live allocation owned by this
    /// allocator. Double-free and mismatched-order calls corrupt free lists.
    pub unsafe fn deallocate(
        &mut self,
        address: PhysicalAddress,
        order: usize,
    ) -> Result<(), BuddyError> {
        if order > MAX_ORDER {
            return Err(BuddyError::InvalidOrder);
        }
        let mut block = address.get();
        let mut current_order = order;
        self.free_pages += 1usize << order;

        while current_order < MAX_ORDER {
            let buddy = block ^ block_size(current_order);
            if !unsafe { self.remove(current_order, buddy)? } {
                break;
            }
            block = block.min(buddy);
            current_order += 1;
        }
        unsafe { self.push(current_order, block) }
    }

    pub const fn free_pages(&self) -> usize {
        self.free_pages
    }

    pub const fn direct_map_base(&self) -> u64 {
        self.direct_map_base
    }

    unsafe fn add_interval(&mut self, mut start: u64, end: u64) -> Result<(), BuddyError> {
        while start < end {
            let remaining_pages = (end - start) / PAGE_SIZE;
            let alignment_order = if start == 0 {
                MAX_ORDER
            } else {
                ((start.trailing_zeros() as usize).saturating_sub(12)).min(MAX_ORDER)
            };
            let size_order = floor_log2(remaining_pages).min(MAX_ORDER);
            let order = alignment_order.min(size_order);
            unsafe { self.push(order, start)? };
            self.free_pages += 1usize << order;
            start = start
                .checked_add(block_size(order))
                .ok_or(BuddyError::AddressOverflow)?;
        }
        Ok(())
    }

    unsafe fn push(&mut self, order: usize, address: u64) -> Result<(), BuddyError> {
        unsafe { self.write_next(address, self.free_lists[order])? };
        self.free_lists[order] = address;
        Ok(())
    }

    unsafe fn pop(&mut self, order: usize) -> Result<u64, BuddyError> {
        let address = self.free_lists[order];
        debug_assert_ne!(address, NONE);
        self.free_lists[order] = unsafe { self.read_next(address)? };
        Ok(address)
    }

    unsafe fn remove(&mut self, order: usize, target: u64) -> Result<bool, BuddyError> {
        let mut previous = NONE;
        let mut current = self.free_lists[order];
        while current != NONE {
            let next = unsafe { self.read_next(current)? };
            if current == target {
                if previous == NONE {
                    self.free_lists[order] = next;
                } else {
                    unsafe { self.write_next(previous, next)? };
                }
                return Ok(true);
            }
            previous = current;
            current = next;
        }
        Ok(false)
    }

    unsafe fn read_next(&self, physical: u64) -> Result<u64, BuddyError> {
        let pointer = self.pointer(physical)? as *const u64;
        Ok(unsafe { read_volatile(pointer) })
    }

    unsafe fn write_next(&self, physical: u64, next: u64) -> Result<(), BuddyError> {
        let pointer = self.pointer(physical)? as *mut u64;
        unsafe { write_volatile(pointer, next) };
        Ok(())
    }

    fn pointer(&self, physical: u64) -> Result<usize, BuddyError> {
        self.direct_map_base
            .checked_add(physical)
            .and_then(|address| usize::try_from(address).ok())
            .ok_or(BuddyError::Unaddressable)
    }
}

const fn block_size(order: usize) -> u64 {
    PAGE_SIZE << order
}

fn floor_log2(value: u64) -> usize {
    (u64::BITS - 1 - value.leading_zeros()) as usize
}

fn align_down(value: u64, alignment: u64) -> u64 {
    value & !(alignment - 1)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, BuddyError> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| align_down(rounded, alignment))
        .ok_or(BuddyError::AddressOverflow)
}
