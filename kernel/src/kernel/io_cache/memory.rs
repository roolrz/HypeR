// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Pressure admission for metadata; physical payloads use the page allocator.

use core::alloc::Layout;

use super::CacheError;

#[cfg(not(test))]
pub(super) type Reservation = crate::kernel::mm::cache_memory::MetadataReservation;
#[cfg(test)]
pub(super) struct Reservation;

#[cfg(test)]
impl Drop for Reservation {
    fn drop(&mut self) {
        // Host allocations have no physical admission counter, but keep the
        // same explicit reservation lifetime as the production RAII token.
    }
}

pub(super) fn reserve(layouts: &[Layout]) -> Result<(Reservation, usize), CacheError> {
    let mut pages = 0_usize;
    for &layout in layouts {
        let bound = allocation_pages(layout).ok_or(CacheError::InvalidCapacity)?;
        pages = pages
            .checked_add(bound)
            .ok_or(CacheError::InvalidCapacity)?;
    }
    #[cfg(not(test))]
    let reservation =
        crate::kernel::mm::cache_memory::reserve_metadata(pages).ok_or(CacheError::Allocation)?;
    #[cfg(test)]
    let reservation = Reservation;
    Ok((reservation, pages))
}

fn allocation_pages(layout: Layout) -> Option<usize> {
    #[cfg(not(test))]
    {
        crate::kernel::mm::cache_memory::allocation_page_bound(layout)
    }
    #[cfg(test)]
    {
        let page = hyper::mm::PAGE_SIZE as usize;
        layout
            .size()
            .max(layout.align())
            .max(1)
            .checked_add(page - 1)
            .and_then(|bytes| (bytes / page).checked_next_power_of_two())
    }
}

pub(super) fn admission_allowed() -> bool {
    #[cfg(not(test))]
    {
        crate::kernel::mm::cache_memory::admission_allowed()
    }
    #[cfg(test)]
    true
}
