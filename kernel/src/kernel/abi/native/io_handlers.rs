// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native device assignment and guest I/O control validation.

use super::status::{
    failure, handle_result, info_result, status_from_process_error, status_from_vm_service_error,
    status_only, success,
};
use super::wire::{
    copy_info_record, optional_user_slice, parse_handle, prepare_info_request, require_zero,
};
use super::{Arguments, DeferredAction, DeviceServices, GuestIoServices};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::vm::service::Error;
use hyper::abi::native::{HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HyperNativeStatus};

fn u32_value(value: u64) -> Result<u32, HyperNativeStatus> {
    u32::try_from(value).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
}

pub(super) fn sys_device_claim(services: &impl DeviceServices, args: &Arguments) -> DeferredAction {
    let result = (|| {
        require_zero(&args[2..])?;
        services
            .claim_device(parse_handle(args[0])?, u32_value(args[1])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

pub(super) fn sys_physical_device_info(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let output = prepare_info_request(args, 16, 16)?;
        let info = services
            .physical_device_info(output.value)
            .map_err(status_from_vm_service_error)?;
        let mut bytes = [0u8; 16];
        bytes[..4].copy_from_slice(&info.device_id.to_le_bytes());
        bytes[4..8].copy_from_slice(&info.transport_version.to_le_bytes());
        bytes[8..16].copy_from_slice(&info.mmio_size.to_le_bytes());
        copy_info_record(services, output, &bytes)
    })();
    DeferredAction::Return(info_result(result))
}

pub(super) fn sys_vmo_get_dma_extent(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let output = prepare_info_request(&[args[0], args[4], args[5], 0, 0, 0], 16, 16)?;
        let extent = services
            .vmo_dma_extent(output.value, parse_handle(args[1])?, args[2], args[3])
            .map_err(status_from_vm_service_error)?;
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&extent.physical_base.to_le_bytes());
        bytes[8..].copy_from_slice(&extent.length.to_le_bytes());
        copy_info_record(services, output, &bytes)
    })();
    DeferredAction::Return(info_result(result))
}

pub(super) fn sys_pending_virtual_machine_assign_device(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[4..])?;
        services
            .assign_physical_device(
                parse_handle(args[0])?,
                parse_handle(args[1])?,
                args[2],
                u32_value(args[3])?,
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

pub(super) fn sys_guest_mailbox_create(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[3..])?;
        services
            .create_guest_mailbox(parse_handle(args[0])?, args[1], u32_value(args[2])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(super) fn sys_guest_mailbox_send(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[3..])?;
        let length = usize::try_from(args[2]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        if length == 0 || length > 256 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let source =
            optional_user_slice(args[1], args[2])?.ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let mailbox = parse_handle(args[0])?;
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

pub(super) fn sys_guest_mailbox_receive(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[3..])?;
        if args[2] == 0 || args[2] > 256 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let _validated = optional_user_slice(args[1], args[2])?;
        let mailbox = parse_handle(args[0])?;
        services
            .receive_guest_mailbox(mailbox, &mut |bytes| {
                if bytes.len() as u64 > args[2] {
                    return Err(Error::InvalidArgument);
                }
                let destination = UserSlice::new(UserAddress::new(args[1]), bytes.len() as u64)
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

pub(super) fn sys_guest_notification_create(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        services
            .create_guest_notification(
                parse_handle(args[0])?,
                parse_handle(args[1])?,
                args[2],
                args[3],
                u32_value(args[4])?,
                u32_value(args[5])?,
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

pub(super) fn sys_guest_notification_control(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[2..])?;
        services
            .control_guest_notification(parse_handle(args[0])?, u32_value(args[1])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(match result {
        Ok(epoch) => success([u64::from(epoch), 0]),
        Err(status) => failure(status),
    })
}
