// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Read sharing without a cache line written by every reader.

use core::cell::UnsafeCell;
use core::marker::PhantomData;

use crate::hal::interrupt::InterruptMask;
use crate::sync::atomic::{AtomicBool, Ordering};

use super::InterruptMaskGuard;

// Separate readers even on machines with 128-byte cache lines.
#[repr(align(128))]
struct Stripe(AtomicBool);

struct Release<'a> {
    stripes: &'a [Stripe],
    held: usize,
}

impl Drop for Release<'_> {
    fn drop(&mut self) {
        for stripe in self.stripes[..self.held].iter().rev() {
            stripe.0.store(false, Ordering::Release);
        }
    }
}

/// IRQ-safe, non-reentrant shared access partitioned into caller-selected lanes.
///
/// Readers acquire just one lane. Writers acquire every lane in ascending
/// order before accessing the value. Use stable CPU indices for local readers;
/// lanes must not be nested, upgraded, or retained across context switches.
/// Writers are intended for infrequent ownership changes, not hot-path data.
pub struct InterruptShardedLock<T, M: InterruptMask, const N: usize> {
    stripes: [Stripe; N],
    value: UnsafeCell<T>,
    policy: PhantomData<fn() -> M>,
}

// SAFETY: a writer excludes every reader and writer. Concurrent readers see
// only shared references; T: Sync permits their simultaneous use.
unsafe impl<T: Send + Sync, M: InterruptMask, const N: usize> Sync
    for InterruptShardedLock<T, M, N>
{
}

impl<T, M: InterruptMask, const N: usize> InterruptShardedLock<T, M, N> {
    pub const fn new(value: T) -> Self {
        assert!(N > 0);
        Self {
            stripes: [const { Stripe(AtomicBool::new(false)) }; N],
            value: UnsafeCell::new(value),
            policy: PhantomData,
        }
    }

    /// Reads under one lane; an invalid lane returns `None` without access.
    pub fn read<R>(&self, lane: usize, operation: impl FnOnce(&T) -> R) -> Option<R> {
        let stripe = self.stripes.get(lane)?;
        // SAFETY: the guard remains lexical and no mask escapes this call.
        let mask = unsafe { InterruptMaskGuard::<M>::acquire() };
        Self::acquire(stripe);
        let release = Release {
            stripes: core::slice::from_ref(stripe),
            held: 1,
        };
        // SAFETY: every writer must acquire this stripe before mutation.
        let result = operation(unsafe { &*self.value.get() });
        drop(release);
        drop(mask);
        Some(result)
    }

    /// Mutates only after excluding all reader lanes.
    pub fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> R {
        // SAFETY: all stripe ownership ends before this lexical mask restores.
        let mask = unsafe { InterruptMaskGuard::<M>::acquire() };
        let mut release = Release {
            stripes: &self.stripes,
            held: 0,
        };
        for stripe in &self.stripes {
            Self::acquire(stripe);
            release.held += 1;
        }
        // SAFETY: owning every stripe excludes every other access to value.
        let result = operation(unsafe { &mut *self.value.get() });
        drop(release);
        drop(mask);
        result
    }

    fn acquire(stripe: &Stripe) {
        while stripe
            .0
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while stripe.0.load(Ordering::Relaxed) {
                M::wait_for_lock_owner();
            }
        }
    }
}
