/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/std.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>

const char *__hyper_std_argument(size_t index)
{
    const hyper_startup_t *startup = hyper_runtime_startup();
    return startup && index < startup->argument_count ? startup->arguments[index] : NULL;
}
const char *__hyper_std_environment(size_t index)
{
    const hyper_startup_t *startup = hyper_runtime_startup();
    return startup && index < startup->environment_count ? startup->environment[index] : NULL;
}
int64_t __hyper_std_read(void *buffer, size_t capacity, size_t *actual)
{ return hyper_runtime_stdio_read(buffer, capacity, actual); }
int64_t __hyper_std_write(uint32_t stream, const void *buffer, size_t count, size_t *actual)
{ return hyper_runtime_stdio_write(stream, buffer, count, actual); }
uint64_t __hyper_std_clock(void)
{
    hyper_call_result_t now = hyper_clock_get_monotonic();
    if (now.status != HYPER_NATIVE_STATUS_OK) hyper_process_exit(now.status);
    return now.value0;
}
_Noreturn void __hyper_std_exit(int32_t code) { hyper_process_exit(code); }
void __hyper_std_yield(void) { (void)hyper_thread_yield(); }
void __hyper_std_sleep(uint64_t nanoseconds)
{
    uint64_t now = __hyper_std_clock();
    uint64_t deadline = nanoseconds >= UINT64_MAX - now ? UINT64_MAX - 1 : now + nanoseconds;
    int64_t status = hyper_thread_sleep(deadline);
    if (status != HYPER_NATIVE_STATUS_OK) hyper_process_exit(status);
}
int64_t __hyper_std_thread_spawn(size_t stack_size, hyper_std_thread_entry_t entry,
    void *argument, uintptr_t *token)
{ return hyper_runtime_thread_spawn(stack_size, entry, argument, token); }
int64_t __hyper_std_thread_join(uintptr_t token) { return hyper_runtime_thread_join(token); }
void __hyper_std_thread_detach(uintptr_t token) { hyper_runtime_thread_release(token); }
