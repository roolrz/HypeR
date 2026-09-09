/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/heap.h>

/* Invoked through the relocated libhyper image, not the interpreter's static
 * copy of runtime primitives, so constructors and main share one heap. */
hyper_native_status_t hyper_runtime_initialize(const uintptr_t *initial_stack)
{
    hyper_startup_t startup;
    hyper_native_status_t status = hyper_startup_parse(initial_stack, &startup);
    if (status != HYPER_NATIVE_STATUS_OK) {
        return status;
    }
    return hyper_heap_initialize(&startup);
}
