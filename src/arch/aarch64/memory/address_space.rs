use hyper::hal::memory::KernelImageLayout;
use hyper::mm::{BootAllocator, PhysicalAddress};
use hyper::platform::PlatformInfo;

use super::page_table::{self, FinalAddressSpace};

pub type Error = page_table::Error;

/// Architecture-owned state for the final EL2 stage-1 address space.
pub struct PreparedAddressSpace {
    address_space: FinalAddressSpace,
}

#[derive(Clone, Copy)]
pub struct ActivationContext {
    pub(in crate::arch::aarch64) root: hyper::mm::PhysicalAddress,
    pub(in crate::arch::aarch64) stack_top: hyper::mm::VirtualAddress,
    pub(in crate::arch::aarch64) kernel_base: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackMapping {
    pub guard_page: usize,
    pub bottom: usize,
    pub top: usize,
}

impl PreparedAddressSpace {
    pub fn root_address(&self) -> u64 {
        self.address_space.root.get()
    }

    pub fn kernel_base(&self) -> u64 {
        self.address_space.kernel_base
    }

    pub fn activation_context(&self) -> ActivationContext {
        ActivationContext {
            root: self.address_space.root,
            stack_top: self.address_space.stack_top,
            kernel_base: self.address_space.kernel_base,
        }
    }

    pub fn retire_identity_mappings(&self, platform: &PlatformInfo) -> Result<(), Error> {
        page_table::retire_identity_mappings(self.address_space.root, platform)
    }

    pub fn map_stack(
        &self,
        slot: usize,
        physical: PhysicalAddress,
        pages: usize,
        allocate_table: &mut dyn FnMut() -> Option<PhysicalAddress>,
    ) -> Result<StackMapping, Error> {
        page_table::map_runtime_stack(
            self.address_space.root,
            slot,
            physical,
            pages,
            allocate_table,
        )
    }

    pub fn unmap_stack(&self, slot: usize, pages: usize) -> Result<(), Error> {
        page_table::unmap_runtime_stack(self.address_space.root, slot, pages)
    }

    pub fn address_is_mapped(&self, address: usize) -> Result<bool, Error> {
        page_table::runtime_address_is_mapped(self.address_space.root, address as u64)
    }
}

/// Builds the AArch64 final EL2 address space from kernel-owned boot memory.
///
/// # Safety
///
/// The allocator must return pages writable through the current bootstrap
/// identity mapping.
pub unsafe fn prepare(
    allocator: &mut BootAllocator,
    platform: &PlatformInfo,
    image: KernelImageLayout,
    kernel_base: u64,
) -> Result<PreparedAddressSpace, Error> {
    let address_space =
        unsafe { page_table::build_final_address_space(allocator, platform, image, kernel_base)? };
    Ok(PreparedAddressSpace { address_space })
}
