// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native system syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::ImmediateServices;
use crate::kernel::abi::native::status::{failure, success};
use crate::kernel::abi::native::wire::require_zero;
use hyper::abi::native::{
    HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE, HYPER_NATIVE_STATUS_INTERNAL,
    HYPER_NATIVE_STATUS_NOT_SUPPORTED, NativeResult,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_abi_query(
    _services: &impl ImmediateServices,
    _arguments: &Arguments,
) -> NativeResult {
    success([HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE])
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_system_config(
    _services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    if let Err(status) = require_zero(&arguments[1..]) {
        return failure(status);
    }
    match arguments[0] {
        hyper::abi::native::HYPER_NATIVE_SYSTEM_CONFIG_PAGE_SIZE => {
            success([hyper::mm::PAGE_SIZE, 0])
        }
        hyper::abi::native::HYPER_NATIVE_SYSTEM_CONFIG_APPLICATION_ADDRESS_LIMIT => {
            match crate::hal::user::address_space_plan() {
                Ok(plan) => success([plan.application_limit(), 0]),
                Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
            }
        }
        _ => failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED),
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_clock_get_monotonic(
    _services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = require_zero(arguments).and_then(|()| {
        crate::kernel::time::monotonic_nanoseconds().map_err(|_| HYPER_NATIVE_STATUS_INTERNAL)
    });
    match result {
        Ok(nanoseconds) => success([nanoseconds, 0]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_not_supported() -> NativeResult {
    failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED)
}
