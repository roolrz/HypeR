// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native process builder syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, ProcessBuilderServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, status_from_process_builder_service_error, status_only, success,
};
use crate::kernel::abi::native::wire::{
    parse_affinity_request, parse_builder_create, parse_builder_handle, parse_builder_text,
    parse_handle,
};
use crate::kernel::capability::HandleValue;
use hyper::abi::native::{
    HYPER_NATIVE_PROCESS_NAME_MAX_BYTES, HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_create(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result =
        parse_builder_create(arguments).and_then(|[factory, group, domain, executable]| {
            services
                .create_process_builder(factory, group, domain, executable)
                .map_err(status_from_process_builder_service_error)
        });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_name(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(arguments, HYPER_NATIVE_PROCESS_NAME_MAX_BYTES).and_then(
        |(builder, text)| {
            services
                .set_process_builder_name(builder, text)
                .map_err(status_from_process_builder_service_error)
        },
    );
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_data(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_text(arguments, HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES)
        .and_then(|(builder, text)| {
            services
                .set_process_builder_data(builder, text)
                .map_err(status_from_process_builder_service_error)
        });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_affinity(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_affinity_request(arguments).and_then(|(builder, words, word_count)| {
        services
            .set_process_builder_affinity(builder, words, word_count)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_add_handle(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_builder_handle(arguments).and_then(
        |(builder, source, purpose, expected_kind, requested_rights, operation)| {
            services
                .add_process_builder_handle(
                    builder,
                    source,
                    purpose,
                    expected_kind,
                    requested_rights,
                    operation,
                )
                .map_err(status_from_process_builder_service_error)
        },
    );
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_seal(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|builder| {
        services
            .seal_process_builder(builder)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_start(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|builder| {
        services
            .start_process_builder(builder)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(match result {
        Ok(handles) => success(handles.map(HandleValue::get)),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_abort(
    services: &impl ProcessBuilderServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|builder| {
        services
            .abort_process_builder(builder)
            .map_err(status_from_process_builder_service_error)
    });
    DeferredAction::Return(status_only(result))
}
