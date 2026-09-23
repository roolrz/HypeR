/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/stack.h>
#include <hyper/syscall.h>
#include "stack-internal.h"
#include <stdatomic.h>
#include <stdbool.h>
#include <stdlib.h>

#ifndef HYPER_STACK_POOL_BASE
#define HYPER_STACK_POOL_BASE UINT64_C(0xf0000000)
#endif
#define PAGE_SIZE ((size_t)HYPER_NATIVE_PAGE_SIZE)
#define DEFAULT_CAPACITY ((size_t)1024 * 1024)

struct hyper_stack {
    uintptr_t reservation;
    uintptr_t top;
    size_t capacity;
    size_t size;
    hyper_native_handle_t vmar;
    bool managed;
    bool retiring;
    bool garbage;
    struct hyper_stack *next;
};

/* Stack topology has one owner, separate from the general heap. Empty and
 * guard addresses remain reserved even while no backing page is installed.
 * Slow VMAR operations may block, so this is a userspace yield lock, never a
 * kernel spinlock or a lock held by the thread whose termination we await. */
static atomic_flag topology_lock = ATOMIC_FLAG_INIT;
static atomic_bool initialized;
static hyper_native_handle_t pool_vmar;
static uintptr_t pool_end;
static hyper_stack_t *stacks;
/* Bootstrap handoff reference; ownership lives in the same registry as every
 * other stack. Initialization only runs before secondary threads exist. */
static hyper_stack_t *initial_stack;

static void lock(void)
{
    while (atomic_flag_test_and_set_explicit(&topology_lock, memory_order_acquire))
        (void)hyper_thread_yield();
}
static void unlock(void)
{
    atomic_flag_clear_explicit(&topology_lock, memory_order_release);
}
static void close_owned(hyper_native_handle_t handle)
{
    if (hyper_handle_close(handle) != HYPER_NATIVE_STATUS_OK)
        hyper_process_exit(HYPER_NATIVE_STATUS_INTERNAL);
}
static bool page_round(size_t size, size_t *rounded)
{
    if (!size || size > SIZE_MAX - (PAGE_SIZE - 1)) return false;
    *rounded = (size + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
    return true;
}

/* Adopt either a loader-created reservation or a newly allocated one. All
 * descriptors have the same allocation, topology and retirement rules. Caller
 * serializes publication (startup is single-threaded). */
static void register_stack(hyper_stack_t *stack, uintptr_t base, size_t capacity,
    size_t size, hyper_native_handle_t vmar)
{
    hyper_stack_t **link = &stacks;
    while (*link && (*link)->reservation < base) link = &(*link)->next;
    *stack = (hyper_stack_t){ .reservation = base, .top = base + PAGE_SIZE + capacity,
        .capacity = capacity, .size = size, .vmar = vmar, .next = *link };
    *link = stack;
}

/* Publication is the successful map. No descriptor allocation follows it;
 * all failed map backing is released before the error returns. */
static hyper_native_status_t extend(hyper_stack_t *stack, size_t size)
{
    size_t added = size - stack->size;
    hyper_call_result_t backing = hyper_vmo_create(added);
    if (backing.status != HYPER_NATIVE_STATUS_OK) return backing.status;
    hyper_native_status_t status = hyper_vmar_map(stack->vmar, backing.value0, 0,
        stack->top - size, added,
        HYPER_NATIVE_VMAR_PERMISSION_READ | HYPER_NATIVE_VMAR_PERMISSION_WRITE);
    if (status == HYPER_NATIVE_STATUS_OK) stack->size = size;
    close_owned(backing.value0);
    return status;
}

/* A failed retirement retains its descriptor, VMAR and VA reservation. It may
 * already have unmapped the payload; it must never permit grow or VA reuse.
 * Internal cleanup retries these owned records on subsequent creation or runtime release. */
static hyper_native_status_t retire(hyper_stack_t *stack)
{
    stack->retiring = true;
    if (stack->size) {
        hyper_native_status_t status = hyper_vmar_unmap(stack->vmar,
            stack->top - stack->size, stack->size);
        if (status != HYPER_NATIVE_STATUS_OK) return status;
        stack->size = 0;
    }
    hyper_native_status_t status = hyper_vmar_destroy(stack->vmar);
    /* Successful VMAR destruction consumes its handle in the Native ABI. */
    return status;
}
static void collect(void)
{
    hyper_stack_t **link = &stacks;
    while (*link) {
        hyper_stack_t *stack = *link;
        if (stack->garbage && retire(stack) == HYPER_NATIVE_STATUS_OK) {
            *link = stack->next;
            free(stack);
        } else {
            link = &stack->next;
        }
    }
}

hyper_native_status_t hyper_stack_initialize(const hyper_startup_t *startup, hyper_stack_t **result)
{
    if (!startup || !result || (startup->auxiliary_count && !startup->auxiliary)) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    if (atomic_load_explicit(&initialized, memory_order_acquire)) {
        *result = initial_stack;
        return HYPER_NATIVE_STATUS_OK;
    }
    uintptr_t base = 0, capacity = 0, size = 0;
    unsigned seen = 0;
    for (size_t index = 0; index < startup->auxiliary_count; ++index) {
        const hyper_auxiliary_entry_t entry = startup->auxiliary[index];
        unsigned bit = 0;
        switch (entry.key) {
        case HYPER_NATIVE_AUXV_INITIAL_STACK_BASE: bit = 1; base = entry.value; break;
        case HYPER_NATIVE_AUXV_INITIAL_STACK_CAPACITY: bit = 2; capacity = entry.value; break;
        case HYPER_NATIVE_AUXV_INITIAL_STACK_SIZE: bit = 4; size = entry.value; break;
        default: break;
        }
        if (seen & bit) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
        seen |= bit;
    }
    if (seen != 7 || !size || size > capacity || base < HYPER_STACK_POOL_BASE ||
        (base | capacity | size) % PAGE_SIZE || capacity > UINTPTR_MAX - 2 * PAGE_SIZE ||
        base > UINTPTR_MAX - capacity - 2 * PAGE_SIZE)
        return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    hyper_native_handle_t root, vmar;
    hyper_native_status_t status = hyper_startup_find_handle(startup,
        HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR, &vmar);
    if (status != HYPER_NATIVE_STATUS_OK) return status;
    status = hyper_startup_find_handle(startup, HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR, &root);
    if (status != HYPER_NATIVE_STATUS_OK) return status;
    hyper_stack_t *stack = calloc(1, sizeof(*stack));
    if (!stack) return HYPER_NATIVE_STATUS_NO_MEMORY;
    /* Loader/CRT calls this before constructors or any secondary thread. */
    if (base > HYPER_STACK_POOL_BASE) {
        hyper_call_result_t pool = hyper_vmar_allocate(root, HYPER_STACK_POOL_BASE,
            base - HYPER_STACK_POOL_BASE);
        if (pool.status != HYPER_NATIVE_STATUS_OK) { free(stack); return pool.status; }
        pool_vmar = pool.value0;
    }
    register_stack(stack, base, capacity, size, vmar);
    hyper_stack_claim_runtime(stack);
    initial_stack = stack;
    pool_end = base;
    atomic_store_explicit(&initialized, true, memory_order_release);
    *result = initial_stack;
    return HYPER_NATIVE_STATUS_OK;
}

hyper_native_status_t hyper_stack_create(size_t size, size_t capacity, hyper_stack_t **result)
{
    if (!result) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    *result = NULL;
    if (!page_round(size, &size)) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    if (!capacity) capacity = size > DEFAULT_CAPACITY ? size : DEFAULT_CAPACITY;
    if (!page_round(capacity, &capacity) || size > capacity || capacity > SIZE_MAX - 2 * PAGE_SIZE)
        return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    if (!atomic_load_explicit(&initialized, memory_order_acquire)) return HYPER_NATIVE_STATUS_BAD_STATE;
    hyper_stack_t *stack = calloc(1, sizeof(*stack));
    if (!stack) return HYPER_NATIVE_STATUS_NO_MEMORY;
    lock();
    collect();
    size_t extent = capacity + 2 * PAGE_SIZE;
    uintptr_t base = HYPER_STACK_POOL_BASE;
    hyper_stack_t **link = &stacks;
    while (*link && (*link)->reservation - base < extent) {
        base = (*link)->top + PAGE_SIZE;
        link = &(*link)->next;
    }
    if (!pool_vmar || base > pool_end || extent > pool_end - base) {
        unlock(); free(stack); return HYPER_NATIVE_STATUS_NO_MEMORY;
    }
    hyper_call_result_t region = hyper_vmar_allocate(pool_vmar, base, extent);
    if (region.status != HYPER_NATIVE_STATUS_OK) {
        unlock(); free(stack); return region.status;
    }
    register_stack(stack, base, capacity, 0, region.value0);
    hyper_native_status_t status = extend(stack, size);
    if (status == HYPER_NATIVE_STATUS_OK) *result = stack;
    else { stack->garbage = true; collect(); }
    unlock();
    return status;
}

hyper_native_status_t hyper_stack_get_info(hyper_stack_t *stack, hyper_stack_info_t *result)
{
    if (!stack || !result) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    lock();
    if (stack->retiring) { unlock(); return HYPER_NATIVE_STATUS_BAD_STATE; }
    *result = (hyper_stack_info_t){ stack->top - stack->size, stack->top, stack->size, stack->capacity };
    unlock();
    return HYPER_NATIVE_STATUS_OK;
}
hyper_native_status_t hyper_stack_grow(hyper_stack_t *stack, size_t size)
{
    if (!stack || !page_round(size, &size)) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    lock();
    hyper_native_status_t status;
    if (stack->retiring) status = HYPER_NATIVE_STATUS_BAD_STATE;
    else if (size < stack->size || size > stack->capacity) status = HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    else status = size == stack->size ? HYPER_NATIVE_STATUS_OK : extend(stack, size);
    unlock();
    return status;
}
hyper_native_status_t hyper_stack_destroy(hyper_stack_t *stack)
{
    if (!stack) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    lock();
    if (stack->managed || stack == hyper_stack_current()) { unlock(); return HYPER_NATIVE_STATUS_BAD_STATE; }
    hyper_native_status_t status = retire(stack);
    if (status == HYPER_NATIVE_STATUS_OK) {
        hyper_stack_t **link = &stacks;
        while (*link != stack) link = &(*link)->next;
        *link = stack->next;
        free(stack);
    }
    unlock();
    return status;
}
void hyper_stack_claim_runtime(hyper_stack_t *stack)
{
    lock();
    stack->managed = true;
    unlock();
}
void hyper_stack_release_runtime(hyper_stack_t *stack)
{
    /* Caller has observed kernel termination (or never published a thread).
     * Cleanup failure keeps explicit ownership, not a dangling pool slot. */
    lock();
    stack->managed = false;
    stack->garbage = true;
    collect();
    unlock();
}
