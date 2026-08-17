//! `AArch64` CPU identity and PSCI secondary-entry ABI.

use core::arch::asm;
use core::ptr::addr_of;

use super::registers;

unsafe extern "C" {
    static aarch64_secondary_entry: u8;
}

/// Parameters consumed by the physical secondary-CPU trampoline.
#[repr(C, align(64))]
pub struct SecondaryBootParameters {
    pub root: u64,
    pub physical_stack_top: u64,
    pub virtual_stack_top: u64,
    pub cpu_index: u64,
    pub rust_entry: u64,
    pub runtime_vectors: u64,
}

impl SecondaryBootParameters {
    pub fn new(
        root: u64,
        physical_stack_top: u64,
        virtual_stack_top: u64,
        cpu_index: usize,
    ) -> Self {
        Self {
            root,
            physical_stack_top,
            virtual_stack_top,
            cpu_index: cpu_index as u64,
            rust_entry: crate::start_secondary_cpu as *const () as usize as u64,
            runtime_vectors: super::exception::runtime_vector_address(),
        }
    }
}

/// Converts the linked secondary trampoline into its loaded physical address.
pub fn secondary_entry_physical(
    image_physical_start: u64,
    kernel_virtual_base: u64,
) -> Option<u64> {
    let virtual_address = addr_of!(aarch64_secondary_entry) as u64;
    let offset = virtual_address.checked_sub(kernel_virtual_base)?;
    image_physical_start.checked_add(offset)
}

/// Returns the logical CPU index installed in `TPIDR_EL2` by the entry path.
pub fn current_cpu_index() -> usize {
    let index: u64;
    // SAFETY: TPIDR_EL2 is private to the current EL2 processing element.
    unsafe {
        asm!(
            "mrs {index}, tpidr_el2",
            index = out(reg) index,
            options(nomem, nostack, preserves_flags)
        );
    }
    index as usize
}

/// Returns the normalized MPIDR affinity used by PSCI and CPU topology data.
pub fn current_hardware_id() -> u64 {
    let mpidr: u64;
    // SAFETY: MPIDR_EL1 is read-only at EL2.
    unsafe {
        asm!(
            "mrs {mpidr}, mpidr_el1",
            mpidr = out(reg) mpidr,
            options(nomem, nostack, preserves_flags)
        );
    }
    let affinity_0_to_2 = mpidr & registers::MPIDR_AFF0_TO_2_MASK;
    let affinity_3 = (mpidr >> registers::MPIDR_AFF3_SHIFT) & registers::MPIDR_AFF3_MASK;
    affinity_0_to_2 | (affinity_3 << registers::MPIDR_AFF3_SHIFT)
}

/// Wakes processing elements waiting in WFE after publishing shared state.
pub fn send_event() {
    // SAFETY: SEV affects only event-register state and orders no memory by
    // itself; callers publish shared state with release semantics first.
    unsafe { asm!("sev", options(nostack, preserves_flags)) };
}
