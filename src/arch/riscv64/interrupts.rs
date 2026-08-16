use core::arch::asm;

use hyper::hal::interrupt::InterruptMask;

use super::registers;

pub struct LocalInterruptMask;

impl InterruptMask for LocalInterruptMask {
    type State = usize;

    fn save_and_disable() -> Self::State {
        let previous: usize;
        unsafe {
            asm!(
                "csrrc {previous}, sstatus, {mask}",
                previous = out(reg) previous,
                mask = in(reg) registers::SSTATUS_SIE as usize,
                options(nomem, nostack)
            );
        }
        previous
    }

    fn restore(state: Self::State) {
        if state & registers::SSTATUS_SIE as usize != 0 {
            unsafe { asm!("csrsi sstatus, 2", options(nomem, nostack)) };
        }
    }
}

pub fn enable_irq() {
    unsafe { asm!("csrsi sstatus, 2", options(nomem, nostack)) };
}

pub fn irq_enabled() -> bool {
    let status: usize;
    unsafe { asm!("csrr {status}, sstatus", status = out(reg) status, options(nomem, nostack)) };
    status & registers::SSTATUS_SIE as usize != 0
}

pub fn disable_all() {
    unsafe {
        asm!(
            "csrci sstatus, 2",
            "csrw sie, zero",
            options(nomem, nostack)
        )
    };
}
