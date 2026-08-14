use core::arch::asm;

use hyper::hal::interrupt::InterruptMask;

/// Local AArch64 interrupt-mask policy used by IRQ-safe kernel locks.
pub struct LocalInterruptMask;

impl InterruptMask for LocalInterruptMask {
    type State = u64;

    #[inline]
    fn save_and_disable() -> Self::State {
        let state: u64;
        // SAFETY: Reading DAIF and masking local exceptions does not access
        // memory and is valid while the kernel executes at EL2.
        unsafe {
            asm!(
                "mrs {state}, daif",
                "msr daifset, #0xf",
                state = out(reg) state,
                options(nomem, nostack, preserves_flags)
            );
        }
        state
    }

    #[inline]
    fn restore(state: Self::State) {
        // SAFETY: `state` was captured from DAIF on this processing element
        // immediately before the matching lock acquisition.
        unsafe {
            asm!(
                "msr daif, {state}",
                state = in(reg) state,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

/// Enables local IRQ exceptions while leaving FIQ, SError, and debug masked.
///
/// Runtime vectors and the interrupt dispatcher must be installed first.
pub fn enable_irq() {
    // SAFETY: DAIFClr with immediate 2 clears only the IRQ mask bit.
    unsafe { asm!("msr daifclr, #2", options(nomem, nostack, preserves_flags)) };
}
