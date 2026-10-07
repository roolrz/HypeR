// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native immediate/deferred syscall routing.

use hyper::abi::native::{self as abi, NativeInvocation, NativeResult};

use super::handlers;
use super::services::{DeferredAction, DeferredServices, ImmediateServices};

/// Defines the complete audited masked-entry route set once.
///
/// Both membership and direct dispatch are generated from this list, so a new
/// immediate syscall cannot accidentally be admitted without a matching leaf
/// or routed through the immediate dispatcher without masked-entry admission.
macro_rules! define_immediate_routes {
    ($( $number:path => $handler:path ),+ $(,)?) => {
        /// Reports whether the current implementation is audited for masked entry.
        ///
        /// New syscall numbers default to the deferred Thread path until their
        /// implementation is proven nonblocking and added deliberately here.
        pub(in crate::kernel) const fn is_immediate(number: u64) -> bool {
            matches!(number, $( $number )|+)
        }

        /// Dispatches one owned, nonblocking invocation by direct match.
        ///
        /// Architecture entry retains its private raw frame but passes no frame
        /// borrow or architecture offset into this adapter. Unknown numbers and
        /// malformed values fail closed.
        pub(in crate::kernel) fn dispatch_immediate(
            services: &impl ImmediateServices,
            invocation: NativeInvocation,
        ) -> NativeResult {
            let arguments = invocation.arguments();
            match invocation.number() {
                $( $number => $handler(services, arguments), )+
                _ => handlers::sys_not_supported(),
            }
        }
    };
}

define_immediate_routes! {
    abi::HYPER_NATIVE_SYS_SYSTEM_CONFIG => handlers::sys_system_config,
    abi::HYPER_NATIVE_SYS_ABI_QUERY => handlers::sys_abi_query,
    abi::HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC => handlers::sys_clock_get_monotonic,
    abi::HYPER_NATIVE_SYS_HANDLE_CLOSE => handlers::sys_handle_close,
}

/// Executes one syscall after the machine context has returned to its Thread.
///
/// The returned action separates ABI decoding from scheduler and Process
/// policy. A blocking service may park the current Thread before producing the
/// action. Unknown calls remain ordinary returning failures even though they
/// conservatively use the deferred path.
pub(in crate::kernel) fn dispatch_deferred(
    services: &impl DeferredServices,
    invocation: NativeInvocation,
) -> DeferredAction {
    match invocation.number() {
        abi::HYPER_NATIVE_SYS_HANDLE_GET_INFO => DeferredAction::Return(
            handlers::sys_handle_get_info(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO => DeferredAction::Return(
            handlers::sys_object_get_basic_info(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_VMO_CREATE_SNAPSHOT => {
            handlers::sys_vmo_create_snapshot(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_CREATE_SNAPSHOT => {
            handlers::sys_file_create_snapshot(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMAR_MAP_PRIVATE => {
            handlers::sys_vmar_map_private(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_HANDLE_DUPLICATE => DeferredAction::Return(
            handlers::sys_handle_duplicate(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_HANDLE_REPLACE => DeferredAction::Return(
            handlers::sys_handle_replace(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_EVENT_CREATE => {
            DeferredAction::Return(handlers::sys_event_create(services, invocation.arguments()))
        }
        abi::HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE => DeferredAction::Return(
            handlers::sys_byte_channel_create(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE => DeferredAction::Return(
            handlers::sys_capability_channel_create(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_THREAD_CREATE => DeferredAction::Return(handlers::sys_thread_create(
            services,
            invocation.arguments(),
        )),
        abi::HYPER_NATIVE_SYS_THREAD_START => {
            DeferredAction::Return(handlers::sys_thread_start(services, invocation.arguments()))
        }
        abi::HYPER_NATIVE_SYS_THREAD_REQUEST_STOP => DeferredAction::Return(
            handlers::sys_thread_request_stop(services, invocation.arguments()),
        ),
        abi::HYPER_NATIVE_SYS_ATOMIC_WAIT => {
            DeferredAction::Return(handlers::sys_atomic_wait(services, invocation.arguments()))
        }
        abi::HYPER_NATIVE_SYS_ATOMIC_WAKE => {
            DeferredAction::Return(handlers::sys_atomic_wake(services, invocation.arguments()))
        }
        abi::HYPER_NATIVE_SYS_THREAD_SLEEP => {
            DeferredAction::Return(handlers::sys_thread_sleep(services, invocation.arguments()))
        }
        abi::HYPER_NATIVE_SYS_THREAD_YIELD => handlers::sys_thread_yield(),
        abi::HYPER_NATIVE_SYS_THREAD_EXIT => handlers::sys_thread_exit(invocation.arguments()),
        abi::HYPER_NATIVE_SYS_PROCESS_EXIT => handlers::sys_process_exit(invocation.arguments()),
        abi::HYPER_NATIVE_SYS_EVENT_SIGNAL => {
            handlers::sys_event_signal(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_WAIT_ONE => {
            handlers::sys_object_wait_one(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_WAIT_MANY => {
            handlers::sys_object_wait_many(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE => {
            handlers::sys_byte_channel_write(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_BYTE_CHANNEL_READ => {
            handlers::sys_byte_channel_read(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND => {
            handlers::sys_capability_channel_try_send(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE => {
            handlers::sys_capability_channel_receive(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_CONSOLE_READ => {
            handlers::sys_console_read(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_CONSOLE_WRITE => {
            handlers::sys_console_write(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE => {
            handlers::sys_directory_open_file(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY => {
            handlers::sys_directory_open_directory(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_READ => {
            handlers::sys_directory_read(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_WRITE_AT => {
            handlers::sys_file_write_at(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_RESIZE => {
            handlers::sys_file_resize(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_CREATE_FILE => {
            handlers::sys_directory_create_file(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_CREATE_DIRECTORY => {
            handlers::sys_directory_create_directory(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_SCOPE_CREATE => {
            handlers::sys_directory_scope_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_GET_METADATA => {
            handlers::sys_directory_get_metadata(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_GET_METADATA => {
            handlers::sys_file_get_metadata(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_GET_SELF_METADATA => {
            handlers::sys_directory_get_self_metadata(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_SET_METADATA => {
            handlers::sys_directory_set_metadata(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_SET_METADATA => {
            handlers::sys_file_set_metadata(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_RENAME => {
            handlers::sys_directory_rename(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_LINK => {
            handlers::sys_directory_link(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_SYMLINK => {
            handlers::sys_directory_symlink(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_READ_LINK => {
            handlers::sys_directory_read_link(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_CANONICALIZE => {
            handlers::sys_directory_canonicalize(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_REMOVE_IF => {
            handlers::sys_directory_remove_if(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW => {
            handlers::sys_directory_open_directory_nofollow(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_SYNC => {
            handlers::sys_file_sync(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_LOCK => {
            handlers::sys_file_lock(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_UNLOCK => {
            handlers::sys_file_unlock(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE_WITH_OPTIONS => {
            handlers::sys_directory_open_file_with_options(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_CLOCK_GET_REALTIME => {
            handlers::sys_clock_get_realtime(invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_REMOVE => {
            handlers::sys_directory_remove(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_WAIT_SET_CREATE => {
            handlers::sys_wait_set_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_WAIT_SET_ADD => {
            handlers::sys_wait_set_add(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_WAIT_SET_REARM => {
            handlers::sys_wait_set_rearm(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_WAIT_SET_REMOVE => {
            handlers::sys_wait_set_remove(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_WAIT_SET_WAIT => {
            handlers::sys_wait_set_wait(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_GET_CURRENT_ID => {
            handlers::sys_process_get_current_id(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_READ_AT => {
            handlers::sys_file_read_at(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_GET_INFO => {
            handlers::sys_file_get_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DIRECTORY_GET_INFO => {
            handlers::sys_directory_get_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_CREATE => {
            handlers::sys_virtual_machine_creation_lease_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATE => {
            handlers::sys_virtual_machine_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_MEMORY => {
            handlers::sys_pending_virtual_machine_set_memory(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP => {
            handlers::sys_pending_virtual_machine_set_bootstrap(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL => {
            handlers::sys_pending_virtual_machine_set_virtual_serial(
                services,
                invocation.arguments(),
            )
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_CREATE => {
            handlers::sys_virtual_serial_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_REGISTER_OUTPUT => {
            handlers::sys_virtual_serial_register_output(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT => {
            handlers::sys_virtual_serial_acknowledge_output(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE => {
            handlers::sys_virtual_serial_write(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SEAL => {
            handlers::sys_pending_virtual_machine_seal(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_INSTALL => {
            handlers::sys_pending_virtual_machine_install(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ABORT => {
            handlers::sys_pending_virtual_machine_abort(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REQUEST_STOP => {
            handlers::sys_virtual_machine_request_stop(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO => {
            handlers::sys_virtual_machine_creation_lease_get_platform_info(
                services,
                invocation.arguments(),
            )
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_POWER_REQUEST => {
            handlers::sys_virtual_machine_get_power_request(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST => {
            handlers::sys_virtual_machine_complete_power_request(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_OPEN_VCPU => {
            handlers::sys_virtual_machine_open_vcpu(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_INFO => {
            handlers::sys_virtual_machine_get_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REGISTER_MMIO => {
            handlers::sys_virtual_machine_register_mmio(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_MMIO_REQUEST => {
            handlers::sys_virtual_cpu_get_mmio_request(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_COMPLETE_MMIO => {
            handlers::sys_virtual_cpu_complete_mmio(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_MEMORY_CREATE => {
            handlers::sys_guest_memory_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_MAP_MEMORY => {
            handlers::sys_pending_virtual_machine_map_memory(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_FIRMWARE_READ => {
            handlers::sys_device_firmware_read(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_CLAIM_BUNDLE => {
            handlers::sys_device_claim_bundle(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_MMIO => {
            handlers::sys_device_mmio(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_IRQ_PENDING => {
            handlers::sys_device_irq_pending(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_IRQ_COMPLETE => {
            handlers::sys_device_irq_complete(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_PROFILE_INFO => {
            handlers::sys_device_profile_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_RESOURCE_INFO => {
            handlers::sys_device_resource_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_CLAIM_MATCHING => {
            handlers::sys_device_claim_matching(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_DEVICE_CLAIM => {
            handlers::sys_device_claim(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PHYSICAL_DEVICE_INFO => {
            handlers::sys_physical_device_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMO_GET_DMA_EXTENT => {
            handlers::sys_vmo_get_dma_extent(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE => {
            handlers::sys_pending_virtual_machine_assign_device(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_MAILBOX_CREATE => {
            handlers::sys_guest_mailbox_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_MAILBOX_SEND => {
            handlers::sys_guest_mailbox_send(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_MAILBOX_RECEIVE => {
            handlers::sys_guest_mailbox_receive(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_NOTIFICATION_CREATE => {
            handlers::sys_guest_notification_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_MAPPING_CREATE => {
            handlers::sys_guest_mapping_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_MAPPING_RELEASE => {
            handlers::sys_guest_mapping_release(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_NATIVE_BLOCK_CREATE => {
            handlers::sys_native_block_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_NATIVE_BLOCK_ACTIVATE => {
            handlers::sys_native_block_activate(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_NATIVE_BLOCK_MOUNT => {
            handlers::sys_native_block_mount(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_GUEST_NOTIFICATION_CONTROL => {
            handlers::sys_guest_notification_control(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMO_CREATE_CONTIGUOUS => {
            handlers::sys_vmo_create_contiguous(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_INFO => {
            handlers::sys_virtual_cpu_get_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_SET_AFFINITY => {
            handlers::sys_virtual_cpu_set_affinity(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VIRTUAL_CPU_START => {
            handlers::sys_virtual_cpu_start(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_RESOURCE_DOMAIN_CREATE => {
            handlers::sys_resource_domain_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_TASK_GROUP_CREATE => {
            handlers::sys_task_group_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMO_CREATE => {
            handlers::sys_vmo_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO => {
            handlers::sys_file_create_executable_vmo(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMO_READ => handlers::sys_vmo_read(services, invocation.arguments()),
        abi::HYPER_NATIVE_SYS_VMO_WRITE => {
            handlers::sys_vmo_write(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMAR_ALLOCATE => {
            handlers::sys_vmar_allocate(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMAR_MAP => handlers::sys_vmar_map(services, invocation.arguments()),
        abi::HYPER_NATIVE_SYS_VMAR_PROTECT => {
            handlers::sys_vmar_protect(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMAR_UNMAP => {
            handlers::sys_vmar_unmap(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_VMAR_DESTROY => {
            handlers::sys_vmar_destroy(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE => {
            handlers::sys_process_builder_create(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME => {
            handlers::sys_process_builder_set_name(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT => {
            handlers::sys_process_builder_add_argument(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT => {
            handlers::sys_process_builder_add_environment(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY => {
            handlers::sys_process_builder_set_affinity(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE => {
            handlers::sys_process_builder_add_handle(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL => {
            handlers::sys_process_builder_seal(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_START => {
            handlers::sys_process_builder_start(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT => {
            handlers::sys_process_builder_abort(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP => {
            handlers::sys_process_request_stop(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_PROCESS_GET_INFO => {
            handlers::sys_process_get_info(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES => {
            handlers::sys_task_inspector_scan_processes(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS => {
            handlers::sys_task_inspector_scan_threads(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS => {
            handlers::sys_task_inspector_derive_process(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_READ_DETAILS => {
            handlers::sys_object_inspector_read_details(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS => {
            handlers::sys_object_inspector_scan_objects(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES => {
            handlers::sys_object_inspector_scan_handles(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS => {
            handlers::sys_object_inspector_derive_process(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP => {
            handlers::sys_task_inspector_derive_task_group(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP => {
            handlers::sys_object_inspector_derive_task_group(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN => {
            handlers::sys_task_inspector_derive_resource_domain(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN => {
            handlers::sys_object_inspector_derive_resource_domain(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_MEMORY_INSPECTOR_READ => {
            handlers::sys_memory_inspector_read(services, invocation.arguments())
        }
        abi::HYPER_NATIVE_SYS_CPU_INSPECTOR_READ => {
            handlers::sys_cpu_inspector_read(services, invocation.arguments())
        }
        _ => DeferredAction::Return(handlers::sys_not_supported()),
    }
}
