// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Typed, bounded multi-object waits for Native applications.

use core::marker::PhantomData;

use crate::handle::{
    ByteChannelObject, CapabilityChannelObject, HandleRef, ObjectType, ProcessObject,
};
use crate::{Error, Result, Status};

const _: () = assert!(hyper_abi::HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS <= usize::MAX as u64);

/// Maximum number of objects accepted by one multi-object wait.
pub const MAX_ITEMS: usize = hyper_abi::HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS as usize;

/// A signal mask whose object kind is carried by the type system.
#[derive(Debug, Eq, PartialEq)]
pub struct ObjectSignals<T: ObjectType> {
    bits: u64,
    _object: PhantomData<fn() -> T>,
}

impl<T: ObjectType> Clone for ObjectSignals<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ObjectType> Copy for ObjectSignals<T> {}

impl<T: ObjectType> ObjectSignals<T> {
    const fn from_trusted_bits(bits: u64) -> Self {
        Self {
            bits,
            _object: PhantomData,
        }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self::from_trusted_bits(self.bits | other.bits)
    }

    /// Returns whether any bit in this typed mask was observed.
    #[must_use]
    pub const fn is_present_in(self, observed: u64) -> bool {
        observed & self.bits != 0
    }
}

impl ObjectSignals<ByteChannelObject> {
    pub const READABLE: Self =
        Self::from_trusted_bits(hyper_abi::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE);
    pub const WRITABLE: Self =
        Self::from_trusted_bits(hyper_abi::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE);
    pub const PEER_CLOSED: Self =
        Self::from_trusted_bits(hyper_abi::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED);
}

impl ObjectSignals<CapabilityChannelObject> {
    pub const PEER_RECEIVING: Self =
        Self::from_trusted_bits(hyper_abi::HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_RECEIVING);
    pub const PEER_CLOSED: Self =
        Self::from_trusted_bits(hyper_abi::HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_CLOSED);
}

impl ObjectSignals<ProcessObject> {
    pub const TERMINATED: Self =
        Self::from_trusted_bits(hyper_abi::HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED);
}

/// One ordered borrowed handle and its typed signal mask.
///
/// The lifetime prevents the owning handle from being consumed while the
/// syscall borrows its process-local value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WaitItem<'owner> {
    handle: u64,
    signals: u64,
    _owner: PhantomData<HandleRef<'owner>>,
}

const _: () = assert!(
    core::mem::size_of::<WaitItem<'static>>()
        == core::mem::size_of::<hyper_abi::HyperNativeObjectWaitItem>()
);
const _: () = assert!(
    core::mem::align_of::<WaitItem<'static>>()
        == core::mem::align_of::<hyper_abi::HyperNativeObjectWaitItem>()
);

impl<'owner> WaitItem<'owner> {
    #[must_use]
    pub fn new<T: ObjectType>(handle: HandleRef<'owner, T>, signals: ObjectSignals<T>) -> Self {
        Self {
            handle: handle.raw().get(),
            signals: signals.bits,
            _owner: PhantomData,
        }
    }
}

/// The ordered item selected by a successful multi-object wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaitObservation {
    pub index: usize,
    pub observed: u64,
}

/// Blocks until one requested level is present, the deadline expires, or the
/// calling Thread is cancelled.
pub fn wait_many(items: &[WaitItem<'_>], deadline: u64) -> Result<WaitObservation> {
    if items.is_empty() || items.len() > MAX_ITEMS {
        return Err(Error::InvalidWaitSet);
    }
    // SAFETY: `WaitItem` has the asserted ABI layout, every item retains a
    // borrow of its owner, and the slice remains readable for the whole call.
    let result = unsafe {
        hyper_sys::object_wait_many(
            items
                .as_ptr()
                .cast::<hyper_abi::HyperNativeObjectWaitItem>(),
            items.len(),
            deadline,
        )
    };
    Status::from_raw(result.status).into_result()?;
    let index = usize::try_from(result.value0).map_err(|_| Error::InvalidResponse)?;
    if index >= items.len() || result.value1 == 0 {
        return Err(Error::InvalidResponse);
    }
    Ok(WaitObservation {
        index,
        observed: result.value1,
    })
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use super::{ObjectSignals, WaitItem};
    use crate::handle::{ByteChannelObject, OwnedHandle};

    #[test]
    fn wait_item_preserves_handle_and_signal_bits() -> crate::Result<()> {
        let raw = NonZeroU64::new(0x100_0001).ok_or(crate::Error::InvalidResponse)?;
        // SAFETY: the test creates one synthetic exclusive SDK owner.
        let owner = unsafe { OwnedHandle::<ByteChannelObject>::from_raw_owned(raw) };
        let item = WaitItem::new(
            owner.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        );
        assert_eq!(item.handle, raw.get());
        assert_eq!(
            item.signals,
            hyper_abi::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE
                | hyper_abi::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED
        );
        Ok(())
    }
}
