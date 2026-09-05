// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallibly prepared Channel messages with sender-sponsored accounting.

use alloc::boxed::Box;
use alloc::vec::Vec;

use hyper::mm::try_box;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::capability::InTransitCapabilities;
use crate::kernel::object::ObjectRetirement;

use super::{ChannelError, MessageInfo, MessageSequence};

pub(super) struct Message {
    sequence: MessageSequence,
    bytes: Vec<u8>,
    handle_count: u64,
    capabilities: Option<InTransitCapabilities>,
    pub(super) next: Option<Box<Message>>,
    _charge: CommittedCharge,
}

impl Message {
    pub(super) fn info(&self) -> MessageInfo {
        MessageInfo::new(self.sequence, self.bytes.len(), self.handle_count)
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn set_sequence(&mut self, sequence: MessageSequence) {
        self.sequence = sequence;
    }

    pub(super) fn take_next(&mut self) -> Option<Box<Self>> {
        self.next.take()
    }

    pub(super) fn replace_next(&mut self, next: Option<Box<Self>>) {
        self.next = next;
    }

    pub(super) fn take_capabilities(&mut self) -> Option<InTransitCapabilities> {
        self.capabilities.take()
    }

    pub(super) fn restore_capabilities(&mut self, capabilities: InTransitCapabilities) {
        let count = match u64::try_from(capabilities.len()) {
            Ok(count) => count,
            Err(_) => message_invariant(),
        };
        if count == 0 || count != self.handle_count || self.capabilities.is_some() {
            message_invariant();
        }
        self.capabilities = Some(capabilities);
    }

    pub(super) fn has_capabilities(&self) -> bool {
        self.capabilities.is_some()
    }
}

/// Complete message storage prepared before any Channel lock is acquired.
#[must_use = "publish the prepared message or release its sponsored resources"]
pub(crate) struct PreparedMessage {
    message: Option<Box<Message>>,
}

impl PreparedMessage {
    /// Allocates sender-sponsored storage before source capabilities are claimed.
    ///
    /// The byte buffer is zeroed so a caller can copy directly from user memory
    /// without first allocating an unaccounted staging buffer.
    pub(crate) fn try_new(
        domain: &ResourceDomain,
        byte_count: usize,
        handle_count: u64,
    ) -> Result<Self, ChannelError> {
        let charged_byte_count =
            u64::try_from(byte_count).map_err(|_| ChannelError::MessageTooLarge)?;
        if charged_byte_count > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES {
            return Err(ChannelError::MessageTooLarge);
        }
        if handle_count > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES {
            return Err(ChannelError::MessageTooLarge);
        }
        let node_bytes = u64::try_from(core::mem::size_of::<Message>())
            .map_err(|_| ChannelError::AllocationSize)?;
        let kernel_bytes = node_bytes
            .checked_add(charged_byte_count)
            .ok_or(ChannelError::AllocationSize)?;
        let charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, kernel_bytes)
                    .with(ResourceKind::IpcMessages, 1)
                    .with(ResourceKind::IpcBytes, charged_byte_count)
                    .with(ResourceKind::IpcHandles, handle_count),
            )?
            .commit();

        let mut owned = Vec::new();
        owned
            .try_reserve_exact(byte_count)
            .map_err(|_| ChannelError::Allocation)?;
        owned.resize(byte_count, 0);
        let message = try_box(Message {
            sequence: MessageSequence::UNASSIGNED,
            bytes: owned,
            handle_count,
            capabilities: None,
            next: None,
            _charge: charge,
        })
        .map_err(|_| ChannelError::Allocation)?;
        Ok(Self {
            message: Some(message),
        })
    }

    /// Test and kernel-internal convenience for copying an existing byte slice.
    pub(crate) fn try_copy_from(
        domain: &ResourceDomain,
        bytes: &[u8],
        handle_count: u64,
    ) -> Result<Self, ChannelError> {
        let mut prepared = Self::try_new(domain, bytes.len(), handle_count)?;
        prepared.bytes_mut().copy_from_slice(bytes);
        Ok(prepared)
    }

    /// Returns the exclusively owned staging buffer before queue reservation.
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        match self.message.as_deref_mut() {
            Some(message) => &mut message.bytes,
            None => message_invariant(),
        }
    }

    pub(crate) fn info(&self) -> MessageInfo {
        self.message().info()
    }

    /// Installs an already-committed capability batch without allocation.
    pub(crate) fn attach_handles(&mut self, capabilities: InTransitCapabilities) {
        let message = match self.message.as_deref_mut() {
            Some(message) => message,
            None => message_invariant(),
        };
        message.restore_capabilities(capabilities);
    }

    pub(super) fn is_complete(&self) -> bool {
        let message = self.message();
        (message.handle_count == 0) != message.has_capabilities()
    }

    fn message(&self) -> &Message {
        match self.message.as_deref() {
            Some(message) => message,
            None => message_invariant(),
        }
    }

    pub(super) fn take(mut self) -> Box<Message> {
        match self.message.take() {
            Some(message) => message,
            None => message_invariant(),
        }
    }
}

impl Drop for PreparedMessage {
    fn drop(&mut self) {
        // A prepared message remains ordinary local ownership. Dropping it is
        // the allocation-free transaction abort and releases its charge.
        release_messages(self.message.take());
    }
}

/// Iteratively releases a detached queue without retaining object locks or
/// recursively destroying the queue's linked-list spine.
pub(super) fn release_messages(mut current: Option<Box<Message>>) {
    let mut retirement = ObjectRetirement::new();
    release_messages_into(current.take(), &mut retirement);
    retirement.drain();
}

/// Releases a detached queue into an enclosing object-retirement transaction.
pub(super) fn release_messages_into(
    mut current: Option<Box<Message>>,
    retirement: &mut ObjectRetirement,
) {
    while let Some(mut message) = current {
        current = message.take_next();
        if let Some(capabilities) = message.take_capabilities() {
            capabilities.release_into(retirement);
        }
        drop(message);
    }
}

impl From<ResourceError> for ChannelError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

#[cold]
fn message_invariant() -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: Channel message invariant failed"))
}
