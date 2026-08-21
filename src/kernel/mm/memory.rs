// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel memory initialization and architecture handoff policy.

use hyper::hal::memory::{AddressTranslation, VirtualMemoryLayout};
use hyper::mm::{BootAllocator, BootAllocatorError, BootMemoryStats, PhysicalAddress};
use hyper::platform::{PhysicalRange, PlatformInfo};

use super::allocator::GlobalAllocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocator(BootAllocatorError),
    AddressSpace(crate::arch::memory::Error),
}

impl From<BootAllocatorError> for Error {
    fn from(error: BootAllocatorError) -> Self {
        Self::Allocator(error)
    }
}

impl From<crate::arch::memory::Error> for Error {
    fn from(error: crate::arch::memory::Error) -> Self {
        Self::AddressSpace(error)
    }
}

/// Kernel-owned boot-memory state and its architecture address space.
pub struct PreparedMemory {
    allocator: BootAllocator,
    address_space: crate::arch::memory::PreparedAddressSpace,
}

impl PreparedMemory {
    pub fn root_address(&self) -> u64 {
        self.address_space.root_address()
    }

    pub fn kernel_base(&self) -> u64 {
        self.address_space.kernel_base()
    }

    pub fn reservation_count(&self) -> usize {
        self.allocator.reservations().len()
    }

    pub fn boot_memory_stats(&self) -> BootMemoryStats {
        self.allocator.stats()
    }

    pub fn activation_context(&self) -> crate::arch::memory::ActivationContext {
        self.address_space.activation_context()
    }

    /// # Safety
    ///
    /// All CPUs must have left the identity mappings and stage-1 page-table
    /// mutation must be serialized.
    pub unsafe fn retire_identity_mappings(&self, platform: &PlatformInfo) -> Result<(), Error> {
        // SAFETY: This method forwards its caller's quiescence and serialized
        // page-table mutation guarantees to the architecture implementation.
        unsafe { self.address_space.retire_identity_mappings(platform) }.map_err(Error::from)
    }

    /// Maps a guarded kernel stack and any page-table pages it requires.
    ///
    /// # Safety
    ///
    /// Every address returned by `allocate_table` must identify a uniquely
    /// owned, zeroed, aligned page that stays live and linearly mapped for the
    /// lifetime of this address space. Page-table mutation must be serialized.
    pub unsafe fn map_kernel_stack(
        &self,
        slot: usize,
        physical: PhysicalAddress,
        pages: usize,
        allocate_table: &mut dyn FnMut() -> Option<PhysicalAddress>,
    ) -> Result<crate::arch::memory::StackMapping, Error> {
        // SAFETY: The caller guarantees the table allocator's ownership,
        // zeroing, alignment, lifetime, and serialized mutation requirements.
        unsafe {
            self.address_space
                .map_stack(slot, physical, pages, allocate_table)
        }
        .map_err(Error::from)
    }

    /// # Safety
    ///
    /// Page-table mutation must be serialized and no CPU may retain a live
    /// translation or stack pointer into the mapping.
    pub unsafe fn unmap_kernel_stack(&self, slot: usize, pages: usize) -> Result<(), Error> {
        // SAFETY: The caller guarantees the stack is abandoned and mutation
        // serialized; this wrapper preserves those conditions.
        unsafe { self.address_space.unmap_stack(slot, pages) }.map_err(Error::from)
    }

    /// # Safety
    ///
    /// The page-table hierarchy must remain live and immutable for this walk.
    pub unsafe fn address_is_mapped(&self, address: usize) -> Result<bool, Error> {
        // SAFETY: The caller pins and prevents mutation of this address space
        // for the complete architecture page-table walk.
        unsafe { self.address_space.address_is_mapped(address) }.map_err(Error::from)
    }

    pub fn initialize_global_allocator(
        &self,
        allocator: &GlobalAllocator,
    ) -> Result<(), hyper::mm::allocator::heap::InitError> {
        let layout = virtual_memory_layout();
        // SAFETY: The architecture address space permanently maps every DTB
        // RAM region at `linear_base + PA` as writable normal memory.
        unsafe { allocator.initialize(&self.allocator.handoff(), layout.linear_base) }
    }
}

/// Reserves the loaded image and DTB, then asks the architecture to build its
/// final kernel address space.
///
/// # Safety
///
/// Firmware RAM below the architecture's bootstrap-accessible limit must be
/// writable through the current early mapping.
pub unsafe fn prepare(
    platform: &PlatformInfo,
    dtb_address: u64,
    initial_ramdisk: PhysicalRange,
    kernel_base: u64,
) -> Result<PreparedMemory, Error> {
    let image = crate::kernel::boot::image::layout();
    let mut allocator = BootAllocator::new(
        &platform.memory,
        &platform.reserved,
        crate::arch::memory::AddressTranslation::bootstrap_accessible_limit(),
    )?;
    allocator.reserve(
        PhysicalRange::new(image.physical_start, image.total_size)
            .ok_or(BootAllocatorError::InvalidRequest)?,
    )?;
    allocator.reserve(
        PhysicalRange::new(dtb_address, platform.dtb_size)
            .ok_or(BootAllocatorError::InvalidRequest)?,
    )?;
    allocator.reserve(initial_ramdisk)?;

    // SAFETY: This function's caller guarantees bootstrap-accessible writable
    // RAM, while `allocator` exclusively reserves every returned table page.
    let address_space =
        unsafe { crate::arch::memory::prepare(&mut allocator, platform, image, kernel_base)? };
    Ok(PreparedMemory {
        allocator,
        address_space,
    })
}

pub fn virtual_memory_layout() -> VirtualMemoryLayout {
    crate::arch::memory::AddressTranslation::layout()
}

pub fn linear_address(physical: u64) -> Option<usize> {
    translated_address(crate::arch::memory::AddressTranslation::linear_address(
        PhysicalAddress::new(physical),
    ))
}

pub fn mmio_address(physical: u64) -> Option<usize> {
    translated_address(crate::arch::memory::AddressTranslation::mmio_address(
        PhysicalAddress::new(physical),
    ))
}

/// Converts an address in the permanent RAM linear map back to a PA.
pub fn linear_physical_address(virtual_address: usize) -> Option<u64> {
    let base = virtual_memory_layout().linear_base;
    (virtual_address as u64).checked_sub(base)
}

fn translated_address(address: Option<hyper::mm::VirtualAddress>) -> Option<usize> {
    address.and_then(|address| usize::try_from(address.get()).ok())
}
