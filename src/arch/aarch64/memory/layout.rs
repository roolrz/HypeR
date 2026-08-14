use hyper::hal::memory::{AddressTranslation, VirtualMemoryLayout};
use hyper::mm::{PhysicalAddress, VirtualAddress};

pub(super) const MMIO_BASE: u64 = 0x0000_1000_0000_0000;
pub(super) const LINEAR_BASE: u64 = super::super::registers::LINEAR_VIRTUAL_BASE;
pub(in crate::arch::aarch64) const KERNEL_BASE: u64 = super::super::registers::KERNEL_VIRTUAL_BASE;
pub(super) const KERNEL_STACK_BASE: u64 = KERNEL_BASE + 0x0100_0000;
const BOOTSTRAP_ACCESSIBLE_LIMIT: u64 = 0x1_0000_0000;
const VIRTUAL_ADDRESS_LIMIT: u64 = 1 << 48;

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
        translated_address(LINEAR_BASE, physical)
    }

    fn mmio_address(physical: PhysicalAddress) -> Option<VirtualAddress> {
        translated_address(MMIO_BASE, physical)
    }
}

fn translated_address(base: u64, physical: PhysicalAddress) -> Option<VirtualAddress> {
    base.checked_add(physical.get())
        .filter(|address| *address < VIRTUAL_ADDRESS_LIMIT)
        .map(VirtualAddress::new)
}
