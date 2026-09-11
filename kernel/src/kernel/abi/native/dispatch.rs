// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native immediate/deferred syscall routing.

use hyper::abi::native::{
    HYPER_NATIVE_SYS_ABI_QUERY, HYPER_NATIVE_SYS_ATOMIC_WAIT, HYPER_NATIVE_SYS_ATOMIC_WAKE,
    HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE, HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
    HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE, HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE,
    HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE, HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
    HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC, HYPER_NATIVE_SYS_CONSOLE_READ,
    HYPER_NATIVE_SYS_CONSOLE_WRITE, HYPER_NATIVE_SYS_CPU_INSPECTOR_READ,
    HYPER_NATIVE_SYS_DIRECTORY_CREATE_DIRECTORY, HYPER_NATIVE_SYS_DIRECTORY_CREATE_FILE,
    HYPER_NATIVE_SYS_DIRECTORY_GET_INFO, HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY,
    HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE, HYPER_NATIVE_SYS_DIRECTORY_READ,
    HYPER_NATIVE_SYS_DIRECTORY_REMOVE, HYPER_NATIVE_SYS_EVENT_CREATE,
    HYPER_NATIVE_SYS_EVENT_SIGNAL, HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO,
    HYPER_NATIVE_SYS_FILE_GET_INFO, HYPER_NATIVE_SYS_FILE_READ_AT, HYPER_NATIVE_SYS_FILE_RESIZE,
    HYPER_NATIVE_SYS_FILE_WRITE_AT, HYPER_NATIVE_SYS_HANDLE_CLOSE,
    HYPER_NATIVE_SYS_HANDLE_DUPLICATE, HYPER_NATIVE_SYS_HANDLE_GET_INFO,
    HYPER_NATIVE_SYS_HANDLE_REPLACE, HYPER_NATIVE_SYS_MEMORY_INSPECTOR_READ,
    HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO, HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP,
    HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES, HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS,
    HYPER_NATIVE_SYS_OBJECT_WAIT_MANY, HYPER_NATIVE_SYS_OBJECT_WAIT_ONE,
    HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ABORT,
    HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_INSTALL,
    HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SEAL,
    HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP,
    HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_MEMORY,
    HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT, HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT, HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE, HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY, HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME,
    HYPER_NATIVE_SYS_PROCESS_BUILDER_START, HYPER_NATIVE_SYS_PROCESS_EXIT,
    HYPER_NATIVE_SYS_PROCESS_GET_CURRENT_ID, HYPER_NATIVE_SYS_PROCESS_GET_INFO,
    HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP, HYPER_NATIVE_SYS_RESOURCE_DOMAIN_CREATE,
    HYPER_NATIVE_SYS_TASK_GROUP_CREATE, HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP,
    HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES, HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS,
    HYPER_NATIVE_SYS_THREAD_CREATE, HYPER_NATIVE_SYS_THREAD_EXIT,
    HYPER_NATIVE_SYS_THREAD_REQUEST_STOP, HYPER_NATIVE_SYS_THREAD_SLEEP,
    HYPER_NATIVE_SYS_THREAD_START, HYPER_NATIVE_SYS_THREAD_YIELD,
    HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_INFO, HYPER_NATIVE_SYS_VIRTUAL_CPU_START,
    HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATE,
    HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_CREATE,
    HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO,
    HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_INFO, HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REQUEST_STOP,
    HYPER_NATIVE_SYS_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT, HYPER_NATIVE_SYS_VIRTUAL_SERIAL_CREATE,
    HYPER_NATIVE_SYS_VIRTUAL_SERIAL_REGISTER_OUTPUT, HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE,
    HYPER_NATIVE_SYS_VMAR_ALLOCATE, HYPER_NATIVE_SYS_VMAR_DESTROY, HYPER_NATIVE_SYS_VMAR_MAP,
    HYPER_NATIVE_SYS_VMAR_PROTECT, HYPER_NATIVE_SYS_VMAR_UNMAP, HYPER_NATIVE_SYS_VMO_CREATE,
    HYPER_NATIVE_SYS_VMO_READ, HYPER_NATIVE_SYS_VMO_WRITE, HYPER_NATIVE_SYS_WAIT_SET_ADD,
    HYPER_NATIVE_SYS_WAIT_SET_CREATE, HYPER_NATIVE_SYS_WAIT_SET_REARM,
    HYPER_NATIVE_SYS_WAIT_SET_REMOVE, HYPER_NATIVE_SYS_WAIT_SET_WAIT, NativeInvocation,
    NativeResult,
};

use super::handlers::{
    sys_abi_query, sys_atomic_wait, sys_atomic_wake, sys_byte_channel_create,
    sys_byte_channel_read, sys_byte_channel_write, sys_capability_channel_create,
    sys_capability_channel_receive, sys_capability_channel_try_send, sys_clock_get_monotonic,
    sys_console_read, sys_console_write, sys_cpu_inspector_read, sys_directory_create_directory,
    sys_directory_create_file, sys_directory_get_info, sys_directory_open_directory,
    sys_directory_open_file, sys_directory_read, sys_directory_remove, sys_event_create,
    sys_event_signal, sys_file_create_executable_vmo, sys_file_get_info, sys_file_read_at,
    sys_file_resize, sys_file_write_at, sys_handle_close, sys_handle_duplicate,
    sys_handle_get_info, sys_handle_replace, sys_memory_inspector_read, sys_not_supported,
    sys_object_get_basic_info, sys_object_inspector_derive_process,
    sys_object_inspector_derive_resource_domain, sys_object_inspector_derive_task_group,
    sys_object_inspector_scan_handles, sys_object_inspector_scan_objects, sys_object_wait_many,
    sys_object_wait_one, sys_pending_virtual_machine_abort, sys_pending_virtual_machine_install,
    sys_pending_virtual_machine_seal, sys_pending_virtual_machine_set_bootstrap,
    sys_pending_virtual_machine_set_memory, sys_pending_virtual_machine_set_virtual_serial,
    sys_process_builder_abort, sys_process_builder_add_argument,
    sys_process_builder_add_environment, sys_process_builder_add_handle,
    sys_process_builder_create, sys_process_builder_seal, sys_process_builder_set_affinity,
    sys_process_builder_set_name, sys_process_builder_start, sys_process_exit,
    sys_process_get_current_id, sys_process_get_info, sys_process_request_stop,
    sys_resource_domain_create, sys_task_group_create, sys_task_inspector_derive_process,
    sys_task_inspector_derive_resource_domain, sys_task_inspector_derive_task_group,
    sys_task_inspector_scan_processes, sys_task_inspector_scan_threads, sys_thread_create,
    sys_thread_exit, sys_thread_request_stop, sys_thread_sleep, sys_thread_start, sys_thread_yield,
    sys_virtual_cpu_get_info, sys_virtual_cpu_start, sys_virtual_machine_create,
    sys_virtual_machine_creation_lease_create,
    sys_virtual_machine_creation_lease_get_platform_info, sys_virtual_machine_get_info,
    sys_virtual_machine_request_stop, sys_virtual_serial_acknowledge_output,
    sys_virtual_serial_create, sys_virtual_serial_register_output, sys_virtual_serial_write,
    sys_vmar_allocate, sys_vmar_destroy, sys_vmar_map, sys_vmar_protect, sys_vmar_unmap,
    sys_vmo_create, sys_vmo_read, sys_vmo_write, sys_wait_set_add, sys_wait_set_create,
    sys_wait_set_rearm, sys_wait_set_remove, sys_wait_set_wait,
};
use super::services::{DeferredAction, DeferredServices, ImmediateServices};

/// Defines the complete audited masked-entry route set once.
///
/// Both membership and direct dispatch are generated from this list, so a new
/// immediate syscall cannot accidentally be admitted without a matching leaf
/// or routed through the immediate dispatcher without masked-entry admission.
macro_rules! define_immediate_routes {
    ($( $number:path => $handler:ident ),+ $(,)?) => {
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
                _ => sys_not_supported(),
            }
        }
    };
}

define_immediate_routes! {
    HYPER_NATIVE_SYS_ABI_QUERY => sys_abi_query,
    HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC => sys_clock_get_monotonic,
    HYPER_NATIVE_SYS_HANDLE_CLOSE => sys_handle_close,
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
        HYPER_NATIVE_SYS_HANDLE_GET_INFO => {
            DeferredAction::Return(sys_handle_get_info(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO => {
            DeferredAction::Return(sys_object_get_basic_info(services, invocation.arguments()))
        }
        hyper::abi::native::HYPER_NATIVE_SYS_VMO_CREATE_SNAPSHOT => {
            super::handlers::sys_vmo_create_snapshot(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_FILE_CREATE_SNAPSHOT => {
            super::handlers::sys_file_create_snapshot(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_VMAR_MAP_PRIVATE => {
            super::handlers::sys_vmar_map_private(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_HANDLE_DUPLICATE => {
            DeferredAction::Return(sys_handle_duplicate(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_HANDLE_REPLACE => {
            DeferredAction::Return(sys_handle_replace(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_EVENT_CREATE => {
            DeferredAction::Return(sys_event_create(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE => {
            DeferredAction::Return(sys_byte_channel_create(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE => DeferredAction::Return(
            sys_capability_channel_create(services, invocation.arguments()),
        ),
        HYPER_NATIVE_SYS_THREAD_CREATE => {
            DeferredAction::Return(sys_thread_create(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_THREAD_START => {
            DeferredAction::Return(sys_thread_start(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_THREAD_REQUEST_STOP => {
            DeferredAction::Return(sys_thread_request_stop(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_ATOMIC_WAIT => {
            DeferredAction::Return(sys_atomic_wait(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_ATOMIC_WAKE => {
            DeferredAction::Return(sys_atomic_wake(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_THREAD_SLEEP => {
            DeferredAction::Return(sys_thread_sleep(services, invocation.arguments()))
        }
        HYPER_NATIVE_SYS_THREAD_YIELD => sys_thread_yield(),
        HYPER_NATIVE_SYS_THREAD_EXIT => sys_thread_exit(invocation.arguments()),
        HYPER_NATIVE_SYS_PROCESS_EXIT => sys_process_exit(invocation.arguments()),
        HYPER_NATIVE_SYS_EVENT_SIGNAL => sys_event_signal(services, invocation.arguments()),
        HYPER_NATIVE_SYS_OBJECT_WAIT_ONE => sys_object_wait_one(services, invocation.arguments()),
        HYPER_NATIVE_SYS_OBJECT_WAIT_MANY => sys_object_wait_many(services, invocation.arguments()),
        HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE => {
            sys_byte_channel_write(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_BYTE_CHANNEL_READ => {
            sys_byte_channel_read(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND => {
            sys_capability_channel_try_send(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE => {
            sys_capability_channel_receive(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CONSOLE_READ => sys_console_read(services, invocation.arguments()),
        HYPER_NATIVE_SYS_CONSOLE_WRITE => sys_console_write(services, invocation.arguments()),
        HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE => {
            sys_directory_open_file(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY => {
            sys_directory_open_directory(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_DIRECTORY_READ => sys_directory_read(services, invocation.arguments()),
        HYPER_NATIVE_SYS_FILE_WRITE_AT => sys_file_write_at(services, invocation.arguments()),
        HYPER_NATIVE_SYS_FILE_RESIZE => sys_file_resize(services, invocation.arguments()),
        HYPER_NATIVE_SYS_DIRECTORY_CREATE_FILE => {
            sys_directory_create_file(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_DIRECTORY_CREATE_DIRECTORY => {
            sys_directory_create_directory(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_SCOPE_CREATE => {
            super::fs_handlers::sys_directory_scope_create(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_GET_METADATA => {
            super::fs_handlers::sys_directory_get_metadata(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_FILE_GET_METADATA => {
            super::fs_handlers::sys_file_get_metadata(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_GET_SELF_METADATA => {
            super::fs_handlers::sys_directory_get_self_metadata(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_SET_METADATA => {
            super::fs_handlers::sys_directory_set_metadata(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_FILE_SET_METADATA => {
            super::fs_handlers::sys_file_set_metadata(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_RENAME => {
            super::fs_handlers::sys_directory_rename(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_LINK => {
            super::fs_handlers::sys_directory_link(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_SYMLINK => {
            super::fs_handlers::sys_directory_symlink(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_READ_LINK => {
            super::fs_handlers::sys_directory_read_link(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_CANONICALIZE => {
            super::fs_handlers::sys_directory_canonicalize(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_REMOVE_IF => {
            super::fs_handlers::sys_directory_remove_if(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW => {
            super::fs_handlers::sys_directory_open_directory_nofollow(
                services,
                invocation.arguments(),
            )
        }
        hyper::abi::native::HYPER_NATIVE_SYS_FILE_SYNC => {
            super::fs_handlers::sys_file_sync(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_FILE_LOCK => {
            super::fs_handlers::sys_file_lock(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_FILE_UNLOCK => {
            super::fs_handlers::sys_file_unlock(services, invocation.arguments())
        }
        hyper::abi::native::HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE_WITH_OPTIONS => {
            super::fs_handlers::sys_directory_open_file_with_options(
                services,
                invocation.arguments(),
            )
        }
        hyper::abi::native::HYPER_NATIVE_SYS_CLOCK_GET_REALTIME => {
            super::fs_handlers::sys_clock_get_realtime(invocation.arguments())
        }
        HYPER_NATIVE_SYS_DIRECTORY_REMOVE => sys_directory_remove(services, invocation.arguments()),
        HYPER_NATIVE_SYS_WAIT_SET_CREATE => sys_wait_set_create(services, invocation.arguments()),
        HYPER_NATIVE_SYS_WAIT_SET_ADD => sys_wait_set_add(services, invocation.arguments()),
        HYPER_NATIVE_SYS_WAIT_SET_REARM => sys_wait_set_rearm(services, invocation.arguments()),
        HYPER_NATIVE_SYS_WAIT_SET_REMOVE => sys_wait_set_remove(services, invocation.arguments()),
        HYPER_NATIVE_SYS_WAIT_SET_WAIT => sys_wait_set_wait(services, invocation.arguments()),
        HYPER_NATIVE_SYS_PROCESS_GET_CURRENT_ID => {
            sys_process_get_current_id(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_FILE_READ_AT => sys_file_read_at(services, invocation.arguments()),
        HYPER_NATIVE_SYS_FILE_GET_INFO => sys_file_get_info(services, invocation.arguments()),
        HYPER_NATIVE_SYS_DIRECTORY_GET_INFO => {
            sys_directory_get_info(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_CREATE => {
            sys_virtual_machine_creation_lease_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATE => {
            sys_virtual_machine_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_MEMORY => {
            sys_pending_virtual_machine_set_memory(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP => {
            sys_pending_virtual_machine_set_bootstrap(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL => {
            sys_pending_virtual_machine_set_virtual_serial(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_SERIAL_CREATE => {
            sys_virtual_serial_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_SERIAL_REGISTER_OUTPUT => {
            sys_virtual_serial_register_output(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT => {
            sys_virtual_serial_acknowledge_output(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE => {
            sys_virtual_serial_write(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SEAL => {
            sys_pending_virtual_machine_seal(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_INSTALL => {
            sys_pending_virtual_machine_install(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ABORT => {
            sys_pending_virtual_machine_abort(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REQUEST_STOP => {
            sys_virtual_machine_request_stop(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO => {
            sys_virtual_machine_creation_lease_get_platform_info(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_INFO => {
            sys_virtual_machine_get_info(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_INFO => {
            sys_virtual_cpu_get_info(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VIRTUAL_CPU_START => {
            sys_virtual_cpu_start(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_RESOURCE_DOMAIN_CREATE => {
            sys_resource_domain_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_GROUP_CREATE => {
            sys_task_group_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VMO_CREATE => sys_vmo_create(services, invocation.arguments()),
        HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO => {
            sys_file_create_executable_vmo(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_VMO_READ => sys_vmo_read(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMO_WRITE => sys_vmo_write(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_ALLOCATE => sys_vmar_allocate(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_MAP => sys_vmar_map(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_PROTECT => sys_vmar_protect(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_UNMAP => sys_vmar_unmap(services, invocation.arguments()),
        HYPER_NATIVE_SYS_VMAR_DESTROY => sys_vmar_destroy(services, invocation.arguments()),
        HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE => {
            sys_process_builder_create(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME => {
            sys_process_builder_set_name(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT => {
            sys_process_builder_add_argument(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT => {
            sys_process_builder_add_environment(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY => {
            sys_process_builder_set_affinity(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE => {
            sys_process_builder_add_handle(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL => {
            sys_process_builder_seal(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_START => {
            sys_process_builder_start(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT => {
            sys_process_builder_abort(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP => {
            sys_process_request_stop(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_PROCESS_GET_INFO => sys_process_get_info(services, invocation.arguments()),
        HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES => {
            sys_task_inspector_scan_processes(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS => {
            sys_task_inspector_scan_threads(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS => {
            sys_task_inspector_derive_process(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS => {
            sys_object_inspector_scan_objects(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES => {
            sys_object_inspector_scan_handles(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS => {
            sys_object_inspector_derive_process(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP => {
            sys_task_inspector_derive_task_group(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP => {
            sys_object_inspector_derive_task_group(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN => {
            sys_task_inspector_derive_resource_domain(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN => {
            sys_object_inspector_derive_resource_domain(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_MEMORY_INSPECTOR_READ => {
            sys_memory_inspector_read(services, invocation.arguments())
        }
        HYPER_NATIVE_SYS_CPU_INSPECTOR_READ => {
            sys_cpu_inspector_read(services, invocation.arguments())
        }
        _ => DeferredAction::Return(sys_not_supported()),
    }
}
