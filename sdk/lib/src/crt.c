/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/heap.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>

__attribute__((noreturn)) void __hyper_crt_start(
    const uintptr_t *initial_stack)
{
    hyper_startup_t startup;
    const hyper_native_status_t status = hyper_startup_parse(initial_stack, &startup);
    if (status != HYPER_NATIVE_STATUS_OK) {
        hyper_process_exit(status);
    }
    const hyper_native_status_t heap_status = hyper_heap_initialize(&startup);
    if (heap_status != HYPER_NATIVE_STATUS_OK) {
        hyper_process_exit(heap_status);
    }
    hyper_process_exit(hyper_main(&startup));
}
