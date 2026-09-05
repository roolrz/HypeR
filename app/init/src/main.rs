// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial `HypeR` Native userspace process.

#![no_std]
#![no_main]

use hyper_os::startup::Startup;
use hyper_rt::ExitCode;

const IO_BUFFER_SIZE: usize = 256;
// Console Streams preserve bytes exactly. Terminal presentation, including
// CRLF policy, will move into the userspace Console service.
const READY_MESSAGE: &[u8] = b"HypeR init: console ready\r\n";
const INPUT_MESSAGE: &[u8] = b"HypeR init: received input\r\n";

fn application_main(startup: Startup<'_>) -> ExitCode {
    match run(&startup) {
        Ok(never) => match never {},
        Err(_) => ExitCode::FAILURE,
    }
}

fn run(startup: &Startup<'_>) -> hyper_os::Result<core::convert::Infallible> {
    hyper_os::require_core_abi()?;
    let console = startup.console()?;
    console.write_all(READY_MESSAGE)?;

    let mut bytes = [0_u8; IO_BUFFER_SIZE];
    let mut announced_input = false;
    loop {
        let count = console.read_blocking(&mut bytes)?;
        let received = bytes.get(..count).ok_or(hyper_os::Error::InvalidResponse)?;
        if !announced_input {
            console.write_all(INPUT_MESSAGE)?;
            announced_input = true;
        }
        console.write_all(received)?;
    }
}

hyper_rt::entry!(application_main);
