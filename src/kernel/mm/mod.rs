//! Kernel memory policy layered over reusable memory-management mechanisms.

use alloc::boxed::Box;
use alloc::vec::Vec;
use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use hyper::hal::cache::CacheMaintenance;

pub mod allocator;
pub mod memory;
pub mod page_block;

pub use memory::PreparedMemory;

/// Activates the kernel allocator and validates Rust allocation plumbing.
pub(crate) fn initialize() {
    if let Err(error) = super::boot::with_boot_state(|state| {
        state
            .memory
            .initialize_global_allocator(&allocator::GLOBAL_ALLOCATOR)
    }) {
        super::boot::fail("global allocator initialization", error);
    }
    if let Err(error) = verify_rust_allocator_interface() {
        super::boot::fail("Rust allocator interface validation", error);
    }
}

/// Removes bootstrap-only mappings and reports the permanent memory layout.
pub(crate) fn finalize_address_space() {
    if let Err(error) =
        super::boot::with_boot_state(|state| state.memory.retire_identity_mappings(&state.platform))
    {
        super::boot::fail("identity-map retirement", error);
    }

    let layout = memory::virtual_memory_layout();
    crate::arch::ArchitectureBarrier::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
    let data_cache_line = crate::arch::ArchitectureCache::data_line_size();
    let instruction_cache_line = crate::arch::ArchitectureCache::instruction_line_size();
    let atomic_capabilities: crate::arch::AtomicCapabilities = crate::arch::atomic_capabilities();
    super::boot::with_boot_state(|state| {
        crate::println!("HypeR: final address space active");
        crate::println!("HypeR: transition identity mappings retired");
        crate::println!("HypeR: linear map base {:#x}", layout.linear_base);
        crate::println!("HypeR: MMIO map base {:#x}", layout.mmio_base);
        crate::println!(
            "HypeR: randomized kernel base {:#x}, KASLR offset {:#x}",
            state.memory.kernel_base(),
            state.memory.kernel_base() - layout.kernel_base
        );
        crate::println!("HypeR: DTB physical address {:#x}", state.dtb_address);
        crate::println!(
            "HypeR: cache line sizes: data {} bytes, instruction {} bytes",
            data_cache_line,
            instruction_cache_line
        );
        crate::println!(
            "HypeR: atomic RMW backend: {}",
            if atomic_capabilities.lse {
                "LSE"
            } else {
                "LL/SC"
            }
        );
        crate::println!(
            "HypeR: {} boot reservations, {} RAM regions, root {:#x}",
            state.memory.reservation_count(),
            state.platform.memory.len(),
            state.memory.root_address()
        );
    });
    if let Some(stats) = allocator::GLOBAL_ALLOCATOR.stats() {
        crate::println!(
            "HypeR: global buddy/slab allocator active: {} free pages, {} live allocations",
            stats.free_pages,
            stats.live_allocations
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocatorSmokeError {
    BoxValue,
    VectorContents,
    VectorLength,
}

fn verify_rust_allocator_interface() -> Result<(), AllocatorSmokeError> {
    let boxed = Box::new(0x0048_5950_4552_u64);
    let mut vector = Vec::with_capacity(1024);
    for value in 0..1024u64 {
        vector.push(value);
    }
    if *boxed != 0x0048_5950_4552 {
        return Err(AllocatorSmokeError::BoxValue);
    }
    if vector.len() != 1024 {
        return Err(AllocatorSmokeError::VectorLength);
    }
    if vector.get(1023) != Some(&1023) {
        return Err(AllocatorSmokeError::VectorContents);
    }
    drop(vector);
    drop(boxed);
    Ok(())
}
