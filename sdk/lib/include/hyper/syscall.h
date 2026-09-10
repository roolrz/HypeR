/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#ifndef HYPER_SYSCALL_H
#define HYPER_SYSCALL_H

#include <hyper/native.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct hyper_call_result {
    hyper_native_status_t status;
    uint64_t value0;
    uint64_t value1;
} hyper_call_result_t;

hyper_call_result_t hyper_native_call6(
    uint64_t number,
    uint64_t argument0,
    uint64_t argument1,
    uint64_t argument2,
    uint64_t argument3,
    uint64_t argument4,
    uint64_t argument5);

hyper_call_result_t hyper_abi_query(void);
/* Returns absolute nanoseconds from the kernel monotonic clock domain. */
hyper_call_result_t hyper_clock_get_monotonic(void);
hyper_native_status_t hyper_handle_close(hyper_native_handle_t handle);
hyper_call_result_t hyper_handle_duplicate(
    hyper_native_handle_t source,
    uint64_t requested_rights);
hyper_call_result_t hyper_handle_replace(
    hyper_native_handle_t source,
    uint64_t requested_rights);
hyper_call_result_t hyper_handle_get_info(
    hyper_native_handle_t handle,
    hyper_native_handle_info_t *info);
hyper_call_result_t hyper_object_get_basic_info(
    hyper_native_handle_t handle,
    hyper_native_object_basic_info_t *info);
hyper_call_result_t hyper_object_wait_one(
    hyper_native_handle_t object,
    uint64_t signals,
    uint64_t deadline);
hyper_call_result_t hyper_object_wait_many(
    const hyper_native_object_wait_item_t *items,
    size_t item_count,
    uint64_t deadline);
hyper_native_status_t hyper_byte_channel_write(
    hyper_native_handle_t endpoint,
    const void *bytes,
    size_t byte_count);
hyper_call_result_t hyper_byte_channel_read(
    hyper_native_handle_t endpoint,
    void *bytes,
    size_t byte_capacity);
hyper_call_result_t hyper_capability_channel_create(void);
hyper_native_status_t hyper_capability_channel_try_send(
    hyper_native_handle_t endpoint,
    const void *bytes,
    size_t byte_count,
    const hyper_native_capability_disposition_t *dispositions,
    size_t disposition_count);
hyper_call_result_t hyper_capability_channel_receive(
    hyper_native_handle_t endpoint,
    uint64_t deadline,
    void *bytes,
    size_t byte_capacity,
    hyper_native_capability_receive_slot_t *slots,
    size_t slot_count);
hyper_call_result_t hyper_console_read(
    hyper_native_handle_t console,
    void *bytes,
    size_t capacity);
hyper_call_result_t hyper_console_write(
    hyper_native_handle_t console,
    const void *bytes,
    size_t count);
hyper_call_result_t hyper_directory_open_file(
    hyper_native_handle_t directory,
    const void *path,
    size_t path_size,
    uint64_t requested_rights);
hyper_call_result_t hyper_directory_open_directory(
    hyper_native_handle_t directory,
    const void *path,
    size_t path_size,
    uint64_t requested_rights);
hyper_call_result_t hyper_directory_read(
    hyper_native_handle_t directory,
    uint64_t cookie,
    hyper_native_directory_entry_t *records,
    size_t capacity);
hyper_call_result_t hyper_directory_get_info(
    hyper_native_handle_t directory,
    hyper_native_directory_info_t *info);
hyper_call_result_t hyper_file_read_at(
    hyper_native_handle_t file,
    uint64_t offset,
    void *output,
    size_t output_capacity);
hyper_call_result_t hyper_file_get_info(
    hyper_native_handle_t file,
    hyper_native_file_info_t *info);
hyper_call_result_t hyper_resource_domain_create(
    hyper_native_handle_t parent,
    const hyper_native_resource_limits_t *limits);
hyper_call_result_t hyper_task_group_create(
    hyper_native_handle_t factory,
    hyper_native_handle_t resource_domain);
hyper_call_result_t hyper_virtual_machine_creation_lease_create(
    hyper_native_handle_t authority,
    hyper_native_handle_t resource_domain);
hyper_call_result_t hyper_virtual_machine_create(
    hyper_native_handle_t lease,
    const hyper_native_virtual_machine_configuration_t *configuration);
hyper_native_status_t hyper_pending_virtual_machine_set_memory(
    hyper_native_handle_t pending,
    hyper_native_handle_t vmo);
/* Consumes virtual_serial only when the binding succeeds. */
hyper_native_status_t hyper_pending_virtual_machine_set_virtual_serial(
    hyper_native_handle_t pending,
    hyper_native_handle_t virtual_serial);
hyper_call_result_t hyper_virtual_serial_create(void);
hyper_native_status_t hyper_virtual_serial_register_output(hyper_native_handle_t serial, hyper_native_handle_t buffer);
hyper_call_result_t hyper_virtual_serial_write(
    hyper_native_handle_t virtual_serial,
    const void *bytes,
    size_t byte_count);
hyper_native_status_t hyper_pending_virtual_machine_set_bootstrap(
    hyper_native_handle_t pending,
    const hyper_native_virtual_cpu_bootstrap_t *bootstrap);
hyper_native_status_t hyper_pending_virtual_machine_seal(hyper_native_handle_t pending);
hyper_call_result_t hyper_pending_virtual_machine_install(hyper_native_handle_t pending);
hyper_native_status_t hyper_virtual_cpu_start(hyper_native_handle_t virtual_cpu);
hyper_native_status_t hyper_pending_virtual_machine_abort(hyper_native_handle_t pending);
hyper_native_status_t hyper_virtual_machine_request_stop(hyper_native_handle_t machine);
hyper_call_result_t hyper_virtual_machine_get_info(
    hyper_native_handle_t machine,
    hyper_native_virtual_machine_info_t *info);
hyper_call_result_t hyper_virtual_cpu_get_info(
    hyper_native_handle_t vcpu,
    hyper_native_virtual_cpu_info_t *info);
hyper_call_result_t hyper_vmo_create(uint64_t size);
hyper_call_result_t hyper_file_create_executable_vmo(hyper_native_handle_t file);
hyper_native_status_t hyper_vmo_read(
    hyper_native_handle_t vmo,
    uint64_t offset,
    void *output,
    size_t byte_count);
hyper_native_status_t hyper_vmo_write(
    hyper_native_handle_t vmo,
    uint64_t offset,
    const void *input,
    size_t byte_count);
hyper_call_result_t hyper_vmar_allocate(
    hyper_native_handle_t parent,
    uintptr_t address,
    size_t size);
hyper_native_status_t hyper_vmar_map(
    hyper_native_handle_t vmar,
    hyper_native_handle_t vmo,
    uint64_t vmo_offset,
    uintptr_t address,
    size_t size,
    uint32_t permissions);
hyper_native_status_t hyper_vmar_protect(
    hyper_native_handle_t vmar,
    uintptr_t address,
    size_t size,
    uint32_t permissions);
hyper_native_status_t hyper_vmar_unmap(
    hyper_native_handle_t vmar,
    uintptr_t address,
    size_t size);
hyper_native_status_t hyper_vmar_destroy(hyper_native_handle_t vmar);
hyper_call_result_t hyper_process_builder_create(
    hyper_native_handle_t factory,
    hyper_native_handle_t group,
    hyper_native_handle_t domain,
    hyper_native_handle_t executable);
hyper_native_status_t hyper_process_builder_set_name(
    hyper_native_handle_t builder,
    const void *name,
    size_t name_size);
hyper_native_status_t hyper_process_builder_add_argument(
    hyper_native_handle_t builder,
    const void *argument,
    size_t argument_size);
hyper_native_status_t hyper_process_builder_add_environment(
    hyper_native_handle_t builder,
    const void *environment,
    size_t environment_size);
hyper_native_status_t hyper_process_builder_set_affinity(
    hyper_native_handle_t builder,
    const uint64_t *words,
    size_t word_count);
hyper_native_status_t hyper_process_builder_add_handle(
    hyper_native_handle_t builder,
    hyper_native_handle_t source,
    uint32_t purpose,
    uint32_t expected_kind,
    uint64_t rights,
    uint32_t operation);
hyper_native_status_t hyper_process_builder_seal(hyper_native_handle_t builder);
hyper_call_result_t hyper_process_builder_start(hyper_native_handle_t builder);
hyper_native_status_t hyper_process_builder_abort(hyper_native_handle_t builder);
hyper_native_status_t hyper_process_request_stop(hyper_native_handle_t process);
hyper_call_result_t hyper_process_get_info(
    hyper_native_handle_t process,
    hyper_native_process_info_t *info);
hyper_call_result_t hyper_task_inspector_scan_processes(
    hyper_native_handle_t inspector,
    uint64_t cursor,
    hyper_native_task_process_t *records,
    size_t capacity);
hyper_call_result_t hyper_task_inspector_scan_threads(
    hyper_native_handle_t inspector,
    uint64_t cursor,
    hyper_native_task_thread_t *records,
    size_t capacity);
hyper_call_result_t hyper_object_inspector_scan_objects(
    hyper_native_handle_t inspector,
    uint64_t cursor,
    hyper_native_object_inspection_t *records,
    size_t capacity);
hyper_call_result_t hyper_object_inspector_scan_handles(
    hyper_native_handle_t inspector,
    uint64_t process_koid,
    uint64_t cursor,
    hyper_native_handle_inspection_t *records,
    size_t capacity);
hyper_call_result_t hyper_memory_inspector_read(
    hyper_native_handle_t inspector,
    hyper_native_memory_observation_t *observation);
hyper_call_result_t hyper_cpu_inspector_read(
    hyper_native_handle_t inspector,
    hyper_native_cpu_observation_t *observation);
hyper_call_result_t hyper_task_inspector_derive_process(
    hyper_native_handle_t inspector,
    hyper_native_handle_t process);
hyper_call_result_t hyper_task_inspector_derive_task_group(
    hyper_native_handle_t inspector,
    hyper_native_handle_t task_group);
hyper_call_result_t hyper_task_inspector_derive_resource_domain(
    hyper_native_handle_t inspector,
    hyper_native_handle_t resource_domain);
hyper_call_result_t hyper_object_inspector_derive_process(
    hyper_native_handle_t inspector,
    hyper_native_handle_t process);
hyper_call_result_t hyper_object_inspector_derive_task_group(
    hyper_native_handle_t inspector,
    hyper_native_handle_t task_group);
hyper_call_result_t hyper_object_inspector_derive_resource_domain(
    hyper_native_handle_t inspector,
    hyper_native_handle_t resource_domain);
hyper_call_result_t hyper_thread_create(uint64_t entry, uint64_t stack, uint64_t tls, uint64_t argument);
hyper_native_status_t hyper_thread_start(hyper_native_handle_t thread);
hyper_native_status_t hyper_thread_request_stop(hyper_native_handle_t thread);
hyper_native_status_t hyper_atomic_wait(const uint32_t *address, uint32_t expected, uint64_t deadline);
hyper_call_result_t hyper_atomic_wake(const uint32_t *address, uint32_t count);
hyper_native_status_t hyper_thread_sleep(uint64_t deadline);
hyper_native_status_t hyper_thread_yield(void);
_Noreturn void hyper_thread_exit(int64_t status);
_Noreturn void hyper_process_exit(int64_t status);

#ifdef __cplusplus
}
#endif

#endif /* HYPER_SYSCALL_H */
