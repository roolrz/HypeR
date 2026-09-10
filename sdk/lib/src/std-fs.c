/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/std.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>

#define READ 1u
#define WRITE 2u
#define APPEND 4u
#define TRUNCATE 8u
#define CREATE 16u
#define EXCLUSIVE 32u

int64_t __hyper_std_fs_open(const char *path, size_t size, uint32_t options, uint64_t *handle)
{
    *handle = 0;
    if ((options & ~63u) || !(options & (READ | WRITE | APPEND)) ||
        ((options & (TRUNCATE | CREATE | EXCLUSIVE)) && !(options & (WRITE | APPEND))))
        return HYPER_NATIVE_STATUS_INVALID_ARGUMENT;
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    hyper_native_handle_info_t info = {0};
    status = hyper_handle_get_info(directory, &info).status;
    if (!status) {
        uint64_t rights = HYPER_NATIVE_RIGHT_INSPECT | HYPER_NATIVE_RIGHT_DUPLICATE;
        rights |= info.rights & (HYPER_NATIVE_RIGHT_SET_ATTRIBUTES | HYPER_NATIVE_RIGHT_LOCK_FILE);
        if (options & READ) rights |= HYPER_NATIVE_RIGHT_READ;
        if (options & (WRITE | APPEND)) rights |= HYPER_NATIVE_RIGHT_WRITE;
        uint32_t native_options = options & EXCLUSIVE ? 2 : options & CREATE ? 1 : 0;
        if ((options & TRUNCATE) && !(options & EXCLUSIVE)) native_options |= 4;
        hyper_call_result_t result = hyper_directory_open_file_with_options(directory, path, size, rights, native_options, 0666);
        status = result.status;
        if (!status) *handle = result.value0;
    }
    (void)hyper_handle_close(directory);
    return status;
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
static int64_t metadata_status(hyper_call_result_t result)
{
    if (result.status) return result.status;
    return result.value0 >= sizeof(hyper_native_file_metadata_t) && result.value0 <= HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES && result.value1 == 0 ? 0 : HYPER_NATIVE_STATUS_BAD_STATE;
}
int64_t __hyper_std_fs_info(uint64_t handle, hyper_native_file_metadata_t *info)
{ return metadata_status(hyper_file_get_metadata(handle, info, sizeof(*info))); }
int64_t __hyper_std_fs_self_info(uint64_t handle, hyper_native_file_metadata_t *info)
{ return metadata_status(hyper_directory_get_self_metadata(handle, info, sizeof(*info))); }
int64_t __hyper_std_fs_directory(const char *path, size_t size, uint64_t *handle)
{
    *handle = 0;
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    hyper_call_result_t result = hyper_directory_open_directory(directory, path, size, HYPER_NATIVE_RIGHT_READ | HYPER_NATIVE_RIGHT_INSPECT);
    (void)hyper_handle_close(directory);
    if (!result.status) *handle = result.value0;
    return result.status;
}
int64_t __hyper_std_fs_stat_at(uint64_t directory, const char *path, size_t size, uint32_t options, hyper_native_file_metadata_t *info)
{ return metadata_status(hyper_directory_get_metadata(directory, path, size, options, info, sizeof(*info))); }
int64_t __hyper_std_fs_stat(const char *path, size_t size, uint32_t options, hyper_native_file_metadata_t *info)
{
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    status = __hyper_std_fs_stat_at(directory, path, size, options, info);
    (void)hyper_handle_close(directory);
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
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    status = hyper_directory_create_directory(directory, path, size, 0777).status;
    (void)hyper_handle_close(directory);
    return status;
}
int64_t __hyper_std_fs_remove(const char *path, size_t size, uint32_t is_directory)
{
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    status = hyper_directory_remove(directory, path, size, is_directory).status;
    (void)hyper_handle_close(directory);
    return status;
}
int64_t __hyper_std_fs_set_info(uint64_t handle, const hyper_native_file_metadata_update_t *update)
{ return hyper_file_set_metadata(handle, update, sizeof(*update)).status; }
int64_t __hyper_std_fs_set_path_info(const char *path, size_t size, uint32_t options, const hyper_native_file_metadata_update_t *update)
{
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    status = hyper_directory_set_metadata(directory, path, size, options, update, sizeof(*update)).status;
    (void)hyper_handle_close(directory);
    return status;
}
int64_t __hyper_std_fs_sync(uint64_t file, uint32_t scope) { return hyper_file_sync(file, scope).status; }
int64_t __hyper_std_fs_lock(uint64_t file, uint32_t mode, uint64_t deadline) { return hyper_file_lock(file, mode, deadline).status; }
int64_t __hyper_std_fs_unlock(uint64_t file) { return hyper_file_unlock(file).status; }
int64_t __hyper_std_fs_rename(const char *from, size_t from_size, const char *to, size_t to_size, uint32_t hardlink)
{
    uint64_t source = 0, destination = 0;
    int64_t status = hyper_runtime_directory_acquire(from, from_size, &source);
    if (status) return status;
    status = hyper_runtime_directory_acquire(to, to_size, &destination);
    if (!status) {
        status = hardlink ? hyper_directory_link(source, from, from_size, destination, to, to_size).status
                          : hyper_directory_rename(source, from, from_size, destination, to, to_size).status;
        (void)hyper_handle_close(destination);
    }
    (void)hyper_handle_close(source);
    return status;
}
int64_t __hyper_std_fs_symlink(const char *target, size_t target_size, const char *path, size_t size)
{
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    status = hyper_directory_symlink(directory, path, size, target, target_size).status;
    (void)hyper_handle_close(directory);
    return status;
}
int64_t __hyper_std_fs_path(const char *path, size_t size, uint32_t canonical, void *output, size_t capacity, size_t *actual)
{
    *actual = 0;
    uint64_t directory = 0;
    int64_t status = hyper_runtime_directory_acquire(path, size, &directory);
    if (status) return status;
    hyper_call_result_t result = canonical ? hyper_directory_canonicalize(directory, path, size, output, capacity)
                                          : hyper_directory_read_link(directory, path, size, output, capacity);
    (void)hyper_handle_close(directory);
    if (!result.status) *actual = result.value0;
    return result.status;
}
int64_t __hyper_std_fs_chdir(const char *path, size_t size) { return hyper_runtime_directory_change(path, size); }
int64_t __hyper_std_fs_acquire(const char *path, size_t size, uint64_t *handle)
{ return hyper_runtime_directory_acquire(path, size, handle); }
int64_t __hyper_std_fs_open_directory_at(uint64_t parent, const char *path, size_t size, uint32_t nofollow, uint64_t *handle)
{
    *handle = 0;
    uint64_t rights = HYPER_NATIVE_RIGHT_READ | HYPER_NATIVE_RIGHT_WRITE | HYPER_NATIVE_RIGHT_INSPECT;
    hyper_call_result_t result = nofollow ? hyper_directory_open_directory_nofollow(parent, path, size, rights)
                                        : hyper_directory_open_directory(parent, path, size, rights);
    if (!result.status) *handle = result.value0;
    return result.status;
}
int64_t __hyper_std_fs_remove_if(uint64_t parent, const char *path, size_t size, uint32_t directory, uint64_t node)
{ return hyper_directory_remove_if(parent, path, size, directory, node).status; }
int64_t __hyper_std_realtime(int64_t *seconds, uint32_t *nanoseconds)
{
    hyper_call_result_t result = hyper_clock_get_realtime();
    if (!result.status && result.value1 >= UINT64_C(1000000000)) return HYPER_NATIVE_STATUS_BAD_STATE;
    if (!result.status) { *seconds = (int64_t)result.value0; *nanoseconds = (uint32_t)result.value1; }
    return result.status;
}
