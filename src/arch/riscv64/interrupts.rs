// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::arch::asm;

use hyper::hal::interrupt::InterruptMask;

use super::registers;

pub struct LocalInterruptMask;

impl InterruptMask for LocalInterruptMask {
    type State = usize;

    fn save_and_disable() -> Self::State {
        let previous: usize;
        // SAFETY: SSTATUS is accessible in HS mode; this atomically clears SIE.
        unsafe {
            asm!(
                "csrrc {previous}, sstatus, {mask}",
                previous = out(reg) previous,
                mask = in(reg) registers::SSTATUS_SIE as usize,
                options(nostack)
            );
        }
        previous
    }

    fn restore(state: Self::State) {
        if state & registers::SSTATUS_SIE as usize != 0 {
            // SAFETY: Protected state has been released before restoring SIE.
            unsafe { asm!("csrsi sstatus, 2", options(nostack)) };
        }
    }
}

pub fn enable_irq() {
    // SAFETY: Interrupt sources and handlers are initialized before this call.
    unsafe { asm!("csrsi sstatus, 2", options(nostack)) };
}

/// Masks ordinary local interrupt delivery while preserving source enables.
///
/// This operation pairs with [`enable_irq`] across controlled context-entry
/// transitions. It deliberately leaves `sie` unchanged so timer, software,
/// and external sources resume when the global SIE bit is restored.
pub fn mask_irq() {
    // SAFETY: SSTATUS.SIE is writable in HS mode. Keep the compiler memory
    // clobber because an interrupt handler may observe ordinary memory around
    // this transition.
    unsafe { asm!("csrci sstatus, 2", options(nostack)) };
}

pub fn irq_enabled() -> bool {
    let status: usize;
    // SAFETY: Reading SSTATUS is valid in HS mode and has no memory operands.
    unsafe { asm!("csrr {status}, sstatus", status = out(reg) status, options(nomem, nostack)) };
    status & registers::SSTATUS_SIE as usize != 0
}

pub fn disable_all() {
    // SAFETY: Both CSRs are writable in HS mode; this only masks delivery.
    unsafe { asm!("csrci sstatus, 2", "csrw sie, zero", options(nostack)) };
}

pub fn enable_kernel_sources() {
    let mask = (registers::SIE_SSIE | registers::SIE_SEIE) as usize;
    // SAFETY: SIE is writable in HS mode and the mask names supported sources.
    unsafe { asm!("csrs sie, {mask}", mask = in(reg) mask, options(nostack)) };
}

pub fn clear_software_interrupt() {
    // SAFETY: SIP.SSIP is writable in HS mode.
    unsafe { asm!("csrci sip, 2", options(nostack)) };
}
