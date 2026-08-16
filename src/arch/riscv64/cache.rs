use core::arch::asm;

use hyper::hal::cache::{CacheError, CacheMaintenance};

pub struct Riscv64Cache;

impl CacheMaintenance for Riscv64Cache {
    fn data_line_size() -> usize {
        64
    }

    fn instruction_line_size() -> usize {
        64
    }

    unsafe fn publish_data_range(start: usize, length: usize) -> Result<(), CacheError> {
        checked_end(start, length)?;
        unsafe { asm!("fence rw, rw", options(nostack)) };
        Ok(())
    }

    unsafe fn discard_data_range(start: usize, length: usize) -> Result<(), CacheError> {
        checked_end(start, length)?;
        unsafe { asm!("fence rw, rw", options(nostack)) };
        Ok(())
    }

    unsafe fn publish_and_discard_data_range(
        start: usize,
        length: usize,
    ) -> Result<(), CacheError> {
        checked_end(start, length)?;
        unsafe { asm!("fence rw, rw", options(nostack)) };
        Ok(())
    }

    unsafe fn publish_instruction_range(start: usize, length: usize) -> Result<(), CacheError> {
        checked_end(start, length)?;
        unsafe { asm!("fence rw, rw", "fence.i", options(nostack)) };
        Ok(())
    }

    fn synchronize_instruction_execution() {
        unsafe { asm!("fence.i", options(nostack)) };
    }

    fn invalidate_instruction_all() {
        unsafe { asm!("fence.i", options(nostack)) };
    }
}

fn checked_end(start: usize, length: usize) -> Result<usize, CacheError> {
    start.checked_add(length).ok_or(CacheError::AddressOverflow)
}
