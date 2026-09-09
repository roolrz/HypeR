/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#ifndef HYPER_THREAD_H
#define HYPER_THREAD_H
#include <hyper/native.h>
#include <stddef.h>
#include <stdint.h>

/* SDK-private thread runtime, not a syscall ABI. Each Native thread starts
 * with TPIDR_EL0 == 0 and must attach before entering language runtimes.
 * Detach runs key destructors on that thread before releasing its storage.
 * The future thread trampoline must attach -> entry -> detach -> thread_exit.
 * Rust uses key TLS; ELF PT_TLS / compiler-native TLS is not supported yet. */
hyper_native_status_t hyper_runtime_thread_attach(void);
void hyper_runtime_thread_detach(void);
typedef void (*hyper_tls_destructor_t)(void *);
uintptr_t hyper_runtime_tls_create(hyper_tls_destructor_t destructor);
void hyper_runtime_tls_destroy(uintptr_t key);
void *hyper_runtime_tls_get(uintptr_t key);
void hyper_runtime_tls_set(uintptr_t key, void *value);

/* Atomic u32 address, aligned and live for the call. Spurious wakes allowed.
 * Absolute monotonic deadline; UINT64_MAX means infinite. Wake does not
 * publish memory: the caller must release-store before waking. */
int hyper_runtime_wait_u32(const uint32_t *address, uint32_t expected, uint64_t deadline);
void hyper_runtime_wake_u32(const uint32_t *address, uint32_t count);
#endif
