// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Typed access boundary for native application virtual memory.

use super::user_space::{
    AddressSpaceError, MemoryAccount, PageBackend, UserAddressSpace, UserSlice,
};

pub type UserCopyError<BackendError, AccountError> = AddressSpaceError<BackendError, AccountError>;

/// Copies from a fully validated user range without exposing a user reference.
/// Backend failure may leave the destination prefix modified.
pub fn copy_from_user<Backend, Account>(
    address_space: &UserAddressSpace<Backend, Account>,
    source: UserSlice,
    destination: &mut [u8],
) -> Result<(), UserCopyError<Backend::Error, Account::Error>>
where
    Backend: PageBackend,
    Account: MemoryAccount,
{
    address_space.copy_from_user(source, destination)
}

/// Copies into a fully validated user range without exposing a user reference.
/// Backend failure may leave an earlier user-memory prefix modified.
pub fn copy_to_user<Backend, Account>(
    address_space: &UserAddressSpace<Backend, Account>,
    destination: UserSlice,
    source: &[u8],
) -> Result<(), UserCopyError<Backend::Error, Account::Error>>
where
    Backend: PageBackend,
    Account: MemoryAccount,
{
    address_space.copy_to_user(destination, source)
}
