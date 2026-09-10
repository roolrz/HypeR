/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/thread.h>
#include <hyper/syscall.h>
#include <stdatomic.h>
#include <stdlib.h>

/* Tokens have one caller owner until join or release. The worker borrows its
 * token until Native termination. A single process-lifetime cleanup worker
 * reclaims detached stacks only after observing that termination signal. */
typedef struct thread_token {
    void *stack;
    hyper_native_handle_t thread;
    hyper_runtime_thread_entry_t entry;
    void *argument;
    atomic_uint completed;
    struct thread_token *next;
} thread_token_t;
static atomic_uint queue_lock;
static atomic_uint changed;
static thread_token_t *detached;
static atomic_uint init_lock;
static int reaper_started;

static void lock(atomic_uint *word)
{
    while (atomic_exchange_explicit(word, 1, memory_order_acquire))
        hyper_runtime_wait_u32((const uint32_t *)word, 1, UINT64_MAX);
}
static void unlock(atomic_uint *word)
{
    atomic_store_explicit(word, 0, memory_order_release);
    hyper_runtime_wake_u32((const uint32_t *)word, UINT32_MAX);
}
static void notify(void)
{
    atomic_store_explicit(&changed, 1, memory_order_release);
    hyper_runtime_wake_u32((const uint32_t *)&changed, UINT32_MAX);
}
static int64_t wait_terminated(hyper_native_handle_t thread)
{
    return hyper_object_wait_one(thread, HYPER_NATIVE_SIGNAL_THREAD_TERMINATED, UINT64_MAX).status;
}
static void reclaim(thread_token_t *token)
{
    if (hyper_handle_close(token->thread) != HYPER_NATIVE_STATUS_OK)
        hyper_process_exit(HYPER_NATIVE_STATUS_INTERNAL);
    free(token->stack);
    free(token);
}
static _Noreturn void worker(void *argument)
{
    thread_token_t *token = argument;
    if (hyper_runtime_thread_attach() != HYPER_NATIVE_STATUS_OK)
        hyper_process_exit(HYPER_NATIVE_STATUS_NO_MEMORY);
    token->entry(token->argument);
    hyper_runtime_thread_detach();
    atomic_store_explicit(&token->completed, 1, memory_order_release);
    notify();
    hyper_thread_exit(0);
}
static _Noreturn void reaper(void *argument)
{
    (void)argument;
    if (hyper_runtime_thread_attach() != HYPER_NATIVE_STATUS_OK)
        hyper_process_exit(HYPER_NATIVE_STATUS_NO_MEMORY);
    for (;;) {
        /* Clear the coalesced prompt before inspecting durable completion
         * predicates. A later notification remains set through the wait;
         * an earlier one is acquired here and visible in the following scan.
         * No finite-width event counter can wrap back to an old observation. */
        atomic_exchange_explicit(&changed, 0, memory_order_acq_rel);
        lock(&queue_lock);
        thread_token_t **link = &detached;
        while (*link && !atomic_load_explicit(&(*link)->completed, memory_order_acquire))
            link = &(*link)->next;
        thread_token_t *token = *link;
        if (token) *link = token->next;
        unlock(&queue_lock);
        if (token) {
            /* completed is only a prompt: the worker may still use its stack
             * in the final wake/exit syscall. TERMINATED proves detachment. */
            if (wait_terminated(token->thread) != HYPER_NATIVE_STATUS_OK)
                hyper_process_exit(HYPER_NATIVE_STATUS_INTERNAL);
            reclaim(token);
        } else {
            hyper_runtime_wait_u32((const uint32_t *)&changed, 0, UINT64_MAX);
        }
    }
}
static int64_t ensure_reaper(void)
{
    lock(&init_lock);
    int64_t status = HYPER_NATIVE_STATUS_OK;
    if (!reaper_started) {
        size_t size = 64 * 1024;
        void *stack = malloc(size);
        if (!stack) { unlock(&init_lock); return HYPER_NATIVE_STATUS_NO_MEMORY; }
        hyper_call_result_t result = hyper_thread_create((uintptr_t)reaper,
            ((uintptr_t)stack + size) & ~(uintptr_t)15, 0, 0);
        status = result.status;
        if (status == HYPER_NATIVE_STATUS_OK) {
            status = hyper_thread_start(result.value0);
            (void)hyper_handle_close(result.value0);
        }
        if (status == HYPER_NATIVE_STATUS_OK) reaper_started = 1;
        else free(stack);
    }
    unlock(&init_lock);
    return status;
}
hyper_native_status_t hyper_runtime_thread_spawn(size_t size, hyper_runtime_thread_entry_t entry,
    void *argument, uintptr_t *result)
{
    if (!entry || !result || size > SIZE_MAX - 15) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    int64_t status = ensure_reaper();
    if (status != HYPER_NATIVE_STATUS_OK) return status;
    if (size < 64 * 1024) size = 64 * 1024;
    size = (size + 15) & ~(size_t)15;
    thread_token_t *token = calloc(1, sizeof(*token));
    if (!token) return HYPER_NATIVE_STATUS_NO_MEMORY;
    token->stack = malloc(size);
    if (!token->stack) { free(token); return HYPER_NATIVE_STATUS_NO_MEMORY; }
    token->entry = entry;
    token->argument = argument;
    atomic_init(&token->completed, 0);
    hyper_call_result_t created = hyper_thread_create((uintptr_t)worker,
        ((uintptr_t)token->stack + size) & ~(uintptr_t)15, 0, (uintptr_t)token);
    if (created.status != HYPER_NATIVE_STATUS_OK) {
        free(token->stack); free(token); return created.status;
    }
    token->thread = created.value0;
    status = hyper_thread_start(token->thread);
    if (status != HYPER_NATIVE_STATUS_OK) { reclaim(token); return status; }
    *result = (uintptr_t)token;
    return HYPER_NATIVE_STATUS_OK;
}
hyper_native_status_t hyper_runtime_thread_join(uintptr_t raw)
{
    thread_token_t *token = (thread_token_t *)raw;
    int64_t status = wait_terminated(token->thread);
    if (status == HYPER_NATIVE_STATUS_OK) reclaim(token);
    return status;
}
void hyper_runtime_thread_release(uintptr_t raw)
{
    thread_token_t *token = (thread_token_t *)raw;
    lock(&queue_lock);
    token->next = detached;
    detached = token;
    unlock(&queue_lock);
    notify();
}
