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
hyper_native_status_t hyper_handle_close(hyper_native_handle_t handle);
hyper_call_result_t hyper_object_wait_one(
    hyper_native_handle_t object,
    uint64_t signals,
    uint64_t deadline);
hyper_call_result_t hyper_console_read(
    hyper_native_handle_t console,
    void *bytes,
    size_t capacity);
hyper_call_result_t hyper_console_write(
    hyper_native_handle_t console,
    const void *bytes,
    size_t count);
hyper_native_status_t hyper_thread_yield(void);
_Noreturn void hyper_thread_exit(int64_t status);
_Noreturn void hyper_process_exit(int64_t status);

#ifdef __cplusplus
}
#endif

#endif /* HYPER_SYSCALL_H */
