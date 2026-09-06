/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/syscall.h>

static hyper_call_result_t call0(uint64_t number)
{
    return hyper_native_call6(number, 0, 0, 0, 0, 0, 0);
}

hyper_call_result_t hyper_abi_query(void)
{
    return call0(HYPER_NATIVE_SYS_ABI_QUERY);
}

hyper_native_status_t hyper_handle_close(hyper_native_handle_t handle)
{
    return hyper_native_call6(HYPER_NATIVE_SYS_HANDLE_CLOSE, handle, 0, 0, 0, 0, 0).status;
}

hyper_call_result_t hyper_handle_duplicate(
    hyper_native_handle_t source,
    uint64_t requested_rights)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_HANDLE_DUPLICATE, source, requested_rights, 0, 0, 0, 0);
}

hyper_call_result_t hyper_handle_replace(
    hyper_native_handle_t source,
    uint64_t requested_rights)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_HANDLE_REPLACE, source, requested_rights, 0, 0, 0, 0);
}

hyper_native_status_t hyper_handle_get_info(
    hyper_native_handle_t handle,
    hyper_native_handle_info_t *info)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_HANDLE_GET_INFO,
        handle,
        (uintptr_t)info,
        sizeof(*info),
        0,
        0,
        0).status;
}

hyper_call_result_t hyper_object_wait_one(
    hyper_native_handle_t object,
    uint64_t signals,
    uint64_t deadline)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_OBJECT_WAIT_ONE, object, signals, deadline, 0, 0, 0);
}

hyper_call_result_t hyper_object_wait_many(
    const hyper_native_object_wait_item_t *items,
    size_t item_count,
    uint64_t deadline)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_OBJECT_WAIT_MANY,
        (uintptr_t)items,
        item_count,
        deadline,
        0,
        0,
        0);
}

hyper_native_status_t hyper_byte_channel_write(
    hyper_native_handle_t endpoint,
    const void *bytes,
    size_t byte_count)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE,
        endpoint,
        0,
        (uintptr_t)bytes,
        byte_count,
        0,
        0).status;
}

hyper_call_result_t hyper_byte_channel_read(
    hyper_native_handle_t endpoint,
    void *bytes,
    size_t byte_capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_BYTE_CHANNEL_READ,
        endpoint,
        0,
        (uintptr_t)bytes,
        byte_capacity,
        0,
        0);
}

hyper_call_result_t hyper_capability_channel_create(void)
{
    return call0(HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE);
}

hyper_native_status_t hyper_capability_channel_try_send(
    hyper_native_handle_t endpoint,
    const void *bytes,
    size_t byte_count,
    const hyper_native_capability_disposition_t *dispositions,
    size_t disposition_count)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND,
        endpoint,
        0,
        (uintptr_t)bytes,
        byte_count,
        (uintptr_t)dispositions,
        disposition_count).status;
}

hyper_call_result_t hyper_capability_channel_receive(
    hyper_native_handle_t endpoint,
    uint64_t deadline,
    void *bytes,
    size_t byte_capacity,
    hyper_native_capability_receive_slot_t *slots,
    size_t slot_count)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE,
        endpoint,
        deadline,
        (uintptr_t)bytes,
        byte_capacity,
        (uintptr_t)slots,
        slot_count);
}

hyper_call_result_t hyper_console_read(
    hyper_native_handle_t console,
    void *bytes,
    size_t capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_CONSOLE_READ,
        console,
        0,
        (uintptr_t)bytes,
        capacity,
        0,
        0);
}

hyper_call_result_t hyper_console_write(
    hyper_native_handle_t console,
    const void *bytes,
    size_t count)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_CONSOLE_WRITE,
        console,
        0,
        (uintptr_t)bytes,
        count,
        0,
        0);
}

hyper_call_result_t hyper_bootfs_open(
    hyper_native_handle_t boot_fs,
    const void *path,
    size_t path_size,
    uint64_t requested_rights)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_BOOTFS_OPEN,
        boot_fs,
        (uintptr_t)path,
        path_size,
        requested_rights,
        0,
        0);
}

hyper_call_result_t hyper_boot_file_read(
    hyper_native_handle_t file,
    uint64_t offset,
    void *output,
    size_t output_capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_BOOT_FILE_READ,
        file,
        0,
        offset,
        (uintptr_t)output,
        output_capacity,
        0);
}

hyper_call_result_t hyper_process_builder_create(
    hyper_native_handle_t factory,
    hyper_native_handle_t group,
    hyper_native_handle_t domain,
    hyper_native_handle_t executable)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE,
        factory,
        group,
        domain,
        executable,
        0,
        0);
}

hyper_native_status_t hyper_process_builder_set_name(
    hyper_native_handle_t builder,
    const void *name,
    size_t name_size)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME,
        builder,
        (uintptr_t)name,
        name_size,
        0,
        0,
        0).status;
}

hyper_native_status_t hyper_process_builder_add_argument(
    hyper_native_handle_t builder,
    const void *argument,
    size_t argument_size)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT,
        builder,
        (uintptr_t)argument,
        argument_size,
        0,
        0,
        0).status;
}

hyper_native_status_t hyper_process_builder_add_environment(
    hyper_native_handle_t builder,
    const void *environment,
    size_t environment_size)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT,
        builder,
        (uintptr_t)environment,
        environment_size,
        0,
        0,
        0).status;
}

hyper_native_status_t hyper_process_builder_set_affinity(
    hyper_native_handle_t builder,
    const uint64_t *words,
    size_t word_count)
{
    uint8_t encoded[HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS * sizeof(uint64_t)] = {0};
    size_t word_index;

    if (word_count > HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS ||
        (word_count != 0 && words == NULL)) {
        return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    }
    for (word_index = 0; word_index < word_count; ++word_index) {
        size_t byte_index;

        for (byte_index = 0; byte_index < sizeof(uint64_t); ++byte_index) {
            encoded[word_index * sizeof(uint64_t) + byte_index] =
                (uint8_t)(words[word_index] >> (byte_index * 8));
        }
    }
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY,
        builder,
        (uintptr_t)encoded,
        word_count,
        0,
        0,
        0).status;
}

hyper_native_status_t hyper_process_builder_add_handle(
    hyper_native_handle_t builder,
    hyper_native_handle_t source,
    uint32_t purpose,
    uint32_t expected_kind,
    uint64_t rights,
    uint32_t operation)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE,
        builder,
        source,
        purpose,
        expected_kind,
        rights,
        operation).status;
}

hyper_native_status_t hyper_process_builder_seal(hyper_native_handle_t builder)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL, builder, 0, 0, 0, 0, 0).status;
}

hyper_call_result_t hyper_process_builder_start(hyper_native_handle_t builder)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_START, builder, 0, 0, 0, 0, 0);
}

hyper_native_status_t hyper_process_builder_abort(hyper_native_handle_t builder)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT, builder, 0, 0, 0, 0, 0).status;
}

hyper_native_status_t hyper_process_request_stop(hyper_native_handle_t process)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP, process, 0, 0, 0, 0, 0).status;
}

hyper_native_status_t hyper_process_get_info(
    hyper_native_handle_t process,
    hyper_native_process_info_t *info)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_PROCESS_GET_INFO,
        process,
        (uintptr_t)info,
        sizeof(*info),
        0,
        0,
        0).status;
}

hyper_call_result_t hyper_task_inspector_scan_processes(
    hyper_native_handle_t inspector,
    uint64_t cursor,
    hyper_native_task_process_t *records,
    size_t capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES,
        inspector,
        cursor,
        (uintptr_t)records,
        capacity,
        0,
        0);
}

hyper_call_result_t hyper_task_inspector_scan_threads(
    hyper_native_handle_t inspector,
    uint64_t cursor,
    hyper_native_task_thread_t *records,
    size_t capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS,
        inspector,
        cursor,
        (uintptr_t)records,
        capacity,
        0,
        0);
}

hyper_call_result_t hyper_object_inspector_scan_objects(
    hyper_native_handle_t inspector,
    uint64_t cursor,
    hyper_native_object_inspection_t *records,
    size_t capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS,
        inspector,
        cursor,
        (uintptr_t)records,
        capacity,
        0,
        0);
}

hyper_call_result_t hyper_object_inspector_scan_handles(
    hyper_native_handle_t inspector,
    uint64_t process_koid,
    uint64_t cursor,
    hyper_native_handle_inspection_t *records,
    size_t capacity)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES,
        inspector,
        process_koid,
        cursor,
        (uintptr_t)records,
        capacity,
        0);
}

static hyper_call_result_t inspector_derive(
    uint64_t number,
    hyper_native_handle_t inspector,
    hyper_native_handle_t scope)
{
    return hyper_native_call6(number, inspector, scope, 0, 0, 0, 0);
}

hyper_call_result_t hyper_task_inspector_derive_process(
    hyper_native_handle_t inspector,
    hyper_native_handle_t process)
{
    return inspector_derive(HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS, inspector, process);
}

hyper_call_result_t hyper_task_inspector_derive_task_group(
    hyper_native_handle_t inspector,
    hyper_native_handle_t task_group)
{
    return inspector_derive(
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP, inspector, task_group);
}

hyper_call_result_t hyper_task_inspector_derive_resource_domain(
    hyper_native_handle_t inspector,
    hyper_native_handle_t resource_domain)
{
    return inspector_derive(
        HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN, inspector, resource_domain);
}

hyper_call_result_t hyper_object_inspector_derive_process(
    hyper_native_handle_t inspector,
    hyper_native_handle_t process)
{
    return inspector_derive(HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS, inspector, process);
}

hyper_call_result_t hyper_object_inspector_derive_task_group(
    hyper_native_handle_t inspector,
    hyper_native_handle_t task_group)
{
    return inspector_derive(
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP, inspector, task_group);
}

hyper_call_result_t hyper_object_inspector_derive_resource_domain(
    hyper_native_handle_t inspector,
    hyper_native_handle_t resource_domain)
{
    return inspector_derive(
        HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN,
        inspector,
        resource_domain);
}

hyper_native_status_t hyper_thread_yield(void)
{
    return call0(HYPER_NATIVE_SYS_THREAD_YIELD).status;
}

_Noreturn void hyper_thread_exit(int64_t status)
{
    (void)hyper_native_call6(HYPER_NATIVE_SYS_THREAD_EXIT, (uint64_t)status, 0, 0, 0, 0, 0);
    __builtin_trap();
}

_Noreturn void hyper_process_exit(int64_t status)
{
    (void)hyper_native_call6(HYPER_NATIVE_SYS_PROCESS_EXIT, (uint64_t)status, 0, 0, 0, 0, 0);
    __builtin_trap();
}
