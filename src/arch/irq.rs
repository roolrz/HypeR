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
use hyper::sync::PublishedOnce;

static KERNEL_RPC_SERVICE: PublishedOnce<fn()> = PublishedOnce::new();

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

static INTERRUPT_ENTRY_SERVICES: PublishedOnce<InterruptEntryServices> = PublishedOnce::new();

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
    INTERRUPT_ENTRY_SERVICES
        .publish(services)
        .map_err(|_| InterruptEntryServiceError::AlreadyInstalled)?;
    Ok(InterruptEntryReady { _private: () })
}

fn interrupt_entry_services() -> InterruptEntryServices {
    let Some(services) = INTERRUPT_ENTRY_SERVICES.get().copied() else {
        super::imp::halt()
    };
    services
}

pub(crate) fn dispatch_entry(
    interrupt: InterruptId,
    native_unwind: Option<unsafe extern "C" fn()>,
) -> EntryAction {
    (interrupt_entry_services().dispatch)(interrupt, native_unwind)
}

#[cfg(CONFIG_ARCH_RISCV64)]
pub(crate) fn claim_and_dispatch_external_entry() -> Option<EntryAction> {
    (interrupt_entry_services().claim_external)()
}

pub(crate) fn stop_entry(context: super::exception::CrashContext) -> ! {
    (interrupt_entry_services().stop)(context)
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
    KERNEL_RPC_SERVICE
        .publish(callback)
        .map_err(|_| KernelRpcServiceError::AlreadyInstalled)
}

/// Enters the opaque kernel dispatcher from architecture-private IRQ paths.
pub(crate) fn service_kernel_rpc() {
    // A Kernel RPC doorbell is enabled only after installation. Seeing it
    // earlier means entry ordering is corrupt, so fail closed.
    let Some(callback) = KERNEL_RPC_SERVICE.get().copied() else {
        super::imp::halt()
    };
    callback();
}
