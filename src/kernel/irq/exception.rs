//! Fatal exception policy above architecture vector decoding.

use hyper::hal::interrupt::InterruptId;

pub fn fatal(vector: u64, esr: u64, elr: u64, far: u64, spsr: u64) -> ! {
    crate::pr_emerg!(
        "HypeR: fatal EL2 exception: vector {}, ESR {:#x}, ELR {:#x}, FAR {:#x}, SPSR {:#x}",
        vector,
        esr,
        elr,
        far,
        spsr
    );
    crate::arch::halt()
}

pub fn fatal_interrupt(reason: &str, interrupt: Option<InterruptId>) -> ! {
    match interrupt {
        Some(interrupt) => {
            crate::pr_emerg!(
                "HypeR: fatal IRQ dispatch: {reason}; INTID {}",
                interrupt.get()
            );
        }
        None => crate::pr_emerg!("HypeR: fatal IRQ dispatch: {reason}"),
    }
    crate::arch::halt()
}

pub fn fatal_timer(error: crate::arch::TimerError) -> ! {
    crate::pr_emerg!("HypeR: fatal timer interrupt: {error:?}");
    crate::arch::halt()
}

pub fn fatal_interrupt_state(error: super::interrupt::Error) -> ! {
    crate::pr_emerg!("HypeR: fatal interrupt-controller state: {error:?}");
    crate::arch::halt()
}
