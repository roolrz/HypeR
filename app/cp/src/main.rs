// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    if hyper_cp::run(hyper_cp::Args::parse()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
