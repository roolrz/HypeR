// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Lock wrapper that composes local interrupt masking with a raw spin lock.

use core::marker::PhantomData;

use crate::hal::interrupt::InterruptMask;

use super::SpinLock;

/// A spin lock that disables local interrupts before acquiring its raw lock.
///
/// This prevents an interrupt handler on the same processing element from
/// attempting to recursively acquire a lock held by interrupted code. The
/// architecture policy remains outside generic synchronization code.
pub struct InterruptSpinLock<T, M: InterruptMask> {
    lock: SpinLock<T>,
    policy: PhantomData<fn() -> M>,
}

/// Ownership of one saved local-interrupt state.
///
/// The guard may outlive the lock acquisition that created it, allowing a
/// scheduler transition to release a spin lock while keeping interrupts
/// masked until the architecture context handoff is complete.
///
/// The guard is CPU-affine and therefore neither `Send` nor `Sync`. It must be
/// destroyed on the CPU that acquired it, in strict reverse acquisition order
/// within one active continuation and mask-state lineage. A scheduler may
/// suspend a continuation that owns the guard only while that Thread remains
/// assigned to the same CPU; future migration must first end the transition
/// and restore its owned interrupt state.
pub struct InterruptMaskGuard<M: InterruptMask> {
    state: M::State,
    policy: PhantomData<fn() -> M>,
    // Raw pointers implement neither Send nor Sync, making mask ownership
    // explicitly CPU-affine without depending on the policy's auto traits.
    cpu_affine: PhantomData<*mut ()>,
}

impl<M: InterruptMask> InterruptMaskGuard<M> {
    /// Captures and masks the calling CPU's local interrupt state.
    ///
    /// # Safety
    ///
    /// The returned guard must be dropped on this CPU in strict reverse order
    /// within its continuation's mask-state lineage. If its continuation is
    /// suspended, the scheduler must keep that continuation on this CPU until
    /// the guard has restored its state.
    pub unsafe fn acquire() -> Self {
        Self {
            state: M::save_and_disable(),
            policy: PhantomData,
            cpu_affine: PhantomData,
        }
    }
}

impl<M: InterruptMask> Drop for InterruptMaskGuard<M> {
    fn drop(&mut self) {
        M::restore(self.state);
    }
}

impl<T, M: InterruptMask> InterruptSpinLock<T, M> {
    pub const fn new(value: T) -> Self {
        Self {
            lock: SpinLock::new(value),
            policy: PhantomData,
        }
    }

    pub fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> R {
        // SAFETY: This lexical guard cannot escape, and nested `with` calls
        // necessarily destroy their guards before this one.
        let restore = unsafe { InterruptMaskGuard::<M>::acquire() };
        let result = self.lock.with_relax(operation, M::wait_for_lock_owner);
        drop(restore);
        result
    }

    /// Releases the lock but transfers interrupt restoration to the caller.
    ///
    /// This is intended for lock-to-scheduler handoff. The returned guard must
    /// remain live across the machine context switch until the original
    /// continuation resumes, or be dropped when no transition is needed.
    ///
    /// # Safety
    ///
    /// The caller assumes ownership of the returned guard and must drop it on
    /// this CPU in strict reverse order within its mask-state lineage. A
    /// continuation carrying it across a context switch must remain CPU-pinned.
    pub unsafe fn with_mask_retained<R>(
        &self,
        operation: impl FnOnce(&mut T) -> R,
    ) -> (R, InterruptMaskGuard<M>) {
        // SAFETY: The caller accepts the retained guard's CPU and nesting
        // obligations stated by this method.
        let restore = unsafe { InterruptMaskGuard::<M>::acquire() };
        let result = self.lock.with_relax(operation, M::wait_for_lock_owner);
        (result, restore)
    }

    /// Runs `operation` only when the underlying lock is immediately free.
    pub fn try_with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> Option<R> {
        // SAFETY: This lexical guard cannot escape, and nested `try_with` calls
        // necessarily destroy their guards before this one.
        let restore = unsafe { InterruptMaskGuard::<M>::acquire() };
        let result = self.lock.try_with(operation);
        drop(restore);
        result
    }
}
