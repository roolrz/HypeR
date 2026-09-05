// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::env;
use std::path::Path;
use std::process::ExitCode;

#[path = "../../generator/mod.rs"]
mod generator;

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let operation = arguments.next();
    if arguments.next().is_some() {
        eprintln!("usage: hyper-abi {{check|write}}");
        return ExitCode::from(2);
    }

    let result = match operation.as_deref().and_then(|value| value.to_str()) {
        Some("check") => generator::check_repository_outputs(Path::new(".")),
        Some("write") => generator::write_repository_outputs(Path::new(".")),
        _ => {
            eprintln!("usage: hyper-abi {{check|write}}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
