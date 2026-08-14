use hyper::hal::memory::KernelImageLayout;
use hyper::mm::BootAllocator;
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
}

impl PreparedAddressSpace {
    pub fn root_address(&self) -> u64 {
        self.address_space.root.get()
    }

    pub fn activation_context(&self) -> ActivationContext {
        ActivationContext {
            root: self.address_space.root,
            stack_top: self.address_space.stack_top,
        }
    }

    pub fn retire_identity_mappings(&self, platform: &PlatformInfo) -> Result<(), Error> {
        page_table::retire_identity_mappings(self.address_space.root, platform)
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
) -> Result<PreparedAddressSpace, Error> {
    let address_space =
        unsafe { page_table::build_final_address_space(allocator, platform, image)? };
    Ok(PreparedAddressSpace { address_space })
}
