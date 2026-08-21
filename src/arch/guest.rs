//! Selected-architecture Linux guest boot ABI.
//!
//! Kernel VM policy owns bundle selection, VM publication, memory sizing, and
//! scheduling. This facade selects the guest-visible architecture name, IPA
//! layout, image validation/loading convention, boot-vCPU register state, and
//! layout diagnostics. Hardware virtualization remains in [`super::vm`].

use hyper::vm::{exit::GuestPhysicalAddress, interrupt::VirtualInterruptId};

pub(crate) use super::imp::{
    linux_kernel_occupied_size, load_linux_payload, validate_linux_host, validate_linux_kernel,
};

/// Architecture-defined failures while validating or laying out a Linux guest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    #[cfg_attr(not(CONFIG_ARCH_X86_64), allow(dead_code))]
    AddressOverflow,
    DeviceTree(hyper::vm::fdt::Error),
    InvalidKernel,
    InvalidLayout,
    #[cfg_attr(not(CONFIG_ARCH_X86_64), allow(dead_code))]
    VirtualizationUnavailable,
}

impl From<hyper::vm::fdt::Error> for Error {
    fn from(error: hyper::vm::fdt::Error) -> Self {
        Self::DeviceTree(error)
    }
}

/// Failure from guest-ABI layout or from the caller-owned memory sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PayloadLoadError<MemoryError> {
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
///
/// The implementation owns allocation, sparse-memory policy, address-space
/// locking, and cache maintenance. Architecture loaders only choose guest IPA
/// destinations and whether loaded bytes contain instructions or data.
pub(crate) trait PayloadMemory {
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
///
/// Construction rejects reversed bounds, so architecture loaders may derive a
/// byte count without relying on an unchecked subtraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PayloadRange {
    start: GuestPhysicalAddress,
    end: GuestPhysicalAddress,
}

impl PayloadRange {
    pub(crate) const fn new(
        start: GuestPhysicalAddress,
        end: GuestPhysicalAddress,
    ) -> Option<Self> {
        if start.get() > end.get() {
            return None;
        }
        Some(Self { start, end })
    }

    pub(crate) const fn start(self) -> GuestPhysicalAddress {
        self.start
    }

    pub(crate) const fn end(self) -> GuestPhysicalAddress {
        self.end
    }

    #[cfg(CONFIG_ARCH_X86_64)]
    pub(crate) const fn length(self) -> u64 {
        self.end.get() - self.start.get()
    }
}

/// Architecture-owned constants which define one Linux guest boot ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LinuxAbi {
    architecture: &'static str,
    ram_base: GuestPhysicalAddress,
    kernel_load: GuestPhysicalAddress,
    timer_interrupt: VirtualInterruptId,
}

impl LinuxAbi {
    pub(crate) const fn architecture(self) -> &'static str {
        self.architecture
    }

    pub(crate) const fn ram_base(self) -> GuestPhysicalAddress {
        self.ram_base
    }

    pub(crate) const fn kernel_load(self) -> GuestPhysicalAddress {
        self.kernel_load
    }

    pub(crate) const fn timer_interrupt(self) -> VirtualInterruptId {
        self.timer_interrupt
    }
}

/// Returns the selected architecture's immutable Linux guest ABI.
pub(crate) const fn linux_abi() -> LinuxAbi {
    LinuxAbi {
        architecture: super::imp::linux_guest_architecture(),
        ram_base: GuestPhysicalAddress::new(super::imp::LINUX_GUEST_RAM_IPA),
        kernel_load: GuestPhysicalAddress::new(super::imp::LINUX_GUEST_KERNEL_IPA),
        timer_interrupt: VirtualInterruptId::constant::<{ super::imp::LINUX_GUEST_TIMER_INTERRUPT }>(
        ),
    }
}

#[inline]
pub(crate) fn prepare_linux_vcpu_context() -> super::vm::VcpuContext {
    super::imp::prepare_linux_vcpu_context()
}

/// Visits host requirements selected by the Linux guest ABI.
#[inline]
pub(crate) fn describe_linux_host(emit: impl FnMut(core::fmt::Arguments<'_>)) {
    super::imp::describe_linux_host(emit);
}

/// Visits guest-layout descriptions without selecting the kernel log sink.
#[inline]
pub(crate) fn describe_linux_layout(
    initramfs_range: Option<PayloadRange>,
    stage2_root: u64,
    emit: impl FnMut(core::fmt::Arguments<'_>),
) {
    super::imp::describe_linux_guest_layout(initramfs_range, stage2_root, emit);
}
