// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial `HypeR` Native userspace supervisor.

#![no_std]
#![no_main]

mod runtime;

use hyper_os::startup::Startup;
use hyper_rt::ExitCode;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match runtime::run(&mut startup) {
        Ok(never) => match never {},
        Err(error) => {
            if let Ok(console) = startup.console() {
                let _ = console.write_all(error.diagnostic());
            }
            ExitCode::FAILURE
        }
    }
}

hyper_rt::entry!(application_main);
