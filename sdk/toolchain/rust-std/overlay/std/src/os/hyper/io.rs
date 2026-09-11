// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ownership transfer from Native file capabilities into standard I/O.
#![stable(feature = "hyper_os", since = "1.97.1")]

use crate::sys::FromInner;

/// Constructs an I/O owner from an exclusively owned Native capability.
#[stable(feature = "hyper_os", since = "1.97.1")]
pub trait FromRawHandle {
    /// Takes ownership of a live Native File handle, initially at offset zero.
    /// The resulting file is not in append mode. Its clones share an offset.
    ///
    /// # Safety
    /// The handle must be a valid, exclusively owned File capability in this
    /// Process. No other owner may close or transfer it after this call.
    #[stable(feature = "hyper_os", since = "1.97.1")]
    unsafe fn from_raw_handle(handle: u64) -> Self;
}

#[stable(feature = "hyper_os", since = "1.97.1")]
impl FromRawHandle for crate::fs::File {
    unsafe fn from_raw_handle(handle: u64) -> Self {
        // SAFETY: The caller transfers the unique capability owner to std.
        Self::from_inner(unsafe { crate::sys::fs::File::from_raw_handle(handle) })
    }
}
