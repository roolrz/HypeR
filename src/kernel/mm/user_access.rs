//! Safe access boundary for application-owned virtual memory.

use hyper::mm::{ForeignCopyError, ForeignMemory};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressSpaceId(pub u64);

/// Capability implemented by a live, locked application address space.
///
/// A raw virtual address is deliberately insufficient to call the user-copy
/// API. The application memory manager must hold its mappings stable for the
/// duration of each operation and implement the page-level access contract.
pub trait UserAddressSpace: ForeignMemory {
    fn id(&self) -> AddressSpaceId;
}

pub type UserCopyError<BackendError> = ForeignCopyError<BackendError>;

pub fn copy_from_user<AddressSpace: UserAddressSpace + ?Sized>(
    address_space: &mut AddressSpace,
    source_address: u64,
    destination: &mut [u8],
) -> Result<(), UserCopyError<AddressSpace::Error>> {
    hyper::mm::copy_from_foreign(address_space, source_address, destination)
}

pub fn copy_to_user<AddressSpace: UserAddressSpace + ?Sized>(
    address_space: &mut AddressSpace,
    destination_address: u64,
    source: &[u8],
) -> Result<(), UserCopyError<AddressSpace::Error>> {
    hyper::mm::copy_to_foreign(address_space, destination_address, source)
}
