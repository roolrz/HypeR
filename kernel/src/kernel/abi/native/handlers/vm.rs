// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native vm syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, VmServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, info_result, status_from_vm_service_error, status_only, success,
};
use crate::kernel::abi::native::wire::{
    copy_info_record, decode_virtual_cpu_bootstrap, decode_virtual_machine_configuration,
    encode_virtual_cpu_info, encode_virtual_machine_info, parse_affinity_request, parse_handle,
    parse_single_handle, parse_two_handles, prepare_info_request, require_zero,
};
use hyper::abi::native::{
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE,
    HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE, HyperNativeVirtualCpuInfo,
    HyperNativeVirtualMachineInfo,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_creation_lease_create(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_two_handles(arguments).and_then(|[authority, domain]| {
        services
            .derive_virtual_machine_creation_lease(authority, domain)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_create(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|lease| {
        let configuration = decode_virtual_machine_configuration(services, arguments)?;
        services
            .create_pending_virtual_machine(lease, configuration)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_pending_virtual_machine_set_memory(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_two_handles(arguments).and_then(|[pending, vmo]| {
        services
            .set_pending_virtual_machine_memory(pending, vmo)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_pending_virtual_machine_set_bootstrap(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|pending| {
        let bootstrap = decode_virtual_cpu_bootstrap(services, arguments)?;
        services
            .set_pending_virtual_machine_bootstrap(pending, bootstrap)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_pending_virtual_machine_seal(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |pending| {
            services
                .seal_pending_virtual_machine(pending)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_pending_virtual_machine_install(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_single_handle(arguments).and_then(|pending| {
        services
            .install_pending_virtual_machine(pending)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(match result {
        Ok([machine, vcpu]) => success([machine.get(), vcpu.get()]),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_cpu_set_affinity(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_affinity_request(arguments).and_then(|(vcpu, words, count)| {
        services
            .set_virtual_cpu_affinity(vcpu, words, count)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_cpu_start(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |vcpu| {
            services
                .start_virtual_cpu(vcpu)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_pending_virtual_machine_abort(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |pending| {
            services
                .abort_pending_virtual_machine(pending)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_request_stop(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    DeferredAction::Return(status_only(parse_single_handle(arguments).and_then(
        |machine| {
            services
                .request_virtual_machine_stop(machine)
                .map_err(status_from_vm_service_error)
        },
    )))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_get_info(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualMachineInfo>(),
    )
    .and_then(|request| {
        let machine = request.value;
        let (configuration, snapshot) = services
            .virtual_machine_info(machine)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(
            services,
            request,
            &encode_virtual_machine_info(configuration, snapshot),
        )
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_cpu_get_info(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualCpuInfo>(),
    )
    .and_then(|request| {
        let vcpu = request.value;
        let snapshot = services
            .virtual_cpu_info(vcpu)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(services, request, &encode_virtual_cpu_info(snapshot))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_creation_lease_get_platform_info(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::abi::native::{
        HYPER_NATIVE_VIRTUAL_MACHINE_PLATFORM_INFO_MIN_SIZE, HyperNativeVirtualMachinePlatformInfo,
    };
    let result = (|| {
        require_zero(&arguments[4..])?;
        let profile =
            u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let request = prepare_info_request(
            &[arguments[0], arguments[2], arguments[3], 0, 0, 0],
            HYPER_NATIVE_VIRTUAL_MACHINE_PLATFORM_INFO_MIN_SIZE,
            core::mem::size_of::<HyperNativeVirtualMachinePlatformInfo>(),
        )?;
        let info = services
            .virtual_machine_platform_info(request.value, profile)
            .map_err(status_from_vm_service_error)?;
        copy_info_record(
            services,
            request,
            &crate::kernel::abi::native::wire::encode_virtual_machine_platform_info(info),
        )
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_get_power_request(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::abi::native::{
        HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_VIRTUAL_MACHINE_POWER_REQUEST_MIN_SIZE,
        HyperNativeVirtualMachinePowerRequest,
    };
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_VIRTUAL_MACHINE_POWER_REQUEST_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualMachinePowerRequest>(),
    )
    .and_then(|output| {
        // Inspection is non-consuming. A failed copyout leaves this exact
        // request pending; only explicit ID completion advances ownership.
        let request = services
            .pending_power_request(output.value)
            .map_err(status_from_vm_service_error)?
            .ok_or(HYPER_NATIVE_STATUS_WOULD_BLOCK)?;
        copy_info_record(
            services,
            output,
            &crate::kernel::abi::native::wire::encode_virtual_machine_power_request(request),
        )
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_complete_power_request(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let machine = parse_handle(arguments[0])?;
        if arguments[1] == 0 || arguments[2] > 1 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .complete_power_request(machine, arguments[1], arguments[2] == 1)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_open_vcpu(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        let machine = parse_handle(arguments[0])?;
        let vcpu = u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        services
            .open_vcpu(machine, vcpu)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_register_mmio(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[4..])?;
        let machine = parse_handle(arguments[0])?;
        if arguments[2] == 0
            || arguments[3] == 0
            || arguments[1].checked_add(arguments[2]).is_none()
        {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .register_mmio(machine, arguments[1], arguments[2], arguments[3])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_cpu_get_device_request(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::abi::native::{
        HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_VIRTUAL_CPU_DEVICE_REQUEST_MIN_SIZE,
        HyperNativeVirtualCpuDeviceRequest,
    };
    let device = arguments[3];
    let mut info_arguments = *arguments;
    info_arguments[3] = 0;
    let result = prepare_info_request(
        &info_arguments,
        HYPER_NATIVE_VIRTUAL_CPU_DEVICE_REQUEST_MIN_SIZE,
        core::mem::size_of::<HyperNativeVirtualCpuDeviceRequest>(),
    )
    .and_then(|output| {
        let request = services
            .pending_mmio(output.value, device)
            .map_err(status_from_vm_service_error)?
            .ok_or(HYPER_NATIVE_STATUS_WOULD_BLOCK)?;
        copy_info_record(
            services,
            output,
            &crate::kernel::abi::native::wire::encode_virtual_cpu_device_request(request),
        )
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_cpu_complete_mmio(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    use hyper::vm::exit::MmioAction;
    let result = (|| {
        require_zero(&arguments[4..])?;
        let vcpu = parse_handle(arguments[0])?;
        if arguments[1] == 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let action = match (arguments[2], arguments[3]) {
            (0, value) => MmioAction::CompleteRead(value),
            (1, 0) => MmioAction::CompleteWrite,
            (2, 0) => MmioAction::Stop,
            _ => return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
        };
        services
            .complete_mmio(vcpu, arguments[1], action)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_memory_create(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_single_handle(arguments).and_then(|vmo| {
        services
            .create_guest_memory(vmo)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_pending_virtual_machine_map_memory(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        let pending = parse_handle(arguments[0])?;
        let memory = parse_handle(arguments[1])?;
        services
            .map_guest_memory(pending, memory, arguments[2], arguments[3], arguments[4])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_register_mmio_event(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[5..])?;
        let machine = parse_handle(arguments[0])?;
        let event = parse_handle(arguments[4])?;
        if arguments[2] == 0
            || arguments[3] == 0
            || arguments[1].checked_add(arguments[2]).is_none()
        {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .register_mmio_event(machine, arguments[1], arguments[2], arguments[3], event)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_set_device_interrupt(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        let machine = parse_handle(arguments[0])?;
        let interrupt =
            u32::try_from(arguments[1]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        let asserted = match arguments[2] {
            0 => false,
            1 => true,
            _ => return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
        };
        services
            .set_device_interrupt(machine, interrupt, asserted)
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_virtual_machine_bind_firmware_console(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        let machine = parse_handle(arguments[0])?;
        if arguments[1] == 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        services
            .bind_firmware_console(machine, arguments[1])
            .map_err(status_from_vm_service_error)
    })();
    DeferredAction::Return(status_only(result))
}
