// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected exception-entry and fail-stop machine capabilities.
//!
//! Kernel crash and IRQ services own failure policy. This facade selects only
//! context capture, vector and stack installation, emergency-stack entry, and
//! crash-stop transport. Raw architecture entry invokes kernel policy only
//! through immutable services published before runtime vectors are installed.

use hyper::cpu::CpuIndex;
use hyper::hal::interrupt::InterruptId;

/// Selected raw diagnostic context and vector-validation error.
///
/// The context deliberately retains each architecture's register model. Crash
/// reporting is compiled for one backend and must not erase useful machine
/// state behind a false lowest-common-denominator representation.
pub(crate) use crate::arch::exception::{CrashContext, RuntimeVectorError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryServiceError {
    Fatal(crate::arch::exception::EntryServiceError),
    Interrupt(crate::arch::irq::InterruptEntryServiceError),
}

/// Proof that fatal and physical-interrupt policy callbacks are immutable.
#[derive(Clone, Copy)]
pub(crate) struct EntryReady {
    fatal: crate::arch::exception::FatalEntryReady,
    interrupt: crate::arch::irq::InterruptEntryReady,
}

#[cfg(CONFIG_ARCH_RISCV64)]
pub(crate) fn install_entry_services(
    fatal: for<'reason> fn(CrashContext, core::fmt::Arguments<'reason>) -> !,
    dispatch: fn(InterruptId, Option<unsafe extern "C" fn()>) -> hyper::hal::interrupt::EntryAction,
    claim_external: fn() -> Option<hyper::hal::interrupt::EntryAction>,
    stop: fn(CrashContext) -> !,
) -> Result<EntryReady, EntryServiceError> {
    let fatal =
        crate::arch::exception::install_fatal_service(fatal).map_err(EntryServiceError::Fatal)?;
    let interrupt = crate::arch::irq::install_interrupt_entry_services(
        crate::arch::irq::InterruptEntryServices::new(dispatch, claim_external, stop),
    )
    .map_err(EntryServiceError::Interrupt)?;
    Ok(EntryReady { fatal, interrupt })
}

#[cfg(not(CONFIG_ARCH_RISCV64))]
pub(crate) fn install_entry_services(
    fatal: for<'reason> fn(CrashContext, core::fmt::Arguments<'reason>) -> !,
    dispatch: fn(InterruptId, Option<unsafe extern "C" fn()>) -> hyper::hal::interrupt::EntryAction,
    stop: fn(CrashContext) -> !,
) -> Result<EntryReady, EntryServiceError> {
    let fatal =
        crate::arch::exception::install_fatal_service(fatal).map_err(EntryServiceError::Fatal)?;
    let interrupt = crate::arch::irq::install_interrupt_entry_services(
        crate::arch::irq::InterruptEntryServices::new(dispatch, stop),
    )
    .map_err(EntryServiceError::Interrupt)?;
    Ok(EntryReady { fatal, interrupt })
}

#[inline]
pub(crate) fn capture_crash_context() -> CrashContext {
    crate::arch::exception::capture_crash_context()
}

#[inline]
pub(crate) fn bootstrap_stack_bounds(stack_pointer: u64) -> Option<(usize, usize)> {
    crate::arch::exception::bootstrap_stack_bounds(stack_pointer)
}

#[inline]
pub(crate) fn crash_stop_interrupt() -> Option<InterruptId> {
    crate::arch::exception::crash_stop_interrupt()
}

#[inline]
pub(crate) fn is_crash_stop_interrupt(interrupt: InterruptId) -> bool {
    crate::arch::exception::is_crash_stop_interrupt(interrupt)
}

#[inline]
pub(crate) fn broadcast_crash_stop() -> bool {
    crate::arch::exception::broadcast_crash_stop()
}

/// Publishes the pinned exception stacks belonging to one logical CPU.
///
/// # Safety
///
/// Both ranges must be nonempty, ABI-aligned, exclusively reserved stack
/// mappings which remain pinned and writable for the CPU's online lifetime.
/// The target CPU must not enter runtime exceptions until publication
/// completes, and its slot may be installed only once.
#[inline]
pub(crate) unsafe fn install_exception_stacks(
    cpu: CpuIndex,
    irq: (usize, usize),
    emergency: (usize, usize),
) -> Result<(), ()> {
    // SAFETY: The selected architecture facade receives the same validated
    // CPU index, exclusive ranges, lifetime, and publication guarantees.
    unsafe { crate::arch::exception::install_exception_stacks(cpu, irq, emergency) }
}

/// Installs the selected architecture's permanent runtime exception vectors.
///
/// # Safety
///
/// The final executable kernel mapping and a valid exception stack must be
/// active. Kernel entry services must be initialized and local interrupts must
/// remain masked until installation and validation complete. The selected
/// backend's one-time installation requirement must also be upheld.
#[inline]
pub(crate) unsafe fn install_runtime_vectors(entry: &EntryReady) {
    let _ = (&entry.fatal, &entry.interrupt);
    // SAFETY: The selected architecture facade receives the same mapping,
    // entry-service, mask-state, and one-time publication guarantees.
    unsafe { crate::arch::exception::install_runtime_vectors() }
}

/// Installs the selected runtime vector state on the calling secondary CPU.
///
/// # Safety
///
/// The global vector representation must already be immutable and published.
/// The calling CPU must own installed exception stacks and keep local
/// interrupts masked through validation.
#[inline]
pub(crate) unsafe fn install_local_runtime_vectors(entry: &EntryReady) {
    let _ = (&entry.fatal, &entry.interrupt);
    // SAFETY: The selected backend receives the same per-CPU stack, mapping,
    // publication, and interrupt-mask guarantees.
    unsafe { crate::arch::exception::install_local_runtime_vectors() }
}

#[inline]
pub(crate) fn validate_runtime_vectors() -> Result<(), RuntimeVectorError> {
    crate::arch::exception::validate_runtime_vectors()
}

#[inline]
pub(crate) fn validate_local_runtime_vectors() -> Result<(), RuntimeVectorError> {
    crate::arch::exception::validate_local_runtime_vectors()
}

/// Permanently invokes fatal handling on the current CPU's emergency stack.
///
/// # Safety
///
/// The current CPU's emergency stack must be installed, pinned, and not
/// already active. Local interrupts must be masked. `argument` and any storage
/// it identifies must remain mapped and unreused until `callback` stops the
/// CPU. The callback must not publish a borrow of abandoned-stack storage or
/// allow one to escape the permanent fail-stop continuation.
#[inline]
pub(crate) unsafe fn run_on_emergency_stack(
    callback: extern "C" fn(usize) -> !,
    argument: usize,
) -> ! {
    // SAFETY: The selected architecture facade receives the same stack
    // lifetime, exclusivity, mask-state, and callback argument guarantees.
    unsafe { crate::arch::exception::run_on_emergency_stack(callback, argument) }
}
