// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::arch::asm;

use hyper::hal::cache::{CacheError, CacheMaintenance};
use hyper::sync::atomic::{AtomicUsize, Ordering};

static BLOCK_SIZE: AtomicUsize = AtomicUsize::new(0);

pub struct Riscv64Cache;

pub fn initialize(block_size: usize) -> Result<(), CacheError> {
    if block_size == 0 || !block_size.is_power_of_two() {
        return Err(CacheError::InvalidLineSize);
    }
    BLOCK_SIZE.store(block_size, Ordering::Release);
    Ok(())
}

impl CacheMaintenance for Riscv64Cache {
    fn data_line_size() -> usize {
        BLOCK_SIZE.load(Ordering::Acquire)
    }

    fn instruction_line_size() -> usize {
        BLOCK_SIZE.load(Ordering::Acquire)
    }

    unsafe fn publish_data_range(start: usize, length: usize) -> Result<(), CacheError> {
        // SAFETY: The trait contract guarantees an accessible complete range.
        unsafe { maintain_range(start, length, riscv64_cbo_clean) }
    }

    unsafe fn discard_data_range(start: usize, length: usize) -> Result<(), CacheError> {
        // SAFETY: The trait contract guarantees an accessible complete range.
        unsafe { maintain_range(start, length, riscv64_cbo_inval) }
    }

    unsafe fn publish_and_discard_data_range(
        start: usize,
        length: usize,
    ) -> Result<(), CacheError> {
        // SAFETY: The trait contract guarantees an accessible complete range.
        unsafe { maintain_range(start, length, riscv64_cbo_flush) }
    }

    unsafe fn publish_instruction_range(start: usize, length: usize) -> Result<(), CacheError> {
        // SAFETY: The trait contract guarantees the range; cleaning precedes FENCE.I.
        unsafe { maintain_range(start, length, riscv64_cbo_clean)? };
        // SAFETY: FENCE.I has no pointer operands and is valid in HS mode.
        unsafe { asm!("fence.i", options(nostack)) };
        Ok(())
    }

    fn synchronize_instruction_execution() {
        // SAFETY: FENCE.I has no pointer operands and is valid in HS mode.
        unsafe { asm!("fence.i", options(nostack)) };
    }

    fn invalidate_instruction_all() {
        // SAFETY: FENCE.I has no pointer operands and is valid in HS mode.
        unsafe { asm!("fence.i", options(nostack)) };
    }
}

unsafe fn maintain_range(
    start: usize,
    length: usize,
    operation: unsafe extern "C" fn(usize),
) -> Result<(), CacheError> {
    let block_size = BLOCK_SIZE.load(Ordering::Acquire);
    if block_size == 0 {
        return Err(CacheError::NotInitialized);
    }
    let end = start
        .checked_add(length)
        .ok_or(CacheError::AddressOverflow)?;
    if length == 0 {
        return Ok(());
    }
    let mut address = start & !(block_size - 1);
    let rounded_end = end
        .checked_add(block_size - 1)
        .map(|value| value & !(block_size - 1))
        .ok_or(CacheError::AddressOverflow)?;

    // CBOs are ordered as writes or device outputs. Full fences also order
    // ordinary memory and MMIO around ownership transfers to other agents.
    // SAFETY: FENCE has no pointer operands and orders accesses before the CBOs.
    unsafe { asm!("fence iorw, iorw", options(nostack)) };
    while address < rounded_end {
        // SAFETY: The caller guarantees the rounded blocks are accessible and
        // `operation` is one of this module's CBO assembly routines.
        unsafe { operation(address) };
        address = address
            .checked_add(block_size)
            .ok_or(CacheError::AddressOverflow)?;
    }
    // SAFETY: FENCE has no pointer operands and orders the completed CBOs.
    unsafe { asm!("fence iorw, iorw", options(nostack)) };
    Ok(())
}

unsafe extern "C" {
    fn riscv64_cbo_clean(address: usize);
    fn riscv64_cbo_flush(address: usize);
    fn riscv64_cbo_inval(address: usize);
}
