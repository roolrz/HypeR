//! Kernel memory initialization and architecture handoff policy.

use hyper::hal::memory::{AddressTranslation, VirtualMemoryLayout};
use hyper::mm::{BootAllocator, BootAllocatorError, PhysicalAddress};
use hyper::platform::{PhysicalRange, PlatformInfo};

use super::allocator::GlobalAllocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocator(BootAllocatorError),
    AddressSpace(crate::arch::MemoryError),
}

impl From<BootAllocatorError> for Error {
    fn from(error: BootAllocatorError) -> Self {
        Self::Allocator(error)
    }
}

impl From<crate::arch::MemoryError> for Error {
    fn from(error: crate::arch::MemoryError) -> Self {
        Self::AddressSpace(error)
    }
}

/// Kernel-owned boot-memory state and its architecture address space.
pub struct PreparedMemory {
    allocator: BootAllocator,
    address_space: crate::arch::PreparedAddressSpace,
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

    pub fn activation_context(&self) -> crate::arch::ActivationContext {
        self.address_space.activation_context()
    }

    pub fn retire_identity_mappings(&self, platform: &PlatformInfo) -> Result<(), Error> {
        self.address_space
            .retire_identity_mappings(platform)
            .map_err(Error::from)
    }

    pub fn initialize_global_allocator(
        &self,
        allocator: &GlobalAllocator,
    ) -> Result<(), hyper::mm::heap::InitError> {
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
    kernel_base: u64,
) -> Result<PreparedMemory, Error> {
    let image = crate::kernel::boot::image::layout();
    let mut allocator = BootAllocator::new(
        &platform.memory,
        &platform.reserved,
        crate::arch::ArchitectureAddressTranslation::bootstrap_accessible_limit(),
    )?;
    allocator.reserve(
        PhysicalRange::new(image.physical_start, image.total_size)
            .ok_or(BootAllocatorError::InvalidRequest)?,
    )?;
    allocator.reserve(
        PhysicalRange::new(dtb_address, platform.dtb_size)
            .ok_or(BootAllocatorError::InvalidRequest)?,
    )?;

    let address_space = unsafe {
        crate::arch::prepare_address_space(&mut allocator, platform, image, kernel_base)?
    };
    Ok(PreparedMemory {
        allocator,
        address_space,
    })
}

pub fn virtual_memory_layout() -> VirtualMemoryLayout {
    crate::arch::ArchitectureAddressTranslation::layout()
}

pub fn linear_address(physical: u64) -> Option<usize> {
    translated_address(crate::arch::ArchitectureAddressTranslation::linear_address(
        PhysicalAddress::new(physical),
    ))
}

pub fn mmio_address(physical: u64) -> Option<usize> {
    translated_address(crate::arch::ArchitectureAddressTranslation::mmio_address(
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
