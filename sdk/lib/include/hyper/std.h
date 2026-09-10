/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#ifndef HYPER_STD_H
#define HYPER_STD_H
#include <hyper/thread.h>
#include <stddef.h>
#include <stdint.h>

/* Version 1 of the SDK-private Rust std bridge. No Rust layouts cross it.
 * Rust sources, this header and libhyper-std.a ship as one pinned SDK.
 * Arg/env pointers are borrowed immutable startup bytes, valid until exit. */
const char *__hyper_std_argument(size_t index);
const char *__hyper_std_environment(size_t index);
int64_t __hyper_std_read(void *buffer, size_t capacity, size_t *actual);
int64_t __hyper_std_write(uint32_t stream, const void *buffer, size_t count, size_t *actual);
uint64_t __hyper_std_clock(void);
_Noreturn void __hyper_std_exit(int32_t code);
void __hyper_std_yield(void);
void __hyper_std_sleep(uint64_t nanoseconds);

/* Successful spawn transfers the entry argument to a newly attached thread.
 * On failure it remains owned by the caller. Join consumes a successful
 * token; detach relinquishes it without stopping the running thread.
 * Stack reclamation follows the Native Thread TERMINATED signal. */
typedef void (*hyper_std_thread_entry_t)(void *);
int64_t __hyper_std_thread_spawn(size_t stack_size, hyper_std_thread_entry_t entry,
    void *argument, uintptr_t *token);
int64_t __hyper_std_thread_join(uintptr_t token);
void __hyper_std_thread_detach(uintptr_t token);

/* Shared process stream buffering lives in libhyper, not each static shim. */
int64_t hyper_runtime_stdio_read(void *buffer, size_t capacity, size_t *actual);
int64_t hyper_runtime_stdio_write(uint32_t stream, const void *buffer, size_t count, size_t *actual);
#endif
