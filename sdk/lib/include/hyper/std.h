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

typedef struct hyper_std_file_info { uint64_t size; uint32_t mode; uint32_t kind; } hyper_std_file_info_t;
int64_t __hyper_std_fs_open(const char *, size_t, uint32_t, uint64_t *);
void __hyper_std_fs_close(uint64_t);
int64_t __hyper_std_fs_read(uint64_t, uint64_t, void *, size_t, size_t *);
int64_t __hyper_std_fs_write(uint64_t, uint64_t, uint32_t, const void *, size_t, size_t *, uint64_t *);
int64_t __hyper_std_fs_resize(uint64_t, uint64_t);
int64_t __hyper_std_fs_info(uint64_t, hyper_std_file_info_t *);
int64_t __hyper_std_fs_directory(const char *, size_t, uint64_t *);
int64_t __hyper_std_fs_stat(const char *, size_t, hyper_std_file_info_t *);
int64_t __hyper_std_fs_readdir(uint64_t, uint64_t, hyper_native_directory_entry_t *, size_t *, uint64_t *);
int64_t __hyper_std_fs_mkdir(const char *, size_t);
int64_t __hyper_std_fs_remove(const char *, size_t, uint32_t);

int64_t __hyper_std_process_begin(const char *, size_t, const char *, size_t, uint64_t *);
int64_t __hyper_std_process_argument(uint64_t, const char *, size_t, uint32_t);
int64_t __hyper_std_process_pipe(uint64_t, uint32_t, uint64_t *);
int64_t __hyper_std_process_inherit(uint64_t, uint32_t, uint64_t, uint32_t);
void __hyper_std_process_abort(uint64_t);
int64_t __hyper_std_process_start(uint64_t, uint64_t *);
uint64_t __hyper_std_current_process_id(void);
int64_t __hyper_std_process_id(uint64_t, uint64_t *);
int64_t __hyper_std_process_kill(uint64_t);
int64_t __hyper_std_process_wait(uint64_t, uint32_t, uint32_t *, uint32_t *, int64_t *);
int64_t __hyper_std_pipe_create(uint64_t *, uint64_t *);
int64_t __hyper_std_pipe_try_read(uint64_t, void *, size_t, size_t *);
int64_t __hyper_std_pipe_read(uint64_t, void *, size_t, size_t *);
int64_t __hyper_std_pipe_write(uint64_t, const void *, size_t, size_t *);
int64_t __hyper_std_wait_pair(uint64_t, uint64_t, uint64_t *, uint64_t *, uint64_t *);
int64_t __hyper_std_wait_ready(uint64_t, uint64_t *);
int64_t __hyper_std_wait_rearm(uint64_t, uint64_t);
int64_t __hyper_std_wait_remove(uint64_t, uint64_t);
#endif
