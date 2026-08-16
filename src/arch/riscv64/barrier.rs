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

fn fence(access: BarrierAccess) {
    // RISC-V FENCE has no Arm-style shareability domains. HypeR supports only
    // coherent application-class platforms, so the full predecessor/successor
    // sets provide the portable interpretation of the HAL contract.
    unsafe {
        match access {
            BarrierAccess::Reads => asm!("fence r, r", options(nostack)),
            BarrierAccess::Writes => asm!("fence w, w", options(nostack)),
            BarrierAccess::All => asm!("fence rw, rw", options(nostack)),
        }
    }
}
