// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected host-interrupt capabilities.
//!
//! Kernel policy owns IRQ domains, handler lifetimes, routing, and failure
//! policy. This facade binds those policies to the selected architecture's
//! local-mask, controller, platform-decoding, and reschedule-notification
//! mechanisms. Guest interrupt virtualization is exposed separately by the VM
//! capability facade.

use hyper::cpu::CpuIndex;
use hyper::hal::interrupt::{InterruptId, KernelRpcReasons};
use hyper::platform::PlatformInterrupt;

pub(crate) type Controller = crate::arch::irq::Controller;
pub(crate) type ControllerError = crate::arch::irq::ControllerError;
pub(crate) type LocalMask = crate::arch::irq::LocalMask;
pub(crate) type KernelRpcServiceError = crate::arch::irq::KernelRpcServiceError;

/// Selected platform interrupt descriptor could not be decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodeError;

/// Decodes one platform interrupt descriptor for the selected controller ABI.
pub(crate) fn decode_platform(descriptor: &[u32]) -> Result<PlatformInterrupt, DecodeError> {
    crate::arch::irq::decode_platform(descriptor).map_err(|_| DecodeError)
}

/// Masks ordinary local IRQ delivery until [`enable_local`] is called.
///
/// Lexical critical sections must use [`LocalMask`] through an interrupt-mask
/// guard so the exact prior state is restored instead of enabling IRQs
/// unconditionally.
pub(crate) fn mask_local() {
    crate::arch::irq::mask_local();
}

/// Irreversibly masks every architecture-supported local interrupt source.
///
/// This is reserved for fail-stop paths. Runtime transitions must use
/// [`mask_local`] so architecture source-enable state remains intact.
pub(crate) fn disable_all_sources() {
    crate::arch::irq::disable_all_sources();
}

/// Enables ordinary local IRQ delivery after runtime vectors are installed.
pub(crate) fn enable_local() {
    crate::arch::irq::enable_local();
}

/// Reports whether ordinary local IRQ delivery is currently enabled.
pub(crate) fn local_enabled() -> bool {
    crate::arch::irq::local_enabled()
}

/// Returns the architecture-reserved physical reschedule interrupt, if any.
pub(crate) fn reschedule_interrupt() -> Option<InterruptId> {
    crate::arch::irq::reschedule_interrupt()
}

/// Prompts `cpu` to evaluate an already-published reschedule request.
///
/// `false` means the selected backend cannot issue a qualified targeted
/// notification. The caller must preserve the durable request and use its
/// architecture-neutral wake fallback.
pub(crate) fn notify_reschedule(cpu: CpuIndex) -> bool {
    crate::arch::irq::notify_reschedule(cpu)
}

pub(crate) fn kernel_rpc_interrupt() -> Option<InterruptId> {
    crate::arch::irq::kernel_rpc_interrupt()
}

pub(crate) fn arm_kernel_rpc_source() {
    crate::arch::irq::arm_kernel_rpc_source();
}

pub(crate) fn notify_kernel_rpc(cpu: CpuIndex, reasons: KernelRpcReasons) -> bool {
    crate::arch::irq::notify_kernel_rpc(cpu, reasons.bits())
}

pub(crate) fn take_kernel_rpc_reasons() -> KernelRpcReasons {
    KernelRpcReasons::from_bits(crate::arch::irq::take_kernel_rpc_reasons())
}

pub(crate) fn install_kernel_rpc_services(
    poll: fn(),
    interrupt: fn(hyper::hal::interrupt::InterruptOrigin) -> hyper::hal::interrupt::EntryAction,
) -> Result<(), KernelRpcServiceError> {
    crate::arch::irq::install_kernel_rpc_services(poll, interrupt)
}
