// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One independently reclaimable physical page, with an initialized prefix.

#[cfg(test)]
use alloc::vec::Vec;

use super::CacheError;

const PAGE_BYTES: usize = hyper::mm::PAGE_SIZE as usize;

pub(super) struct PageBytes {
    #[cfg(not(test))]
    _page: crate::kernel::mm::page_block::PageBlock,
    #[cfg(not(test))]
    address: usize,
    #[cfg(not(test))]
    length: usize,
    #[cfg(test)]
    bytes: Vec<u8>,
}

impl PageBytes {
    pub(super) fn try_copy(bytes: &[u8]) -> Result<Self, CacheError> {
        if bytes.len() > PAGE_BYTES {
            return Err(CacheError::InvalidCapacity);
        }
        #[cfg(not(test))]
        {
            let page = crate::kernel::mm::cache_memory::allocate_page()
                .map_err(|_| CacheError::Allocation)?;
            let address = crate::kernel::mm::memory::linear_address(page.physical().get())
                .ok_or(CacheError::Allocation)?;
            let mut storage = Self {
                _page: page,
                address,
                length: 0,
            };
            storage.replace(bytes);
            Ok(storage)
        }
        #[cfg(test)]
        {
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(PAGE_BYTES)
                .map_err(|_| CacheError::Allocation)?;
            owned.extend_from_slice(bytes);
            Ok(Self { bytes: owned })
        }
    }

    pub(super) fn replace(&mut self, bytes: &[u8]) {
        if bytes.len() > PAGE_BYTES {
            super::cache_invariant_violation();
        }
        #[cfg(not(test))]
        {
            // SAFETY: This owner retains one order-0 block in the permanent
            // writable RAM map. Exclusive access prevents a reader observing
            // replacement; only the initialized prefix becomes visible.
            unsafe {
                core::ptr::with_exposed_provenance_mut::<u8>(self.address)
                    .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
            }
            self.length = bytes.len();
        }
        #[cfg(test)]
        {
            self.bytes.clear();
            self.bytes.extend_from_slice(bytes);
        }
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        #[cfg(not(test))]
        {
            // SAFETY: The retained block outlives this borrow, and exactly this
            // prefix was initialized before publication. Mutation needs &mut.
            unsafe {
                core::slice::from_raw_parts(
                    core::ptr::with_exposed_provenance::<u8>(self.address),
                    self.length,
                )
            }
        }
        #[cfg(test)]
        &self.bytes
    }
}
