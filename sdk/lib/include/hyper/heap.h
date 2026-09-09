/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#ifndef HYPER_HEAP_H
#define HYPER_HEAP_H

#include <hyper/startup.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Native address-layout contract: after the loader's library range and below
 * the user stack. Reservation consumes virtual space, not physical pages. */
#ifndef HYPER_HEAP_BASE
#define HYPER_HEAP_BASE UINT64_C(0xe0000000)
#endif
#define HYPER_HEAP_SIZE UINT64_C(0x10000000)

/* Idempotent loader/CRT setup; borrows ROOT_VMAR only during initialization. */
hyper_native_status_t hyper_heap_initialize(const hyper_startup_t *startup);

/* Runtime allocation interface. Alignment must be a nonzero power of two.
 * Failure returns NULL; realloc failure preserves the original allocation.
 * Size zero allocates a freeable minimum block; realloc(ptr, 0, align) frees.
 * Pointers must be live allocations from this process's libhyper heap.
 * Calls are thread-safe, but not reentrant from asynchronous handlers. */
void *hyper_alloc(size_t size, size_t alignment);
void hyper_free(void *pointer);
void *hyper_realloc(void *pointer, size_t size, size_t alignment);

#ifdef __cplusplus
}
#endif

#endif /* HYPER_HEAP_H */
