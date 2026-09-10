// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture native-user machine contracts.
//!
//! Each supported backend defines its own execution mechanism. This facade
//! contains no process or ABI policy and exposes no runnable entry before
//! address-space residency exists.

#[cfg(all(target_arch = "riscv64", feature = "kernel-self-test"))]
pub(crate) use super::imp::{
    native_fault_test_programs_for_test, native_register_test_program_for_test,
};

#[cfg(all(
    any(target_arch = "aarch64", target_arch = "riscv64"),
    feature = "kernel-self-test"
))]
pub(crate) use super::imp::direct_native_call_count_for_test;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(crate) use super::imp::{
    PreparedUserAddressSpace, UserAddressSpaceError, UserLocalActivation, UserLocalIdentity,
    UserLocalOperation, UserLocalRequest, UserMappingPage, activate_user_local,
    deactivate_user_local, service_user_local_request, user_local_identity_is_active,
};
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(crate) use super::imp::{
    UserCompletionFailure, UserContext, UserEntryError, UserExit, UserReturnCapability, run_user,
};
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(crate) use super::imp::{
    UserMachineContractError, copy_from_exposed, copy_to_exposed, user_address_limit,
};

#[cfg(target_arch = "riscv64")]
pub(crate) use super::imp::{
    assert_kernel_access, prepare_host_user_address_space, user_translation_identifier_bits,
};
#[cfg(target_arch = "aarch64")]
pub(crate) use super::imp::{
    assert_kernel_pan as assert_kernel_access, prepare_nvhe_user_address_space,
    prepare_vhe_user_address_space as prepare_host_user_address_space, user_uses_vhe_translation,
};
