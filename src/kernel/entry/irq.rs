//! Physical-interrupt entry policy.

use hyper::hal::interrupt::InterruptId;

/// Kernel action selected for an acknowledged physical interrupt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub(crate) enum Action {
    /// Complete exception return after ordinary IRQ dispatch.
    Resume,
    /// Capture the interrupted architecture frame and stop this CPU.
    Stop,
}

/// Dispatches one architecture-acknowledged interrupt.
///
/// The caller owns interrupt acknowledgement and raw-frame lifetime. Local
/// interrupts must remain masked. Dispatch is allocation-free but invokes
/// registered interrupt-safe handlers under the IRQ registry lock.
pub(crate) fn dispatch(interrupt: InterruptId) -> Action {
    if crate::kernel::crash::is_stop_interrupt(interrupt) {
        Action::Stop
    } else {
        crate::kernel::irq::interrupt::dispatch(interrupt);
        Action::Resume
    }
}

/// Claims and dispatches one external controller interrupt, when pending.
///
/// This is the RISC-V external-interrupt entry seam. Controller claim and
/// registry dispatch are kernel-owned policy; architecture code only decodes
/// the architectural trap cause.
// Only the RISC-V trap backend uses a controller claim distinct from its
// architectural interrupt acknowledgement. Keeping target selection below
// `arch` leaves this narrow adapter intentionally unused on other targets.
#[allow(dead_code)]
pub(crate) fn claim_and_dispatch_external() -> Option<Action> {
    crate::kernel::irq::acknowledge_external().map(dispatch)
}

/// Publishes a remote CPU's exact interrupt snapshot and stops that CPU.
pub(crate) fn stop(context: crate::arch::exception::CrashContext) -> ! {
    crate::kernel::crash::stop_this_cpu(context)
}
