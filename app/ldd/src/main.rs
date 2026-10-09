// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = hyper_ldd::cli::Ldd::parse();
    match hyper_ldd::run(&args, &mut io::stdout().lock(), &mut io::stderr().lock()) {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::FAILURE,
        Err(error) => {
            if error.kind() != io::ErrorKind::BrokenPipe {
                eprintln!("ldd: {error}");
            }
            ExitCode::FAILURE
        }
    }
}
