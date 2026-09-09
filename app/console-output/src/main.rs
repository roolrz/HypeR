// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Physical Console output worker for the initial foreground session.

use std::convert::Infallible;

use hyper_os::startup::Startup;
use hyper_service::console as console_contract;
use std::process::ExitCode;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(never) => match never {},
        Err(()) => ExitCode::FAILURE,
    }
}

fn run(startup: &mut Startup<'_>) -> Result<Infallible, ()> {
    hyper_os::require_core_abi().map_err(|_| ())?;
    let console_owner = startup
        .take(console_contract::SYSTEM_CONSOLE)
        .map_err(|_| ())?;
    let channel_owner = startup
        .take(console_contract::DATA_CHANNEL)
        .map_err(|_| ())?;
    let console = console_owner.as_console();
    let channel = channel_owner.as_byte_channel();
    let mut output = [0_u8; hyper_os::channel::MAX_MESSAGE_BYTES];

    loop {
        let count = channel.receive(&mut output).map_err(|_| ())?;
        console
            .write_all(output.get(..count).ok_or(())?)
            .map_err(|_| ())?;
    }
}

fn main() -> ExitCode {
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup),
        Err(_) => ExitCode::FAILURE,
    }
}
