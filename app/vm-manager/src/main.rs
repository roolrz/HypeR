// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet manager process entry.

mod manager;

use hyper_os::startup::Startup;
use manager::FleetManager;
use std::process::ExitCode;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let mut manager = match FleetManager::from_startup(&mut startup) {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("vm-manager: startup failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    match manager.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vm-manager: supervisor failed: {error}");
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
