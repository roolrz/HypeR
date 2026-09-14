// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit startup authority for the trusted physical I/O VM owner.

use crate::{StartupContract, vm};
use hyper_os::{handle::Rights, startup};

pub const DEVICE_AUTHORITY_NAME: &str = "io.device-authority";
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    vm::MANAGER_CREATION_AUTHORITY_CONTRACT,
    READY_CONTRACT,
    BROKER_SERVER_CONTRACT,
    StartupContract::exact(
        DEVICE_AUTHORITY_NAME,
        startup::DEVICE_ASSIGNMENT_AUTHORITY,
        Rights::INSPECT,
    ),
];

/// Explicit one-shot notification that HypeR's configured data volume is mounted.
pub const READY_NAME: &str = "io.ready";
pub const READY: startup::StartupPurpose<hyper_os::handle::ByteChannelObject> =
    startup::StartupPurpose::new(0x8006_0001);
pub const READY_CONTRACT: StartupContract =
    StartupContract::exact(READY_NAME, READY, Rights::WAIT.union(Rights::WRITE));
pub const READY_MESSAGE: &[u8] = b"HYPER-IO-READY/1\n";
pub const READY_TIMEOUT_SECONDS: u64 = 120;

/// The trusted manager delegates individual runtime sessions over this channel.
pub const BROKER_SERVER: startup::StartupPurpose<hyper_os::handle::CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8006_0002);
pub const BROKER_CLIENT: startup::StartupPurpose<hyper_os::handle::CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8006_0003);
pub const SESSION: startup::StartupPurpose<hyper_os::handle::CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8006_0004);
pub const BROKER_SERVER_CONTRACT: StartupContract = StartupContract::exact(
    "io.broker-server",
    BROKER_SERVER,
    Rights::WAIT.union(Rights::READ),
);
pub const BROKER_CLIENT_CONTRACT: StartupContract = StartupContract::exact(
    "io.broker-client",
    BROKER_CLIENT,
    Rights::WAIT.union(Rights::WRITE),
);
pub const SESSION_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const SESSION_CONTRACT: StartupContract =
    StartupContract::exact("io.session", SESSION, SESSION_RIGHTS);

/// Capability rendezvous records have exact lengths and no native-layout fields.
pub const CONNECT_BYTES: usize = 48;
pub const MEMORY_BYTES: usize = 24;
pub const BOUND_MESSAGE: &[u8] = b"HIOBOUND1";
pub const FRONTEND_MMIO: u64 = 0x0a00_0000;
pub const FRONTEND_IRQ: u32 = 48;
pub const MAILBOX_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const NOTIFICATION_RIGHTS: Rights = Rights::INSPECT.union(Rights::WRITE).union(Rights::WAIT);
pub const FRONTEND_RIGHTS: Rights = Rights::INSPECT.union(Rights::WRITE).union(Rights::WAIT);
pub const MEMORY_RIGHTS: Rights = Rights::INSPECT.union(Rights::MAP);

pub fn encode_connect(client: u32, volume: &str) -> Option<[u8; CONNECT_BYTES]> {
    if !(1..=127).contains(&client)
        || volume.is_empty()
        || volume.len() > 32
        || !volume
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return None;
    }
    let mut bytes = [0; CONNECT_BYTES];
    bytes[..8].copy_from_slice(b"HIOCONN1");
    bytes[8..12].copy_from_slice(&client.to_le_bytes());
    bytes[12] = volume.len() as u8;
    bytes[16..16 + volume.len()].copy_from_slice(volume.as_bytes());
    Some(bytes)
}
pub fn decode_connect(bytes: &[u8]) -> Option<(u32, &str)> {
    if bytes.len() != CONNECT_BYTES || &bytes[..8] != b"HIOCONN1" {
        return None;
    }
    let client = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let length = bytes[12] as usize;
    let volume = core::str::from_utf8(bytes.get(16..16usize.checked_add(length)?)?).ok()?;
    (encode_connect(client, volume)?.as_slice() == bytes).then_some((client, volume))
}
pub fn encode_memory(base: u64, length: u64) -> Option<[u8; MEMORY_BYTES]> {
    if length == 0 || (base | length) & 4095 != 0 || base.checked_add(length).is_none() {
        return None;
    }
    let mut bytes = [0; MEMORY_BYTES];
    bytes[..8].copy_from_slice(b"HIOMEM01");
    bytes[8..16].copy_from_slice(&base.to_le_bytes());
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    Some(bytes)
}
pub fn decode_memory(bytes: &[u8]) -> Option<(u64, u64)> {
    if bytes.len() != MEMORY_BYTES || &bytes[..8] != b"HIOMEM01" {
        return None;
    }
    let base = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
    let length = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
    encode_memory(base, length).map(|_| (base, length))
}

/// Rendezvous has no queued capability ownership; the caller keeps every MOVE
/// source until the exact receive commits, and bounds waiting with one deadline.
pub fn send_capabilities(
    endpoint: &hyper_os::capability_channel::CapabilityChannel,
    bytes: &[u8],
    dispositions: &mut [hyper_os::capability_channel::CapabilityDisposition<'_>],
    deadline: u64,
) -> hyper_os::Result<()> {
    use hyper_os::handle::CapabilityChannelObject;
    use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
    loop {
        let observation = wait_many(
            &[WaitItem::new(
                endpoint.as_handle_ref(),
                ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
            )],
            deadline,
        )?;
        if !ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
            .is_present_in(observation.observed)
        {
            return Err(hyper_os::Error::InvalidResponse);
        }
        match endpoint.try_send(bytes, dispositions) {
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            result => return result,
        }
    }
}
