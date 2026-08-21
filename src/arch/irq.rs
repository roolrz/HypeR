//! Selected-architecture host interrupt mechanisms.
//!
//! Kernel IRQ policy owns domains, handler lifetimes, routing, and failure
//! policy. This facade exposes only local masking, controller construction,
//! and platform interrupt decoding. Guest interrupt virtualization belongs to
//! `arch::vm`.

pub(crate) use super::imp::{
    ArchitectureInterruptController as Controller, InterruptControllerError as ControllerError,
    LocalInterruptMask as LocalMask,
};

pub(crate) use super::imp::{
    decode_platform_interrupt as decode_platform, disable_local_interrupts as disable_local,
    enable_local_irq as enable_local, local_irq_enabled as local_enabled,
};
