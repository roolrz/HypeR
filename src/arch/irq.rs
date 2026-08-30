// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture host interrupt mechanisms.
//!
//! Kernel IRQ policy owns domains, handler lifetimes, routing, and failure
//! policy. This facade exposes only local masking, controller construction,
//! platform interrupt decoding, and scheduler cross-call notification. Guest
//! interrupt virtualization belongs to `arch::vm`.

use hyper::cpu::CpuIndex;
use hyper::hal::interrupt::{EntryAction, InterruptId};
use hyper::sync::atomic::{AtomicU8, Ordering};

use core::cell::UnsafeCell;

const SERVICE_EMPTY: u8 = 0;
const SERVICE_INSTALLING: u8 = 1;
const SERVICE_READY: u8 = 2;

struct KernelRpcServiceSlot {
    state: AtomicU8,
    callback: UnsafeCell<Option<fn()>>,
}

// SAFETY: A single successful state transition owns the only write. The
// callback becomes immutable before SERVICE_READY is Release-published, and
// every reader observes that publication with Acquire ordering.
unsafe impl Sync for KernelRpcServiceSlot {}

impl KernelRpcServiceSlot {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(SERVICE_EMPTY),
            callback: UnsafeCell::new(None),
        }
    }

    fn install(&self, callback: fn()) -> Result<(), KernelRpcServiceError> {
        self.state
            .compare_exchange(
                SERVICE_EMPTY,
                SERVICE_INSTALLING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .map_err(|_| KernelRpcServiceError::AlreadyInstalled)?;
        // SAFETY: The successful state transition above grants this CPU the
        // only write to the slot. No reader dereferences it before READY.
        unsafe { *self.callback.get() = Some(callback) };
        self.state.store(SERVICE_READY, Ordering::Release);
        Ok(())
    }

    fn service(&self) {
        if self.state.load(Ordering::Acquire) != SERVICE_READY {
            // A Kernel RPC doorbell is enabled only after installation. Seeing
            // it earlier means entry ordering is corrupt, so fail closed.
            super::imp::halt()
        }
        // SAFETY: Acquire observed the immutable callback published before the
        // READY store. READY can never transition back to another state.
        let Some(callback) = (unsafe { *self.callback.get() }) else {
            super::imp::halt()
        };
        callback();
    }
}

static KERNEL_RPC_SERVICE: KernelRpcServiceSlot = KernelRpcServiceSlot::new();

/// Immutable kernel policy callbacks reachable from physical-interrupt entry.
#[derive(Clone, Copy)]
pub(crate) struct InterruptEntryServices {
    dispatch: fn(InterruptId, Option<unsafe extern "C" fn()>) -> EntryAction,
    #[cfg(CONFIG_ARCH_RISCV64)]
    claim_external: fn() -> Option<EntryAction>,
    stop: fn(super::exception::CrashContext) -> !,
}

impl InterruptEntryServices {
    pub(crate) const fn new(
        dispatch: fn(InterruptId, Option<unsafe extern "C" fn()>) -> EntryAction,
        #[cfg(CONFIG_ARCH_RISCV64)] claim_external: fn() -> Option<EntryAction>,
        stop: fn(super::exception::CrashContext) -> !,
    ) -> Self {
        Self {
            dispatch,
            #[cfg(CONFIG_ARCH_RISCV64)]
            claim_external,
            stop,
        }
    }
}

struct InterruptEntryServiceSlot {
    state: AtomicU8,
    services: UnsafeCell<Option<InterruptEntryServices>>,
}

// SAFETY: A single installer initializes the immutable table before Release-
// publishing READY. Every architecture entry reader observes it with Acquire.
unsafe impl Sync for InterruptEntryServiceSlot {}

impl InterruptEntryServiceSlot {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(SERVICE_EMPTY),
            services: UnsafeCell::new(None),
        }
    }

    fn install(
        &self,
        services: InterruptEntryServices,
    ) -> Result<InterruptEntryReady, InterruptEntryServiceError> {
        self.state
            .compare_exchange(
                SERVICE_EMPTY,
                SERVICE_INSTALLING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .map_err(|_| InterruptEntryServiceError::AlreadyInstalled)?;
        // SAFETY: The state transition grants the only write to this slot.
        unsafe { *self.services.get() = Some(services) };
        self.state.store(SERVICE_READY, Ordering::Release);
        Ok(InterruptEntryReady { _private: () })
    }

    fn services(&self) -> InterruptEntryServices {
        if self.state.load(Ordering::Acquire) != SERVICE_READY {
            super::imp::halt()
        }
        // SAFETY: Acquire observed the immutable table published at READY.
        let Some(services) = (unsafe { *self.services.get() }) else {
            super::imp::halt()
        };
        services
    }
}

static INTERRUPT_ENTRY_SERVICES: InterruptEntryServiceSlot = InterruptEntryServiceSlot::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InterruptEntryServiceError {
    AlreadyInstalled,
}

#[derive(Clone, Copy)]
pub(crate) struct InterruptEntryReady {
    _private: (),
}

pub(crate) fn install_interrupt_entry_services(
    services: InterruptEntryServices,
) -> Result<InterruptEntryReady, InterruptEntryServiceError> {
    INTERRUPT_ENTRY_SERVICES.install(services)
}

pub(crate) fn dispatch_entry(
    interrupt: InterruptId,
    native_unwind: Option<unsafe extern "C" fn()>,
) -> EntryAction {
    (INTERRUPT_ENTRY_SERVICES.services().dispatch)(interrupt, native_unwind)
}

#[cfg(CONFIG_ARCH_RISCV64)]
pub(crate) fn claim_and_dispatch_external_entry() -> Option<EntryAction> {
    (INTERRUPT_ENTRY_SERVICES.services().claim_external)()
}

pub(crate) fn stop_entry(context: super::exception::CrashContext) -> ! {
    (INTERRUPT_ENTRY_SERVICES.services().stop)(context)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KernelRpcServiceError {
    AlreadyInstalled,
}

pub(crate) use super::imp::{
    ArchitectureInterruptController as Controller, InterruptControllerError as ControllerError,
    LocalInterruptMask as LocalMask,
};

pub(crate) use super::imp::{
    decode_platform_interrupt as decode_platform, disable_all_interrupts as disable_all_sources,
    enable_local_irq as enable_local, local_irq_enabled as local_enabled,
    mask_local_irq as mask_local,
};

/// Returns the architecture-reserved physical reschedule interrupt, if any.
pub(crate) fn reschedule_interrupt() -> Option<InterruptId> {
    super::imp::reschedule_interrupt()
}

/// Prompts `cpu` to evaluate its already-published reschedule request.
pub(crate) fn notify_reschedule(cpu: CpuIndex) -> bool {
    super::imp::notify_reschedule(cpu)
}

pub(crate) fn kernel_rpc_interrupt() -> Option<InterruptId> {
    super::imp::kernel_rpc_interrupt()
}

pub(crate) fn arm_kernel_rpc_source() {
    super::imp::arm_kernel_rpc_source();
}

pub(crate) fn notify_kernel_rpc(cpu: CpuIndex, reasons: u8) -> bool {
    super::imp::notify_kernel_rpc(cpu, reasons)
}

pub(crate) fn take_kernel_rpc_reasons() -> u8 {
    super::imp::take_kernel_rpc_reasons()
}

/// Installs the allocation-free kernel dispatcher before its doorbell is armed.
pub(crate) fn install_kernel_rpc_service(callback: fn()) -> Result<(), KernelRpcServiceError> {
    KERNEL_RPC_SERVICE.install(callback)
}

/// Enters the opaque kernel dispatcher from architecture-private IRQ paths.
pub(crate) fn service_kernel_rpc() {
    KERNEL_RPC_SERVICE.service();
}
