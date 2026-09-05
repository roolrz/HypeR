// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-selected Linux boot contracts.
//!
//! These types describe guest-visible layout and payload publication. They are
//! VM product policy rather than host-machine capabilities.

use super::Error;
use hyper::vm::exit::GuestPhysicalAddress;
use hyper::vm::interrupt::VirtualInterruptId;

/// Failure from guest-ABI layout or from the caller-owned memory sink.
#[derive(Debug)]
pub(super) enum PayloadLoadError<MemoryError> {
    Abi(Error),
    Memory(MemoryError),
}

impl<MemoryError> From<Error> for PayloadLoadError<MemoryError> {
    fn from(error: Error) -> Self {
        Self::Abi(error)
    }
}

impl<MemoryError> From<hyper::vm::fdt::Error> for PayloadLoadError<MemoryError> {
    fn from(error: hyper::vm::fdt::Error) -> Self {
        Self::Abi(error.into())
    }
}

/// Minimal write-and-publish capability required by a guest image loader.
pub(in crate::kernel::vm) trait PayloadMemory {
    type Error;

    fn copy_to(&mut self, address: GuestPhysicalAddress, bytes: &[u8]) -> Result<(), Self::Error>;
    fn publish_instruction(
        &self,
        address: GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error>;
    fn publish_data(&self, address: GuestPhysicalAddress, length: usize)
    -> Result<(), Self::Error>;
}

/// Validated half-open guest-physical range occupied by one boot payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PayloadRange {
    start: GuestPhysicalAddress,
    end: GuestPhysicalAddress,
}

impl PayloadRange {
    pub(super) const fn new(
        start: GuestPhysicalAddress,
        end: GuestPhysicalAddress,
    ) -> Option<Self> {
        if start.get() > end.get() {
            return None;
        }
        Some(Self { start, end })
    }

    pub(super) const fn start(self) -> GuestPhysicalAddress {
        self.start
    }

    pub(super) const fn end(self) -> GuestPhysicalAddress {
        self.end
    }

    pub(super) const fn length(self) -> u64 {
        self.end.get() - self.start.get()
    }
}

/// Immutable architecture-selected Linux boot parameters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LinuxAbi {
    architecture: &'static str,
    ram_base: GuestPhysicalAddress,
    kernel_load: GuestPhysicalAddress,
    timer_interrupt: VirtualInterruptId,
}

impl LinuxAbi {
    pub(super) const fn new(
        architecture: &'static str,
        ram_base: u64,
        kernel_load: u64,
        timer_interrupt: u32,
    ) -> Self {
        Self {
            architecture,
            ram_base: GuestPhysicalAddress::new(ram_base),
            kernel_load: GuestPhysicalAddress::new(kernel_load),
            timer_interrupt: VirtualInterruptId::new(timer_interrupt),
        }
    }

    pub(super) const fn architecture(self) -> &'static str {
        self.architecture
    }
    pub(super) const fn ram_base(self) -> GuestPhysicalAddress {
        self.ram_base
    }
    pub(super) const fn kernel_load(self) -> GuestPhysicalAddress {
        self.kernel_load
    }
    pub(super) const fn timer_interrupt(self) -> VirtualInterruptId {
        self.timer_interrupt
    }
}
