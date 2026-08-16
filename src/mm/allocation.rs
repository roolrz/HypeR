//! Fallible ownership primitives built on the installed global allocator.

use alloc::boxed::Box;
use core::alloc::Layout;
use core::ptr::NonNull;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationError;

/// Allocates and initializes one owned value without invoking the infallible
/// allocation-error path.
pub fn try_box<T>(value: T) -> Result<Box<T>, AllocationError> {
    let layout = Layout::new::<T>();
    if layout.size() == 0 {
        let pointer = NonNull::<T>::dangling().as_ptr();
        // SAFETY: A dangling aligned pointer is the canonical Box
        // representation for a zero-sized value and is never dereferenced.
        return Ok(unsafe { Box::from_raw(pointer) });
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
