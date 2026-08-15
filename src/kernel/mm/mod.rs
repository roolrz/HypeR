//! Kernel memory policy layered over reusable memory-management mechanisms.

use alloc::boxed::Box;
use alloc::vec::Vec;
use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use hyper::hal::cache::CacheMaintenance;

pub mod allocator;
pub mod memory;
pub mod page_block;
pub mod stack;

pub use memory::PreparedMemory;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryStats {
    pub boot: hyper::mm::BootMemoryStats,
    pub runtime: hyper::mm::heap::HeapStats,
}

/// Returns a lock-consistent allocator snapshot once memory initialization is complete.
pub fn statistics() -> Option<MemoryStats> {
    let boot = super::boot::try_with_boot_state(|state| state.memory.boot_memory_stats())?;
    let runtime = allocator::GLOBAL_ALLOCATOR.stats()?;
    Some(MemoryStats { boot, runtime })
}

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
    if let Err(error) = stack::prepare_cpu(crate::arch::current_cpu_index()) {
        super::boot::fail("bootstrap exception-stack initialization", error);
    }
    crate::println!(
        "HypeR: guarded kernel stacks: thread {} KiB, per-CPU IRQ {} KiB, emergency {} KiB",
        hyper::config::KERNEL_STACK_SIZE_KB,
        hyper::config::IRQ_STACK_SIZE_KB,
        hyper::config::EMERGENCY_STACK_SIZE_KB
    );
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
    report_statistics("kernel initialized");
}

pub(crate) fn report_statistics(reason: &str) {
    let Some(stats) = statistics() else {
        return;
    };
    let runtime = stats.runtime;
    let buddy = runtime.buddy;
    crate::println!(
        "HypeR: memory ({reason}): {} MiB RAM, {} reserved pages, {} managed pages",
        stats.boot.ram_pages * hyper::mm::PAGE_SIZE as usize / (1024 * 1024),
        stats.boot.reserved_pages,
        buddy.managed_pages
    );
    crate::println!(
        "HypeR: pages: {} allocated (peak {}), {} free, largest free order {}, {} failures",
        buddy.allocated_pages,
        buddy.peak_allocated_pages,
        buddy.free_pages,
        buddy.largest_free_order().map_or(0, |order| order),
        buddy.allocation_failures
    );
    crate::println!(
        "HypeR: page owners: guest {} (peak {}), page tables {}, kernel {}, heap {}",
        runtime.guest_pages.pages,
        runtime.guest_pages.peak_pages,
        runtime.page_table_pages.pages,
        runtime.kernel_pages.pages,
        runtime.slab_pages + runtime.large_heap_pages
    );
    crate::println!(
        "HypeR: heap: {} live objects (peak {}), {} KiB requested (peak {} KiB), {} failures",
        runtime.live_allocations,
        runtime.peak_live_allocations,
        runtime.requested_bytes.div_ceil(1024),
        runtime.peak_requested_bytes.div_ceil(1024),
        runtime.allocation_failures
    );
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
