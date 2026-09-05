// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel IPC objects and message transport policy.

mod channel;
mod service;

pub(crate) use channel::{
    ChannelEndpoint, ChannelError, MessageInfo, PreparedMessage, ReceiveClaim, ReceivedMessage,
    WriteReservation,
};
pub(crate) use service::{
    ChannelReadOutcome, ChannelServiceError, ReadBuffers, channel_create, channel_read,
    channel_write,
};
