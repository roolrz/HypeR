//! RAII ownership for physically contiguous kernel page allocations.

use hyper::mm::{BuddyError, PhysicalAddress};

/// A live buddy allocation returned to the kernel by the global page allocator.
///
/// This type is intentionally kernel-owned: allocation policy and the fatal
/// response to allocator corruption do not belong in the reusable buddy core.
pub struct PageBlock {
    physical: PhysicalAddress,
    order: usize,
}

impl PageBlock {
    pub fn allocate(order: usize) -> Result<Self, BuddyError> {
        let physical = super::allocator::GLOBAL_ALLOCATOR.allocate_pages(order)?;
        Ok(Self { physical, order })
    }

    pub const fn physical(&self) -> PhysicalAddress {
        self.physical
    }

    pub const fn order(&self) -> usize {
        self.order
    }
}

impl Drop for PageBlock {
    fn drop(&mut self) {
        // SAFETY: PageBlock is the unique owner of this exact buddy block and
        // relinquishes it exactly once from Drop.
        if let Err(error) = unsafe {
            super::allocator::GLOBAL_ALLOCATOR.deallocate_pages(self.physical, self.order)
        } {
            crate::pr_crit!(
                "HypeR: failed to release page block at {:#x}, order {}: {error:?}",
                self.physical.get(),
                self.order
            );
            crate::arch::halt();
        }
    }
}
