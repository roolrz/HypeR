// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallible ownership primitives built on the installed global allocator.

use alloc::boxed::Box;
use core::alloc::Layout;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering, fence};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationError;

#[repr(C)]
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

/// Unique owner which retains a `FallibleArc` allocation without refcounting.
///
/// This is the linear teardown form for large hardware owners: conversion is
/// allocation-free, the value remains pinned, and sharing can be restored
/// without moving the payload.
pub struct UniqueFallibleArc<T> {
    inner: NonNull<SharedInner<T>>,
    ownership: PhantomData<SharedInner<T>>,
}

// SAFETY: UniqueFallibleArc has exclusive ownership, so moving the owner
// across CPUs is sound exactly when moving T is sound.
unsafe impl<T: Send> Send for UniqueFallibleArc<T> {}
// SAFETY: Shared references to the uniquely owned value may cross CPUs exactly
// when shared references to T may cross CPUs.
unsafe impl<T: Sync> Sync for UniqueFallibleArc<T> {}

// SAFETY: FallibleArc grants shared access to T from every clone. Moving or
// sharing that access across CPUs is sound exactly when T is Send + Sync.
unsafe impl<T: Send + Sync> Send for FallibleArc<T> {}
// SAFETY: See the Send implementation. Reference-count changes are atomic and
// the contained value is exposed only through shared references.
unsafe impl<T: Send + Sync> Sync for FallibleArc<T> {}

impl<T> FallibleArc<T> {
    /// Returns the allocator-requested size of one shared allocation.
    pub const fn allocation_size() -> usize {
        core::mem::size_of::<SharedInner<T>>()
    }

    /// Allocates one shared owner without invoking the allocation-error path.
    pub fn try_new(value: T) -> Result<Self, AllocationError> {
        Self::try_new_or_return(value).map_err(|(error, _)| error)
    }

    /// Allocates one shared owner while preserving `value` on failure.
    pub fn try_new_or_return(value: T) -> Result<Self, (AllocationError, T)> {
        let inner = try_box_or_return(SharedInner {
            references: AtomicUsize::new(1),
            value,
        })
        .map_err(|(error, inner)| (error, inner.value))?;
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

    /// Extracts the value when this is the unique, non-saturated owner.
    ///
    /// Failure returns the original owner unchanged. This is the retirement
    /// boundary for resources which may be shared while active but require a
    /// linear owner for final hardware teardown.
    pub fn try_unwrap(self) -> Result<T, Self> {
        self.try_into_unique().map(UniqueFallibleArc::into_inner)
    }

    /// Converts the sole shared reference into a pinned linear owner.
    pub fn try_into_unique(self) -> Result<UniqueFallibleArc<T>, Self> {
        if self
            .inner()
            .references
            .compare_exchange(1, 0, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(self);
        }
        let owner = ManuallyDrop::new(self);
        Ok(UniqueFallibleArc {
            inner: owner.inner,
            ownership: PhantomData,
        })
    }

    fn inner(&self) -> &SharedInner<T> {
        // SAFETY: Every FallibleArc clone retains one reference. The allocation
        // cannot be freed while `self` is alive, and the pointee never moves.
        unsafe { self.inner.as_ref() }
    }
}

impl<T> UniqueFallibleArc<T> {
    pub const fn allocation_size() -> usize {
        core::mem::size_of::<SharedInner<T>>()
    }

    /// Restores one shared reference without allocating or moving `T`.
    pub fn into_shared(self) -> FallibleArc<T> {
        let owner = ManuallyDrop::new(self);
        // No FallibleArc exists while the unique token is live. Release makes
        // all unique-owner mutation visible before future Arc clones.
        // SAFETY: The unique owner keeps the allocation live and excludes
        // concurrent access to the zero-valued reference counter.
        unsafe { owner.inner.as_ref() }
            .references
            .store(1, Ordering::Release);
        FallibleArc {
            inner: owner.inner,
            ownership: PhantomData,
        }
    }

    /// Extracts the value and releases the retained shared allocation.
    pub fn into_inner(self) -> T {
        let owner = ManuallyDrop::new(self);
        // SAFETY: A UniqueFallibleArc is the only owner and keeps the count at
        // zero, so exactly this operation may reconstruct the allocation.
        let inner = unsafe { Box::from_raw(owner.inner.as_ptr()) };
        let SharedInner { value, .. } = *inner;
        value
    }
}

impl<T> UniqueFallibleArc<MaybeUninit<T>> {
    /// Allocates a pinned linear slot before fallible hardware publication.
    pub fn try_new_uninit() -> Result<Self, AllocationError> {
        FallibleArc::try_new(MaybeUninit::uninit()).map(|owner| {
            // The newly created Arc is uniquely owned by construction.
            match owner.try_into_unique() {
                Ok(unique) => unique,
                Err(_) => unreachable_unique_creation(),
            }
        })
    }

    /// Initializes the retained slot without moving the resulting owner.
    pub fn write(self, value: T) -> UniqueFallibleArc<T> {
        let owner = ManuallyDrop::new(self);
        // SAFETY: MaybeUninit<T> has the same layout as T. This unique owner
        // grants exclusive access to the value field, which is written once.
        unsafe {
            core::ptr::addr_of_mut!((*owner.inner.as_ptr()).value).write(MaybeUninit::new(value));
        }
        UniqueFallibleArc {
            inner: owner.inner.cast(),
            ownership: PhantomData,
        }
    }
}

impl<T> Deref for UniqueFallibleArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The unique allocation stays live and initialized.
        unsafe { &self.inner.as_ref().value }
    }
}

impl<T> DerefMut for UniqueFallibleArc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: The linear owner excludes every FallibleArc and other unique
        // owner, so it grants exclusive access to the pinned value.
        unsafe { &mut self.inner.as_mut().value }
    }
}

impl<T> Drop for UniqueFallibleArc<T> {
    fn drop(&mut self) {
        // SAFETY: Unique ownership and the zero reference count prove this is
        // the only destructor which can reconstruct the allocation.
        unsafe { drop(Box::from_raw(self.inner.as_ptr())) };
    }
}

#[cold]
fn unreachable_unique_creation() -> ! {
    // A newly allocated FallibleArc has exactly one reference. Keep this
    // dependency-free primitive fail-stop if its own constructor is violated.
    loop {
        core::hint::spin_loop();
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
    try_box_or_return(value).map_err(|(error, _)| error)
}

/// Allocates one owned value while preserving it on allocation failure.
pub fn try_box_or_return<T>(value: T) -> Result<Box<T>, (AllocationError, T)> {
    let layout = Layout::new::<T>();
    if layout.size() == 0 {
        // Box does not allocate storage for a zero-sized type. Using the safe
        // constructor is important here because it moves `value` into the Box;
        // reconstructing a Box from a dangling pointer would instead leave a
        // zero-sized value with Drop to be destroyed twice.
        return Ok(Box::new(value));
    }
    // SAFETY: A successful allocation has the exact layout required by T.
    let Some(pointer) = NonNull::new(unsafe { alloc::alloc::alloc(layout) } as *mut T) else {
        return Err((AllocationError, value));
    };
    // SAFETY: pointer is aligned, writable, and uniquely owned for one T.
    unsafe {
        pointer.as_ptr().write(value);
        Ok(Box::from_raw(pointer.as_ptr()))
    }
}
