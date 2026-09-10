// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Must match sdk/lib/include/hyper/std.h and thread.h. No Rust-private layouts.
#[link(name = "hyper-std", kind = "static")]
unsafe extern "C" {
    pub fn __hyper_std_argument(index: usize) -> *const u8;
    pub fn __hyper_std_environment(index: usize) -> *const u8;
    pub fn __hyper_std_read(buffer: *mut u8, capacity: usize, actual: *mut usize) -> i64;
    pub fn __hyper_std_write(
        stream: u32,
        buffer: *const u8,
        count: usize,
        actual: *mut usize,
    ) -> i64;
    pub fn __hyper_std_clock() -> u64;
    pub fn __hyper_std_exit(code: i32) -> !;
    pub fn __hyper_std_yield();
    pub fn __hyper_std_sleep(nanoseconds: u64);
    pub fn __hyper_std_thread_spawn(
        stack: usize,
        entry: extern "C" fn(*mut u8),
        argument: *mut u8,
        token: *mut usize,
    ) -> i64;
    pub fn __hyper_std_thread_join(token: usize) -> i64;
    pub fn __hyper_std_thread_detach(token: usize);
    pub fn hyper_alloc(size: usize, align: usize) -> *mut u8;
    pub fn hyper_free(pointer: *mut u8);
    pub fn hyper_realloc(pointer: *mut u8, size: usize, align: usize) -> *mut u8;
    pub fn hyper_runtime_tls_create(dtor: Option<unsafe extern "C" fn(*mut u8)>) -> usize;
    pub fn hyper_runtime_tls_destroy(key: usize);
    pub fn hyper_runtime_tls_get(key: usize) -> *mut u8;
    pub fn hyper_runtime_tls_set(key: usize, value: *mut u8);
    pub fn hyper_runtime_wait_u32(address: *const u32, expected: u32, deadline: u64) -> i32;
    pub fn hyper_runtime_wake_u32(address: *const u32, count: u32) -> u32;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FileInfo {
    pub filesystem_id: u64,
    pub mount_id: u64,
    pub node_id: u64,
    pub size: u64,
    pub mode: u32,
    pub kind: u32,
    pub valid_times: u32,
    pub reserved: u32,
    pub accessed_seconds: i64,
    pub accessed_nanoseconds: u32,
    pub accessed_reserved: u32,
    pub modified_seconds: i64,
    pub modified_nanoseconds: u32,
    pub modified_reserved: u32,
    pub created_seconds: i64,
    pub created_nanoseconds: u32,
    pub created_reserved: u32,
    pub changed_seconds: i64,
    pub changed_nanoseconds: u32,
    pub changed_reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FileUpdate {
    pub mask: u32,
    pub mode: u32,
    pub accessed_seconds: i64,
    pub accessed_nanoseconds: u32,
    pub accessed_reserved: u32,
    pub modified_seconds: i64,
    pub modified_nanoseconds: u32,
    pub modified_reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DirectoryEntry {
    pub size: u64,
    pub mode: u32,
    pub kind: u32,
    pub name_length: u32,
    pub reserved: u32,
    pub name: [u8; 256],
}
impl DirectoryEntry {
    pub const EMPTY: Self = Self {
        size: 0,
        mode: 0,
        kind: 0,
        name_length: 0,
        reserved: 0,
        name: [0; 256],
    };
}
#[link(name = "hyper-std", kind = "static")]
unsafe extern "C" {
    pub fn __hyper_std_fs_open(path: *const u8, size: usize, options: u32, handle: *mut u64)
    -> i64;
    pub fn __hyper_std_fs_close(handle: u64);
    pub fn __hyper_std_fs_read(
        handle: u64,
        offset: u64,
        buffer: *mut u8,
        size: usize,
        actual: *mut usize,
    ) -> i64;
    pub fn __hyper_std_fs_write(
        handle: u64,
        offset: u64,
        append: u32,
        buffer: *const u8,
        size: usize,
        actual: *mut usize,
        end: *mut u64,
    ) -> i64;
    pub fn __hyper_std_fs_resize(handle: u64, size: u64) -> i64;
    pub fn __hyper_std_fs_info(handle: u64, info: *mut FileInfo) -> i64;
    pub fn __hyper_std_fs_directory(path: *const u8, size: usize, handle: *mut u64) -> i64;
    pub fn __hyper_std_fs_stat(
        path: *const u8,
        size: usize,
        options: u32,
        info: *mut FileInfo,
    ) -> i64;
    pub fn __hyper_std_fs_readdir(
        handle: u64,
        cookie: u64,
        entries: *mut DirectoryEntry,
        count: *mut usize,
        next: *mut u64,
    ) -> i64;
    pub fn __hyper_std_fs_mkdir(path: *const u8, size: usize) -> i64;
    pub fn __hyper_std_fs_remove(path: *const u8, size: usize, directory: u32) -> i64;
    pub fn __hyper_std_fs_self_info(handle: u64, info: *mut FileInfo) -> i64;
    pub fn __hyper_std_fs_stat_at(
        handle: u64,
        path: *const u8,
        size: usize,
        options: u32,
        info: *mut FileInfo,
    ) -> i64;
    pub fn __hyper_std_fs_set_info(handle: u64, update: *const FileUpdate) -> i64;
    pub fn __hyper_std_fs_set_path_info(
        path: *const u8,
        size: usize,
        options: u32,
        update: *const FileUpdate,
    ) -> i64;
    pub fn __hyper_std_fs_sync(handle: u64, scope: u32) -> i64;
    pub fn __hyper_std_fs_lock(handle: u64, mode: u32, deadline: u64) -> i64;
    pub fn __hyper_std_fs_unlock(handle: u64) -> i64;
    pub fn __hyper_std_fs_rename(
        from: *const u8,
        from_size: usize,
        to: *const u8,
        to_size: usize,
        hardlink: u32,
    ) -> i64;
    pub fn __hyper_std_fs_symlink(
        target: *const u8,
        target_size: usize,
        path: *const u8,
        size: usize,
    ) -> i64;
    pub fn __hyper_std_fs_path(
        path: *const u8,
        size: usize,
        canonical: u32,
        output: *mut u8,
        capacity: usize,
        actual: *mut usize,
    ) -> i64;
    pub fn __hyper_std_fs_chdir(path: *const u8, size: usize) -> i64;
    pub fn __hyper_std_fs_acquire(path: *const u8, size: usize, handle: *mut u64) -> i64;
    pub fn __hyper_std_fs_open_directory_at(
        parent: u64,
        path: *const u8,
        size: usize,
        nofollow: u32,
        handle: *mut u64,
    ) -> i64;
    pub fn __hyper_std_fs_remove_if(
        parent: u64,
        path: *const u8,
        size: usize,
        directory: u32,
        node: u64,
    ) -> i64;
    pub fn __hyper_std_realtime(seconds: *mut i64, nanoseconds: *mut u32) -> i64;

}

#[link(name = "hyper-std", kind = "static")]
unsafe extern "C" {
    pub fn __hyper_std_process_begin(
        program: *const u8,
        size: usize,
        cwd: *const u8,
        cwd_size: usize,
        builder: *mut u64,
    ) -> i64;
    pub fn __hyper_std_process_argument(
        builder: u64,
        bytes: *const u8,
        size: usize,
        environment: u32,
    ) -> i64;
    pub fn __hyper_std_process_pipe(builder: u64, stream: u32, parent: *mut u64) -> i64;
    pub fn __hyper_std_process_inherit(
        builder: u64,
        stream: u32,
        source: u64,
        parent_stream: u32,
    ) -> i64;
    pub fn __hyper_std_process_abort(builder: u64);
    pub fn __hyper_std_process_start(builder: u64, process: *mut u64) -> i64;
    pub fn __hyper_std_current_process_id() -> u64;
    pub fn __hyper_std_process_id(process: u64, id: *mut u64) -> i64;
    pub fn __hyper_std_process_kill(process: u64) -> i64;
    pub fn __hyper_std_process_wait(
        process: u64,
        block: u32,
        done: *mut u32,
        reason: *mut u32,
        code: *mut i64,
    ) -> i64;
    pub fn __hyper_std_pipe_create(first: *mut u64, second: *mut u64) -> i64;
    pub fn __hyper_std_pipe_try_read(
        handle: u64,
        buffer: *mut u8,
        capacity: usize,
        actual: *mut usize,
    ) -> i64;
    pub fn __hyper_std_pipe_read(
        handle: u64,
        buffer: *mut u8,
        capacity: usize,
        actual: *mut usize,
    ) -> i64;
    pub fn __hyper_std_pipe_write(
        handle: u64,
        buffer: *const u8,
        size: usize,
        actual: *mut usize,
    ) -> i64;
    pub fn __hyper_std_wait_pair(
        first: u64,
        second: u64,
        set: *mut u64,
        first_id: *mut u64,
        second_id: *mut u64,
    ) -> i64;
    pub fn __hyper_std_wait_ready(set: u64, id: *mut u64) -> i64;
    pub fn __hyper_std_wait_rearm(set: u64, id: u64) -> i64;
    pub fn __hyper_std_wait_remove(set: u64, id: u64) -> i64;
}
