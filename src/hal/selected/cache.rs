// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected host cache-maintenance capabilities.
//!
//! Callers own buffer lifetime, cache-line exclusivity, and publication policy.
//! This facade selects only the cache geometry and instructions needed to make
//! data or instructions visible in the architecture's coherence domains.

use hyper::hal::cache::{CacheError, CacheMaintenance};

pub(crate) fn prepare(platform: &super::platform::EssentialInfo) -> Result<(), CacheError> {
    crate::arch::memory::prepare_cache(platform.as_backend())?;
    // Kernel cache-publication owners are page-granular. Requiring every line
    // to divide one page proves that rounding an owned page-local chunk cannot
    // reach an adjacent allocation. Supporting a larger line would require a
    // correspondingly larger ownership unit throughout the memory subsystem.
    if !valid_page_subdivision(data_line_size()) || !valid_page_subdivision(instruction_line_size())
    {
        return Err(CacheError::InvalidLineSize);
    }
    Ok(())
}

fn valid_page_subdivision(line_size: usize) -> bool {
    let Ok(page_size) = usize::try_from(hyper::mm::PAGE_SIZE) else {
        return false;
    };
    line_size != 0 && line_size.is_power_of_two() && line_size <= page_size
}

pub(crate) fn data_line_size() -> usize {
    crate::arch::memory::Cache::data_line_size()
}

pub(crate) fn instruction_line_size() -> usize {
    crate::arch::memory::Cache::instruction_line_size()
}

/// Publishes CPU writes to the platform's coherent memory domain.
///
/// # Safety
///
/// The complete cache-line-rounded range must be mapped and readable. The
/// caller must own the buffer and exclude concurrent CPU writes until the
/// receiving agent has acquired ownership. This is not a DMA completion API.
pub(crate) unsafe fn publish_data_range(start: usize, length: usize) -> Result<(), CacheError> {
    // SAFETY: The facade forwards mapped-range ownership and writer exclusion.
    unsafe { crate::arch::memory::Cache::publish_data_range(start, length) }
}

/// Publishes newly written instructions to the instruction-coherence domain.
///
/// # Safety
///
/// The range must be mapped and writable before this call, with concurrent
/// execution and modification excluded. Every CPU that later executes it must
/// perform [`synchronize_instruction_execution`] after observing publication.
pub(crate) unsafe fn publish_instruction_range(
    start: usize,
    length: usize,
) -> Result<(), CacheError> {
    // SAFETY: The facade forwards mapping, ownership, and execution exclusion.
    unsafe { crate::arch::memory::Cache::publish_instruction_range(start, length) }
}

/// Completes local instruction-stream synchronization after code publication.
pub(crate) fn synchronize_instruction_execution() {
    crate::arch::memory::Cache::synchronize_instruction_execution();
}
