// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::ffi::CStr;
use crate::io;
use crate::num::NonZero;
use crate::sys::pal::{cvt, ffi};
use crate::thread::ThreadInit;
use crate::time::Duration;

pub const DEFAULT_MIN_STACK_SIZE: usize = 256 * 1024;
pub struct Thread(usize);
impl Thread {
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Self> {
        extern "C" fn entry(argument: *mut u8) {
            let init = unsafe { Box::from_raw(argument.cast::<ThreadInit>()) };
            let main = init.init();
            main();
            // The Native trampoline owns TLS attach/detach and thread_exit.
        }
        let pointer = Box::into_raw(init);
        let mut token = 0;
        let status =
            unsafe { ffi::__hyper_std_thread_spawn(stack, entry, pointer.cast(), &mut token) };
        if let Err(error) = cvt(status) {
            drop(unsafe { Box::from_raw(pointer) });
            return Err(error);
        }
        Ok(Self(token))
    }
    pub fn join(self) {
        let token = self.0;
        crate::mem::forget(self);
        cvt(unsafe { ffi::__hyper_std_thread_join(token) }).expect("Native thread join failed");
    }
}
impl Drop for Thread {
    fn drop(&mut self) {
        unsafe { ffi::__hyper_std_thread_detach(self.0) }
    }
}
pub fn yield_now() {
    unsafe { ffi::__hyper_std_yield() }
}
pub fn sleep(mut duration: Duration) {
    while duration.as_nanos() > u128::from(u64::MAX) {
        unsafe { ffi::__hyper_std_sleep(u64::MAX) };
        duration -= Duration::from_nanos(u64::MAX);
    }
    unsafe { ffi::__hyper_std_sleep(duration.as_nanos() as u64) };
}
pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    Err(io::Error::UNKNOWN_THREAD_COUNT)
}
pub fn current_os_id() -> Option<u64> {
    None
}
pub fn set_name(_: &CStr) {}
