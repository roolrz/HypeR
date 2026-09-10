// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    if hyper_rm::run(hyper_rm::Args::parse()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
