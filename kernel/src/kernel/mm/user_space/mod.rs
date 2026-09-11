// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent ownership for native user virtual memory.
//!
//! The mechanism is generic over page allocation and resource accounting so
//! its transaction and protection rules can be tested on a host. Architecture
//! page tables, translation identifiers, and invalidation live below HAL.

#![cfg_attr(test, allow(unexpected_cfgs))]

mod address_space;
#[cfg(not(test))]
mod authority;
mod contract;
#[cfg(not(test))]
mod kernel_adapter;
#[cfg(not(test))]
mod machine;
#[cfg(not(test))]
mod objects;
#[cfg(not(test))]
mod service;
mod transaction;
mod vmo;
#[cfg(test)]
pub(crate) use transaction::retry_stale;
#[cfg(not(test))]
pub(crate) use vmo::{ExclusiveHardwareWriteLease, WritableMappingLease};

pub(crate) use address_space::{
    AddressSpaceError, AddressSpaceId, CommittedMappingChange, MappingChange, MappingSnapshot,
    MappingToken, PreparedMappingChange, PreparedPageSnapshot, PreparedUserWrite, UserAddressSpace,
    Vmar,
};
#[cfg(not(test))]
pub(crate) use authority::{ExecutableAuthority, ExecutableAuthorityError};
pub(crate) use contract::{
    Access, AddressError, MemoryAccount, MemoryCharge, PageBackend, Permissions, UserAddress,
    UserAddressWindow, UserSlice,
};
#[cfg(not(test))]
pub(crate) use kernel_adapter::address_window;
#[cfg(all(not(test), feature = "kernel-self-test"))]
pub(crate) use kernel_adapter::fail_exposed_write_after_copy_for_test;
#[cfg(not(test))]
pub(crate) use kernel_adapter::{DomainAccount, KernelPageBackend, KernelPageError};
#[cfg(not(test))]
pub(crate) use machine::{
    ActiveNativeAddressSpace, Error as MachineError, NativeAddressSpace, NativeImageSegment,
    StoppedNativeRun, UserWriteReservation,
};
#[cfg(all(not(test), feature = "kernel-self-test"))]
pub(crate) use machine::{prepare_native_entry_self_test, run_dormant_self_test};
#[cfg(not(test))]
pub(crate) use objects::{GuestMemoryBacking, MemoryObjectError, VmarObject, VmoObject};
#[cfg(not(test))]
pub(crate) use service::{ServiceError as MemoryServiceError, permissions as abi_permissions};
#[cfg(not(test))]
pub(crate) use service::{
    allocate_vmar, create_file_executable_vmo, create_vmo, destroy as destroy_vmar, map_vmo,
    protect, read_vmo, unmap, write_vmo,
};

#[cfg(not(test))]
pub(crate) use machine::service_local_rpc;
pub(crate) use vmo::{
    ExecutableProvenance, ExecutableVmo, PrivateMappingMode, SnapshotVmo, VmoError,
    VmoPopulateError, WeakSnapshotVmo, WritableVmo,
};

#[cfg(not(test))]
pub(crate) use service::{
    PrivateMappingRequest, create_file_snapshot, create_vmo_snapshot, map_private,
};
