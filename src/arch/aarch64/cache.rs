// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::arch::asm;

use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use hyper::hal::cache::{CacheError, CacheMaintenance};
use hyper::vm::aarch64::cache::{GuestInstructionCachePolicy, guest_instruction_cache_policy};

use super::barrier::Aarch64Barrier;
use super::registers;

/// `AArch64` cache maintenance using virtual-address operations at EL2.
pub struct Aarch64Cache;

impl CacheMaintenance for Aarch64Cache {
    fn data_line_size() -> usize {
        cache_line_size(registers::CTR_EL0_DMINLINE_SHIFT)
    }

    fn instruction_line_size() -> usize {
        cache_line_size(registers::CTR_EL0_IMINLINE_SHIFT)
    }

    unsafe fn publish_data_range(start: usize, length: usize) -> Result<(), CacheError> {
        // SAFETY: The trait contract guarantees the rounded range is mapped
        // and remains exclusively owned for this maintenance operation.
        unsafe { maintain_range(start, length, Self::data_line_size(), dc_cvac)? };
        Aarch64Barrier::data_synchronization(BarrierDomain::FullSystem, BarrierAccess::All);
        Ok(())
    }

    unsafe fn discard_data_range(start: usize, length: usize) -> Result<(), CacheError> {
        Aarch64Barrier::data_synchronization(BarrierDomain::FullSystem, BarrierAccess::All);
        // SAFETY: The caller owns every rounded line and guarantees that
        // invalidation cannot discard unrelated dirty state.
        unsafe { maintain_range(start, length, Self::data_line_size(), dc_ivac)? };
        Aarch64Barrier::data_synchronization(BarrierDomain::FullSystem, BarrierAccess::All);
        Ok(())
    }

    unsafe fn publish_and_discard_data_range(
        start: usize,
        length: usize,
    ) -> Result<(), CacheError> {
        Aarch64Barrier::data_synchronization(BarrierDomain::FullSystem, BarrierAccess::All);
        // SAFETY: Exclusive ownership of the rounded range is required by the
        // trait contract and held through both maintenance phases.
        unsafe { maintain_range(start, length, Self::data_line_size(), dc_civac)? };
        Aarch64Barrier::data_synchronization(BarrierDomain::FullSystem, BarrierAccess::All);
        Ok(())
    }

    unsafe fn publish_instruction_range(start: usize, length: usize) -> Result<(), CacheError> {
        if length == 0 {
            return Ok(());
        }
        // SAFETY: The one yielded range has the mapping and ownership promised
        // by the single-range trait contract and remains stable for both
        // possible enumeration passes.
        unsafe { Self::publish_instruction_ranges(|visit| visit(start, length)) }
    }

    unsafe fn publish_instruction_ranges(
        mut ranges: impl FnMut(&mut dyn FnMut(usize, usize)),
    ) -> Result<(), CacheError> {
        let ctr = read_ctr_el0();
        let data_line_size = cache_line_size_from(ctr, registers::CTR_EL0_DMINLINE_SHIFT);
        let mut clean_result = Ok(());
        ranges(&mut |start, length| {
            if clean_result.is_ok() {
                // SAFETY: The batch contract keeps every enumerated range
                // mapped and exclusively owned throughout both phases.
                clean_result = unsafe { maintain_range(start, length, data_line_size, dc_cvau) };
            }
        });
        // All dirty instruction bytes must reach PoU before any instruction
        // cache invalidation. One barrier covers the complete batch.
        Aarch64Barrier::data_synchronization(BarrierDomain::InnerShareable, BarrierAccess::All);
        clean_result?;

        let invalidate_result = match guest_instruction_cache_policy(ctr) {
            GuestInstructionCachePolicy::Range => {
                let instruction_line_size =
                    cache_line_size_from(ctr, registers::CTR_EL0_IMINLINE_SHIFT);
                let mut result = Ok(());
                ranges(&mut |start, length| {
                    if result.is_ok() {
                        // SAFETY: PIPT indexing makes each stable host VA
                        // sufficient to select all copies of that line.
                        result = unsafe {
                            maintain_range(start, length, instruction_line_size, ic_ivau)
                        };
                    }
                });
                result
            }
            GuestInstructionCachePolicy::WholeInnerShareableDomain => {
                // SAFETY: Guest execution uses a different VA from the host
                // linear alias. IALLUIS covers every alias in the execution
                // domain without dereferencing a virtual address.
                unsafe { invalidate_instruction_domain() };
                Ok(())
            }
        };
        // Complete every invalidation issued before returning, including when
        // validation of a later range detected an address overflow.
        Aarch64Barrier::data_synchronization(BarrierDomain::InnerShareable, BarrierAccess::All);
        invalidate_result
    }

    fn synchronize_instruction_execution() {
        Aarch64Barrier::instruction_synchronization();
    }

    fn invalidate_instruction_all() {
        // SAFETY: IALLUIS invalidates instruction-cache entries in the
        // inner-shareable domain and does not dereference a virtual address.
        unsafe { invalidate_instruction_domain() };
        Aarch64Barrier::data_synchronization(BarrierDomain::InnerShareable, BarrierAccess::All);
        Aarch64Barrier::instruction_synchronization();
    }
}

fn cache_line_size(shift: u64) -> usize {
    cache_line_size_from(read_ctr_el0(), shift)
}

fn read_ctr_el0() -> u64 {
    let ctr: u64;
    // SAFETY: CTR_EL0 is readable at EL2 and has no side effects.
    unsafe {
        asm!(
            "mrs {ctr}, ctr_el0",
            ctr = out(reg) ctr,
            options(nomem, nostack, preserves_flags)
        );
    }
    ctr
}

const fn cache_line_size_from(ctr: u64, shift: u64) -> usize {
    let encoded = (ctr >> shift) & registers::CTR_EL0_LINE_SIZE_MASK;
    4usize << encoded
}

unsafe fn invalidate_instruction_domain() {
    // SAFETY: IALLUIS has no address operand and invalidates instruction-cache
    // entries throughout the inner-shareable domain.
    unsafe { asm!("ic ialluis", options(nostack, preserves_flags)) };
}

unsafe fn maintain_range(
    start: usize,
    length: usize,
    line_size: usize,
    operation: unsafe fn(usize),
) -> Result<(), CacheError> {
    if length == 0 {
        return Ok(());
    }
    let end = start
        .checked_add(length)
        .ok_or(CacheError::AddressOverflow)?;
    let mut line = start & !(line_size - 1);
    while line < end {
        // SAFETY: The caller guarantees each rounded line is mapped and valid
        // for the selected cache operation.
        unsafe { operation(line) };
        line = line
            .checked_add(line_size)
            .ok_or(CacheError::AddressOverflow)?;
    }
    Ok(())
}

unsafe fn dc_cvac(address: usize) {
    // Clean by virtual address to the AArch64 point of coherency.
    // SAFETY: The caller guarantees that this cache line is mapped.
    unsafe { asm!("dc cvac, {address}", address = in(reg) address, options(nostack)) };
}

unsafe fn dc_cvau(address: usize) {
    // Clean by virtual address to the AArch64 point of unification.
    // SAFETY: The caller guarantees that this cache line is mapped.
    unsafe { asm!("dc cvau, {address}", address = in(reg) address, options(nostack)) };
}

unsafe fn dc_ivac(address: usize) {
    // Invalidate by virtual address to the AArch64 point of coherency.
    // SAFETY: The caller guarantees exclusive ownership of this cache line.
    unsafe { asm!("dc ivac, {address}", address = in(reg) address, options(nostack)) };
}

unsafe fn dc_civac(address: usize) {
    // Clean and invalidate by virtual address to the AArch64 point of coherency.
    // SAFETY: The caller guarantees exclusive ownership of this cache line.
    unsafe { asm!("dc civac, {address}", address = in(reg) address, options(nostack)) };
}

unsafe fn ic_ivau(address: usize) {
    // Invalidate by virtual address to the AArch64 point of unification.
    // SAFETY: The caller guarantees that this cache line is mapped.
    unsafe { asm!("ic ivau, {address}", address = in(reg) address, options(nostack)) };
}
