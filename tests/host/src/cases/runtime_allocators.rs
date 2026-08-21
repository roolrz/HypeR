// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Buddy, slab, and owner-accounted runtime allocation contracts.

use std::alloc::{GlobalAlloc, Layout, alloc_zeroed, dealloc};

use hyper::hal::interrupt::InterruptMask;
use hyper::mm::allocator::heap::{InitError, KernelGlobalAllocator, PageOwner, SlabAllocator};
use hyper::mm::{BootAllocator, BuddyAllocator, BuddyError, PAGE_SIZE};
use hyper::platform::{MAX_MEMORY_REGIONS, MAX_RESERVED_REGIONS, PhysicalRange, RegionList};

struct AlignedMemory {
    pointer: *mut u8,
    layout: Layout,
}

impl AlignedMemory {
    fn new(pages: usize) -> Self {
        let layout = crate::require_ok(Layout::from_size_align(
            pages * PAGE_SIZE as usize,
            PAGE_SIZE as usize,
        ));
        // SAFETY: The test owns the allocation until `Drop`.
        let pointer = unsafe { alloc_zeroed(layout) };
        assert!(!pointer.is_null());
        Self { pointer, layout }
    }
}

impl Drop for AlignedMemory {
    fn drop(&mut self) {
        // SAFETY: `pointer` was allocated with this exact layout.
        unsafe { dealloc(self.pointer, self.layout) };
    }
}

fn handoff(pages: usize) -> (AlignedMemory, hyper::mm::MemoryHandoff) {
    let memory_buffer = AlignedMemory::new(pages);
    let mut memory = RegionList::<MAX_MEMORY_REGIONS>::new();
    crate::require_ok(memory.insert(crate::require_some(PhysicalRange::new(
        0,
        pages as u64 * PAGE_SIZE,
    ))));
    let reserved = RegionList::<MAX_RESERVED_REGIONS>::new();
    let boot = crate::require_ok(BootAllocator::new(
        &memory,
        &reserved,
        pages as u64 * PAGE_SIZE,
    ));
    (memory_buffer, boot.handoff())
}

#[test]
fn buddy_splits_and_coalesces_blocks() {
    let (memory, handoff) = handoff(64);
    // SAFETY: The aligned test buffer is the direct map for physical zero.
    let mut buddy =
        crate::require_ok(unsafe { BuddyAllocator::from_handoff(&handoff, memory.pointer as u64) });
    let initial = buddy.free_pages();
    let initial_stats = buddy.stats();
    assert_eq!(initial_stats.managed_pages, 64);
    assert_eq!(initial_stats.free_blocks[6], 1);
    let first = crate::require_ok(buddy.allocate(0));
    let second = crate::require_ok(buddy.allocate(2));
    assert_eq!(buddy.free_pages(), initial - 5);
    let allocated = buddy.stats();
    assert_eq!(allocated.allocated_pages, 5);
    assert_eq!(allocated.peak_allocated_pages, 5);
    assert_eq!(allocated.allocation_requests, 2);
    assert_eq!(
        free_pages_from_blocks(&allocated.free_blocks),
        allocated.free_pages
    );

    // SAFETY: Both blocks are live allocations with matching orders.
    unsafe {
        crate::require_ok(buddy.deallocate(first, 0));
        crate::require_ok(buddy.deallocate(second, 2));
    }
    assert_eq!(buddy.free_pages(), initial);
    assert_eq!(buddy.stats().deallocations, 2);
    assert!(buddy.allocate(6).is_ok());
}

#[test]
fn slab_reuses_small_objects_and_returns_empty_pages() {
    let (memory, handoff) = handoff(128);
    // SAFETY: The aligned test buffer is a stable writable direct map.
    let mut slab =
        crate::require_ok(unsafe { SlabAllocator::from_handoff(&handoff, memory.pointer as u64) });
    let initial = slab.stats().free_pages;
    let small = crate::require_ok(Layout::from_size_align(24, 16));
    let large = crate::require_ok(Layout::from_size_align(9000, 4096));

    // SAFETY: Allocations are paired with matching deallocations below.
    unsafe {
        let mut small_objects = Vec::new();
        for _ in 0..200 {
            let pointer = slab.allocate(small);
            assert!(!pointer.is_null());
            small_objects.push(pointer);
        }
        let big = slab.allocate(large);
        assert!(!big.is_null());
        assert_eq!((small_objects[0] as usize) & 15, 0);
        assert_eq!((big as usize) & 4095, 0);
        assert_ne!(small_objects[0], small_objects[1]);
        for pointer in small_objects.into_iter().rev() {
            slab.deallocate(pointer, small);
        }
        slab.deallocate(big, large);
    }

    let stats = slab.stats();
    assert_eq!(stats.live_allocations, 0);
    assert_eq!(stats.slab_pages, 0);
    assert_eq!(stats.large_heap_pages, 0);
    assert_eq!(stats.requested_bytes, 0);
    assert_eq!(stats.peak_live_allocations, 201);
    assert!(stats.peak_requested_bytes >= 200 * 24 + 9000);
    assert_eq!(stats.free_pages, initial);
}

#[test]
fn oversized_heap_request_is_rejected_without_consuming_pages() {
    let (memory, handoff) = handoff(64);
    // SAFETY: The aligned test buffer is a stable writable direct map.
    let mut heap =
        crate::require_ok(unsafe { SlabAllocator::from_handoff(&handoff, memory.pointer as u64) });
    let before = heap.stats();
    let layout = crate::require_ok(Layout::from_size_align(
        (PAGE_SIZE as usize) << 19,
        PAGE_SIZE as usize,
    ));

    assert!(heap.allocate(layout).is_null());
    let after = heap.stats();
    assert_eq!(after.free_pages, before.free_pages);
    assert_eq!(after.buddy.allocated_pages, before.buddy.allocated_pages);
    assert_eq!(after.live_allocations, 0);
    assert_eq!(after.allocation_failures, before.allocation_failures + 1);
}

struct TestInterruptMask;

impl InterruptMask for TestInterruptMask {
    type State = ();

    fn save_and_disable() -> Self::State {}

    fn restore(_: Self::State) {}
}

#[test]
fn accounts_direct_pages_by_owner() {
    let (memory, handoff) = handoff(64);
    let allocator = KernelGlobalAllocator::<TestInterruptMask>::new();
    // SAFETY: The aligned test buffer is the direct map and outlives the allocator.
    crate::require_ok(unsafe { allocator.initialize(&handoff, memory.pointer as u64) });

    let guest = crate::require_ok(allocator.allocate_pages_for(3, PageOwner::Guest));
    let table = crate::require_ok(allocator.allocate_pages_for(0, PageOwner::PageTable));
    let stats = crate::require_some(allocator.stats());
    assert_eq!(stats.guest_pages.pages, 8);
    assert_eq!(stats.page_table_pages.pages, 1);
    assert_eq!(stats.buddy.allocated_pages, 9);

    // SAFETY: These are the exact live blocks and owners returned above.
    unsafe {
        crate::require_ok(allocator.deallocate_pages_for(table, 0, PageOwner::PageTable));
        crate::require_ok(allocator.deallocate_pages_for(guest, 3, PageOwner::Guest));
    }
    let stats = crate::require_some(allocator.stats());
    assert_eq!(stats.guest_pages.pages, 0);
    assert_eq!(stats.guest_pages.peak_pages, 8);
    assert_eq!(stats.page_table_pages.pages, 0);
    assert_eq!(stats.buddy.allocated_pages, 0);
}

#[test]
fn rejects_misaligned_direct_map_without_publishing_heap_state() {
    let (memory, handoff) = handoff(64);
    let allocator = KernelGlobalAllocator::<TestInterruptMask>::new();

    // SAFETY: The constructor rejects the misaligned base before deriving or
    // dereferencing any direct-map pointer from it.
    let error = unsafe { allocator.initialize(&handoff, memory.pointer as u64 + 1) };
    assert_eq!(error, Err(InitError::Buddy(BuddyError::Unaddressable)));
    assert!(allocator.stats().is_none());

    // SAFETY: The aligned test buffer is a stable writable direct map and the
    // failed attempt above did not publish allocator state.
    let initialized = unsafe { allocator.initialize(&handoff, memory.pointer as u64) };
    assert_eq!(initialized, Ok(()));
    assert!(allocator.stats().is_some());

    // SAFETY: This deliberately verifies that one-time publication rejects a
    // second initializer without replacing the live heap.
    let duplicate = unsafe { allocator.initialize(&handoff, memory.pointer as u64) };
    assert_eq!(duplicate, Err(InitError::AlreadyInitialized));
}

#[test]
fn global_adapter_zeroes_reused_storage_and_updates_accounting() {
    let (memory, handoff) = handoff(64);
    let allocator = KernelGlobalAllocator::<TestInterruptMask>::new();
    // SAFETY: The aligned test buffer is a stable writable direct map and
    // outlives every allocation made below.
    crate::require_ok(unsafe { allocator.initialize(&handoff, memory.pointer as u64) });
    let layout = crate::require_ok(Layout::from_size_align(96, 32));

    // SAFETY: `layout` is valid and the returned allocation is handled using
    // this same allocator and layout.
    let dirty = unsafe { GlobalAlloc::alloc(&allocator, layout) };
    assert!(!dirty.is_null());
    // SAFETY: `dirty` names 96 exclusive writable bytes.
    unsafe { dirty.write_bytes(0xa5, layout.size()) };
    // SAFETY: `dirty` is the live allocation obtained above with this layout.
    unsafe { GlobalAlloc::dealloc(&allocator, dirty, layout) };

    // SAFETY: `layout` is valid and the returned allocation is released below.
    let zeroed = unsafe { GlobalAlloc::alloc_zeroed(&allocator, layout) };
    assert!(!zeroed.is_null());
    // SAFETY: `zeroed` names `layout.size()` initialized bytes until dealloc.
    let bytes = unsafe { std::slice::from_raw_parts(zeroed, layout.size()) };
    assert!(bytes.iter().all(|&byte| byte == 0));
    // SAFETY: `zeroed` is the live allocation obtained above with this layout.
    unsafe { GlobalAlloc::dealloc(&allocator, zeroed, layout) };

    let stats = crate::require_some(allocator.stats());
    assert_eq!(stats.live_allocations, 0);
    assert_eq!(stats.requested_bytes, 0);
    assert_eq!(stats.slab_pages, 0);
}

fn free_pages_from_blocks(blocks: &[usize]) -> usize {
    blocks
        .iter()
        .enumerate()
        .map(|(order, blocks)| blocks << order)
        .sum()
}
