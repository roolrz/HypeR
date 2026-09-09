// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::alloc::{GlobalAlloc, Layout, System};
use crate::sys::pal::ffi;

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { ffi::hyper_alloc(layout.size(), layout.align()) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, _: Layout) {
        unsafe { ffi::hyper_free(pointer) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        unsafe { ffi::hyper_realloc(pointer, size, layout.align()) }
    }
}
