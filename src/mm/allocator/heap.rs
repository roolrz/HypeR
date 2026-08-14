//! Slab heap and Rust GlobalAlloc adapter.

use core::alloc::{GlobalAlloc, Layout};
use core::hint::spin_loop;
use core::mem::MaybeUninit;
use core::ptr::{null_mut, write_bytes};

use crate::hal::interrupt::InterruptMask;
use crate::sync::InterruptSpinLock;
use crate::sync::atomic::{AtomicU8, Ordering};

use crate::mm::{BuddyAllocator, BuddyError, MAX_ORDER, MemoryHandoff, PAGE_SIZE, PhysicalAddress};

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

#[derive(Clone, Copy, Debug)]
pub struct HeapStats {
    pub free_pages: usize,
    pub slab_pages: usize,
    pub live_allocations: usize,
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

pub struct SlabAllocator {
    buddy: BuddyAllocator,
    partial: [u64; CLASS_SIZES.len()],
    slab_pages: usize,
    live_allocations: usize,
}

// SAFETY: All access occurs while `GLOBAL_ALLOCATOR.state` is locked.
unsafe impl Send for SlabAllocator {}

impl SlabAllocator {
    /// Creates a slab heap backed by an intrusive buddy allocator.
    ///
    /// # Safety
    ///
    /// The direct map must remain writable for the allocator's lifetime.
    pub unsafe fn from_handoff(
        handoff: &MemoryHandoff,
        direct_map_base: u64,
    ) -> Result<Self, BuddyError> {
        Ok(Self {
            buddy: unsafe { BuddyAllocator::from_handoff(handoff, direct_map_base)? },
            partial: [NONE; CLASS_SIZES.len()],
            slab_pages: 0,
            live_allocations: 0,
        })
    }

    /// Allocates memory satisfying `layout`.
    ///
    /// # Safety
    ///
    /// The result must be released once with the same layout.
    pub unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        if let Some(class) = class_for(layout) {
            unsafe { self.allocate_slab(class) }
        } else {
            unsafe { self.allocate_large(layout) }
        }
    }

    /// Releases a live allocation returned by `allocate`.
    ///
    /// # Safety
    ///
    /// `pointer` and `layout` must describe one live allocation from this heap.
    pub unsafe fn deallocate(&mut self, pointer: *mut u8, layout: Layout) {
        if let Some(class) = class_for(layout) {
            unsafe { self.deallocate_slab(pointer, class) };
        } else {
            unsafe { self.deallocate_large(pointer) };
        }
    }

    unsafe fn allocate_slab(&mut self, class: usize) -> *mut u8 {
        if self.partial[class] == NONE && unsafe { self.create_slab(class) }.is_err() {
            return null_mut();
        }

        let physical = self.partial[class];
        let Some(header) = self.slab_header(physical) else {
            return null_mut();
        };
        // SAFETY: `physical` is the head of this class's partial-slab list.
        let header = unsafe { &mut *header };
        if header.magic != SLAB_MAGIC {
            allocator_fault(AllocatorFault::InvalidSlabHeader);
        }
        if usize::from(header.class) != class {
            allocator_fault(AllocatorFault::SlabClassMismatch);
        }
        let pointer = header.free_head as *mut u8;
        if pointer.is_null() {
            return null_mut();
        }
        // SAFETY: Every free object stores the next virtual pointer in its first
        // machine word.
        header.free_head = unsafe { *(pointer as *const usize) };
        header.in_use += 1;
        self.live_allocations += 1;
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
        if header.in_use == 0 || self.live_allocations == 0 {
            allocator_fault(AllocatorFault::AllocationCountUnderflow);
        }
        let was_full = header.free_head == 0;
        unsafe { *(pointer as *mut usize) = header.free_head };
        header.free_head = pointer as usize;
        header.in_use -= 1;
        self.live_allocations -= 1;

        if was_full {
            header.next_partial = self.partial[class];
            self.partial[class] = physical;
        }
        if header.in_use == 0 {
            unsafe { self.remove_partial(class, physical) };
            header.magic = 0;
            self.slab_pages -= 1;
            // SAFETY: The empty slab page is exclusively owned by this heap.
            if unsafe { self.buddy.deallocate(PhysicalAddress::new(physical), 0) }.is_err() {
                allocator_fault(AllocatorFault::BuddyDeallocation);
            }
        }
    }

    unsafe fn create_slab(&mut self, class: usize) -> Result<(), BuddyError> {
        let physical = self.buddy.allocate(0)?.get();
        let header_pointer = self
            .slab_header(physical)
            .ok_or(BuddyError::Unaddressable)?;
        let object_size = CLASS_SIZES[class];
        let page_virtual = header_pointer as usize;
        let object_start = align_up_usize(
            page_virtual + core::mem::size_of::<SlabHeader>(),
            object_size,
        );
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
                next_partial: self.partial[class],
                free_head: object_start,
                in_use: 0,
                capacity: capacity as u16,
                class: class as u8,
                _reserved: [0; 3],
            })
        };
        self.partial[class] = physical;
        self.slab_pages += 1;
        Ok(())
    }

    unsafe fn remove_partial(&mut self, class: usize, target: u64) {
        let mut previous = NONE;
        let mut current = self.partial[class];
        while current != NONE {
            let header_pointer = match self.slab_header(current) {
                Some(header) => header,
                None => allocator_fault(AllocatorFault::InvalidPartialList),
            };
            // SAFETY: `current` came from the class's slab list.
            let next = unsafe { (*header_pointer).next_partial };
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

    unsafe fn allocate_large(&mut self, layout: Layout) -> *mut u8 {
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
        let physical = match self.buddy.allocate(order) {
            Ok(address) => address.get(),
            Err(_) => return null_mut(),
        };
        let Some(base) = self.virtual_address(physical) else {
            // SAFETY: This allocation has not been exposed to a caller.
            if unsafe { self.buddy.deallocate(PhysicalAddress::new(physical), order) }.is_err() {
                allocator_fault(AllocatorFault::BuddyDeallocation);
            }
            return null_mut();
        };
        let user = align_up_usize(base + header_size, layout.align());
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
        self.live_allocations += 1;
        user as *mut u8
    }

    unsafe fn deallocate_large(&mut self, pointer: *mut u8) {
        let header_size = core::mem::size_of::<LargeHeader>();
        // SAFETY: GlobalAlloc requires `pointer` to be a live large allocation.
        let header = unsafe { *((pointer as usize - header_size) as *const LargeHeader) };
        if header.magic != LARGE_MAGIC {
            allocator_fault(AllocatorFault::InvalidLargeHeader);
        }
        if self.live_allocations == 0 {
            allocator_fault(AllocatorFault::AllocationCountUnderflow);
        }
        self.live_allocations -= 1;
        // SAFETY: The header records the exact buddy allocation.
        if unsafe {
            self.buddy.deallocate(
                PhysicalAddress::new(header.physical),
                usize::from(header.order),
            )
        }
        .is_err()
        {
            allocator_fault(AllocatorFault::BuddyDeallocation);
        }
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
        HeapStats {
            free_pages: self.buddy.free_pages(),
            slab_pages: self.slab_pages,
            live_allocations: self.live_allocations,
        }
    }
}

struct AllocatorState {
    initialized: bool,
    heap: MaybeUninit<SlabAllocator>,
}

impl AllocatorState {
    const fn uninitialized() -> Self {
        Self {
            initialized: false,
            heap: MaybeUninit::uninit(),
        }
    }

    unsafe fn heap_mut(&mut self) -> Option<&mut SlabAllocator> {
        if self.initialized {
            // SAFETY: `initialized` is set only after writing a SlabAllocator.
            Some(unsafe { self.heap.assume_init_mut() })
        } else {
            None
        }
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
    /// Normal memory. Initialization must occur exactly once before allocation.
    pub unsafe fn initialize(
        &self,
        handoff: &MemoryHandoff,
        direct_map_base: u64,
    ) -> Result<(), InitError> {
        self.state.with(|state| {
            if state.initialized {
                return Err(InitError::AlreadyInitialized);
            }
            state
                .heap
                .write(unsafe { SlabAllocator::from_handoff(handoff, direct_map_base)? });
            state.initialized = true;
            Ok(())
        })
    }

    pub fn stats(&self) -> Option<HeapStats> {
        self.state.with(|state| {
            // SAFETY: Access is serialized and checked by `initialized`.
            unsafe { state.heap_mut() }.map(|heap| heap.stats())
        })
    }
}

impl<M: InterruptMask> Default for KernelGlobalAllocator<M> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: The state lock serializes allocator metadata. Returned allocations
// are disjoint and remain owned by callers until a matching deallocation.
unsafe impl<M: InterruptMask> GlobalAlloc for KernelGlobalAllocator<M> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.state.with(|state| {
            // SAFETY: Access is serialized and checked by `initialized`.
            match unsafe { state.heap_mut() } {
                Some(heap) => unsafe { heap.allocate(layout) },
                None => null_mut(),
            }
        })
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.alloc(layout) };
        if !pointer.is_null() {
            unsafe { write_bytes(pointer, 0, layout.size()) };
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        self.state.with(|state| {
            // SAFETY: Access is serialized and checked by `initialized`.
            if let Some(heap) = unsafe { state.heap_mut() } {
                unsafe { heap.deallocate(pointer, layout) };
            }
        });
    }
}

fn class_for(layout: Layout) -> Option<usize> {
    let required = layout.size().max(layout.align()).max(1);
    CLASS_SIZES.iter().position(|&size| size >= required)
}

fn align_up_usize(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}
