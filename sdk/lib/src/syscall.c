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

hyper_call_result_t hyper_object_wait_one(
    hyper_native_handle_t object,
    uint64_t signals,
    uint64_t deadline)
{
    return hyper_native_call6(
        HYPER_NATIVE_SYS_OBJECT_WAIT_ONE, object, signals, deadline, 0, 0, 0);
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
