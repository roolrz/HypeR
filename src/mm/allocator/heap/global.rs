//! Synchronized publication and Rust global-allocation ABI adapter.
//!
//! This module owns the interrupt-masking lock and the one-time transition from
//! an unavailable allocator to a live runtime heap. It does not implement slab
//! layout, buddy policy, or page-owner accounting.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{null_mut, write_bytes};

use crate::hal::interrupt::InterruptMask;
use crate::mm::{BuddyError, MemoryHandoff, PhysicalAddress};
use crate::sync::InterruptSpinLock;

use super::{AllocatorFault, HeapStats, InitError, PageOwner, SlabAllocator, allocator_fault};

struct AllocatorState {
    heap: Option<SlabAllocator>,
}

impl AllocatorState {
    const fn uninitialized() -> Self {
        Self { heap: None }
    }
}

pub struct KernelGlobalAllocator<M: InterruptMask> {
    state: InterruptSpinLock<AllocatorState, M>,
}

impl<M: InterruptMask> KernelGlobalAllocator<M> {
    pub const fn new() -> Self {
        Self {
            state: InterruptSpinLock::new(AllocatorState::uninitialized()),
        }
    }

    /// Installs the physical-page handoff as Rust's global heap.
    ///
    /// # Safety
    ///
    /// `direct_map_base` must permanently map all handed-off RAM as writable
    /// Normal memory and preserve page alignment. Initialization must occur
    /// exactly once before allocation.
    pub unsafe fn initialize(
        &self,
        handoff: &MemoryHandoff,
        direct_map_base: u64,
    ) -> Result<(), InitError> {
        self.state.with(|state| {
            if state.heap.is_some() {
                return Err(InitError::AlreadyInitialized);
            }
            // SAFETY: The public initialize contract guarantees the permanent
            // direct map, and the lock plus Option enforces one-time creation.
            state.heap = Some(unsafe { SlabAllocator::from_handoff(handoff, direct_map_base)? });
            Ok(())
        })
    }

    pub fn stats(&self) -> Option<HeapStats> {
        self.state
            .with(|state| state.heap.as_ref().map(SlabAllocator::stats))
    }

    /// Allocates one physically contiguous, naturally aligned buddy block.
    ///
    /// The returned block is not a Rust heap allocation. Its owner must either
    /// retain it permanently or return it once with [`Self::deallocate_pages`].
    pub fn allocate_pages(&self, order: usize) -> Result<PhysicalAddress, BuddyError> {
        self.allocate_pages_for(order, PageOwner::Kernel)
    }

    pub fn allocate_pages_for(
        &self,
        order: usize,
        owner: PageOwner,
    ) -> Result<PhysicalAddress, BuddyError> {
        self.state.with(|state| {
            let heap = state.heap.as_mut().ok_or(BuddyError::OutOfMemory)?;
            heap.allocate_pages(order, owner)
        })
    }

    /// Returns a contiguous block issued by [`Self::allocate_pages`].
    ///
    /// # Safety
    ///
    /// `address` and `order` must identify one live block issued by this
    /// allocator. The block must no longer be mapped into an active consumer.
    pub unsafe fn deallocate_pages(
        &self,
        address: PhysicalAddress,
        order: usize,
    ) -> Result<(), BuddyError> {
        // SAFETY: This method forwards its live allocation and inactivity
        // requirements unchanged while fixing the owner to Kernel.
        unsafe { self.deallocate_pages_for(address, order, PageOwner::Kernel) }
    }

    /// Returns a block allocated for the same `owner`.
    ///
    /// # Safety
    ///
    /// The address, order, and owner must exactly match a live allocation.
    pub unsafe fn deallocate_pages_for(
        &self,
        address: PhysicalAddress,
        order: usize,
        owner: PageOwner,
    ) -> Result<(), BuddyError> {
        self.state.with(|state| {
            let heap = state.heap.as_mut().ok_or(BuddyError::OutOfMemory)?;
            // SAFETY: The public method contract supplies the exact live block,
            // order, and owner; this lock gives exclusive allocator access.
            unsafe { heap.deallocate_pages(address, order, owner) }
        })
    }
}

impl<M: InterruptMask> Default for KernelGlobalAllocator<M> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: The interrupt-masking lock serializes every metadata access. Returned
// allocations are disjoint and remain owned by callers until matching dealloc.
unsafe impl<M: InterruptMask> GlobalAlloc for KernelGlobalAllocator<M> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.state.with(|state| match state.heap.as_mut() {
            Some(heap) => heap.allocate(layout),
            None => null_mut(),
        })
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc forwards the valid requested layout to alloc.
        let pointer = unsafe { self.alloc(layout) };
        if !pointer.is_null() {
            // SAFETY: alloc returned at least layout.size() writable exclusive
            // bytes for this exact layout.
            unsafe { write_bytes(pointer, 0, layout.size()) };
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        self.state.with(|state| {
            let Some(heap) = state.heap.as_mut() else {
                allocator_fault(AllocatorFault::UninitializedDeallocation);
            };
            // SAFETY: GlobalAlloc requires pointer/layout to identify one live
            // allocation from this allocator; the lock provides exclusivity.
            unsafe { heap.deallocate(pointer, layout) };
        });
    }
}
