// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::ffi;
use crate::sync::atomic::Atomic;
use crate::time::Duration;

pub type Primitive = u32;
pub type SmallPrimitive = u32;
pub type Futex = Atomic<u32>;
pub type SmallFutex = Atomic<u32>;

pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    let deadline = timeout.map_or(u64::MAX, |duration| {
        let now = unsafe { ffi::__hyper_std_clock() };
        now.saturating_add(duration.as_nanos().min(u128::from(u64::MAX - 1)) as u64)
            .min(u64::MAX - 1)
    });
    unsafe { ffi::hyper_runtime_wait_u32(futex.as_ptr(), expected, deadline) != 0 }
}
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    unsafe { ffi::hyper_runtime_wake_u32(futex.as_ptr(), 1) };
    // Polling does not tell us whether a waiter was present. RwLock uses
    // this result to decide whether readers also need notification.
    false
}
pub fn futex_wake_all(futex: &Atomic<u32>) {
    unsafe { ffi::hyper_runtime_wake_u32(futex.as_ptr(), u32::MAX) };
}
