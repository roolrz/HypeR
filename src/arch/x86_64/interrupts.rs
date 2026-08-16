use core::arch::asm;

use hyper::hal::interrupt::InterruptMask;

const RFLAGS_IF: usize = 1 << 9;

pub struct LocalInterruptMask;

impl InterruptMask for LocalInterruptMask {
    type State = usize;

    fn save_and_disable() -> Self::State {
        let flags: usize;
        unsafe { asm!("pushfq", "pop {}", "cli", out(reg) flags, options(nomem)) };
        flags
    }

    fn restore(state: Self::State) {
        if state & RFLAGS_IF != 0 {
            super::timer::prepare_interrupt_enable();
            unsafe { asm!("sti", options(nomem, nostack)) };
        }
    }
}

pub fn enable_irq() {
    super::timer::prepare_interrupt_enable();
    unsafe { asm!("sti", options(nomem, nostack)) };
}

pub fn irq_enabled() -> bool {
    let flags: usize;
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(nomem)) };
    flags & RFLAGS_IF != 0
}

pub fn disable_all() {
    unsafe { asm!("cli", options(nomem, nostack)) };
}
