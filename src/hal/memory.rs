use crate::mm::{PhysicalAddress, VirtualAddress};

/// Linker-derived physical layout of the loaded kernel image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelImageLayout {
    pub physical_start: u64,
    pub text_size: u64,
    pub rodata_size: u64,
    pub total_size: u64,
}

/// Architecture-selected permanent virtual address regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualMemoryLayout {
    pub mmio_base: u64,
    pub linear_base: u64,
    pub kernel_base: u64,
}

/// Address translation capabilities consumed by architecture-neutral kernel
/// and driver initialization code.
pub trait AddressTranslation {
    /// Highest physical address reachable through the early boot mapping.
    fn bootstrap_accessible_limit() -> u64;
    fn layout() -> VirtualMemoryLayout;
    fn linear_address(physical: PhysicalAddress) -> Option<VirtualAddress>;
    fn mmio_address(physical: PhysicalAddress) -> Option<VirtualAddress>;
}
