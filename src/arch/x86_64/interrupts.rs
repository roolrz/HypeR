// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::arch::asm;

use hyper::hal::interrupt::InterruptMask;

const RFLAGS_IF: usize = 1 << 9;

pub struct LocalInterruptMask;

impl InterruptMask for LocalInterruptMask {
    type State = usize;

    fn save_and_disable() -> Self::State {
        let flags: usize;
        // SAFETY: The balanced push/pop preserves RSP. Do not claim `nomem`:
        // PUSHFQ transiently writes the current kernel stack.
        unsafe { asm!("pushfq", "pop {}", "cli", out(reg) flags) };
        flags
    }

    fn restore(state: Self::State) {
        if state & RFLAGS_IF != 0 {
            super::timer::prepare_interrupt_enable();
            // SAFETY: Protected state is published before enabling interrupts.
            unsafe { asm!("sti", options(nostack)) };
        }
    }
}

pub fn enable_irq() {
    super::timer::prepare_interrupt_enable();
    // SAFETY: Interrupt state is initialized and STI is valid at CPL0.
    unsafe { asm!("sti", options(nostack)) };
}

pub fn irq_enabled() -> bool {
    let flags: usize;
    // SAFETY: The balanced push/pop preserves RSP and reads the current flags.
    // PUSHFQ touches the stack, so this assembly intentionally has a compiler
    // memory clobber.
    unsafe { asm!("pushfq", "pop {}", out(reg) flags) };
    flags & RFLAGS_IF != 0
}

pub fn disable_all() {
    // SAFETY: CLI is valid at CPL0 and remains a compiler memory boundary.
    unsafe { asm!("cli", options(nostack)) };
}
