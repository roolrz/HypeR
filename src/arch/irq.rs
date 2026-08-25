// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture host interrupt mechanisms.
//!
//! Kernel IRQ policy owns domains, handler lifetimes, routing, and failure
//! policy. This facade exposes only local masking, controller construction,
//! platform interrupt decoding, and scheduler cross-call notification. Guest
//! interrupt virtualization belongs to `arch::vm`.

use hyper::cpu::CpuIndex;
use hyper::hal::interrupt::InterruptId;

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
