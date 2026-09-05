// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture native-user machine contracts.
//!
//! This facade is intentionally unavailable on secondary architectures until
//! they define their own execution mechanism. It contains no process or ABI
//! policy and exposes no runnable entry before address-space residency exists.

#[cfg(all(target_arch = "aarch64", feature = "kernel-self-test"))]
pub(crate) use super::imp::direct_native_call_count_for_test;
#[cfg(target_arch = "aarch64")]
pub(crate) use super::imp::{
    PreparedUserAddressSpace, UserAddressSpaceError, UserLocalActivation, UserLocalIdentity,
    UserLocalOperation, UserLocalRequest, UserMappingPage, activate_user_local,
    deactivate_user_local, prepare_nvhe_user_address_space, prepare_vhe_user_address_space,
    service_user_local_request, user_local_identity_is_active, user_uses_vhe_translation,
};
#[cfg(target_arch = "aarch64")]
pub(crate) use super::imp::{
    UserCompletionFailure, UserContext, UserEntryError, UserExit, UserReturnCapability, run_user,
};
#[cfg(target_arch = "aarch64")]
pub(crate) use super::imp::{
    UserMachineContractError, assert_kernel_pan, copy_from_exposed, copy_to_exposed,
    user_address_limit,
};
