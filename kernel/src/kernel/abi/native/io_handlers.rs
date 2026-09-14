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

fn status_from_match_error(
    error: crate::kernel::device::assigned::service::MatchError,
) -> HyperNativeStatus {
    use crate::kernel::device::assigned::service::MatchError;
    match error {
        MatchError::Service(error) => status_from_vm_service_error(error),
        MatchError::Missing => hyper::abi::native::HYPER_NATIVE_STATUS_NOT_FOUND,
        MatchError::Ambiguous => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    }
}

pub(super) fn sys_device_firmware_read(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[4..])?;
        let authority = parse_handle(args[0])?;
        if args[3] > 65536 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let mut query = [0u8; 24];
        let source =
            optional_user_slice(args[1], 24)?.ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .copy_from_user(source, &mut query)
            .map_err(status_from_process_error)?;
        let node = u32::from_le_bytes(
            query[..4]
                .try_into()
                .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
        );
        let field = u32::from_le_bytes(
            query[4..8]
                .try_into()
                .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
        );
        let name_address = u64::from_le_bytes(
            query[8..16]
                .try_into()
                .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
        );
        let name_length = u64::from_le_bytes(
            query[16..24]
                .try_into()
                .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
        );
        if field > 4 || name_length > 128 || (field == 4) != (name_length != 0) {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let mut name = [0u8; 128];
        let name_length =
            usize::try_from(name_length).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        if let Some(source) = optional_user_slice(name_address, name_length as u64)? {
            services
                .copy_from_user(source, &mut name[..name_length])
                .map_err(status_from_process_error)?;
        }
        let name = core::str::from_utf8(&name[..name_length])
            .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        if name.contains('\0') {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        // All firmware/device locks have been released before copying to user
        // memory, whose pages may fault or require a COW allocation.
        let bytes = services
            .device_firmware_read(authority, node, field, name)
            .map_err(status_from_match_error)?;
        if bytes.len() > 65536 {
            return Err(hyper::abi::native::HYPER_NATIVE_STATUS_INTERNAL);
        }
        let length = bytes.len() as u64;
        if args[3] == 0 {
            return Ok(length);
        }
        if args[3] < length {
            return Err(hyper::abi::native::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL);
        }
        if let Some(destination) = optional_user_slice(args[2], length)? {
            services
                .copy_to_user(destination, &bytes)
                .map_err(status_from_process_error)?;
        }
        Ok(length)
    })();
    DeferredAction::Return(info_result(result))
}

pub(super) fn sys_device_claim_bundle(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[4..])?;
        let authority = parse_handle(args[0])?;
        if args[2] == 0 || args[2] > 8 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let irq_node = u32_value(args[3])?;
        let count = usize::try_from(args[2]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let mut bytes = [0u8; 8 * 16];
        let source = optional_user_slice(args[1], args[2] * 16)?
            .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .copy_from_user(source, &mut bytes[..count * 16])
            .map_err(status_from_process_error)?;
        let mut entries = [(0, 0, 0); 8];
        for (entry, bytes) in entries[..count]
            .iter_mut()
            .zip(bytes[..count * 16].chunks_exact(16))
        {
            *entry = (
                u32::from_le_bytes(
                    bytes[..4]
                        .try_into()
                        .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
                ),
                u32::from_le_bytes(
                    bytes[4..8]
                        .try_into()
                        .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
                ),
                u64::from_le_bytes(
                    bytes[8..16]
                        .try_into()
                        .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?,
                ),
            );
        }
        services
            .device_claim_bundle(authority, &entries[..count], irq_node)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

pub(super) fn sys_device_mmio(services: &impl DeviceServices, args: &Arguments) -> DeferredAction {
    let result = (|| {
        require_zero(&args[5..])?;
        if !matches!(args[2], 1 | 2 | 4) || args[3] > 1 || (args[3] == 0 && args[4] != 0) {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .device_mmio(
                parse_handle(args[0])?,
                args[1],
                u32_value(args[2])?,
                args[3] != 0,
                args[4],
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(info_result(result))
}
pub(super) fn sys_device_irq_pending(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[1..])?;
        services
            .device_irq_pending(parse_handle(args[0])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(info_result(result))
}
pub(super) fn sys_device_irq_complete(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[3..])?;
        if args[2] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .device_irq_complete(parse_handle(args[0])?, args[1], args[2] != 0)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

pub(super) fn sys_device_profile_info(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let output = prepare_info_request(args, 32, 32)?;
        let bytes = services
            .device_profile_info(output.value)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(services, output, &bytes)
    })();
    DeferredAction::Return(info_result(result))
}
pub(super) fn sys_device_resource_info(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[4..])?;
        let output = prepare_info_request(&[args[0], args[2], args[3], 0, 0, 0], 32, 32)?;
        let bytes = services
            .device_resource_info(output.value, u32_value(args[1])?)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(services, output, &bytes)
    })();
    DeferredAction::Return(info_result(result))
}

pub(super) fn sys_device_claim_matching(
    services: &impl DeviceServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[5..])?;
        let length = usize::try_from(args[4]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        if length == 0 || length > 512 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let source =
            optional_user_slice(args[3], args[4])?.ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let mut bytes = [0u8; 512];
        services
            .copy_from_user(source, &mut bytes[..length])
            .map_err(status_from_process_error)?;
        let identity = core::str::from_utf8(&bytes[..length])
            .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .claim_device_matching(
                parse_handle(args[0])?,
                u32_value(args[1])?,
                u32_value(args[2])?,
                identity,
            )
            .map_err(status_from_match_error)
    })();
    DeferredAction::Return(handle_result(result))
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

pub(super) fn sys_native_block_create(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[5..])?;
        services
            .create_native_block(
                parse_handle(args[0])?,
                parse_handle(args[1])?,
                args[2],
                args[3],
                u32_value(args[4])?,
            )
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

pub(super) fn sys_native_block_activate(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[2..])?;
        if args[1] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .activate_native_block(parse_handle(args[0])?, args[1] != 0)
            .map_err(|error| {
                use crate::kernel::block::service::ActivationError;
                use hyper::abi::native::*;
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

pub(super) fn sys_native_block_mount(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[4..])?;
        if args[3] == 0 || args[3] > 4096 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let path = UserSlice::new(UserAddress::new(args[2]), args[3])
            .map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .mount_native_block(parse_handle(args[0])?, parse_handle(args[1])?, path)
            .map_err(super::status::status_from_vfs_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

pub(super) fn sys_guest_mapping_create(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[3..])?;
        services
            .create_guest_mapping(parse_handle(args[0])?, parse_handle(args[1])?, args[2])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(match result {
        Ok((handle, token)) => success([handle.get(), token]),
        Err(error) => failure(error),
    })
}
pub(super) fn sys_guest_mapping_release(
    services: &impl GuestIoServices,
    args: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&args[1..])?;
        services
            .release_guest_mapping(parse_handle(args[0])?)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
