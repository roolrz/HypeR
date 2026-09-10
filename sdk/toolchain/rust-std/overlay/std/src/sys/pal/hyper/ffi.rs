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
