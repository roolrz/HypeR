/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/heap.h>
#include <hyper/syscall.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <string.h>

/* One process heap. Region/block metadata lives in the mappings it describes;
 * allocator bookkeeping never allocates recursively. All topology is locked.
 * First-fit blocks split and coalesce; wholly free regions are unmapped. */
#define BLOCK_ALIGNMENT _Alignof(max_align_t)
#define REGION_GRANULE ((size_t)65536)
#define PAGE_SIZE ((size_t)HYPER_NATIVE_PAGE_SIZE)

typedef struct region region_t;
typedef struct block block_t;
struct block {
    size_t span;
    size_t requested;
    block_t *previous;
    block_t *next;
    region_t *region;
    bool available;
};
struct region {
    size_t size;
    region_t *next;
};

static atomic_flag heap_lock = ATOMIC_FLAG_INIT;
static hyper_native_handle_t heap_vmar;
static region_t *regions;

static void lock(void)
{
    while (atomic_flag_test_and_set_explicit(&heap_lock, memory_order_acquire)) {
        (void)hyper_thread_yield();
    }
}

static void unlock(void)
{
    atomic_flag_clear_explicit(&heap_lock, memory_order_release);
}

static void require_success(hyper_native_status_t status)
{
    if (status != HYPER_NATIVE_STATUS_OK) {
        hyper_process_exit(HYPER_NATIVE_STATUS_INTERNAL);
    }
}

/* All alignments supplied here are nonzero powers of two. */
static bool round_up(size_t value, size_t alignment, size_t *result)
{
    if (value > SIZE_MAX - (alignment - 1)) {
        return false;
    }
    *result = (value + alignment - 1) & ~(alignment - 1);
    return true;
}

static size_t region_header_size(void)
{
    return (sizeof(region_t) + BLOCK_ALIGNMENT - 1) & ~(BLOCK_ALIGNMENT - 1);
}

static block_t *first_block(region_t *region)
{
    return (block_t *)((unsigned char *)region + region_header_size());
}

hyper_native_status_t hyper_heap_initialize(const hyper_startup_t *startup)
{
    hyper_native_handle_t root;
    hyper_native_status_t status = hyper_startup_find_handle(
        startup, HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR, &root);
    if (status != HYPER_NATIVE_STATUS_OK) {
        return status;
    }
    lock();
    if (heap_vmar != 0) {
        unlock();
        return HYPER_NATIVE_STATUS_OK;
    }
    hyper_call_result_t result = hyper_vmar_allocate(root, HYPER_HEAP_BASE, HYPER_HEAP_SIZE);
    if (result.status == HYPER_NATIVE_STATUS_OK) {
        heap_vmar = result.value0;
    }
    unlock();
    return result.status;
}

static region_t *grow(size_t minimum)
{
    size_t size;
    if (!round_up(minimum, PAGE_SIZE, &size) || size > HYPER_HEAP_SIZE) {
        return NULL;
    }
    if (size < REGION_GRANULE) {
        size = REGION_GRANULE;
    }
    uintptr_t address = HYPER_HEAP_BASE;
    region_t **link = &regions;
    while (*link != NULL) {
        if ((uintptr_t)*link - address >= size) {
            break;
        }
        address = (uintptr_t)*link + (*link)->size;
        link = &(*link)->next;
    }
    if (size > HYPER_HEAP_BASE + HYPER_HEAP_SIZE - address) {
        return NULL;
    }
    hyper_call_result_t backing = hyper_vmo_create(size);
    if (backing.status != HYPER_NATIVE_STATUS_OK) {
        return NULL;
    }
    hyper_native_status_t status = hyper_vmar_map(
        heap_vmar, backing.value0, 0, address, size,
        HYPER_NATIVE_VMAR_PERMISSION_READ | HYPER_NATIVE_VMAR_PERMISSION_WRITE);
    /* The successful mapping retains backing ownership. Failed maps leave no
     * published region and must release any pages populated by the kernel. */
    require_success(hyper_handle_close(backing.value0));
    if (status != HYPER_NATIVE_STATUS_OK) {
        return NULL;
    }
    region_t *region = (region_t *)address;
    *region = (region_t){ .size = size, .next = *link };
    *link = region;
    block_t *block = first_block(region);
    *block = (block_t){
        .span = size - region_header_size(), .region = region, .available = true,
    };
    return region;
}

static void split(block_t *block, size_t used)
{
    size_t remainder = block->span - used;
    if (remainder < sizeof(block_t) + sizeof(block_t *) + BLOCK_ALIGNMENT) {
        return;
    }
    block_t *tail = (block_t *)((unsigned char *)block + used);
    *tail = (block_t){
        .span = remainder, .previous = block, .next = block->next,
        .region = block->region, .available = true,
    };
    if (tail->next != NULL) {
        tail->next->previous = tail;
    }
    block->next = tail;
    block->span = used;
}

static void *take(block_t *block, size_t size, size_t alignment)
{
    size_t address;
    size_t used;
    if (!block->available
        || !round_up((uintptr_t)block + sizeof(*block) + sizeof(block_t *), alignment, &address)
        || size > SIZE_MAX - (address - (uintptr_t)block)
        || !round_up(address - (uintptr_t)block + size, BLOCK_ALIGNMENT, &used)
        || used > block->span) {
        return NULL;
    }
    split(block, used);
    block->available = false;
    block->requested = size;
    /* Alignment is at least pointer alignment, so the backlink is aligned. */
    ((block_t **)address)[-1] = block;
    return (void *)address;
}

void *hyper_alloc(size_t size, size_t alignment)
{
    if (alignment == 0 || (alignment & (alignment - 1)) != 0) {
        return NULL;
    }
    if (alignment < BLOCK_ALIGNMENT) {
        alignment = BLOCK_ALIGNMENT;
    }
    if (size == 0) {
        size = 1;
    }
    /* Bound every later metadata/padding addition before touching the heap. */
    size_t overhead = region_header_size() + sizeof(block_t) + sizeof(block_t *) + BLOCK_ALIGNMENT;
    if (alignment > HYPER_HEAP_SIZE || size > HYPER_HEAP_SIZE - alignment
        || size + alignment > HYPER_HEAP_SIZE - overhead) {
        return NULL;
    }
    lock();
    if (heap_vmar == 0) {
        unlock();
        return NULL;
    }
    for (region_t *region = regions; region != NULL; region = region->next) {
        for (block_t *block = first_block(region); block != NULL; block = block->next) {
            void *pointer = take(block, size, alignment);
            if (pointer != NULL) {
                unlock();
                return pointer;
            }
        }
    }
    region_t *region = grow(size + alignment + overhead);
    void *pointer = region == NULL ? NULL : take(first_block(region), size, alignment);
    unlock();
    return pointer;
}

static void merge_next(block_t *block)
{
    block_t *next = block->next;
    block->span += next->span;
    block->next = next->next;
    if (block->next != NULL) {
        block->next->previous = block;
    }
}

void hyper_free(void *pointer)
{
    if (pointer == NULL) {
        return;
    }
    lock();
    block_t *block = ((block_t **)pointer)[-1];
    block->available = true;
    if (block->next != NULL && block->next->available) {
        merge_next(block);
    }
    if (block->previous != NULL && block->previous->available) {
        block = block->previous;
        merge_next(block);
    }
    if (block->previous == NULL && block->next == NULL) {
        region_t *region = block->region;
        region_t **link = &regions;
        while (*link != region) {
            link = &(*link)->next;
        }
        const size_t size = region->size;
        *link = region->next;
        require_success(hyper_vmar_unmap(heap_vmar, (uintptr_t)region, size));
    }
    unlock();
}

void *hyper_realloc(void *pointer, size_t size, size_t alignment)
{
    if (pointer == NULL) {
        return hyper_alloc(size, alignment);
    }
    if (size == 0) {
        hyper_free(pointer);
        return NULL;
    }
    /* The caller exclusively owns this allocation, even while unrelated blocks
     * are changed by other threads. requested is immutable until free. */
    block_t *block = ((block_t **)pointer)[-1];
    size_t previous = block->requested;
    void *replacement = hyper_alloc(size, alignment);
    if (replacement != NULL) {
        memcpy(replacement, pointer, size < previous ? size : previous);
        hyper_free(pointer);
    }
    return replacement;
}
