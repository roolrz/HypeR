// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::sys::pal::ffi;
pub type Key = usize;
pub fn create(dtor: Option<unsafe extern "C" fn(*mut u8)>) -> Key {
    unsafe { ffi::hyper_runtime_tls_create(dtor) }
}
pub unsafe fn destroy(key: Key) {
    unsafe { ffi::hyper_runtime_tls_destroy(key) }
}
pub unsafe fn get(key: Key) -> *mut u8 {
    unsafe { ffi::hyper_runtime_tls_get(key) }
}
pub unsafe fn set(key: Key, value: *mut u8) {
    unsafe { ffi::hyper_runtime_tls_set(key, value) }
}
