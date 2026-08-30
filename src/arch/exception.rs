// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture exception and fail-stop mechanisms.
//!
//! Kernel crash and IRQ policy owns the decision to stop or resume. This
//! facade exposes only machine context capture, vector installation, emergency
//! stack entry, and crash-stop delivery. Raw exception entry remains in the
//! selected backend and reaches policy through immutable registered services.

use core::cell::UnsafeCell;
use core::fmt;

use hyper::sync::atomic::{AtomicU8, Ordering};

use hyper::cpu::CpuIndex;

pub(crate) use super::imp::{CrashContext, RuntimeVectorError};

const SERVICE_EMPTY: u8 = 0;
const SERVICE_INSTALLING: u8 = 1;
const SERVICE_READY: u8 = 2;

type FatalCallback = for<'reason> fn(CrashContext, fmt::Arguments<'reason>) -> !;

struct FatalServiceSlot {
    state: AtomicU8,
    callback: UnsafeCell<Option<FatalCallback>>,
}

// SAFETY: The single installer writes the callback before Release-publishing
// READY. It is immutable thereafter and every entry reader observes READY with
// Acquire ordering.
unsafe impl Sync for FatalServiceSlot {}

impl FatalServiceSlot {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(SERVICE_EMPTY),
            callback: UnsafeCell::new(None),
        }
    }

    fn install(&self, callback: FatalCallback) -> Result<FatalEntryReady, EntryServiceError> {
        self.state
            .compare_exchange(
                SERVICE_EMPTY,
                SERVICE_INSTALLING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .map_err(|_| EntryServiceError::AlreadyInstalled)?;
        // SAFETY: The successful state transition grants the sole write.
        unsafe { *self.callback.get() = Some(callback) };
        self.state.store(SERVICE_READY, Ordering::Release);
        Ok(FatalEntryReady { _private: () })
    }

    fn invoke(&self, context: CrashContext, reason: fmt::Arguments<'_>) -> ! {
        if self.state.load(Ordering::Acquire) != SERVICE_READY {
            super::imp::halt()
        }
        // SAFETY: Acquire observed the immutable callback published at READY.
        let Some(callback) = (unsafe { *self.callback.get() }) else {
            super::imp::halt()
        };
        callback(context, reason)
    }
}

static FATAL_SERVICE: FatalServiceSlot = FatalServiceSlot::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryServiceError {
    AlreadyInstalled,
}

#[derive(Clone, Copy)]
pub(crate) struct FatalEntryReady {
    _private: (),
}

pub(crate) fn install_fatal_service(
    callback: FatalCallback,
) -> Result<FatalEntryReady, EntryServiceError> {
    FATAL_SERVICE.install(callback)
}

pub(crate) fn fatal(context: CrashContext, reason: fmt::Arguments<'_>) -> ! {
    FATAL_SERVICE.invoke(context, reason)
}

pub(crate) use super::imp::{
    bootstrap_stack_bounds, broadcast_crash_stop, capture_crash_context, crash_stop_interrupt,
    is_crash_stop_interrupt, validate_local_runtime_vectors, validate_runtime_vectors,
};

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
    // SAFETY: The facade preserves both backend range/lifetime contracts and
    // passes an index already validated against the common CPU-slot capacity.
    unsafe { super::imp::install_exception_stacks(cpu.get(), irq, emergency) }
}

/// Installs the selected architecture's permanent runtime exception vectors.
///
/// # Safety
///
/// The final executable kernel mapping and a valid exception stack must be
/// active. Kernel entry services must be initialized and local interrupts must
/// remain masked until installation and validation complete. Any backend
/// one-time installation requirement must also be upheld.
#[inline]
pub(crate) unsafe fn install_runtime_vectors() {
    // SAFETY: This facade preserves the selected backend's mapping, entry
    // service, interrupt-state, and one-time publication requirements.
    unsafe { super::imp::install_runtime_vectors() }
}

/// Installs already-published runtime vector state on the calling CPU.
///
/// # Safety
///
/// The global vector representation must be immutable and published. This CPU
/// must own installed exception stacks and keep local interrupts masked until
/// its local vector state has been validated.
#[inline]
pub(crate) unsafe fn install_local_runtime_vectors() {
    // SAFETY: The facade preserves the selected backend's per-CPU stack,
    // mapping, publication, and interrupt-mask requirements.
    unsafe { super::imp::install_local_runtime_vectors() }
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
    // SAFETY: The facade forwards the strict common emergency-stack lifetime,
    // exclusivity, interrupt-state, and callback argument contract.
    unsafe { super::imp::run_on_emergency_stack(callback, argument) }
}
