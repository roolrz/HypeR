// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One-time volume handoff; file requests never traverse the I/O runtime.

use hyper_os::block::{self, NativeBlock};
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::handle::{ByteChannelObject, CapabilityChannelObject, Rights, RightsOffer};
use hyper_os::wait::{self, ObjectSignals, WaitItem};
use hyper_service::filesystem;

use super::super::{Result, check_deadline, deadline, show};

pub(super) fn attach(
    provider: CapabilityChannel,
    block: NativeBlock,
    volume: block::VolumeInfo,
) -> Result<()> {
    let request = filesystem::encode_attach(volume).ok_or("invalid volume geometry")?;
    let (ready, receiver) = hyper_os::channel::create_pair().map_err(show)?;
    let mut block = Some(block.into_handle());
    let mut ready = Some(ready);
    let limit = deadline(hyper_service::io::READY_TIMEOUT_SECONDS)?;
    loop {
        let result = provider.try_send(
            &request,
            &mut [
                CapabilityDisposition::move_handle(&mut block, RightsOffer::Exact(block::RIGHTS))
                    .map_err(show)?,
                CapabilityDisposition::move_handle(
                    &mut ready,
                    RightsOffer::Exact(Rights::WAIT.union(Rights::WRITE).union(Rights::TRANSFER)),
                )
                .map_err(show)?,
            ],
        );
        match result {
            Ok(()) => break,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            Err(error) => return Err(show(error)),
        }
        check_deadline(limit)?;
        let item = WaitItem::new(
            provider.as_handle_ref(),
            ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
        );
        let observed = wait::wait_many(&[item], limit).map_err(show)?;
        if ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED.is_present_in(observed.observed) {
            return Err("filesystem manager closed volume attachment".into());
        }
    }
    // The move is committed: the selected filesystem owns block authority.
    // Keep waiting on the dedicated result, not the now-consumed rendezvous.
    drop(provider);
    let mut response = [0_u8; 32];
    loop {
        match receiver.as_byte_channel().try_receive(&mut response) {
            Ok(length) if &response[..length] == filesystem::READY_MESSAGE => return Ok(()),
            Ok(_) => return Err("invalid filesystem mount readiness record".into()),
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            Err(error) => return Err(format!("filesystem mount failed: {error:?}")),
        }
        check_deadline(limit)?;
        let item = WaitItem::new(
            receiver.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        );
        wait::wait_many(&[item], limit).map_err(show)?;
    }
}
