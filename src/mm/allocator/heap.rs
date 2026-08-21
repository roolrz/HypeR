//! Runtime heap mechanisms and allocation accounting.
//!
//! This module owns slab, large-allocation, and direct-page metadata over one
//! buddy allocator. The private `global` module adapts that mechanism to
//! Rust's process-wide allocation ABI and owns initialization/locking policy.
//! Consumer lifetime policy remains above this layer; `PageOwner` is diagnostic
//! accounting, not an ownership token.

use core::alloc::Layout;
use core::hint::spin_loop;
use core::ptr::null_mut;

use crate::sync::atomic::{AtomicU8, Ordering};

use crate::mm::{
    BuddyAllocator, BuddyError, BuddyStats, MAX_ORDER, MemoryHandoff, PAGE_SIZE, PhysicalAddress,
};

mod global;

pub use global::KernelGlobalAllocator;

const NONE: u64 = u64::MAX;
const SLAB_MAGIC: u64 = 0x4859_5045_5253_4c42;
const LARGE_MAGIC: u64 = 0x4859_5045_524c_4152;
const CLASS_SIZES: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];

#[derive(Clone, Copy)]
#[repr(u8)]
enum AllocatorFault {
    InvalidSlabHeader = 1,
    SlabClassMismatch = 2,
    InvalidSlabPointer = 3,
    AllocationCountUnderflow = 4,
    InvalidPartialList = 5,
    BuddyDeallocation = 6,
    InvalidLargeHeader = 7,
    PageOwnerUnderflow = 8,
    InvalidSlabPhysical = 9,
    AllocationCountOverflow = 10,
    InvalidLargePointer = 11,
    UninitializedDeallocation = 12,
}

static LAST_ALLOCATOR_FAULT: AtomicU8 = AtomicU8::new(0);

/// Returns zero or the stable numeric code of the first allocator fault.
pub fn allocator_fault_code() -> u8 {
    LAST_ALLOCATOR_FAULT.load(Ordering::Acquire)
}

fn allocator_fault(fault: AllocatorFault) -> ! {
    let _ =
        LAST_ALLOCATOR_FAULT.compare_exchange(0, fault as u8, Ordering::AcqRel, Ordering::Acquire);
    loop {
        spin_loop();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitError {
    AlreadyInitialized,
    Buddy(BuddyError),
}

impl From<BuddyError> for InitError {
    fn from(error: BuddyError) -> Self {
        Self::Buddy(error)
    }
}

/// Subsystem ownership for allocations made directly from the page allocator.
///
/// Heap backing pages are accounted separately. `Guest` means physically
/// committed guest memory, not the guest's advertised address-space size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PageOwner {
    Kernel = 0,
    PageTable = 1,
    Guest = 2,
}

impl PageOwner {
    const COUNT: usize = 3;

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageOwnerStats {
    pub pages: usize,
    pub peak_pages: usize,
    pub allocation_requests: u64,
    pub allocation_failures: u64,
    pub deallocations: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeapStats {
    pub buddy: BuddyStats,
    /// Compatibility shortcut for `buddy.free_pages`.
    pub free_pages: usize,
    pub slab_pages: usize,
    pub large_heap_pages: usize,
    pub live_allocations: usize,
    pub live_slab_allocations: usize,
    pub live_large_allocations: usize,
    pub peak_live_allocations: usize,
    pub requested_bytes: usize,
    pub peak_requested_bytes: usize,
    pub allocation_requests: u64,
    pub allocation_failures: u64,
    pub kernel_pages: PageOwnerStats,
    pub page_table_pages: PageOwnerStats,
    pub guest_pages: PageOwnerStats,
}

#[repr(C)]
struct SlabHeader {
    magic: u64,
    physical: u64,
    next_partial: u64,
    free_head: usize,
    in_use: u16,
    capacity: u16,
    class: u8,
    _reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LargeHeader {
    magic: u64,
    physical: u64,
    order: u8,
    _reserved: [u8; 7],
}

/// Owns a buddy block until the surrounding heap metadata is publishable.
///
/// Slab and large allocations both perform fallible address arithmetic after
/// obtaining physical memory. Keeping that memory in this guard makes every
/// pre-publication return path restore the buddy allocator automatically.
struct PendingBuddyBlock<'a> {
    buddy: &'a mut BuddyAllocator,
    physical: PhysicalAddress,
    order: usize,
    published: bool,
}

impl<'a> PendingBuddyBlock<'a> {
    fn allocate(buddy: &'a mut BuddyAllocator, order: usize) -> Result<Self, BuddyError> {
        let physical = buddy.allocate(order)?;
        Ok(Self {
            buddy,
            physical,
            order,
            published: false,
        })
    }

    fn physical(&self) -> PhysicalAddress {
        self.physical
    }

    fn virtual_address(&self) -> Option<usize> {
        self.buddy
            .direct_map_base()
            .checked_add(self.physical.get())
            .and_then(|address| usize::try_from(address).ok())
    }

    fn publish(mut self) -> PhysicalAddress {
        self.published = true;
        self.physical
    }
}

impl Drop for PendingBuddyBlock<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        // SAFETY: An unpublished guard uniquely owns the exact buddy block
        // acquired by `allocate`; no pointer to it has escaped.
        if unsafe { self.buddy.deallocate(self.physical, self.order) }.is_err() {
            allocator_fault(AllocatorFault::BuddyDeallocation);
        }
    }
}

pub struct SlabAllocator {
    buddy: BuddyAllocator,
    partial: [u64; CLASS_SIZES.len()],
    slab_pages: usize,
    large_heap_pages: usize,
    live_allocations: usize,
    live_slab_allocations: usize,
    live_large_allocations: usize,
    peak_live_allocations: usize,
    requested_bytes: usize,
    peak_requested_bytes: usize,
    allocation_requests: u64,
    allocation_failures: u64,
    page_owners: [PageOwnerStats; PageOwner::COUNT],
}

impl SlabAllocator {
    /// Creates a slab heap backed by an intrusive buddy allocator.
    ///
    /// # Safety
    ///
    /// The direct map must remain writable for the allocator's lifetime. Its
    /// base must preserve page alignment so a slab object address can be
    /// converted back to the physical page that owns its header.
    pub unsafe fn from_handoff(
        handoff: &MemoryHandoff,
        direct_map_base: u64,
    ) -> Result<Self, BuddyError> {
        if direct_map_base & (PAGE_SIZE - 1) != 0 {
            return Err(BuddyError::Unaddressable);
        }
        Ok(Self {
            // SAFETY: This constructor inherits and preserves its caller's
            // permanent writable direct-map contract for the buddy lifetime.
            buddy: unsafe { BuddyAllocator::from_handoff(handoff, direct_map_base)? },
            partial: [NONE; CLASS_SIZES.len()],
            slab_pages: 0,
            large_heap_pages: 0,
            live_allocations: 0,
            live_slab_allocations: 0,
            live_large_allocations: 0,
            peak_live_allocations: 0,
            requested_bytes: 0,
            peak_requested_bytes: 0,
            allocation_requests: 0,
            allocation_failures: 0,
            page_owners: [PageOwnerStats::default(); PageOwner::COUNT],
        })
    }

    /// Allocates memory satisfying `layout`.
    pub fn allocate(&mut self, layout: Layout) -> *mut u8 {
        self.allocation_requests = self.allocation_requests.saturating_add(1);
        let pointer = if let Some(class) = class_for(layout) {
            self.allocate_slab(class)
        } else {
            self.allocate_large(layout)
        };
        if pointer.is_null() {
            self.allocation_failures = self.allocation_failures.saturating_add(1);
        } else {
            let requested = layout.size().max(1);
            self.requested_bytes = self.requested_bytes.saturating_add(requested);
            self.peak_requested_bytes = self.peak_requested_bytes.max(self.requested_bytes);
            self.peak_live_allocations = self.peak_live_allocations.max(self.live_allocations);
        }
        pointer
    }

    /// Releases a live allocation returned by `allocate`.
    ///
    /// # Safety
    ///
    /// `pointer` and `layout` must describe one live allocation from this heap.
    pub unsafe fn deallocate(&mut self, pointer: *mut u8, layout: Layout) {
        if let Some(class) = class_for(layout) {
            // SAFETY: The caller's exact pointer/layout contract identifies a
            // live object in this computed slab size class.
            unsafe { self.deallocate_slab(pointer, class) };
        } else {
            // SAFETY: A non-slab layout was allocated with an adjacent valid
            // LargeHeader and the caller relinquishes that live allocation.
            unsafe { self.deallocate_large(pointer, layout) };
        }
        self.requested_bytes = match self.requested_bytes.checked_sub(layout.size().max(1)) {
            Some(requested_bytes) => requested_bytes,
            None => allocator_fault(AllocatorFault::AllocationCountUnderflow),
        };
    }

    fn allocate_slab(&mut self, class: usize) -> *mut u8 {
        if self.partial[class] == NONE && self.create_slab(class).is_err() {
            return null_mut();
        }

        let physical = self.partial[class];
        let header = match self.slab_header(physical) {
            Some(header) => header,
            None => allocator_fault(AllocatorFault::InvalidPartialList),
        };
        // SAFETY: `physical` is the head of this class's partial-slab list.
        let header = unsafe { &mut *header };
        if header.magic != SLAB_MAGIC {
            allocator_fault(AllocatorFault::InvalidSlabHeader);
        }
        if usize::from(header.class) != class {
            allocator_fault(AllocatorFault::SlabClassMismatch);
        }
        if header.physical != physical {
            allocator_fault(AllocatorFault::InvalidSlabPhysical);
        }
        if header.in_use >= header.capacity {
            allocator_fault(AllocatorFault::InvalidSlabHeader);
        }
        let pointer = header.free_head as *mut u8;
        if pointer.is_null() {
            allocator_fault(AllocatorFault::InvalidPartialList);
        }
        // SAFETY: Every free object stores the next virtual pointer in its first
        // machine word.
        header.free_head = unsafe { *(pointer as *const usize) };
        header.in_use = match header.in_use.checked_add(1) {
            Some(in_use) => in_use,
            None => allocator_fault(AllocatorFault::AllocationCountOverflow),
        };
        increment(&mut self.live_allocations);
        increment(&mut self.live_slab_allocations);
        if header.free_head == 0 {
            self.partial[class] = header.next_partial;
            header.next_partial = NONE;
        }
        pointer
    }

    unsafe fn deallocate_slab(&mut self, pointer: *mut u8, class: usize) {
        let page_virtual = (pointer as usize) & !(PAGE_SIZE as usize - 1);
        let physical = match (page_virtual as u64).checked_sub(self.buddy.direct_map_base()) {
            Some(physical) => physical,
            None => allocator_fault(AllocatorFault::InvalidSlabPointer),
        };
        let header_pointer = match self.slab_header(physical) {
            Some(header) => header,
            None => allocator_fault(AllocatorFault::InvalidSlabPointer),
        };
        // SAFETY: GlobalAlloc requires `pointer` and `layout` to describe one
        // live allocation returned by this allocator.
        let header = unsafe { &mut *header_pointer };
        if header.magic != SLAB_MAGIC {
            allocator_fault(AllocatorFault::InvalidSlabHeader);
        }
        if usize::from(header.class) != class {
            allocator_fault(AllocatorFault::SlabClassMismatch);
        }
        if header.physical != physical {
            allocator_fault(AllocatorFault::InvalidSlabPhysical);
        }
        let object_size = CLASS_SIZES[class];
        let object_start = match page_virtual
            .checked_add(core::mem::size_of::<SlabHeader>())
            .and_then(|header_end| align_up_usize(header_end, object_size))
        {
            Some(object_start) => object_start,
            None => allocator_fault(AllocatorFault::InvalidSlabHeader),
        };
        let object_bytes = match usize::from(header.capacity).checked_mul(object_size) {
            Some(object_bytes) => object_bytes,
            None => allocator_fault(AllocatorFault::InvalidSlabHeader),
        };
        let object_end = match object_start.checked_add(object_bytes) {
            Some(object_end) => object_end,
            None => allocator_fault(AllocatorFault::InvalidSlabHeader),
        };
        let page_end = match page_virtual.checked_add(PAGE_SIZE as usize) {
            Some(page_end) => page_end,
            None => allocator_fault(AllocatorFault::InvalidSlabHeader),
        };
        let pointer_address = pointer as usize;
        if header.capacity == 0
            || object_end > page_end
            || pointer_address < object_start
            || pointer_address >= object_end
            || !(pointer_address - object_start).is_multiple_of(object_size)
        {
            allocator_fault(AllocatorFault::InvalidSlabPointer);
        }
        if header.in_use == 0
            || header.in_use > header.capacity
            || self.live_allocations == 0
            || self.live_slab_allocations == 0
        {
            allocator_fault(AllocatorFault::AllocationCountUnderflow);
        }
        let was_full = header.free_head == 0;
        // SAFETY: The caller relinquishes this live slab object, whose size is
        // at least one word; it is now exclusive free-list storage.
        unsafe { *(pointer as *mut usize) = header.free_head };
        header.free_head = pointer as usize;
        header.in_use -= 1;
        self.live_allocations -= 1;
        self.live_slab_allocations -= 1;

        if was_full {
            header.next_partial = self.partial[class];
            self.partial[class] = physical;
        }
        if header.in_use == 0 {
            self.remove_partial(class, physical);
            header.magic = 0;
            self.slab_pages -= 1;
            // SAFETY: The empty slab page is exclusively owned by this heap.
            if unsafe { self.buddy.deallocate(PhysicalAddress::new(physical), 0) }.is_err() {
                allocator_fault(AllocatorFault::BuddyDeallocation);
            }
        }
    }

    fn create_slab(&mut self, class: usize) -> Result<(), BuddyError> {
        let next_partial = self.partial[class];
        let pending = PendingBuddyBlock::allocate(&mut self.buddy, 0)?;
        let physical = pending.physical().get();
        let header_pointer = pending
            .virtual_address()
            .map(|address| address as *mut SlabHeader)
            .ok_or(BuddyError::Unaddressable)?;
        let object_size = CLASS_SIZES[class];
        let page_virtual = header_pointer as usize;
        let object_start = page_virtual
            .checked_add(core::mem::size_of::<SlabHeader>())
            .and_then(|header_end| align_up_usize(header_end, object_size))
            .ok_or(BuddyError::Unaddressable)?;
        let capacity = (PAGE_SIZE as usize - (object_start - page_virtual)) / object_size;
        if capacity == 0 || capacity > u16::MAX as usize {
            allocator_fault(AllocatorFault::InvalidSlabHeader);
        }

        for index in 0..capacity {
            let object = (object_start + index * object_size) as *mut usize;
            let next = if index + 1 == capacity {
                0
            } else {
                object_start + (index + 1) * object_size
            };
            // SAFETY: The newly allocated slab page is writable and exclusive.
            unsafe { object.write(next) };
        }

        // SAFETY: The header is suitably aligned at the start of a page.
        unsafe {
            header_pointer.write(SlabHeader {
                magic: SLAB_MAGIC,
                physical,
                next_partial,
                free_head: object_start,
                in_use: 0,
                capacity: capacity as u16,
                class: class as u8,
                _reserved: [0; 3],
            })
        };
        let physical = pending.publish().get();
        self.partial[class] = physical;
        increment(&mut self.slab_pages);
        Ok(())
    }

    fn remove_partial(&mut self, class: usize, target: u64) {
        let mut previous = NONE;
        let mut current = self.partial[class];
        while current != NONE {
            let header_pointer = match self.slab_header(current) {
                Some(header) => header,
                None => allocator_fault(AllocatorFault::InvalidPartialList),
            };
            // SAFETY: `current` came from the class's slab list.
            let header = unsafe { &*header_pointer };
            if header.magic != SLAB_MAGIC
                || usize::from(header.class) != class
                || header.physical != current
            {
                allocator_fault(AllocatorFault::InvalidPartialList);
            }
            let next = header.next_partial;
            if current == target {
                if previous == NONE {
                    self.partial[class] = next;
                } else {
                    let previous_header = match self.slab_header(previous) {
                        Some(header) => header,
                        None => allocator_fault(AllocatorFault::InvalidPartialList),
                    };
                    // SAFETY: `previous` is the preceding list element.
                    unsafe { (*previous_header).next_partial = next };
                }
                return;
            }
            previous = current;
            current = next;
        }
        allocator_fault(AllocatorFault::InvalidPartialList);
    }

    fn allocate_large(&mut self, layout: Layout) -> *mut u8 {
        let header_size = core::mem::size_of::<LargeHeader>();
        let required = match layout
            .size()
            .max(1)
            .checked_add(layout.align() - 1)
            .and_then(|size| size.checked_add(header_size))
        {
            Some(required) => required,
            None => return null_mut(),
        };
        let pages = required.div_ceil(PAGE_SIZE as usize);
        let Some(power_of_two_pages) = pages.checked_next_power_of_two() else {
            return null_mut();
        };
        let order = power_of_two_pages.trailing_zeros() as usize;
        if order > MAX_ORDER {
            return null_mut();
        }
        let pending = match PendingBuddyBlock::allocate(&mut self.buddy, order) {
            Ok(pending) => pending,
            Err(_) => return null_mut(),
        };
        let physical = pending.physical().get();
        let Some(base) = pending.virtual_address() else {
            return null_mut();
        };
        let Some(user) = base
            .checked_add(header_size)
            .and_then(|header_end| align_up_usize(header_end, layout.align()))
        else {
            return null_mut();
        };
        let block_size = match power_of_two_pages.checked_mul(PAGE_SIZE as usize) {
            Some(block_size) => block_size,
            None => return null_mut(),
        };
        let allocation_end = match base.checked_add(block_size) {
            Some(allocation_end) => allocation_end,
            None => return null_mut(),
        };
        let object_end = match user.checked_add(layout.size().max(1)) {
            Some(object_end) => object_end,
            None => return null_mut(),
        };
        if object_end > allocation_end {
            return null_mut();
        }
        let header = (user - header_size) as *mut LargeHeader;
        // SAFETY: The header lies inside the exclusively owned buddy block.
        unsafe {
            header.write(LargeHeader {
                magic: LARGE_MAGIC,
                physical,
                order: order as u8,
                _reserved: [0; 7],
            })
        };
        let _ = pending.publish();
        increment(&mut self.live_allocations);
        increment(&mut self.live_large_allocations);
        self.large_heap_pages = match self.large_heap_pages.checked_add(1usize << order) {
            Some(pages) => pages,
            None => allocator_fault(AllocatorFault::AllocationCountOverflow),
        };
        user as *mut u8
    }

    unsafe fn deallocate_large(&mut self, pointer: *mut u8, layout: Layout) {
        let header_size = core::mem::size_of::<LargeHeader>();
        let header_address = match (pointer as usize).checked_sub(header_size) {
            Some(header_address) => header_address,
            None => allocator_fault(AllocatorFault::InvalidLargePointer),
        };
        // SAFETY: GlobalAlloc requires `pointer` to be a live large allocation.
        let header = unsafe { *(header_address as *const LargeHeader) };
        if header.magic != LARGE_MAGIC {
            allocator_fault(AllocatorFault::InvalidLargeHeader);
        }
        let order = usize::from(header.order);
        if order > MAX_ORDER {
            allocator_fault(AllocatorFault::InvalidLargeHeader);
        }
        let pages = 1usize << order;
        let Some(base) = self.virtual_address(header.physical) else {
            allocator_fault(AllocatorFault::InvalidLargePointer);
        };
        let expected = base
            .checked_add(header_size)
            .and_then(|header_end| align_up_usize(header_end, layout.align()));
        let block_size = match pages.checked_mul(PAGE_SIZE as usize) {
            Some(block_size) => block_size,
            None => allocator_fault(AllocatorFault::InvalidLargeHeader),
        };
        let allocation_end = base.checked_add(block_size);
        let object_end = (pointer as usize).checked_add(layout.size().max(1));
        let object_fits = matches!(
            (object_end, allocation_end),
            (Some(object_end), Some(allocation_end)) if object_end <= allocation_end
        );
        if expected != Some(pointer as usize) || !object_fits {
            allocator_fault(AllocatorFault::InvalidLargePointer);
        }
        if self.live_allocations == 0
            || self.live_large_allocations == 0
            || self.large_heap_pages < pages
        {
            allocator_fault(AllocatorFault::AllocationCountUnderflow);
        }
        // SAFETY: The header records the exact buddy allocation.
        if unsafe {
            self.buddy
                .deallocate(PhysicalAddress::new(header.physical), order)
        }
        .is_err()
        {
            allocator_fault(AllocatorFault::BuddyDeallocation);
        }
        self.live_allocations -= 1;
        self.live_large_allocations -= 1;
        self.large_heap_pages -= pages;
    }

    fn slab_header(&self, physical: u64) -> Option<*mut SlabHeader> {
        self.virtual_address(physical)
            .map(|address| address as *mut SlabHeader)
    }

    fn virtual_address(&self, physical: u64) -> Option<usize> {
        self.buddy
            .direct_map_base()
            .checked_add(physical)
            .and_then(|address| usize::try_from(address).ok())
    }

    pub fn stats(&self) -> HeapStats {
        let buddy = self.buddy.stats();
        HeapStats {
            buddy,
            free_pages: buddy.free_pages,
            slab_pages: self.slab_pages,
            large_heap_pages: self.large_heap_pages,
            live_allocations: self.live_allocations,
            live_slab_allocations: self.live_slab_allocations,
            live_large_allocations: self.live_large_allocations,
            peak_live_allocations: self.peak_live_allocations,
            requested_bytes: self.requested_bytes,
            peak_requested_bytes: self.peak_requested_bytes,
            allocation_requests: self.allocation_requests,
            allocation_failures: self.allocation_failures,
            kernel_pages: self.page_owners[PageOwner::Kernel.index()],
            page_table_pages: self.page_owners[PageOwner::PageTable.index()],
            guest_pages: self.page_owners[PageOwner::Guest.index()],
        }
    }

    fn allocate_pages(
        &mut self,
        order: usize,
        owner: PageOwner,
    ) -> Result<PhysicalAddress, BuddyError> {
        let owner_stats = &mut self.page_owners[owner.index()];
        owner_stats.allocation_requests = owner_stats.allocation_requests.saturating_add(1);
        match self.buddy.allocate(order) {
            Ok(address) => {
                let pages = match 1usize.checked_shl(order as u32) {
                    Some(pages) => pages,
                    None => allocator_fault(AllocatorFault::AllocationCountOverflow),
                };
                owner_stats.pages = match owner_stats.pages.checked_add(pages) {
                    Some(pages) => pages,
                    None => allocator_fault(AllocatorFault::AllocationCountOverflow),
                };
                owner_stats.peak_pages = owner_stats.peak_pages.max(owner_stats.pages);
                Ok(address)
            }
            Err(error) => {
                owner_stats.allocation_failures = owner_stats.allocation_failures.saturating_add(1);
                Err(error)
            }
        }
    }

    unsafe fn deallocate_pages(
        &mut self,
        address: PhysicalAddress,
        order: usize,
        owner: PageOwner,
    ) -> Result<(), BuddyError> {
        let pages = 1usize
            .checked_shl(order as u32)
            .ok_or(BuddyError::InvalidOrder)?;
        let owner_stats = &mut self.page_owners[owner.index()];
        if owner_stats.pages < pages {
            allocator_fault(AllocatorFault::PageOwnerUnderflow);
        }
        // SAFETY: The caller guarantees this exact live owner/order allocation;
        // accounting above validates its page quantity before relinquishment.
        unsafe { self.buddy.deallocate(address, order)? };
        owner_stats.pages -= pages;
        owner_stats.deallocations = owner_stats.deallocations.saturating_add(1);
        Ok(())
    }
}

fn class_for(layout: Layout) -> Option<usize> {
    let required = layout.size().max(layout.align()).max(1);
    CLASS_SIZES.iter().position(|&size| size >= required)
}

fn align_up_usize(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
}

fn increment(value: &mut usize) {
    *value = match value.checked_add(1) {
        Some(value) => value,
        None => allocator_fault(AllocatorFault::AllocationCountOverflow),
    };
}
