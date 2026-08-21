use hyper::hal::memory::{AddressTranslation, VirtualMemoryLayout};
use hyper::mm::{PhysicalAddress, VirtualAddress};

const ADDRESS_SPACE_SIZE: u64 = super::super::address::STAGE1_VA_LIMIT;
const KERNEL_REGION_SIZE: u64 = 1 << 40;

pub(super) const MMIO_BASE: u64 = ADDRESS_SPACE_SIZE >> 4;
pub(in crate::arch::aarch64) const LINEAR_BASE: u64 = ADDRESS_SPACE_SIZE >> 2;
pub(in crate::arch::aarch64) const KERNEL_BASE: u64 = ADDRESS_SPACE_SIZE - KERNEL_REGION_SIZE;
pub(super) const KERNEL_STACK_BASE: u64 = KERNEL_BASE + super::super::kaslr::WINDOW_SIZE;
pub(super) const KERNEL_STACK_ARENA_BASE: u64 = KERNEL_STACK_BASE + 2 * 1024 * 1024;
const BOOTSTRAP_ACCESSIBLE_LIMIT: u64 = 0x1_0000_0000;

const _: () = {
    assert!(MMIO_BASE < LINEAR_BASE);
    assert!(LINEAR_BASE < KERNEL_BASE);
    assert!(KERNEL_STACK_ARENA_BASE < ADDRESS_SPACE_SIZE);
};

pub struct Aarch64AddressTranslation;

impl AddressTranslation for Aarch64AddressTranslation {
    fn bootstrap_accessible_limit() -> u64 {
        BOOTSTRAP_ACCESSIBLE_LIMIT
    }

    fn layout() -> VirtualMemoryLayout {
        VirtualMemoryLayout {
            mmio_base: MMIO_BASE,
            linear_base: LINEAR_BASE,
            kernel_base: KERNEL_BASE,
        }
    }

    fn linear_address(physical: PhysicalAddress) -> Option<VirtualAddress> {
        translated_address(LINEAR_BASE, KERNEL_BASE, physical)
    }

    fn mmio_address(physical: PhysicalAddress) -> Option<VirtualAddress> {
        translated_address(MMIO_BASE, LINEAR_BASE, physical)
    }
}

fn translated_address(base: u64, end: u64, physical: PhysicalAddress) -> Option<VirtualAddress> {
    base.checked_add(physical.get())
        .filter(|address| *address < end)
        .map(VirtualAddress::new)
}
