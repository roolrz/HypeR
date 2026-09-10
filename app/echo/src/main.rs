// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! External echo using standard argument parsing and output.

use clap::Parser;
use std::io::{self, Write};

fn run() -> io::Result<()> {
    let args = hyper_echo::cli::Echo::parse();
    writeln!(io::stdout().lock(), "{}", args.words.join(" "))
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
