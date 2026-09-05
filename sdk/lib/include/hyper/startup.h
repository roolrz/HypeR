/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#ifndef HYPER_STARTUP_H
#define HYPER_STARTUP_H

#include <hyper/native.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct hyper_auxiliary_entry {
    uintptr_t key;
    uintptr_t value;
} hyper_auxiliary_entry_t;

typedef struct hyper_startup {
    size_t argument_count;
    char *const *arguments;
    size_t environment_count;
    char *const *environment;
    size_t auxiliary_count;
    const hyper_auxiliary_entry_t *auxiliary;
    size_t handle_count;
    const hyper_native_startup_handle_t *handles;
} hyper_startup_t;

hyper_native_status_t hyper_startup_parse(
    const uintptr_t *initial_stack,
    hyper_startup_t *startup);

hyper_native_status_t hyper_startup_find_handle(
    const hyper_startup_t *startup,
    uint32_t purpose,
    hyper_native_handle_t *handle);

int hyper_main(const hyper_startup_t *startup);

#ifdef __cplusplus
}
#endif

#endif /* HYPER_STARTUP_H */
