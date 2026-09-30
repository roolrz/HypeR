// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native ipc syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, IpcServices};
use crate::kernel::abi::native::status::{
    failure, status_from_byte_channel_service_error, status_from_capability_channel_error,
    status_from_capability_channel_service_error, status_only, success,
};
use crate::kernel::abi::native::wire::{
    parse_byte_channel_io, parse_capability_channel_receive, parse_capability_channel_send,
};
use crate::kernel::ipc::{
    ByteChannelReadOutcome, CapabilityChannelError, CapabilityReceiveOutcome,
};
use hyper::abi::native::{
    HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, HYPER_NATIVE_STATUS_INTERNAL,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
    HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE, HyperNativeStatus, NativeResult,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_byte_channel_create(
    services: &impl IpcServices,
    arguments: &Arguments,
) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    match services
        .create_byte_channel()
        .map_err(status_from_byte_channel_service_error)
    {
        Ok([first, second]) => success([first.get(), second.get()]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_capability_channel_create(
    services: &impl IpcServices,
    arguments: &Arguments,
) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    match services
        .create_capability_channel()
        .map_err(status_from_capability_channel_service_error)
    {
        Ok([first, second]) => success([first.get(), second.get()]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_byte_channel_write(
    services: &impl IpcServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_byte_channel_io(arguments).and_then(|(endpoint, bytes)| {
        services
            .write_byte_channel(endpoint, bytes)
            .map_err(status_from_byte_channel_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_byte_channel_read(
    services: &impl IpcServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_byte_channel_io(arguments).and_then(|(endpoint, bytes)| {
        services
            .read_byte_channel(endpoint, bytes)
            .map_err(status_from_byte_channel_service_error)
    });
    let result = match result {
        Ok(ByteChannelReadOutcome::Received { bytes }) => success([bytes, 0]),
        Ok(ByteChannelReadOutcome::BufferTooSmall { bytes }) => NativeResult::for_syscall(
            HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
            HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
            [bytes, 0],
        ),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_capability_channel_try_send(
    services: &impl IpcServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result =
        parse_capability_channel_send(arguments).and_then(|(endpoint, bytes, dispositions)| {
            services
                .try_send_capability_channel(endpoint, bytes, dispositions)
                .map_err(status_from_capability_channel_service_error)
        });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_capability_channel_receive(
    services: &impl IpcServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_capability_channel_receive(arguments).and_then(
        |(endpoint, deadline, bytes, slots)| {
            services
                .receive_capability_channel(endpoint, deadline, bytes, slots)
                .map_err(status_from_capability_channel_service_error)
        },
    );
    DeferredAction::Return(capability_receive_result(result))
}

pub(in crate::kernel::abi::native) fn capability_receive_result(
    result: Result<CapabilityReceiveOutcome, HyperNativeStatus>,
) -> NativeResult {
    match result {
        Ok(CapabilityReceiveOutcome::Delivered(info)) => {
            match (u64::try_from(info.bytes), u64::try_from(info.handles)) {
                (Ok(bytes), Ok(handles)) => success([bytes, handles]),
                _ => failure(HYPER_NATIVE_STATUS_INTERNAL),
            }
        }
        Ok(CapabilityReceiveOutcome::Failed(CapabilityChannelError::BufferTooSmall {
            required_bytes,
            required_handles,
        })) => match (
            u64::try_from(required_bytes),
            u64::try_from(required_handles),
        ) {
            (Ok(bytes), Ok(handles)) => NativeResult::for_syscall(
                HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE,
                HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
                [bytes, handles],
            ),
            _ => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Ok(CapabilityReceiveOutcome::Failed(error)) => {
            failure(status_from_capability_channel_error(error))
        }
        Err(status) => failure(status),
    }
}
