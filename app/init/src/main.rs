// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial `HypeR` Native userspace supervisor.

mod runtime;

use hyper_os::startup::Startup;
use std::process::ExitCode;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match runtime::run(&mut startup) {
        Ok(never) => match never {},
        Err(error) => {
            use std::io::Write;
            let _ = std::io::stderr().write_all(error.diagnostic());
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup),
        Err(_) => ExitCode::FAILURE,
    }
}
