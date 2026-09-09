// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial foreground-session router for one capability-bound client.

#![no_std]
#![no_main]

use core::convert::Infallible;

use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::startup::Startup;
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_rt::ExitCode;
use hyper_service::session as session_contract;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(never) => match never {},
        Err(_) => ExitCode::FAILURE,
    }
}

fn run(startup: &mut Startup<'_>) -> Result<Infallible, ()> {
    hyper_os::require_core_abi().map_err(|_| ())?;
    let console_input_owner = startup
        .take(session_contract::CONSOLE_INPUT)
        .map_err(|_| ())?;
    let console_output_owner = startup
        .take(session_contract::CONSOLE_OUTPUT)
        .map_err(|_| ())?;
    let client_input_owner = startup
        .take(session_contract::CLIENT_INPUT)
        .map_err(|_| ())?;
    let client_output_owner = startup
        .take(session_contract::CLIENT_OUTPUT)
        .map_err(|_| ())?;
    let client_error_owner = startup
        .take(session_contract::CLIENT_ERROR)
        .map_err(|_| ())?;
    let readable = ObjectSignals::<ByteChannelObject>::READABLE
        .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED);
    let waits = [
        WaitItem::new(client_output_owner.as_handle_ref(), readable),
        WaitItem::new(client_error_owner.as_handle_ref(), readable),
        WaitItem::new(console_input_owner.as_handle_ref(), readable),
    ];
    let mut bytes = [0_u8; hyper_os::channel::MAX_MESSAGE_BYTES];

    loop {
        let observation = wait_many(&waits, hyper_os::DEADLINE_INFINITE).map_err(|_| ())?;
        match observation.index {
            0 => route(
                &client_output_owner,
                &console_output_owner,
                observation.observed,
                &mut bytes,
            )?,
            1 => route(
                &client_error_owner,
                &console_output_owner,
                observation.observed,
                &mut bytes,
            )?,
            2 => route(
                &console_input_owner,
                &client_input_owner,
                observation.observed,
                &mut bytes,
            )?,
            _ => return Err(()),
        }
    }
}

fn route(
    source: &OwnedHandle<ByteChannelObject>,
    destination: &OwnedHandle<ByteChannelObject>,
    observed: u64,
    buffer: &mut [u8],
) -> Result<(), ()> {
    if ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) {
        let count = source.as_byte_channel().receive(buffer).map_err(|_| ())?;
        destination
            .as_byte_channel()
            .send(buffer.get(..count).ok_or(())?)
            .map_err(|_| ())?;
        return Ok(());
    }
    Err(())
}

hyper_rt::entry!(application_main);
