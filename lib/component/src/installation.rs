// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Private system-composition endpoint. Only the designated startup coordinator
//! receives it. Ordinary component connectors cannot install grants or builders.

use crate::objects::Object;
use core::mem::MaybeUninit;
use hyper_os::{Error, Result, capability_channel::*, handle::*};
use serde::{Deserialize, Serialize};

pub const INSTALLER: hyper_os::startup::StartupPurpose<CapabilityChannelObject> =
    hyper_os::startup::StartupPurpose::new(0x800c_0020);
pub const MAX_BYTES: usize = 4096;
pub const MAX_GRANTS: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    /// Exactly Builder and `TaskGroup`; builder remains unsealed so the manager
    /// can add its private connector and arm bounded diagnostics. The source
    /// factory must authorize retention. Ownership moves even if activation fails.
    Prepared {
        path: String,
    },
    /// Install one declared realm-owned object. Its declaration determines
    /// whether consumers receive duplicates or consume the only handle.
    Export {
        realm: String,
        name: String,
        kind: u32,
        rights: u64,
    },
    Progress {
        milestone: String,
    },
    /// End the finite startup status report; ordinary logs remain independent.
    FinishDisplay {
        success: bool,
    },
    Management,
    Logging,
    Abort {
        path: String,
    },
}

#[derive(Deserialize, Serialize)]
pub struct Reply {
    pub error: Option<String>,
}

pub struct Call {
    pub request: Request,
    pub objects: Vec<Object>,
    pub reply: CapabilityChannel,
    pub deadline: u64,
}

pub fn prepared(
    connector: HandleRef<'_, CapabilityChannelObject>,
    path: &str,
    builder: hyper_os::task::ProcessBuilder,
    group: OwnedHandle<TaskGroupObject>,
) -> Result<OwnedHandle<ProcessObject>> {
    let builder = builder.into_handle().map_err(|f| f.error())?;
    let mut objects = [Object::Builder(Some(builder)), Object::Group(Some(group))];
    let mut result = transact(
        connector,
        &Request::Prepared { path: path.into() },
        &mut objects,
    )?;
    match result.pop() {
        Some(Object::Process(Some(p))) => Ok(p),
        _ => Err(Error::MissingHandle),
    }
}

pub fn export(
    connector: HandleRef<'_, CapabilityChannelObject>,
    realm: &str,
    name: &str,
    rights: Rights,
    object: &mut Object,
) -> Result<()> {
    let kind = object.info()?.kind.as_raw();
    transact(
        connector,
        &Request::Export {
            realm: realm.into(),
            name: name.into(),
            kind,
            rights: rights.bits(),
        },
        core::slice::from_mut(object),
    )
    .map(|_| ())
}

pub fn progress(connector: HandleRef<'_, CapabilityChannelObject>, milestone: &str) -> Result<()> {
    transact(
        connector,
        &Request::Progress {
            milestone: milestone.into(),
        },
        &mut [],
    )
    .map(|_| ())
}

pub fn management(
    connector: HandleRef<'_, CapabilityChannelObject>,
) -> Result<(
    OwnedHandle<CapabilityChannelObject>,
    OwnedHandle<CapabilityChannelObject>,
)> {
    let mut reply = transact(connector, &Request::Management, &mut [])?;
    let Some(Object::CapabilityChannel(Some(control))) = reply.pop() else {
        return Err(Error::MissingHandle);
    };
    let Some(Object::CapabilityChannel(Some(observe))) = reply.pop() else {
        return Err(Error::MissingHandle);
    };
    Ok((observe, control))
}

pub fn abort(connector: HandleRef<'_, CapabilityChannelObject>, path: &str) -> Result<()> {
    transact(connector, &Request::Abort { path: path.into() }, &mut []).map(|_| ())
}

fn transact(
    connector: HandleRef<'_, CapabilityChannelObject>,
    request: &Request,
    objects: &mut [Object],
) -> Result<Vec<Object>> {
    let bytes = serde_json::to_vec(request).map_err(|_| Error::InvalidResponse)?;
    if bytes.len() > MAX_BYTES || objects.len() > MAX_GRANTS {
        return Err(Error::InvalidResponse);
    }
    let until = hyper_os::time::deadline_after(std::time::Duration::from_secs(60))?.as_raw();
    let listener =
        CapabilityChannel::from_handle(connector.duplicate(hyper_service_ipc::CLIENT_RIGHTS)?);
    let (local, remote) = CapabilityChannel::create()?;
    let mut remote = Some(remote.into_handle());
    hyper_service_ipc::send(
        &listener,
        until,
        &bytes,
        &mut [CapabilityDisposition::move_handle(
            &mut remote,
            RightsOffer::Exact(crate::connection::SESSION_RIGHTS),
        )?],
    )?;
    let descriptors = descriptors(request)?;
    if descriptors.len() != objects.len() {
        return Err(Error::InvalidResponse);
    }
    let mut offers = Vec::new();
    for (object, (_, rights)) in objects.iter_mut().zip(descriptors) {
        offers.push(object.move_offer(rights)?);
    }
    hyper_service_ipc::send(&local, until, b"HYPER-COMPONENT-INSTALL/1", &mut offers)?;
    drop(offers);
    let mut bytes = [MaybeUninit::uninit(); 1024];
    let expected = match request {
        Request::Prepared { .. } => vec![(
            ProcessObject::KIND.as_raw(),
            Rights::WAIT.union(Rights::TRANSFER),
        )],
        Request::Management => vec![
            (
                CapabilityChannelObject::KIND.as_raw(),
                hyper_service_ipc::CLIENT_RIGHTS
            );
            2
        ],
        _ => Vec::new(),
    };
    let mut slots = expected
        .iter()
        .map(|(k, r)| Object::receive_slot(*k, *r))
        .collect::<Result<Vec<_>>>()?;
    let message = local.receive(until, &mut bytes, &mut slots)?;
    let reply: Reply =
        serde_json::from_slice(message.bytes()).map_err(|_| Error::InvalidResponse)?;
    if let Some(error) = reply.error {
        eprintln!("component installation rejected: {error}");
        return Err(Error::Status(hyper_os::Status::BAD_STATE));
    }
    if message.capability_count() != expected.len() {
        return Err(Error::InvalidResponse);
    }
    expected
        .iter()
        .zip(&mut slots)
        .map(|((kind, _), slot)| Object::take(*kind, slot))
        .collect()
}

pub fn descriptors(request: &Request) -> Result<Vec<(u32, Rights)>> {
    Ok(match request {
        Request::Prepared { .. } => vec![
            (
                ProcessBuilderObject::KIND.as_raw(),
                Rights::TRANSFER
                    .union(Rights::INSPECT)
                    .union(Rights::WRITE)
                    .union(Rights::START)
                    .union(Rights::REQUEST_STOP)
                    .union(Rights::RETAIN_PROCESS),
            ),
            (
                TaskGroupObject::KIND.as_raw(),
                Rights::TRANSFER
                    .union(Rights::WAIT)
                    .union(Rights::INSPECT)
                    .union(Rights::REQUEST_STOP),
            ),
        ],
        Request::Export { kind, rights, .. } => vec![(
            *kind,
            Rights::from_bits(*rights).ok_or(Error::InvalidResponse)?,
        )],
        Request::Logging => vec![(
            CapabilityChannelObject::KIND.as_raw(),
            hyper_service_ipc::CLIENT_RIGHTS,
        )],
        Request::Progress { .. }
        | Request::FinishDisplay { .. }
        | Request::Management
        | Request::Abort { .. } => Vec::new(),
    })
}

/// This worker may block on a malformed coordinator, but never the lifecycle
/// reactor. The endpoint is not delegated to arbitrary components.
pub fn receive(listener: &CapabilityChannel) -> Result<Call> {
    let mut bytes = [MaybeUninit::uninit(); MAX_BYTES];
    let mut slots = [CapabilityReceiveSlot::new::<CapabilityChannelObject>(
        crate::connection::SESSION_RIGHTS,
    )];
    let message = listener.receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)?;
    if message.capability_count() != 1 {
        return Err(Error::InvalidResponse);
    }
    let request: Request =
        serde_json::from_slice(message.bytes()).map_err(|_| Error::InvalidResponse)?;
    let reply = CapabilityChannel::from_handle(slots[0].take()?.ok_or(Error::MissingHandle)?);
    let deadline = hyper_os::time::deadline_after(std::time::Duration::from_secs(60))?.as_raw();
    let descriptors = descriptors(&request)?;
    let mut slots: Vec<_> = descriptors
        .iter()
        .map(|(kind, rights)| Object::receive_slot(*kind, *rights))
        .collect::<Result<_>>()?;
    let mut bytes = [MaybeUninit::uninit(); 64];
    let message = reply.receive(deadline, &mut bytes, &mut slots)?;
    if message.bytes() != b"HYPER-COMPONENT-INSTALL/1"
        || message.capability_count() != descriptors.len()
    {
        return Err(Error::InvalidResponse);
    }
    let objects = descriptors
        .iter()
        .zip(&mut slots)
        .map(|((kind, _), slot)| Object::take(*kind, slot))
        .collect::<Result<_>>()?;
    Ok(Call {
        request,
        objects,
        reply,
        deadline,
    })
}

pub fn logging(
    connector: HandleRef<'_, CapabilityChannelObject>,
    log: HandleRef<'_, CapabilityChannelObject>,
) -> Result<()> {
    let mut objects = [Object::CapabilityChannel(Some(
        log.duplicate(hyper_service_ipc::CLIENT_RIGHTS)?,
    ))];
    transact(connector, &Request::Logging, &mut objects).map(|_| ())
}

pub fn finish_display(
    connector: HandleRef<'_, CapabilityChannelObject>,
    success: bool,
) -> Result<()> {
    transact(connector, &Request::FinishDisplay { success }, &mut []).map(|_| ())
}
