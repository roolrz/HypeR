// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native guest I/O syscall validation.

use crate::kernel::abi::native::services::DeferredAction;
use crate::kernel::abi::native::status::{
    failure, handle_result, status_from_process_error, status_from_vm_service_error, status_only,
    success,
};
use crate::kernel::abi::native::wire::{
    optional_user_slice, parse_handle, parse_u32, require_zero,
};
use crate::kernel::abi::native::{Arguments, GuestIoServices};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::vm::service::Error;
use hyper::abi::native::HYPER_NATIVE_STATUS_INVALID_ARGUMENT;

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_mailbox_create(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        services
            .create_guest_mailbox(
                parse_handle(arguments[0])?,
                arguments[1],
                parse_u32(arguments[2])?,
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_mailbox_send(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let length =
            usize::try_from(arguments[2]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        if length == 0 || length > 256 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let source = optional_user_slice(arguments[1], arguments[2])?
            .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let mailbox = parse_handle(arguments[0])?;
        let mut bytes = [0u8; 256];
        services
            .copy_from_user(source, &mut bytes[..length])
            .map_err(status_from_process_error)?;
        services
            .send_guest_mailbox(mailbox, &bytes[..length])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_mailbox_receive(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        if arguments[2] == 0 || arguments[2] > 256 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let _validated = optional_user_slice(arguments[1], arguments[2])?;
        let mailbox = parse_handle(arguments[0])?;
        services
            .receive_guest_mailbox(mailbox, &mut |bytes| {
                if bytes.len() as u64 > arguments[2] {
                    return Err(Error::InvalidArgument);
                }
                let destination =
                    UserSlice::new(UserAddress::new(arguments[1]), bytes.len() as u64)
                        .map_err(|_| Error::Fault)?;
                services
                    .copy_to_user(destination, bytes)
                    .map_err(|_| Error::Fault)
            })
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(length) => success([length as u64, 0]),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_notification_create(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        services
            .create_guest_notification(
                parse_handle(arguments[0])?,
                parse_handle(arguments[1])?,
                arguments[2],
                arguments[3],
                parse_u32(arguments[4])?,
                parse_u32(arguments[5])?,
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_notification_control(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        services
            .control_guest_notification(parse_handle(arguments[0])?, parse_u32(arguments[1])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(epoch) => success([u64::from(epoch), 0]),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_native_block_create(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        services
            .create_native_block(
                parse_handle(arguments[0])?,
                parse_handle(arguments[1])?,
                arguments[2],
                arguments[3],
                parse_u32(arguments[4])?,
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_native_block_activate(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        if arguments[1] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .activate_native_block(parse_handle(arguments[0])?, arguments[1] != 0)
            .map_err(|error| {
                use crate::kernel::block::service::ActivationError;
                use hyper::abi::native::{
                    HYPER_NATIVE_STATUS_IO_ERROR, HYPER_NATIVE_STATUS_NOT_SUPPORTED,
                    HYPER_NATIVE_STATUS_READ_ONLY, HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
                };
                use hyper::fs::block::Error;
                match error {
                    ActivationError::Capability(error) => status_from_process_error(error),
                    ActivationError::Device(error) => match error {
                        Error::InvalidRange => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
                        Error::ReadOnly => HYPER_NATIVE_STATUS_READ_ONLY,
                        Error::Disconnected | Error::Io | Error::Corrupt => {
                            HYPER_NATIVE_STATUS_IO_ERROR
                        }
                        Error::Unsupported => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
                        Error::Exhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
                    },
                }
            })
    })();
    DeferredAction::Return(match result {
        Ok(sectors) => success([sectors, 0]),
        Err(error) => failure(error),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_native_block_mount(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[4..])?;
        if arguments[3] == 0 || arguments[3] > 4096 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let path = UserSlice::new(UserAddress::new(arguments[2]), arguments[3])
            .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .mount_native_block(
                parse_handle(arguments[0])?,
                parse_handle(arguments[1])?,
                path,
            )
            .map_err(crate::kernel::abi::native::status::status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_mapping_create(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        services
            .create_guest_mapping(
                parse_handle(arguments[0])?,
                parse_handle(arguments[1])?,
                arguments[2],
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(match result {
        Ok((handle, token)) => success([handle.get(), token]),
        Err(error) => failure(error),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_mapping_release(
    services: &impl GuestIoServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[1..])?;
        services
            .release_guest_mapping(parse_handle(arguments[0])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
