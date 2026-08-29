// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallible ownership primitives built on the installed global allocator.

use alloc::boxed::Box;
use core::alloc::Layout;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering, fence};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationError;

struct SharedInner<T> {
    references: AtomicUsize,
    value: T,
}

/// A fallibly allocated, thread-safe shared owner.
///
/// This deliberately small primitive exists because the stable allocator API
/// does not provide `Arc::try_new`. It supports strong references only. Once
/// the reference counter saturates, the allocation is permanently retained;
/// leaking is preferable to wrapping the counter and freeing a live value.
pub struct FallibleArc<T> {
    inner: NonNull<SharedInner<T>>,
    ownership: PhantomData<SharedInner<T>>,
}

// SAFETY: FallibleArc grants shared access to T from every clone. Moving or
// sharing that access across CPUs is sound exactly when T is Send + Sync.
unsafe impl<T: Send + Sync> Send for FallibleArc<T> {}
// SAFETY: See the Send implementation. Reference-count changes are atomic and
// the contained value is exposed only through shared references.
unsafe impl<T: Send + Sync> Sync for FallibleArc<T> {}

impl<T> FallibleArc<T> {
    /// Allocates one shared owner without invoking the allocation-error path.
    pub fn try_new(value: T) -> Result<Self, AllocationError> {
        let inner = try_box(SharedInner {
            references: AtomicUsize::new(1),
            value,
        })?;
        // SAFETY: Box never contains a null pointer. FallibleArc assumes the
        // allocation and becomes responsible for reconstructing the Box after
        // the final non-saturated reference is released.
        let inner = unsafe { NonNull::new_unchecked(Box::into_raw(inner)) };
        Ok(Self {
            inner,
            ownership: PhantomData,
        })
    }

    /// Returns a non-owning reference-count snapshot for diagnostics.
    pub fn strong_count(&self) -> usize {
        self.inner().references.load(Ordering::Relaxed)
    }

    fn inner(&self) -> &SharedInner<T> {
        // SAFETY: Every FallibleArc clone retains one reference. The allocation
        // cannot be freed while `self` is alive, and the pointee never moves.
        unsafe { self.inner.as_ref() }
    }
}

impl<T> Clone for FallibleArc<T> {
    fn clone(&self) -> Self {
        let references = &self.inner().references;
        let mut current = references.load(Ordering::Relaxed);
        loop {
            if current == usize::MAX {
                break;
            }
            match references.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
        Self {
            inner: self.inner,
            ownership: PhantomData,
        }
    }
}

impl<T> Deref for FallibleArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T> Drop for FallibleArc<T> {
    fn drop(&mut self) {
        let references = &self.inner().references;
        let mut current = references.load(Ordering::Relaxed);
        loop {
            if current == usize::MAX || current == 0 {
                // Saturation deliberately leaks every clone. A zero count is
                // unreachable through the safe API because `self` owns one
                // reference; treating it as another leak avoids arithmetic
                // underflow if an unsafe caller has already broken that rule.
                return;
            }
            match references.compare_exchange_weak(
                current,
                current - 1,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(1) => break,
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }

        // This acquire fence pairs with every releasing decrement whose
        // reference preceded the final one, so destruction observes all writes
        // performed through those owners before they were released.
        fence(Ordering::Acquire);
        // SAFETY: This thread changed the unique final reference from one to
        // zero. A saturated counter never reaches this path, so exactly one
        // thread reconstructs and destroys the original allocation.
        unsafe { drop(Box::from_raw(self.inner.as_ptr())) };
    }
}

/// Allocates and initializes one owned value without invoking the infallible
/// allocation-error path.
pub fn try_box<T>(value: T) -> Result<Box<T>, AllocationError> {
    let layout = Layout::new::<T>();
    if layout.size() == 0 {
        // Box does not allocate storage for a zero-sized type. Using the safe
        // constructor is important here because it moves `value` into the Box;
        // reconstructing a Box from a dangling pointer would instead leave a
        // zero-sized value with Drop to be destroyed twice.
        return Ok(Box::new(value));
    }
    // SAFETY: A successful allocation has the exact layout required by T.
    let pointer =
        NonNull::new(unsafe { alloc::alloc::alloc(layout) } as *mut T).ok_or(AllocationError)?;
    // SAFETY: pointer is aligned, writable, and uniquely owned for one T.
    unsafe {
        pointer.as_ptr().write(value);
        Ok(Box::from_raw(pointer.as_ptr()))
    }
}
