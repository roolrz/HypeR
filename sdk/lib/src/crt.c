/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/startup.h>
#include <hyper/syscall.h>

__attribute__((noreturn, visibility("hidden"))) void __hyper_crt_start(
    const uintptr_t *initial_stack)
{
    hyper_startup_t startup;
    const hyper_native_status_t status = hyper_startup_parse(initial_stack, &startup);
    if (status != HYPER_NATIVE_STATUS_OK) {
        hyper_process_exit(status);
    }
    hyper_process_exit(hyper_main(&startup));
}
