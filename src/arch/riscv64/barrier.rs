use core::arch::asm;

use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};

pub struct Riscv64Barrier;

impl Barrier for Riscv64Barrier {
    fn data_memory(_domain: BarrierDomain, access: BarrierAccess) {
        fence(access);
    }

    fn data_synchronization(_domain: BarrierDomain, access: BarrierAccess) {
        fence(access);
    }

    fn instruction_synchronization() {
        // SAFETY: FENCE.I is part of the selected Zifencei execution contract.
        unsafe { asm!("fence.i", options(nostack)) };
    }
}

fn fence(_access: BarrierAccess) {
    // RISC-V FENCE has no Arm-style shareability domains. Include both memory
    // and device-I/O sets: platform MMIO may be classified as I/O rather than
    // ordinary reads and writes by the execution environment.
    // The HAL access classes follow Arm's DMB/DSB semantics, which do not map
    // exactly onto RISC-V predecessor/successor sets. A full fence is the
    // conservative portable implementation for every class.
    // SAFETY: FENCE has no pointer operands and is valid in HS mode.
    unsafe { asm!("fence iorw, iorw", options(nostack)) }
}
