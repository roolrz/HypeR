// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native inspect syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{
    DeferredAction, InspectServices, SystemInspectServices,
};
use crate::kernel::abi::native::status::{
    handle_result, info_result, scan_result, status_from_inspection_error,
};
use crate::kernel::abi::native::wire::{
    copy_encoded_page, copy_info_record, encode_cpu_observation, encode_handle_inspection,
    encode_memory_observation, encode_object_inspection, encode_task_process, encode_task_thread,
    parse_handle_inspector_scan, parse_inspector_derivation, parse_inspector_scan,
    prepare_info_request,
};
use crate::kernel::inspect::{OBJECT_PAGE_CAPACITY, Page};
use hyper::abi::native::{
    HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE, HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE,
    HYPER_NATIVE_OBJECT_DETAILS_MIN_SIZE, HyperNativeCpuObservation, HyperNativeMemoryObservation,
    HyperNativeObjectDetails, HyperNativeObjectInspection, HyperNativeTaskProcess,
    HyperNativeTaskThread,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_read_details(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = (|| {
        let request = prepare_info_request(
            &[arguments[0], arguments[4], arguments[5], 0, 0, 0],
            HYPER_NATIVE_OBJECT_DETAILS_MIN_SIZE,
            core::mem::size_of::<HyperNativeObjectDetails>(),
        )?;
        let value = services
            .read_object_details(request.value, arguments[1], arguments[2], arguments[3])
            .map_err(status_from_inspection_error)?;
        let record = crate::kernel::abi::native::wire::encode_object_details(&value);
        copy_info_record(services, request, &record)
    })();
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_scan_processes(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_scan(
        arguments,
        crate::kernel::inspect::PROCESS_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeTaskProcess>(),
    )
    .and_then(|(inspector, cursor, destination)| {
        let mut page = Page::empty();
        services
            .scan_processes(inspector, cursor, &mut page)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_task_process)
    });
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_scan_threads(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_scan(
        arguments,
        crate::kernel::inspect::THREAD_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeTaskThread>(),
    )
    .and_then(|(inspector, cursor, destination)| {
        let mut page = Page::empty();
        services
            .scan_threads(inspector, cursor, &mut page)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_task_thread)
    });
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_scan_objects(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_scan(
        arguments,
        OBJECT_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeObjectInspection>(),
    )
    .and_then(|(inspector, cursor, destination)| {
        let mut page = Page::empty();
        services
            .scan_objects(inspector, cursor, &mut page)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_object_inspection)
    });
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_scan_handles(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_handle_inspector_scan(arguments).and_then(
        |(inspector, process_koid, cursor, destination)| {
            let page = services
                .scan_process_handles(inspector, process_koid, cursor)
                .map_err(status_from_inspection_error)?;
            copy_encoded_page(services, destination, &page, encode_handle_inspection)
        },
    );
    DeferredAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_derive_process(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, process)| {
        services
            .derive_task_inspector(inspector, process)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_derive_process(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, process)| {
        services
            .derive_object_inspector(inspector, process)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_derive_task_group(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, group)| {
        services
            .derive_task_inspector_for_task_group(inspector, group)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_derive_task_group(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, group)| {
        services
            .derive_object_inspector_for_task_group(inspector, group)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_derive_resource_domain(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, domain)| {
        services
            .derive_task_inspector_for_resource_domain(inspector, domain)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_derive_resource_domain(
    services: &impl InspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_inspector_derivation(arguments).and_then(|(inspector, domain)| {
        services
            .derive_object_inspector_for_resource_domain(inspector, domain)
            .map_err(status_from_inspection_error)
    });
    DeferredAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_memory_inspector_read(
    services: &impl SystemInspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE,
        core::mem::size_of::<HyperNativeMemoryObservation>(),
    )
    .and_then(|request| {
        let inspector = request.value;
        let observation = services
            .memory_observation(inspector)
            .map_err(status_from_inspection_error)?;
        copy_info_record(services, request, &encode_memory_observation(observation))
    });
    DeferredAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_cpu_inspector_read(
    services: &impl SystemInspectServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE,
        core::mem::size_of::<HyperNativeCpuObservation>(),
    )
    .and_then(|request| {
        let inspector = request.value;
        let observation = services
            .cpu_observation(inspector)
            .map_err(status_from_inspection_error)?;
        copy_info_record(services, request, &encode_cpu_observation(observation))
    });
    DeferredAction::Return(info_result(result))
}
