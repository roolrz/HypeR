// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native object syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, ObjectServiceError, ObjectServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, status_from_address_error, status_from_object_service_error,
    status_only, success,
};
use crate::kernel::abi::native::wire::{parse_handle, parse_wait_many};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::object::{SignalWaitManyOutcome, SignalWaitOutcome};
use hyper::abi::native::{
    HYPER_NATIVE_STATUS_CANCELLED, HYPER_NATIVE_STATUS_INTERNAL,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_STATUS_TIMED_OUT, NativeResult,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_event_create(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    handle_result(
        services
            .create_event()
            .map_err(status_from_object_service_error),
    )
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_event_signal(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .signal_event(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_wait_one(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .wait_one(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    let result = match result {
        Ok(SignalWaitOutcome::Observed(snapshot)) => success([snapshot.signals().bits(), 0]),
        Ok(SignalWaitOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_wait_many(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_wait_many(arguments).and_then(|(items, item_count, deadline)| {
        services
            .wait_many(items, item_count, deadline)
            .map_err(status_from_object_service_error)
    });
    let result = match result {
        Ok(SignalWaitManyOutcome::Observed { index, snapshot }) => match u64::try_from(index) {
            Ok(index) => success([index, snapshot.signals().bits()]),
            Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Ok(SignalWaitManyOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitManyOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_create(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[1..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_create(
            usize::try_from(arguments[0]).map_err(|_| ObjectServiceError::InvalidInput)?,
        )
    })();
    DeferredAction::Return(match result {
        Ok(value) => success([value.get(), 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_add(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[3..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_add(
            parse_handle(arguments[0]).map_err(|_| ObjectServiceError::InvalidInput)?,
            parse_handle(arguments[1]).map_err(|_| ObjectServiceError::InvalidInput)?,
            arguments[2],
        )
    })();
    DeferredAction::Return(match result {
        Ok(value) => success([value, 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_rearm(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[2..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_rearm(
            parse_handle(arguments[0]).map_err(|_| ObjectServiceError::InvalidInput)?,
            arguments[1],
        )
    })();
    DeferredAction::Return(match result {
        Ok(_) => success([0, 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_remove(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[2..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_remove(
            parse_handle(arguments[0]).map_err(|_| ObjectServiceError::InvalidInput)?,
            arguments[1],
        )
    })();
    DeferredAction::Return(match result {
        Ok(_) => success([0, 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_wait(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        if arguments[3] != 24 || arguments[4] != 0 || arguments[5] != 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let set = parse_handle(arguments[0])?;
        let output = UserSlice::new(UserAddress::new(arguments[2]), 24)
            .map_err(status_from_address_error)?;
        services
            .wait_set_wait(set, arguments[1], output)
            .map_err(status_from_object_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
