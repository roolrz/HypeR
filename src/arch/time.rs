//! Selected-architecture monotonic time mechanisms.
//!
//! Kernel timekeeping owns clock and timer policy. This facade selects the
//! machine counter, one-shot comparator, and platform timer description
//! without exposing an architecture backend to policy code.

pub(crate) use super::imp::{
    ArchitectureCounter as Counter, ArchitectureTimer as Timer, TimerError as Error,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptionError {
    InvalidInterruptTrigger,
    UnsupportedTimer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Description {
    pub hardware: hyper::platform::PlatformInterrupt,
    pub guest_virtual_interrupt: hyper::hal::interrupt::InterruptId,
    pub map_guest_virtual_interrupt: bool,
}

pub(crate) use super::imp::decode_kernel_timer as describe;

pub(crate) fn prepare(platform: &super::platform::EssentialInfo) -> Result<(), Error> {
    super::imp::prepare_timekeeping(platform.as_backend())
}
