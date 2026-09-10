// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn run() -> io::Result<bool> {
    let mut args = hyper_cat::cli::Cat::parse();
    if args.files.is_empty() {
        args.files.push(PathBuf::from("-"));
    }
    let mut output = io::stdout().lock();
    let mut lines = hyper_cat::Lines::default();
    let mut failed = false;
    for path in args.files {
        let result = if path == std::path::Path::new("-") {
            lines.copy(io::stdin().lock(), &mut output, args.number)
        } else {
            File::open(&path)
                .and_then(|file| lines.copy(BufReader::new(file), &mut output, args.number))
        };
        if let Err(error) = result {
            if error.kind() == io::ErrorKind::BrokenPipe {
                return Err(error);
            }
            eprintln!("cat: {}: {error}", path.display());
            failed = true;
        }
    }
    output.flush()?;
    Ok(failed)
}

fn main() -> ExitCode {
    match run() {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::FAILURE,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("cat: {error}");
            ExitCode::FAILURE
        }
    }
}
