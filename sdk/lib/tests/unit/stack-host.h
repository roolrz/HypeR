/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#ifndef HYPER_STACK_HOST_H
#define HYPER_STACK_HOST_H
#include <stdint.h>
#include <unistd.h>
#include <hyper/native.h>
#undef HYPER_NATIVE_PAGE_SIZE
#define HYPER_NATIVE_PAGE_SIZE ((size_t)sysconf(_SC_PAGESIZE))
extern uintptr_t hyper_stack_test_base;
#define HYPER_STACK_POOL_BASE hyper_stack_test_base
#endif
