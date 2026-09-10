/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/std.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>
#include <string.h>

#define CWD UINT32_C(0x80040001)
#define CHILD_LIB UINT32_C(0x80040002)
#define STDIN UINT32_C(0x80030001)

static int64_t delegate(uint64_t builder, uint64_t source, uint32_t purpose, uint64_t ceiling)
{
    if (!source) return HYPER_NATIVE_STATUS_ACCESS_DENIED;
    hyper_native_handle_info_t info = {0};
    int64_t status = hyper_handle_get_info(source, &info).status;
    if (status != 0) return status;
    return hyper_process_builder_add_handle(builder, source, purpose, info.object_kind, info.rights & ceiling, HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE);
}

int64_t __hyper_std_process_begin(const char *program, size_t size, const char *cwd, size_t cwd_size, uint64_t *output)
{
    *output = 0;
    uint64_t root = 0, working = 0, builder = 0;
    int64_t status = hyper_runtime_directory_root(&root);
    if (status) return status;
    status = hyper_runtime_directory_acquire(".", 1, &working);
    if (status) goto fail;
    if (cwd_size) {
        uint64_t base = cwd[0] == '/' ? root : working;
        hyper_native_handle_info_t info = {0};
        status = hyper_handle_get_info(base, &info).status;
        if (status) goto fail;
        hyper_call_result_t opened = hyper_directory_open_directory(base, cwd, cwd_size, info.rights);
        if (opened.status) { status = opened.status; goto fail; }
        uint64_t scope = 0;
        status = hyper_runtime_directory_scope(opened.value0, &scope);
        (void)hyper_handle_close(opened.value0);
        if (status) goto fail;
        (void)hyper_handle_close(working);
        working = scope;
    }
    uint64_t base = size && program[0] == '/' ? root : working;
    uint64_t factory = hyper_runtime_capability(HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY);
    uint64_t group = hyper_runtime_capability(HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP);
    uint64_t domain = hyper_runtime_capability(HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN);
    if (!base || !factory || !group || !domain) { status = HYPER_NATIVE_STATUS_ACCESS_DENIED; goto fail; }
    hyper_call_result_t file = hyper_directory_open_file(base, program, size, HYPER_NATIVE_RIGHT_EXECUTE);
    if (file.status) { status = file.status; goto fail; }
    hyper_call_result_t created = hyper_process_builder_create(factory, group, domain, file.value0);
    (void)hyper_handle_close(file.value0);
    if (created.status) { status = created.status; goto fail; }
    builder = created.value0;
    size_t name_start = 0;
    for (size_t i = 0; i < size; ++i) if (program[i] == '/') name_start = i + 1;
    size_t name_size = size - name_start;
    if (name_size > HYPER_NATIVE_PROCESS_NAME_MAX_BYTES) {
        name_size = HYPER_NATIVE_PROCESS_NAME_MAX_BYTES;
        while (name_size && ((unsigned char)program[name_start + name_size] & 0xc0) == 0x80) --name_size;
    }
    status = hyper_process_builder_set_name(builder, program + name_start, name_size);
    if (status) goto fail;
    uint64_t library = hyper_runtime_capability(CHILD_LIB);
    if (!library) library = hyper_runtime_capability(HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DYNAMIC_LIBRARY_DIRECTORY);
    if (library) {
        status = delegate(builder, library, HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DYNAMIC_LIBRARY_DIRECTORY, UINT64_MAX);
        if (status) goto fail;
    }
    status = delegate(builder, working, CWD, UINT64_MAX);
    if (status) goto fail;
    const uint32_t inherited[] = {
        HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY,
        HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY,
        HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP,
        HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN,
    };
    for (size_t i = 0; i < sizeof(inherited) / sizeof(inherited[0]); ++i) {
        uint64_t source = inherited[i] == HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY
            ? root : hyper_runtime_capability(inherited[i]);
        if (!source) continue;
        status = delegate(builder, source, inherited[i], UINT64_MAX);
        if (status) goto fail;
    }
    (void)hyper_handle_close(working);
    (void)hyper_handle_close(root);
    *output = builder;
    return 0;
fail:
    if (working) (void)hyper_handle_close(working);
    (void)hyper_handle_close(root);
    if (builder) (void)hyper_process_builder_abort(builder);
    return status;
}

int64_t __hyper_std_process_argument(uint64_t builder, const char *bytes, size_t size, uint32_t environment)
{
    return environment ? hyper_process_builder_add_environment(builder, bytes, size) : hyper_process_builder_add_argument(builder, bytes, size);
}

int64_t __hyper_std_process_pipe(uint64_t builder, uint32_t stream, uint64_t *parent)
{
    *parent = 0;
    if (stream > 2) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    hyper_call_result_t pair = hyper_byte_channel_create();
    if (pair.status != 0) return pair.status;
    uint64_t rights = HYPER_NATIVE_RIGHT_WAIT | HYPER_NATIVE_RIGHT_DUPLICATE | HYPER_NATIVE_RIGHT_TRANSFER |
        (stream == 0 ? HYPER_NATIVE_RIGHT_READ : HYPER_NATIVE_RIGHT_WRITE);
    int64_t status = hyper_process_builder_add_handle(builder, pair.value1, STDIN + stream, HYPER_NATIVE_OBJECT_BYTE_CHANNEL, rights, HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE);
    if (status != 0) { (void)hyper_handle_close(pair.value0); (void)hyper_handle_close(pair.value1); return status; }
    *parent = pair.value0;
    return 0;
}

int64_t __hyper_std_process_inherit(uint64_t builder, uint32_t stream, uint64_t source, uint32_t parent_stream)
{
    if (stream > 2 || parent_stream > 2) return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    if (!source) source = hyper_runtime_capability(STDIN + parent_stream);
    uint64_t rights = HYPER_NATIVE_RIGHT_DUPLICATE | HYPER_NATIVE_RIGHT_TRANSFER | HYPER_NATIVE_RIGHT_WAIT |
        (stream == 0 ? HYPER_NATIVE_RIGHT_READ : HYPER_NATIVE_RIGHT_WRITE);
    return delegate(builder, source, STDIN + stream, rights);
}

void __hyper_std_process_abort(uint64_t builder) { (void)hyper_process_builder_abort(builder); }

int64_t __hyper_std_process_start(uint64_t builder, uint64_t *process)
{
    *process = 0;
    int64_t status = hyper_process_builder_seal(builder);
    if (status != 0) return status;
    hyper_call_result_t result = hyper_process_builder_start(builder);
    if (result.status == 0) *process = result.value0;
    return result.status;
}

int64_t __hyper_std_process_id(uint64_t process, uint64_t *id)
{
    hyper_native_object_basic_info_t info = {0};
    int64_t status = hyper_object_get_basic_info(process, &info).status;
    if (status == 0) *id = info.koid;
    return status;
}

int64_t __hyper_std_process_kill(uint64_t process) { return hyper_process_request_stop(process); }

int64_t __hyper_std_process_wait(uint64_t process, uint32_t block, uint32_t *done, uint32_t *reason, int64_t *code)
{
    *done = 0;
    hyper_call_result_t wait = hyper_object_wait_one(process, HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED, block ? HYPER_NATIVE_DEADLINE_INFINITE : 0);
    if (wait.status == HYPER_NATIVE_STATUS_TIMED_OUT && !block) return 0;
    if (wait.status != 0) return wait.status;
    hyper_native_process_info_t info = {0};
    int64_t status = hyper_process_get_info(process, &info).status;
    if (status != 0) return status;
    *done = 1;
    *reason = info.terminal_reason;
    *code = (int64_t)info.detail0;
    return 0;
}

/* One nonblocking message attempt. Empty messages are not stream EOF and
 * must yield control back to a multi-stream collector. */
int64_t __hyper_std_pipe_try_read(uint64_t handle, void *buffer, size_t capacity, size_t *actual)
{
    *actual = 0;
    hyper_call_result_t result = hyper_byte_channel_read(handle, buffer, capacity);
    if (result.status == HYPER_NATIVE_STATUS_PEER_CLOSED) return 0;
    if (result.status != 0) return result.status;
    if (result.value0 == 0) return HYPER_NATIVE_STATUS_WOULD_BLOCK;
    *actual = result.value0;
    return 0;
}

int64_t __hyper_std_pipe_read(uint64_t handle, void *buffer, size_t capacity, size_t *actual)
{
    *actual = 0;
    for (;;) {
        hyper_call_result_t result = hyper_byte_channel_read(handle, buffer, capacity);
        if (result.status == 0) { if (result.value0 == 0) continue; *actual = result.value0; return 0; }
        if (result.status == HYPER_NATIVE_STATUS_PEER_CLOSED) return 0;
        if (result.status != HYPER_NATIVE_STATUS_WOULD_BLOCK) return result.status;
        result = hyper_object_wait_one(handle, HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE | HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED, HYPER_NATIVE_DEADLINE_INFINITE);
        if (result.status != 0) return result.status;
    }
}

int64_t __hyper_std_pipe_write(uint64_t handle, const void *buffer, size_t size, size_t *actual)
{
    *actual = 0;
    if (!size) return 0;
    if (size > HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES) size = HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES;
    for (;;) {
        int64_t status = hyper_byte_channel_write(handle, buffer, size);
        if (status == 0) { *actual = size; return 0; }
        if (status != HYPER_NATIVE_STATUS_WOULD_BLOCK) return status;
        hyper_call_result_t wait = hyper_object_wait_one(handle, HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE | HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED, HYPER_NATIVE_DEADLINE_INFINITE);
        if (wait.status != 0) return wait.status;
    }
}

int64_t __hyper_std_wait_pair(uint64_t first, uint64_t second, uint64_t *set, uint64_t *first_id, uint64_t *second_id)
{
    *set = 0;
    hyper_call_result_t created = hyper_wait_set_create(2);
    if (created.status != 0) return created.status;
    const uint64_t mask = HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE | HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED;
    hyper_call_result_t a = hyper_wait_set_add(created.value0, first, mask);
    hyper_call_result_t b = a.status == 0 ? hyper_wait_set_add(created.value0, second, mask) : a;
    if (b.status != 0) { (void)hyper_handle_close(created.value0); return b.status; }
    *set = created.value0; *first_id = a.value0; *second_id = b.value0;
    return 0;
}
int64_t __hyper_std_wait_ready(uint64_t set, uint64_t *id)
{
    uint64_t event[3] = {0};
    hyper_call_result_t result = hyper_wait_set_wait(set, HYPER_NATIVE_DEADLINE_INFINITE, event, sizeof(event));
    if (result.status == 0) *id = event[0];
    return result.status;
}
int64_t __hyper_std_wait_rearm(uint64_t set, uint64_t id) { return hyper_wait_set_rearm(set, id).status; }
int64_t __hyper_std_wait_remove(uint64_t set, uint64_t id) { return hyper_wait_set_remove(set, id).status; }

int64_t __hyper_std_pipe_create(uint64_t *first, uint64_t *second)
{
    hyper_call_result_t result = hyper_byte_channel_create();
    if (result.status == 0) { *first = result.value0; *second = result.value1; }
    return result.status;
}

uint64_t __hyper_std_current_process_id(void)
{
    hyper_call_result_t result = hyper_process_get_current_id();
    if (result.status != 0) hyper_process_exit(result.status);
    return result.value0;
}
