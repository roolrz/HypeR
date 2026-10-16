// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::mem::MaybeUninit;
use hyper_os::{
    Error, Result, Status, capability_channel::*, channel::create_pair, handle::*, wait::*,
};
use serde::{Deserialize, Serialize};

pub const SESSION_RIGHTS: Rights = Rights::READ
    .union(Rights::WRITE)
    .union(Rights::WAIT)
    .union(Rights::TRANSFER)
    .union(Rights::DUPLICATE);
pub const REPLY_RIGHTS: Rights = Rights::WRITE.union(Rights::WAIT);
pub const MAX_MESSAGE: usize = 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Connect {
        name: String,
        deadline_ns: u64,
    },
    Publish {
        name: String,
    },
    Status {
        message: String,
    },
    /// A supervised child or a local service subsystem failed. The manager
    /// supplies the authenticated parent identity and reporting policy.
    ReportFailure {
        message: String,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub error: Option<Rejection>,
}

/// Routing failures describe the requested service, independently of transport
/// failure. Restarting a provider never reconnects an existing session.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RejectionKind {
    NotOffered,
    InvalidRequest,
    Unavailable,
    Restarting,
    TimedOut,
    ResourceLimit,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rejection {
    pub kind: RejectionKind,
    pub message: String,
}

impl Rejection {
    pub fn new(kind: RejectionKind, message: impl AsRef<str>) -> Self {
        Self {
            kind,
            message: message.as_ref().chars().take(256).collect(),
        }
    }

    pub const fn status(&self) -> Status {
        match self.kind {
            RejectionKind::NotOffered => Status::ACCESS_DENIED,
            RejectionKind::InvalidRequest => Status::INVALID_ARGUMENT,
            RejectionKind::Unavailable => Status::PEER_CLOSED,
            RejectionKind::Restarting => Status::BUSY,
            RejectionKind::TimedOut => Status::TIMED_OUT,
            RejectionKind::ResourceLimit => Status::RESOURCE_LIMIT,
        }
    }
}

#[derive(Debug)]
pub enum ConnectionError {
    Transport(Error),
    Rejected(Rejection),
}

impl From<Error> for ConnectionError {
    fn from(error: Error) -> Self {
        Self::Transport(error)
    }
}

impl From<ConnectionError> for Error {
    fn from(error: ConnectionError) -> Self {
        match error {
            ConnectionError::Transport(error) => error,
            ConnectionError::Rejected(error) => Error::Status(error.status()),
        }
    }
}

impl core::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "component transport: {error}"),
            Self::Rejected(error) => write!(f, "component {:?}: {}", error.kind, error.message),
        }
    }
}

impl std::error::Error for ConnectionError {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub capability: String,
}

/// Establish a new session. Successful return means the server endpoint was
/// delivered, not that any application-level request has already completed.
pub fn connect(
    connector: HandleRef<'_, CapabilityChannelObject>,
    name: &str,
    until: u64,
) -> core::result::Result<CapabilityChannel, ConnectionError> {
    let (client, server) = CapabilityChannel::create()?;
    transact(
        connector,
        &Request::Connect {
            name: name.into(),
            deadline_ns: until,
        },
        Some(server.into_handle()),
        until,
    )?;
    Ok(client)
}

pub fn publish(connector: HandleRef<'_, CapabilityChannelObject>, name: &str) -> Result<()> {
    transact(
        connector,
        &Request::Publish { name: name.into() },
        None,
        hyper_service_ipc::deadline()?,
    )
    .map_err(Into::into)
}

pub fn status(connector: HandleRef<'_, CapabilityChannelObject>, message: &str) -> Result<()> {
    transact(
        connector,
        &Request::Status {
            message: message.into(),
        },
        None,
        hyper_service_ipc::deadline()?,
    )
    .map_err(Into::into)
}

/// Report a delegated failure without relying on journal or a surviving child.
/// Bounded waiting keeps an unavailable manager out of resource retirement.
pub fn report_failure(
    connector: HandleRef<'_, CapabilityChannelObject>,
    message: &str,
) -> Result<()> {
    transact(
        connector,
        &Request::ReportFailure {
            message: message.chars().take(256).collect(),
        },
        None,
        hyper_os::time::deadline_after(std::time::Duration::from_millis(500))?.as_raw(),
    )
    .map_err(Into::into)
}

fn transact(
    connector: HandleRef<'_, CapabilityChannelObject>,
    request: &Request,
    mut session: Option<OwnedHandle<CapabilityChannelObject>>,
    until: u64,
) -> core::result::Result<(), ConnectionError> {
    let bytes = serde_json::to_vec(request).map_err(|_| Error::InvalidResponse)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::InvalidResponse.into());
    }
    let channel =
        CapabilityChannel::from_handle(connector.duplicate(hyper_service_ipc::CLIENT_RIGHTS)?);
    let (reader, writer) = create_pair()?;
    let mut writer = Some(writer);
    let mut offers = vec![CapabilityDisposition::move_handle(
        &mut writer,
        RightsOffer::Exact(REPLY_RIGHTS),
    )?];
    if session.is_some() {
        offers.push(CapabilityDisposition::move_handle(
            &mut session,
            RightsOffer::Exact(SESSION_RIGHTS),
        )?);
    }
    hyper_service_ipc::send(&channel, until, &bytes, &mut offers)?;
    drop(offers);
    wait_many(
        &[WaitItem::new(
            reader.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )],
        until,
    )?;
    let mut bytes = [0; MAX_MESSAGE];
    let n = reader.as_byte_channel().try_receive(&mut bytes)?;
    let reply: Reply = serde_json::from_slice(&bytes[..n]).map_err(|_| Error::InvalidResponse)?;
    if let Some(error) = reply.error {
        return Err(ConnectionError::Rejected(error));
    }
    Ok(())
}

pub struct Call {
    pub request: Request,
    pub reply: OwnedHandle<ByteChannelObject>,
    pub session: Option<OwnedHandle<CapabilityChannelObject>>,
}

/// Receive on the caller's private endpoint. No caller-selected instance ID is
/// accepted, and every request's reply storage is allocated by that caller.
pub fn receive(connector: &CapabilityChannel, until: u64) -> Result<Call> {
    let mut bytes = [MaybeUninit::uninit(); MAX_MESSAGE];
    let mut slots = [
        CapabilityReceiveSlot::new::<ByteChannelObject>(REPLY_RIGHTS),
        CapabilityReceiveSlot::new::<CapabilityChannelObject>(SESSION_RIGHTS),
    ];
    let message = connector.receive(until, &mut bytes, &mut slots)?;
    let request: Request =
        serde_json::from_slice(message.bytes()).map_err(|_| Error::InvalidResponse)?;
    let expected = if matches!(request, Request::Connect { .. }) {
        2
    } else {
        1
    };
    if message.capability_count() != expected {
        return Err(Error::InvalidResponse);
    }
    Ok(Call {
        request,
        reply: slots[0].take()?.ok_or(Error::MissingHandle)?,
        session: slots[1].take()?,
    })
}

/// Small replies never block the reactor. A failed send closes the reply owner;
/// the caller then observes peer closure rather than an unbounded wait.
pub fn respond(reply: OwnedHandle<ByteChannelObject>, error: Option<Rejection>) {
    if let Ok(bytes) = serde_json::to_vec(&Reply { error }) {
        let _ = reply.as_byte_channel().try_send(&bytes);
    }
}

/// Providers receive independent session endpoints, then speak their own wire
/// protocol directly. The manager neither parses nor forwards that protocol.
pub fn accept(outgoing: &CapabilityChannel, until: u64) -> Result<(String, CapabilityChannel)> {
    let mut bytes = [MaybeUninit::uninit(); MAX_MESSAGE];
    let mut slots = [CapabilityReceiveSlot::new::<CapabilityChannelObject>(
        SESSION_RIGHTS,
    )];
    let message = outgoing.receive(until, &mut bytes, &mut slots)?;
    if message.capability_count() != 1 {
        return Err(Error::InvalidResponse);
    }
    let delivery: Delivery =
        serde_json::from_slice(message.bytes()).map_err(|_| Error::InvalidResponse)?;
    Ok((
        delivery.capability,
        CapabilityChannel::from_handle(slots[0].take()?.ok_or(Error::MissingHandle)?),
    ))
}
