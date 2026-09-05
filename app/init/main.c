/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/native.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>

#include <stdbool.h>

enum { IO_BUFFER_SIZE = 256 };

static int native_abi_is_supported(void)
{
    const hyper_call_result_t abi = hyper_abi_query();

    return abi.status == HYPER_NATIVE_STATUS_OK &&
           abi.value0 == HYPER_NATIVE_ABI_REVISION &&
           (abi.value1 & HYPER_NATIVE_FEATURE_CORE) != 0;
}

static int write_all(hyper_native_handle_t console, const void *bytes, size_t count)
{
    const unsigned char *cursor = bytes;
    while (count != 0) {
        const hyper_call_result_t result = hyper_console_write(console, cursor, count);
        if (result.value0 > count) {
            return -1;
        }
        cursor += result.value0;
        count -= result.value0;
        if (result.status == HYPER_NATIVE_STATUS_OK) {
            if (result.value0 == 0 && count != 0) {
                return -1;
            }
            continue;
        }
        if (result.status != HYPER_NATIVE_STATUS_WOULD_BLOCK) {
            return -1;
        }
        const hyper_call_result_t wait = hyper_object_wait_one(
            console,
            HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE,
            HYPER_NATIVE_DEADLINE_INFINITE);
        if (wait.status != HYPER_NATIVE_STATUS_OK ||
            (wait.value0 & HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE) == 0) {
            return -1;
        }
    }
    return 0;
}

static int wait_for_input(hyper_native_handle_t console)
{
    const hyper_call_result_t result = hyper_object_wait_one(
        console,
        HYPER_NATIVE_SIGNAL_CONSOLE_READABLE,
        HYPER_NATIVE_DEADLINE_INFINITE);
    return result.status == HYPER_NATIVE_STATUS_OK &&
                   (result.value0 & HYPER_NATIVE_SIGNAL_CONSOLE_READABLE) != 0
               ? 0
               : -1;
}

int hyper_main(const hyper_startup_t *startup)
{
    static const char ready[] = "HypeR init: console ready\n";
    static const char received[] = "HypeR init: received input\n";
    hyper_native_handle_t console;

    if (!native_abi_is_supported() ||
        hyper_startup_find_handle(
            startup,
            HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE,
            &console) != HYPER_NATIVE_STATUS_OK ||
        write_all(console, ready, sizeof(ready) - 1) != 0) {
        return 1;
    }

    unsigned char bytes[IO_BUFFER_SIZE];
    bool announced_input = false;
    for (;;) {
        const hyper_call_result_t result = hyper_console_read(console, bytes, sizeof(bytes));
        if (result.status != HYPER_NATIVE_STATUS_OK &&
            result.status != HYPER_NATIVE_STATUS_WOULD_BLOCK) {
            return 1;
        }
        if (result.value0 > sizeof(bytes)) {
            return 1;
        }
        if (result.value0 != 0) {
            if ((!announced_input &&
                 write_all(console, received, sizeof(received) - 1) != 0) ||
                write_all(console, bytes, result.value0) != 0) {
                return 1;
            }
            announced_input = true;
        }
        if (result.status == HYPER_NATIVE_STATUS_OK) {
            if (result.value0 == 0 && wait_for_input(console) != 0) {
                return 1;
            }
            continue;
        }
        if (wait_for_input(console) != 0) {
            return 1;
        }
    }
}
