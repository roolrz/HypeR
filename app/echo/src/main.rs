// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! External echo using standard argument parsing and output.

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Parser;
use std::io::{self, Write};

fn run() -> io::Result<()> {
    let args = hyper_echo::cli::Echo::parse();
    let mut output = io::stdout().lock();
    hyper_echo::write(&args, &mut output)?;
    output.flush()
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("echo: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
