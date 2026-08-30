// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Domain types and injectable ownership mechanisms for user memory.

use core::fmt::Debug;

use hyper::mm::PhysicalAddress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AddressError {
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub(crate) struct UserAddress(u64);

impl UserAddress {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn checked_add(self, offset: u64) -> Option<Self> {
        self.0.checked_add(offset).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UserSlice {
    base: UserAddress,
    length: u64,
}

/// HAL-validated virtual-address authority for one machine regime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UserAddressWindow {
    range: UserSlice,
}

impl UserAddressWindow {
    /// Constructs a window from selected machine limits inside this subsystem.
    pub(super) fn from_range(range: UserSlice) -> Result<Self, AddressError> {
        if range.length() == 0 {
            return Err(AddressError::Overflow);
        }
        Ok(Self { range })
    }

    pub(super) fn from_limit(exclusive_limit: u64) -> Result<Self, AddressError> {
        Self::from_range(UserSlice::new(UserAddress::new(0), exclusive_limit)?)
    }

    pub(crate) const fn range(self) -> UserSlice {
        self.range
    }

    #[cfg(test)]
    pub(crate) fn for_test(base: u64, length: u64) -> Result<Self, AddressError> {
        Self::from_range(UserSlice::new(UserAddress::new(base), length)?)
    }
}

impl UserSlice {
    pub(crate) fn new(base: UserAddress, length: u64) -> Result<Self, AddressError> {
        base.checked_add(length).ok_or(AddressError::Overflow)?;
        Ok(Self { base, length })
    }

    pub(crate) const fn base(self) -> UserAddress {
        self.base
    }

    pub(crate) const fn length(self) -> u64 {
        self.length
    }

    pub(crate) fn end(self) -> UserAddress {
        match self.base.checked_add(self.length) {
            Some(end) => end,
            None => address_invariant_violation(),
        }
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        other.base >= self.base && other.end() <= self.end()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Access {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub(crate) struct Permissions(u8);

impl Permissions {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const READ: Self = Self(1 << 0);
    pub(crate) const WRITE: Self = Self(1 << 1);
    pub(crate) const EXECUTE: Self = Self(1 << 2);

    pub(crate) const fn read_only() -> Self {
        Self::READ
    }

    pub(crate) const fn read_write() -> Self {
        Self(Self::READ.0 | Self::WRITE.0)
    }

    pub(crate) const fn read_execute() -> Self {
        Self(Self::READ.0 | Self::EXECUTE.0)
    }

    pub(crate) const fn contains(self, access: Access) -> bool {
        let bit = match access {
            Access::Read => Self::READ.0,
            Access::Write => Self::WRITE.0,
            Access::Execute => Self::EXECUTE.0,
        };
        self.0 & bit != 0
    }

    pub(crate) const fn is_valid(self) -> bool {
        self.0 & !(Self::READ.0 | Self::WRITE.0 | Self::EXECUTE.0) == 0
            && (!self.contains(Access::Write) || self.contains(Access::Read))
            && (!self.contains(Access::Execute) || self.contains(Access::Read))
            && !(self.contains(Access::Write) && self.contains(Access::Execute))
    }

    pub(crate) const fn is_subset_of(self, maximum: Self) -> bool {
        self.0 & !maximum.0 == 0
    }
}

/// Semantic resource ownership requested before the corresponding allocation.
///
/// `kernel_bytes` accounts the requested allocator layout. Allocator size-class
/// fragmentation belongs to the allocator's own accounting and is not inferred
/// from a container's implementation-defined spare capacity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MemoryCharge {
    pub(crate) kernel_bytes: u64,
    pub(crate) kernel_objects: u64,
    pub(crate) committed_pages: u64,
    pub(crate) pinned_pages: u64,
    pub(crate) address_spaces: u64,
    pub(crate) mappings: u64,
}

/// Resource-domain adapter used by the portable user-memory core.
pub(crate) trait MemoryAccount: Clone {
    type Charge;
    type Error: Debug;

    /// Reserves and commits the complete amount or leaves usage unchanged.
    fn try_charge(&self, amount: MemoryCharge) -> Result<Self::Charge, Self::Error>;
}

/// Physically owned page adapter. Methods never retain caller buffers.
pub(crate) trait PageBackend: Clone {
    type Page;
    type Error: Debug;
    /// Environment-specific proof needed by instruction publication.
    ///
    /// A hardware backend uses a CPU-pinning capability. A pure model backend
    /// may use `()` when publication has no processor-local effects.
    type InstructionPublicationContext: ?Sized;

    fn allocate_zeroed(&self) -> Result<Self::Page, Self::Error>;
    fn physical_address(&self, page: &Self::Page) -> PhysicalAddress;

    /// Copies through a permanently mapped, machine-quiesced owned page.
    ///
    /// The caller serializes kernel access and proves no writable machine
    /// mapping is active. This method must not block or fault.
    fn read_owned(
        &self,
        page: &Self::Page,
        offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>;
    fn write_owned(
        &self,
        page: &mut Self::Page,
        offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error>;

    /// Copies from memory that may be concurrently visible to a machine.
    ///
    /// Production implementations use an architecture-audited external-memory
    /// sequence rather than a compiler memory intrinsic. An error may be
    /// reported after an earlier byte was copied.
    fn read_exposed(
        &self,
        page: &Self::Page,
        offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>;

    /// Copies to memory that may be concurrently visible to a machine.
    ///
    /// The external-memory and partial-effect contract of
    /// [`PageBackend::read_exposed`] also applies here.
    fn write_exposed(
        &self,
        page: &mut Self::Page,
        offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error>;

    /// Publishes a stable, repeatable page set to instruction coherence.
    ///
    /// Every enumeration must visit the same pages in the same order. When the
    /// backend's cache protocol is processor-local, `context` must prove that
    /// execution remains on one CPU across every operation and final barrier.
    /// The operation must neither block nor fault. It does not synchronize a
    /// future executing CPU's instruction stream; address-space activation
    /// performs that local context synchronization after observing the image.
    fn publish_instruction_pages(
        &self,
        context: &Self::InstructionPublicationContext,
        pages: impl FnMut(&mut dyn FnMut(&mut Self::Page)),
    ) -> Result<(), Self::Error>;
}

#[cold]
fn address_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
