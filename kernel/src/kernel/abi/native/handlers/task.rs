// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native task syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{
    DeferredAction, HierarchyServices, ObjectServices, TaskServices,
};
use crate::kernel::abi::native::status::{
    failure, handle_result, info_result, status_from_hierarchy_error,
    status_from_object_service_error, status_from_process_error, status_only, success,
};
use crate::kernel::abi::native::wire::{
    PROCESS_INFO_SIZE, copy_info_record, decode_resource_limits, encode_process_info,
    parse_affinity_words, parse_handle, parse_single_handle, prepare_info_request, require_zero,
};
use hyper::abi::native::{
    HYPER_NATIVE_PROCESS_INFO_MIN_SIZE, HYPER_NATIVE_STATUS_CANCELLED,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_STATUS_TIMED_OUT, HyperNativeStatus,
    NativeResult,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_resource_domain_create(
    services: &impl HierarchyServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|parent| {
        let limits = decode_resource_limits(services, arguments)?;
        services
            .create_resource_domain(parent, limits)
            .map_err(status_from_hierarchy_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_group_create(
    services: &impl HierarchyServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|factory| {
        let domain = parse_handle(arguments[1])?;
        require_zero(&arguments[2..])?;
        services
            .create_task_group(factory, domain)
            .map_err(status_from_hierarchy_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_thread_yield() -> DeferredAction {
    DeferredAction::Yield(success([0, 0]))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_thread_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitThread {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitProcess {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_request_stop(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|process| {
        services
            .request_process_stop(process)
            .map_err(status_from_process_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_get_info(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_PROCESS_INFO_MIN_SIZE,
        PROCESS_INFO_SIZE,
    )
    .and_then(|request| {
        let process = request.value;
        let snapshot = services
            .process_info(process)
            .map_err(status_from_process_error)?;
        let record = encode_process_info(snapshot);
        copy_info_record(services, request, &record)
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_thread_create(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> NativeResult {
    let affinity = if arguments[4] == 0 && arguments[5] == 0 {
        Ok((None, 0))
    } else {
        parse_affinity_words(arguments[4], arguments[5])
    };
    handle_result(affinity.and_then(|(words, count)| {
        services
            .create_thread(
                arguments[0],
                arguments[1],
                arguments[2],
                arguments[3],
                words,
                count,
            )
            .map_err(status_from_object_service_error)
    }))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_thread_start(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> NativeResult {
    status_only(parse_single_handle(arguments).and_then(|handle| {
        services
            .start_thread(handle)
            .map_err(status_from_process_error)
    }))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_thread_request_stop(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> NativeResult {
    status_only(parse_single_handle(arguments).and_then(|handle| {
        services
            .stop_thread(handle)
            .map_err(status_from_process_error)
    }))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_atomic_wait(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> NativeResult {
    let result = require_zero(&arguments[3..]).and_then(|()| {
        let expected =
            u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .atomic_wait(arguments[0], expected, arguments[2])
            .map_err(status_from_object_service_error)
    });
    wait_status(result, false)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_atomic_wake(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> NativeResult {
    match require_zero(&arguments[2..]).and_then(|()| {
        let count =
            u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .atomic_wake(arguments[0], count)
            .map_err(status_from_object_service_error)
    }) {
        Ok(count) => success([count, 0]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_thread_sleep(
    services: &impl TaskServices,
    arguments: &Arguments,
) -> NativeResult {
    wait_status(
        require_zero(&arguments[1..]).and_then(|()| {
            services
                .sleep_thread(arguments[0])
                .map_err(status_from_object_service_error)
        }),
        true,
    )
}

fn wait_status(
    result: Result<crate::kernel::task::WaitOutcome, HyperNativeStatus>,
    sleep: bool,
) -> NativeResult {
    use crate::kernel::task::WaitOutcome;
    match result {
        Ok(WaitOutcome::Notified) => success([0, 0]),
        Ok(WaitOutcome::TimedOut) if sleep => success([0, 0]),
        Ok(WaitOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(WaitOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_get_current_id(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(if arguments.iter().any(|value| *value != 0) {
        failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
    } else {
        success([services.current_process_id(), 0])
    })
}
