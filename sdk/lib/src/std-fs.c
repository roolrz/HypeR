/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/std.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>
#include <string.h>

#define READ 1u
#define WRITE 2u
#define APPEND 4u
#define TRUNCATE 8u
#define CREATE 16u
#define EXCLUSIVE 32u

static hyper_native_handle_t directory_for(const char *path, size_t size)
{
    if (size == 0 || size > HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES) return 0;
    hyper_native_handle_t directory = 0;
    if (path[0] != '/') directory = hyper_runtime_capability(UINT32_C(0x80040001));
    if (!directory) directory = hyper_runtime_capability(HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY);
    return directory;
}

int64_t __hyper_std_fs_open(const char *path, size_t size, uint32_t options, uint64_t *handle)
{
    *handle = 0;
    if ((options & ~63u) || !(options & (READ | WRITE | APPEND)) ||
        ((options & (TRUNCATE | CREATE | EXCLUSIVE)) && !(options & (WRITE | APPEND))))
        return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    hyper_native_handle_t directory = directory_for(path, size);
    if (!directory) return HYPER_NATIVE_STATUS_ACCESS_DENIED;
    uint64_t rights = HYPER_NATIVE_RIGHT_INSPECT | HYPER_NATIVE_RIGHT_DUPLICATE;
    if (options & READ) rights |= HYPER_NATIVE_RIGHT_READ;
    if (options & (WRITE | APPEND)) rights |= HYPER_NATIVE_RIGHT_WRITE;
    hyper_call_result_t result;
    if (options & (CREATE | EXCLUSIVE)) {
        result = hyper_directory_create_file(directory, path, size, rights, 0666);
        if (result.status == HYPER_NATIVE_STATUS_OK) { *handle = result.value0; return 0; }
        if (result.status != HYPER_NATIVE_STATUS_ALREADY_EXISTS || (options & EXCLUSIVE)) return result.status;
    }
    result = hyper_directory_open_file(directory, path, size, rights);
    if (result.status != HYPER_NATIVE_STATUS_OK) return result.status;
    if (options & TRUNCATE) {
        int64_t status = hyper_file_resize(result.value0, 0).status;
        if (status != 0) { (void)hyper_handle_close(result.value0); return status; }
    }
    *handle = result.value0;
    return 0;
}

void __hyper_std_fs_close(uint64_t handle) { (void)hyper_handle_close(handle); }

int64_t __hyper_std_fs_read(uint64_t handle, uint64_t offset, void *buffer, size_t size, size_t *actual)
{
    if (size > HYPER_NATIVE_FILE_MAX_READ_BYTES) size = HYPER_NATIVE_FILE_MAX_READ_BYTES;
    hyper_call_result_t result = hyper_file_read_at(handle, offset, buffer, size);
    *actual = result.status == 0 ? (size_t)result.value0 : 0;
    return result.status;
}

int64_t __hyper_std_fs_write(uint64_t handle, uint64_t offset, uint32_t append, const void *buffer, size_t size, size_t *actual, uint64_t *end)
{
    if (size > HYPER_NATIVE_FILE_MAX_READ_BYTES) size = HYPER_NATIVE_FILE_MAX_READ_BYTES;
    hyper_call_result_t result = hyper_file_write_at(handle, append, append ? 0 : offset, buffer, size);
    *actual = result.status == 0 ? (size_t)result.value0 : 0;
    *end = result.status == 0 ? result.value1 : offset;
    return result.status;
}

int64_t __hyper_std_fs_resize(uint64_t handle, uint64_t size) { return hyper_file_resize(handle, size).status; }

int64_t __hyper_std_fs_info(uint64_t handle, hyper_std_file_info_t *info)
{
    hyper_native_file_info_t native = {0};
    hyper_call_result_t result = hyper_file_get_info(handle, &native);
    if (result.status == 0) { info->size = native.size; info->mode = native.mode; info->kind = HYPER_NATIVE_DIRECTORY_ENTRY_KIND_FILE; }
    return result.status;
}

int64_t __hyper_std_fs_directory(const char *path, size_t size, uint64_t *handle)
{
    *handle = 0;
    hyper_native_handle_t directory = directory_for(path, size);
    if (!directory) return HYPER_NATIVE_STATUS_ACCESS_DENIED;
    hyper_call_result_t result = hyper_directory_open_directory(directory, path, size, HYPER_NATIVE_RIGHT_READ | HYPER_NATIVE_RIGHT_INSPECT);
    if (result.status == 0) *handle = result.value0;
    return result.status;
}

int64_t __hyper_std_fs_stat(const char *path, size_t size, hyper_std_file_info_t *info)
{
    hyper_native_handle_t directory = directory_for(path, size);
    if (!directory) return HYPER_NATIVE_STATUS_ACCESS_DENIED;
    hyper_call_result_t file = hyper_directory_open_file(directory, path, size, HYPER_NATIVE_RIGHT_INSPECT);
    if (file.status == 0) {
        int64_t status = __hyper_std_fs_info(file.value0, info);
        (void)hyper_handle_close(file.value0);
        return status;
    }
    if (file.status != HYPER_NATIVE_STATUS_BAD_STATE) return file.status;
    uint64_t handle;
    int64_t status = __hyper_std_fs_directory(path, size, &handle);
    if (status != 0) return status;
    hyper_native_directory_info_t native = {0};
    status = hyper_directory_get_info(handle, &native).status;
    if (status == 0) { info->size = 0; info->mode = native.mode; info->kind = HYPER_NATIVE_DIRECTORY_ENTRY_KIND_DIRECTORY; }
    (void)hyper_handle_close(handle);
    return status;
}

int64_t __hyper_std_fs_readdir(uint64_t handle, uint64_t cookie, hyper_native_directory_entry_t *entries, size_t *count, uint64_t *next)
{
    hyper_call_result_t result = hyper_directory_read(handle, cookie, entries, HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY);
    *count = result.status == 0 ? result.value0 : 0;
    *next = result.status == 0 ? result.value1 : 0;
    return result.status;
}

int64_t __hyper_std_fs_mkdir(const char *path, size_t size)
{
    hyper_native_handle_t directory = directory_for(path, size);
    return directory ? hyper_directory_create_directory(directory, path, size, 0777).status : HYPER_NATIVE_STATUS_ACCESS_DENIED;
}

int64_t __hyper_std_fs_remove(const char *path, size_t size, uint32_t is_directory)
{
    hyper_native_handle_t directory = directory_for(path, size);
    return directory ? hyper_directory_remove(directory, path, size, is_directory).status : HYPER_NATIVE_STATUS_ACCESS_DENIED;
}
