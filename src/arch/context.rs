//! Selected-architecture schedulable context and stack-transition mechanisms.
//!
//! Task policy owns thread lifecycle and scheduling. This facade owns only the
//! architecture register images and the machine operations which initialize or
//! switch them. Stack ownership and mapping remain kernel memory policy.

pub(crate) use super::imp::{ThreadContext, UserContext};

/// Switches between two pinned scheduler-owned machine contexts.
///
/// # Safety
///
/// Both contexts and their associated stacks must remain pinned and
/// exclusively scheduler-owned until control eventually switches back.
/// `next` must contain a context prepared for the selected architecture.
#[inline]
pub(crate) unsafe fn switch_thread_context(previous: &mut ThreadContext, next: &ThreadContext) {
    // SAFETY: This facade preserves the selected backend's context ownership,
    // pinning, and prepared-state preconditions unchanged.
    unsafe { super::imp::switch_thread_context(previous, next) }
}

/// Abandons the current call chain and enters a continuation on a clean stack.
///
/// # Safety
///
/// `bottom..top` must be a nonempty, exclusively owned, writable stack range
/// satisfying the selected architecture's ABI alignment. Local interrupts
/// must be masked, no live value on the abandoned stack may be used again, and
/// `callback` must not return.
#[inline]
pub(crate) unsafe fn reset_stack_and_enter(
    bottom: usize,
    top: usize,
    watermark: u64,
    canary: u64,
    callback: extern "C" fn(usize) -> !,
    argument: usize,
) -> ! {
    // SAFETY: This facade forwards every backend stack, alignment, lifetime,
    // interrupt-state, and non-returning callback precondition.
    unsafe { super::imp::reset_stack_and_enter(bottom, top, watermark, canary, callback, argument) }
}
