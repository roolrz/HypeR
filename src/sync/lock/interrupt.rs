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

struct InterruptRestore<M: InterruptMask> {
    state: M::State,
    policy: PhantomData<fn() -> M>,
}

impl<M: InterruptMask> Drop for InterruptRestore<M> {
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
        let restore = InterruptRestore::<M> {
            state: M::save_and_disable(),
            policy: PhantomData,
        };
        let result = self.lock.with(operation);
        drop(restore);
        result
    }

    /// Runs `operation` only when the underlying lock is immediately free.
    pub fn try_with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> Option<R> {
        let restore = InterruptRestore::<M> {
            state: M::save_and_disable(),
            policy: PhantomData,
        };
        let result = self.lock.try_with(operation);
        drop(restore);
        result
    }
}
