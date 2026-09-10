// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial foreground-session router for one capability-bound client.

use std::convert::Infallible;

use hyper_os::channel::ByteRelay;
use hyper_os::startup::Startup;
use hyper_os::wait::wait_many;
use hyper_service::session as session_contract;
use std::process::ExitCode;

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
    let mut output_buffer = vec![0; hyper_os::channel::MAX_MESSAGE_BYTES];
    let mut error_buffer = vec![0; hyper_os::channel::MAX_MESSAGE_BYTES];
    let mut input_buffer = vec![0; hyper_os::channel::MAX_MESSAGE_BYTES];
    let mut routes = [
        ByteRelay::new(
            client_output_owner.as_byte_channel(),
            console_output_owner.as_byte_channel(),
            &mut output_buffer,
        ),
        ByteRelay::new(
            client_error_owner.as_byte_channel(),
            console_output_owner.as_byte_channel(),
            &mut error_buffer,
        ),
        ByteRelay::new(
            console_input_owner.as_byte_channel(),
            client_input_owner.as_byte_channel(),
            &mut input_buffer,
        ),
    ];
    let mut first = 0;
    loop {
        // Rotate priority so a continuously readable stream cannot starve keys
        // or stderr. Each direction retains at most one pending message.
        let order = [
            first,
            (first + 1) % routes.len(),
            (first + 2) % routes.len(),
        ];
        let waits = [
            routes[order[0]].wait_item().ok_or(())?,
            routes[order[1]].wait_item().ok_or(())?,
            routes[order[2]].wait_item().ok_or(())?,
        ];
        let observation = wait_many(&waits, hyper_os::DEADLINE_INFINITE).map_err(|_| ())?;
        let index = *order.get(observation.index).ok_or(())?;
        routes[index].poll().map_err(|_| ())?;
        first = (index + 1) % routes.len();
    }
}

fn main() -> ExitCode {
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup),
        Err(_) => ExitCode::FAILURE,
    }
}
