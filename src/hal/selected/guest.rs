// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected Linux guest boot ABI.
//!
//! Kernel VM policy owns bundle selection, VM publication, memory sizing, and
//! scheduling. This facade selects the architecture name, guest-physical boot
//! layout, image validation/loading convention, initial vCPU register state,
//! and diagnostic layout descriptions. Stage-2 translation and hardware vCPU
//! lifecycle remain in [`super::vm`].

use hyper::vm::bundle::VmBundle;
use hyper::vm::exit::GuestPhysicalAddress;
use hyper::vm::interrupt::VirtualInterruptId;

/// Architecture-defined failure while validating or laying out a Linux guest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AddressOverflow,
    DeviceTree(hyper::vm::fdt::Error),
    InvalidKernel,
    InvalidLayout,
    VirtualizationUnavailable,
}

impl From<crate::arch::guest::Error> for Error {
    fn from(error: crate::arch::guest::Error) -> Self {
        match error {
            crate::arch::guest::Error::AddressOverflow => Self::AddressOverflow,
            crate::arch::guest::Error::DeviceTree(error) => Self::DeviceTree(error),
            crate::arch::guest::Error::InvalidKernel => Self::InvalidKernel,
            crate::arch::guest::Error::InvalidLayout => Self::InvalidLayout,
            crate::arch::guest::Error::VirtualizationUnavailable => Self::VirtualizationUnavailable,
        }
    }
}

/// Failure from the selected boot ABI or from the caller-owned memory sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PayloadLoadError<MemoryError> {
    Abi(Error),
    Memory(MemoryError),
}

/// Minimal write-and-publish capability required by a guest image loader.
///
/// Implementations retain allocation, sparse-memory, locking, and cache-
/// maintenance policy. The selected loader chooses only guest destinations
/// and whether bytes will be executed or consumed as data.
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PayloadRange {
    backend: crate::arch::guest::PayloadRange,
}

impl PayloadRange {
    pub(crate) const fn new(
        start: GuestPhysicalAddress,
        end: GuestPhysicalAddress,
    ) -> Option<Self> {
        match crate::arch::guest::PayloadRange::new(start, end) {
            Some(backend) => Some(Self { backend }),
            None => None,
        }
    }

    pub(crate) const fn end(self) -> GuestPhysicalAddress {
        self.backend.end()
    }

    const fn length(self) -> u64 {
        // The backend constructor establishes the half-open range invariant.
        self.backend.end().get() - self.backend.start().get()
    }

    fn into_backend(self) -> crate::arch::guest::PayloadRange {
        self.backend
    }
}

/// Immutable architecture-selected Linux boot parameters.
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

pub(crate) const fn linux_abi() -> LinuxAbi {
    let abi = crate::arch::guest::linux_abi();
    LinuxAbi {
        architecture: abi.architecture(),
        ram_base: abi.ram_base(),
        kernel_load: abi.kernel_load(),
        timer_interrupt: abi.timer_interrupt(),
    }
}

pub(crate) fn validate_linux_host() -> Result<(), Error> {
    crate::arch::guest::validate_linux_host().map_err(Error::from)
}

pub(crate) fn validate_linux_kernel(image: &[u8]) -> Result<(), Error> {
    crate::arch::guest::validate_linux_kernel(image).map_err(Error::from)
}

pub(crate) fn linux_kernel_occupied_size(image: &[u8]) -> Result<u64, Error> {
    crate::arch::guest::linux_kernel_occupied_size(image).map_err(Error::from)
}

pub(crate) fn prepare_linux_vcpu_context() -> super::vm::VcpuContext {
    crate::arch::guest::prepare_linux_vcpu_context()
}

pub(crate) fn load_linux_payload<Memory: PayloadMemory>(
    guest: &VmBundle<'_>,
    memory: &mut Memory,
    initramfs_range: Option<PayloadRange>,
) -> Result<(), PayloadLoadError<Memory::Error>> {
    // Architecture loaders treat this range as both a copy destination and
    // the guest-visible initramfs extent. Keep that implicit backend contract
    // behind this safe facade: absence and byte length must agree exactly.
    let initramfs_matches_range = match (guest.initramfs(), initramfs_range) {
        (None, None) => true,
        (Some(bytes), Some(range)) => u64::try_from(bytes.len()).ok() == Some(range.length()),
        _ => false,
    };
    if !initramfs_matches_range {
        return Err(PayloadLoadError::Abi(Error::InvalidLayout));
    }

    let mut adapter = BackendPayloadMemory(memory);
    crate::arch::guest::load_linux_payload(
        guest,
        &mut adapter,
        initramfs_range.map(PayloadRange::into_backend),
    )
    .map_err(|error| match error {
        crate::arch::guest::PayloadLoadError::Abi(error) => PayloadLoadError::Abi(error.into()),
        crate::arch::guest::PayloadLoadError::Memory(error) => PayloadLoadError::Memory(error),
    })
}

struct BackendPayloadMemory<'memory, Memory>(&'memory mut Memory);

impl<Memory: PayloadMemory> crate::arch::guest::PayloadMemory for BackendPayloadMemory<'_, Memory> {
    type Error = Memory::Error;

    fn copy_to(&mut self, address: GuestPhysicalAddress, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.copy_to(address, bytes)
    }

    fn publish_instruction(
        &self,
        address: GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error> {
        self.0.publish_instruction(address, length)
    }

    fn publish_data(
        &self,
        address: GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error> {
        self.0.publish_data(address, length)
    }
}

pub(crate) fn describe_linux_host(emit: impl FnMut(core::fmt::Arguments<'_>)) {
    crate::arch::guest::describe_linux_host(emit);
}

pub(crate) fn describe_linux_layout(
    initramfs_range: Option<PayloadRange>,
    stage2_root: u64,
    emit: impl FnMut(core::fmt::Arguments<'_>),
) {
    crate::arch::guest::describe_linux_layout(
        initramfs_range.map(PayloadRange::into_backend),
        stage2_root,
        emit,
    );
}
