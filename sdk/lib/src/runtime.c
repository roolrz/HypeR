/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/heap.h>
#include <hyper/thread.h>
#include <stdatomic.h>

static hyper_startup_t process_startup;
static atomic_bool initialized;

const hyper_startup_t *hyper_runtime_startup(void)
{
    return atomic_load_explicit(&initialized, memory_order_acquire) ? &process_startup : NULL;
}

/* Invoked through the relocated libhyper image, not the interpreter's static
 * copy of runtime primitives, so constructors and main share one heap. */
hyper_native_status_t hyper_runtime_initialize(const uintptr_t *initial_stack)
{
    if (atomic_load_explicit(&initialized, memory_order_acquire))
        return HYPER_NATIVE_STATUS_OK;
    hyper_startup_t startup;
    hyper_native_status_t status = hyper_startup_parse(initial_stack, &startup);
    if (status != HYPER_NATIVE_STATUS_OK) {
        return status;
    }
    status = hyper_heap_initialize(&startup);
    if (status != HYPER_NATIVE_STATUS_OK) return status;
    status = hyper_runtime_thread_attach();
    if (status != HYPER_NATIVE_STATUS_OK) return status;
    /* Initial loader/CRT initialization runs before secondary threads exist. */
    process_startup = startup;
    atomic_store_explicit(&initialized, 1, memory_order_release);
    return HYPER_NATIVE_STATUS_OK;
}
