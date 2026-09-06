// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial foreground-session manager for raw Console data channels.

#![no_std]
#![no_main]

use core::convert::Infallible;

use hyper_app::session_contract;
use hyper_os::startup::Startup;
use hyper_rt::ExitCode;

const INPUT_BYTES: usize = 256;
const READY_MESSAGE: &[u8] = b"HypeR session: console ready\n";

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(never) => match never {},
        Err(_) => ExitCode::FAILURE,
    }
}

fn run(startup: &mut Startup<'_>) -> Result<Infallible, ()> {
    hyper_os::require_core_abi().map_err(|_| ())?;
    let input_owner = startup.take(session_contract::INPUT).map_err(|_| ())?;
    let output_owner = startup.take(session_contract::OUTPUT).map_err(|_| ())?;
    let input_channel = input_owner.as_byte_channel();
    let output_channel = output_owner.as_byte_channel();
    output_channel.send(READY_MESSAGE).map_err(|_| ())?;
    let mut input = [0_u8; INPUT_BYTES];

    loop {
        let count = input_channel.receive(&mut input).map_err(|_| ())?;
        let bytes = input.get(..count).ok_or(())?;
        output_channel.send(bytes).map_err(|_| ())?;
    }
}

hyper_rt::entry!(application_main);
