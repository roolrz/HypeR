// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded command exchange; a timeout never rolls back an admitted operation.

use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_os::{Error, Status};
use hyper_vm_policy::fleet::{self, Request, Response};

fn wait(
    control: &OwnedHandle<ByteChannelObject>,
    signals: ObjectSignals<ByteChannelObject>,
    deadline: u64,
) -> hyper_os::Result<()> {
    wait_many(
        &[WaitItem::new(
            control.as_handle_ref(),
            signals.union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )],
        deadline,
    )?;
    Ok(())
}

pub(crate) fn exchange(
    control: &OwnedHandle<ByteChannelObject>,
    request: &Request,
    deadline: u64,
) -> Result<Response, Box<dyn std::error::Error>> {
    let bytes = fleet::encode(request).map_err(std::io::Error::other)?;
    loop {
        match control.as_byte_channel().try_send(&bytes) {
            Ok(()) | Err(Error::Status(Status::PEER_CLOSED)) => break,
            Err(Error::Status(Status::WOULD_BLOCK)) => wait(
                control,
                ObjectSignals::<ByteChannelObject>::WRITABLE,
                deadline,
            )?,
            Err(error) => return Err(error.into()),
        }
    }
    // A rejection may already be queued when admission closes the peer.
    let mut bytes = vec![0; fleet::MAX_MESSAGE_BYTES];
    loop {
        match control.as_byte_channel().try_receive(&mut bytes) {
            Ok(length) => {
                return fleet::response(&bytes[..length])
                    .map_err(|e| std::io::Error::other(e).into());
            }
            Err(Error::Status(Status::WOULD_BLOCK)) => wait(
                control,
                ObjectSignals::<ByteChannelObject>::READABLE,
                deadline,
            )?,
            Err(error) => return Err(error.into()),
        }
    }
}

pub(crate) fn complete(
    control: &OwnedHandle<ByteChannelObject>,
    completion: &hyper_vmm::completion::Completion,
    deadline: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = Request::Control {
        name: completion.name.clone(),
        action: fleet::Action::Status,
    };
    loop {
        let now = hyper_os::time::monotonic_now()?.as_nanoseconds();
        if now >= deadline {
            return Err(Error::Status(Status::TIMED_OUT).into());
        }
        let response = exchange(control, &request, deadline)?;
        if completion
            .observe(&response)
            .map_err(std::io::Error::other)?
        {
            return Ok(());
        }
        let now = hyper_os::time::monotonic_now()?.as_nanoseconds();
        std::thread::sleep(std::time::Duration::from_nanos(
            deadline.saturating_sub(now).min(250_000_000),
        ));
    }
}
