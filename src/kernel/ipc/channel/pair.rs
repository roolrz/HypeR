// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ordered endpoint queues and signal reconciliation for one Channel pair.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};

use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;

use super::message::{Message, release_messages, release_messages_into};
use super::{
    ChannelError, MessageInfo, MessageSequence, PreparedMessage, ReceiveClaim, ReceivedMessage,
    Side, WriteReservation,
};
use crate::kernel::accounting::CommittedCharge;
use crate::kernel::object::{ObjectRetirement, SignalMask, SignalState};

type PairLock = InterruptSpinLock<PairState, crate::hal::irq::LocalMask>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum SideLifecycle {
    Open,
    Closing,
    Closed,
}

struct MessageQueue {
    head: Option<Box<Message>>,
    messages: u64,
    bytes: u64,
    handles: u64,
}

impl MessageQueue {
    const fn new() -> Self {
        Self {
            head: None,
            messages: 0,
            bytes: 0,
            handles: 0,
        }
    }

    fn push_back(&mut self, message: Box<Message>) {
        let mut link = &mut self.head;
        while let Some(current) = link {
            link = &mut current.next;
        }
        *link = Some(message);
    }

    fn pop_front(&mut self) -> Option<Box<Message>> {
        let mut head = self.head.take()?;
        self.head = head.take_next();
        Some(head)
    }

    fn restore_front(&mut self, mut message: Box<Message>) {
        message.replace_next(self.head.take());
        self.head = Some(message);
    }

    fn detach(&mut self) -> Option<Box<Message>> {
        self.messages = 0;
        self.bytes = 0;
        self.handles = 0;
        self.head.take()
    }
}

struct EndpointState {
    lifecycle: SideLifecycle,
    incoming: MessageQueue,
    incoming_reservations: u64,
    reserved_bytes: u64,
    reserved_handles: u64,
    receive_claimed: bool,
}

impl EndpointState {
    const fn new() -> Self {
        Self {
            lifecycle: SideLifecycle::Open,
            incoming: MessageQueue::new(),
            incoming_reservations: 0,
            reserved_bytes: 0,
            reserved_handles: 0,
            receive_claimed: false,
        }
    }
}

struct PairState {
    endpoints: [EndpointState; 2],
    next_message_sequence: u64,
}

pub(super) struct Pair {
    state: PairLock,
    signals: [SignalState; 2],
    publishing: [AtomicBool; 2],
    _charge: CommittedCharge,
}

impl Pair {
    pub(super) fn new(charge: CommittedCharge) -> Self {
        Self {
            state: PairLock::new(PairState {
                endpoints: [EndpointState::new(), EndpointState::new()],
                next_message_sequence: 1,
            }),
            signals: [
                SignalState::with_initial_level(super::ChannelEndpoint::WRITABLE),
                SignalState::with_initial_level(super::ChannelEndpoint::WRITABLE),
            ],
            publishing: [AtomicBool::new(false), AtomicBool::new(false)],
            _charge: charge,
        }
    }

    pub(super) fn signal_state(&self, side: Side) -> &SignalState {
        &self.signals[side.index()]
    }

    pub(super) fn prepare_write(
        pair: &FallibleArc<Self>,
        side: Side,
        message: &PreparedMessage,
    ) -> Result<WriteReservation, ChannelError> {
        let info = message.info();
        let sequence = pair.state.with(|state| {
            let source = &state.endpoints[side.index()];
            let target = &state.endpoints[side.peer().index()];
            if source.lifecycle != SideLifecycle::Open {
                return Err(ChannelError::EndpointClosed);
            }
            if target.lifecycle != SideLifecycle::Open {
                return Err(ChannelError::PeerClosed);
            }
            ensure_capacity(target, info)?;
            let sequence = MessageSequence::new(state.next_message_sequence)
                .ok_or(ChannelError::SequenceExhausted)?;
            state.next_message_sequence = state
                .next_message_sequence
                .checked_add(1)
                .ok_or(ChannelError::SequenceExhausted)?;
            let target = &mut state.endpoints[side.peer().index()];
            target.incoming_reservations = checked_add(target.incoming_reservations, 1);
            target.reserved_bytes = checked_add(target.reserved_bytes, info.bytes_u64());
            target.reserved_handles = checked_add(target.reserved_handles, info.handles());
            Ok(sequence)
        })?;
        pair.reconcile(side);
        Ok(WriteReservation::new(
            pair.clone(),
            side.peer(),
            sequence,
            info,
        ))
    }

    pub(super) fn publish_write(
        &self,
        target: Side,
        sequence: MessageSequence,
        expected: MessageInfo,
        prepared: PreparedMessage,
    ) {
        if !prepared.is_complete() {
            pair_invariant("Channel write committed without its capability batch");
        }
        let mut message = prepared.take();
        if message.info().sizes() != expected.sizes() {
            pair_invariant("write reservation message changed");
        }
        message.set_sequence(sequence);
        let detached = self.state.with(|state| {
            let endpoint = &mut state.endpoints[target.index()];
            release_reservation(endpoint, expected);
            endpoint.incoming.messages = checked_add(endpoint.incoming.messages, 1);
            endpoint.incoming.bytes = checked_add(endpoint.incoming.bytes, expected.bytes_u64());
            endpoint.incoming.handles = checked_add(endpoint.incoming.handles, expected.handles());
            endpoint.incoming.push_back(message);
            finalize_close(endpoint)
        });
        self.reconcile_all();
        release_messages(detached);
    }

    pub(super) fn abort_write(&self, target: Side, expected: MessageInfo) {
        let detached = self.state.with(|state| {
            let endpoint = &mut state.endpoints[target.index()];
            release_reservation(endpoint, expected);
            finalize_close(endpoint)
        });
        self.reconcile_all();
        release_messages(detached);
    }

    pub(super) fn peek(&self, side: Side) -> Result<MessageInfo, ChannelError> {
        self.state.with(|state| {
            let endpoint = &state.endpoints[side.index()];
            if endpoint.lifecycle == SideLifecycle::Closed {
                return Err(ChannelError::EndpointClosed);
            }
            if endpoint.receive_claimed {
                return Err(ChannelError::Busy);
            }
            if let Some(message) = endpoint.incoming.head.as_deref() {
                return Ok(message.info());
            }
            if state.endpoints[side.peer().index()].lifecycle == SideLifecycle::Closed {
                Err(ChannelError::PeerClosed)
            } else {
                Err(ChannelError::WouldBlock)
            }
        })
    }

    pub(super) fn claim(
        pair: &FallibleArc<Self>,
        side: Side,
        expected: MessageInfo,
    ) -> Result<ReceiveClaim, ChannelError> {
        let message = pair.state.with(|state| {
            let endpoint = &mut state.endpoints[side.index()];
            if endpoint.lifecycle == SideLifecycle::Closed {
                return Err(ChannelError::EndpointClosed);
            }
            if endpoint.receive_claimed {
                return Err(ChannelError::Busy);
            }
            let Some(head) = endpoint.incoming.head.as_deref() else {
                return Err(ChannelError::StaleMessage);
            };
            if head.info() != expected {
                return Err(ChannelError::StaleMessage);
            }
            endpoint.receive_claimed = true;
            match endpoint.incoming.pop_front() {
                Some(message) => Ok(message),
                None => pair_invariant("peeked Channel head disappeared"),
            }
        })?;
        pair.reconcile(side);
        Ok(ReceiveClaim::new(pair.clone(), side, message))
    }

    pub(super) fn commit_receive(&self, side: Side, message: Box<Message>) -> ReceivedMessage {
        let info = message.info();
        let detached = self.state.with(|state| {
            let endpoint = &mut state.endpoints[side.index()];
            if !endpoint.receive_claimed {
                pair_invariant("Channel receive commit without claim");
            }
            endpoint.receive_claimed = false;
            endpoint.incoming.messages = checked_sub(endpoint.incoming.messages, 1);
            endpoint.incoming.bytes = checked_sub(endpoint.incoming.bytes, info.bytes_u64());
            endpoint.incoming.handles = checked_sub(endpoint.incoming.handles, info.handles());
            finalize_close(endpoint)
        });
        self.reconcile_all();
        release_messages(detached);
        ReceivedMessage::new(message)
    }

    pub(super) fn abort_receive(&self, side: Side, message: Box<Message>) {
        let detached = self.state.with(|state| {
            let endpoint = &mut state.endpoints[side.index()];
            if !endpoint.receive_claimed {
                pair_invariant("Channel receive abort without claim");
            }
            endpoint.receive_claimed = false;
            endpoint.incoming.restore_front(message);
            finalize_close(endpoint)
        });
        self.reconcile_all();
        release_messages(detached);
    }

    pub(super) fn close(&self, side: Side, retirement: &mut ObjectRetirement) {
        let detached = self.state.with(|state| {
            let endpoint = &mut state.endpoints[side.index()];
            if endpoint.lifecycle != SideLifecycle::Open {
                pair_invariant("Channel endpoint closed more than once");
            }
            endpoint.lifecycle = SideLifecycle::Closing;
            finalize_close(endpoint)
        });
        self.reconcile_all();
        release_messages_into(detached, retirement);
    }

    fn reconcile_all(&self) {
        self.reconcile(Side::First);
        self.reconcile(Side::Second);
    }

    fn reconcile(&self, side: Side) {
        let publisher = &self.publishing[side.index()];
        if publisher
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        loop {
            let desired = self.state.with(|state| signal_level(state, side));
            if let Err(error) = self.signals[side.index()]
                .update(super::ChannelEndpoint::SUPPORTED_SIGNALS, desired)
            {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: Channel signal publication failed after commit: {error:?}"
                ));
            }
            publisher.store(false, Ordering::Release);

            let current = self.state.with(|state| signal_level(state, side));
            if current == desired {
                return;
            }
            if publisher
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                return;
            }
        }
    }
}

fn ensure_capacity(endpoint: &EndpointState, info: MessageInfo) -> Result<(), ChannelError> {
    if endpoint
        .incoming
        .messages
        .checked_add(endpoint.incoming_reservations)
        .and_then(|used| used.checked_add(1))
        .is_none_or(|used| used > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_QUEUED_MESSAGES)
        || endpoint
            .incoming
            .bytes
            .checked_add(endpoint.reserved_bytes)
            .and_then(|used| used.checked_add(info.bytes_u64()))
            .is_none_or(|used| used > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_QUEUED_BYTES)
        || endpoint
            .incoming
            .handles
            .checked_add(endpoint.reserved_handles)
            .and_then(|used| used.checked_add(info.handles()))
            .is_none_or(|used| used > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_QUEUED_HANDLES)
    {
        Err(ChannelError::WouldBlock)
    } else {
        Ok(())
    }
}

fn release_reservation(endpoint: &mut EndpointState, info: MessageInfo) {
    endpoint.incoming_reservations = checked_sub(endpoint.incoming_reservations, 1);
    endpoint.reserved_bytes = checked_sub(endpoint.reserved_bytes, info.bytes_u64());
    endpoint.reserved_handles = checked_sub(endpoint.reserved_handles, info.handles());
}

fn finalize_close(endpoint: &mut EndpointState) -> Option<Box<Message>> {
    if endpoint.lifecycle == SideLifecycle::Closing
        && endpoint.incoming_reservations == 0
        && !endpoint.receive_claimed
    {
        endpoint.lifecycle = SideLifecycle::Closed;
        endpoint.incoming.detach()
    } else {
        None
    }
}

fn signal_level(state: &PairState, side: Side) -> SignalMask {
    let endpoint = &state.endpoints[side.index()];
    let peer = &state.endpoints[side.peer().index()];
    let mut level = SignalMask::EMPTY;
    if endpoint.incoming.head.is_some() && !endpoint.receive_claimed {
        level =
            SignalMask::from_trusted_bits(level.bits() | super::ChannelEndpoint::READABLE.bits());
    }
    if endpoint.lifecycle == SideLifecycle::Open
        && peer.lifecycle == SideLifecycle::Open
        && ensure_capacity(peer, MessageInfo::EMPTY).is_ok()
    {
        level =
            SignalMask::from_trusted_bits(level.bits() | super::ChannelEndpoint::WRITABLE.bits());
    }
    if peer.lifecycle == SideLifecycle::Closed {
        level = SignalMask::from_trusted_bits(
            level.bits() | super::ChannelEndpoint::PEER_CLOSED.bits(),
        );
    }
    level
}

fn checked_sub(value: u64, amount: u64) -> u64 {
    match value.checked_sub(amount) {
        Some(result) => result,
        None => pair_invariant("Channel queue accounting underflow"),
    }
}

fn checked_add(value: u64, amount: u64) -> u64 {
    match value.checked_add(amount) {
        Some(result) => result,
        None => pair_invariant("Channel queue accounting overflow"),
    }
}

#[cold]
fn pair_invariant(message: &str) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {message}"))
}
