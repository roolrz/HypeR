//! Kernel memory policy layered over reusable memory-management mechanisms.

use alloc::vec::Vec;
use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use hyper::hal::cache::CacheMaintenance;
use hyper::sync::atomic::{AtomicBool, Ordering};

pub mod allocator;
pub mod memory;
pub mod page_block;
pub mod stack;
mod user_access;

pub use memory::PreparedMemory;
pub use user_access::{
    AddressSpaceId, UserAddressSpace, UserCopyError, copy_from_user, copy_to_user,
};

static READY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Allocator(hyper::mm::allocator::heap::InitError),
    AllocatorInterface(AllocatorSmokeError),
    BootstrapStack(stack::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FinalizationError {
    IdentityMappings(memory::Error),
    MemoryProtection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryStats {
    pub boot: hyper::mm::BootMemoryStats,
    pub runtime: hyper::mm::allocator::heap::HeapStats,
}

/// Returns a lock-consistent allocator snapshot once memory initialization is complete.
pub fn statistics() -> Option<MemoryStats> {
    let boot = super::boot::try_with_boot_state(|state| state.memory.boot_memory_stats())?;
    let runtime = allocator::GLOBAL_ALLOCATOR.stats()?;
    Some(MemoryStats { boot, runtime })
}

/// Activates the kernel allocator and validates Rust allocation plumbing.
pub(crate) fn initialize() -> Result<(), InitializationError> {
    super::boot::with_boot_state(|state| {
        state
            .memory
            .initialize_global_allocator(&allocator::GLOBAL_ALLOCATOR)
    })
    .map_err(InitializationError::Allocator)?;
    verify_rust_allocator_interface().map_err(InitializationError::AllocatorInterface)?;
    let boot_cpu = super::cpu::current_index()
        .ok_or(stack::Error::InvalidCpuIndex)
        .map_err(InitializationError::BootstrapStack)?;
    stack::prepare_cpu(boot_cpu).map_err(InitializationError::BootstrapStack)?;
    READY.store(true, Ordering::Release);
    crate::println!(
        "HypeR: guarded kernel stacks: thread {} KiB, per-CPU IRQ {} KiB, emergency {} KiB",
        hyper::config::KERNEL_STACK_SIZE_KB,
        hyper::config::IRQ_STACK_SIZE_KB,
        hyper::config::EMERGENCY_STACK_SIZE_KB
    );
    Ok(())
}

pub(crate) fn is_ready() -> bool {
    READY.load(Ordering::Acquire)
}

/// Removes bootstrap-only mappings and reports the permanent memory layout.
pub(crate) fn finalize_address_space() -> Result<(), FinalizationError> {
    super::boot::with_boot_state(|state| {
        // SAFETY: SMP initialization has moved every online CPU to permanent
        // high mappings, and finalization runs once before normal scheduling
        // can introduce any concurrent stage-1 mutation.
        unsafe { state.memory.retire_identity_mappings(&state.platform) }
    })
    .map_err(FinalizationError::IdentityMappings)?;
    crate::arch::memory::enable_local_protection();
    if !memory_protection_active() {
        return Err(FinalizationError::MemoryProtection);
    }

    let layout = memory::virtual_memory_layout();
    crate::arch::memory::Barrier::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
    let data_cache_line = crate::arch::memory::Cache::data_line_size();
    let instruction_cache_line = crate::arch::memory::Cache::instruction_line_size();
    let atomic_capabilities: crate::arch::memory::AtomicCapabilities =
        crate::arch::memory::atomic_capabilities();
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
            atomic_capabilities.backend_name()
        );
        crate::arch::platform::describe_runtime(|description| {
            crate::println!("{description}");
        });
        crate::println!(
            "HypeR: {} boot reservations, {} RAM regions, root {:#x}",
            state.memory.reservation_count(),
            state.platform.memory.len(),
            state.memory.root_address()
        );
    });
    report_statistics("kernel initialized");
    Ok(())
}

pub(crate) fn memory_protection_active() -> bool {
    crate::arch::memory::local_protection_enabled()
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
pub(crate) enum AllocatorSmokeError {
    Allocation,
    BoxValue,
    VectorContents,
    VectorLength,
}

fn verify_rust_allocator_interface() -> Result<(), AllocatorSmokeError> {
    let boxed =
        hyper::mm::try_box(0x0048_5950_4552_u64).map_err(|_| AllocatorSmokeError::Allocation)?;
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(1024)
        .map_err(|_| AllocatorSmokeError::Allocation)?;
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
