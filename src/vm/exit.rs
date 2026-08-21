//! Architecture-neutral events reported by guest execution.
//!
//! This module describes guest-visible facts consumed by VM policy. It does
//! not own raw exception frames, decode architecture syndromes, or apply
//! actions to hardware state; those responsibilities remain in the selected
//! architecture backend.

/// The memory operation which caused a guest-memory fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAccess {
    Execute,
    Read,
    Write,
}

/// One decoded access to a memory-mapped virtual device.
///
/// The architecture backend retains the raw exception frame and any register
/// completion metadata. VM policy receives only this owned description.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MmioAccess {
    address: GuestPhysicalAddress,
    size: usize,
    operation: MmioOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioOperation {
    Read,
    Write(u64),
}

impl MmioAccess {
    pub const fn new(address: GuestPhysicalAddress, size: usize, operation: MmioOperation) -> Self {
        Self {
            address,
            size,
            operation,
        }
    }

    pub const fn address(self) -> GuestPhysicalAddress {
        self.address
    }

    pub const fn size(self) -> usize {
        self.size
    }

    pub const fn operation(self) -> MmioOperation {
        self.operation
    }
}

/// An address in a virtual machine's physical address space.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GuestPhysicalAddress(u64);

impl GuestPhysicalAddress {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A failed access resolved through the guest's second-stage address space.
///
/// Architecture backends preserve the most specific fact common to `AArch64`
/// stage-2 faults, `RISC-V` guest-page faults, and x86 EPT/NPT violations. In
/// particular, this event does not claim that the failure was caused by a
/// missing translation: some architectures report permission failures through
/// the same exit class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestMemoryFault {
    address: GuestPhysicalAddress,
    access: MemoryAccess,
    during_guest_page_walk: bool,
}

impl GuestMemoryFault {
    pub const fn new(
        address: GuestPhysicalAddress,
        access: MemoryAccess,
        during_guest_page_walk: bool,
    ) -> Self {
        Self {
            address,
            access,
            during_guest_page_walk,
        }
    }

    pub const fn address(self) -> GuestPhysicalAddress {
        self.address
    }

    pub const fn access(self) -> MemoryAccess {
        self.access
    }

    /// Returns whether hardware faulted while walking a guest page table.
    pub const fn during_guest_page_walk(self) -> bool {
        self.during_guest_page_walk
    }
}
