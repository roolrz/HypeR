// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native thread lifecycle and process-private atomic address waits.

use crate::handle::{AnyObject, HandleRef, OwnedHandle, ThreadObject};
use crate::{Result, Status};
use core::sync::atomic::AtomicU32;

/// Prepares a dormant thread in the current Process.
///
/// # Safety
/// Entry must follow the Native function ABI and exit through `thread_exit`.
/// The caller owns a distinct valid stack and TLS area until TERMINATED, and
/// must preserve every value reached through the entry argument. Prefer
/// `std::thread` for language-level spawn and automatic resource reclamation.
pub unsafe fn create(
    entry: u64,
    stack: u64,
    tls: u64,
    argument: u64,
) -> Result<OwnedHandle<ThreadObject>> {
    // SAFETY: the caller supplies valid initial execution state.
    let result = unsafe { hyper_sys::thread_create(entry, stack, tls, argument) };
    Status::from_raw(result.status).into_result()?;
    // SAFETY: success transfers one fresh handle.
    let owner =
        unsafe { crate::handle::adopt_produced_handle_excluding::<AnyObject>(result.value0, &[])? };
    owner
        .downcast::<ThreadObject>()
        .map_err(|failure| failure.error())
}

pub fn start(thread: HandleRef<'_, ThreadObject>) -> Result<()> {
    // SAFETY: the typed borrowed handle remains valid for this call.
    Status::from_raw(unsafe { hyper_sys::thread_start(thread.raw().get()) }).into_result()
}

/// Requests termination without executing language cleanup.
///
/// # Safety
/// The caller must tolerate abandonment of all target thread resources and
/// must not reclaim its stack or reachable state before observing TERMINATED.
pub unsafe fn request_stop(thread: HandleRef<'_, ThreadObject>) -> Result<()> {
    // SAFETY: the caller accepts asynchronous termination; the handle is live.
    Status::from_raw(unsafe { hyper_sys::thread_request_stop(thread.raw().get()) }).into_result()
}

/// Parks only if the aligned word still equals expected. Spurious wakes are
/// permitted. Deadline is absolute monotonic nanoseconds; `u64::MAX` is infinite.
/// Keep the mapping live until this call returns. A replacement mapping cannot
/// wake a waiter on the old mapping, even at the same virtual address.
pub fn atomic_wait(word: &AtomicU32, expected: u32, deadline: u64) -> Result<()> {
    // SAFETY: the borrowed AtomicU32 is aligned and live for the entire wait.
    Status::from_raw(unsafe { hyper_sys::atomic_wait(word.as_ptr(), expected, deadline) })
        .into_result()
}

/// Wakes at most count current waiters; wake does not publish ordinary data.
pub fn atomic_wake(word: &AtomicU32, count: u32) -> Result<u64> {
    // SAFETY: the borrowed AtomicU32 is aligned and live for the syscall.
    let result = unsafe { hyper_sys::atomic_wake(word.as_ptr(), count) };
    Status::from_raw(result.status).into_result()?;
    Ok(result.value0)
}

pub fn sleep_until(deadline: u64) -> Result<()> {
    // SAFETY: a scalar monotonic deadline carries no borrowed resources.
    Status::from_raw(unsafe { hyper_sys::thread_sleep(deadline) }).into_result()
}
