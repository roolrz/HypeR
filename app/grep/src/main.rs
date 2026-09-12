// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use hyper_grep::Grep;
use std::io::{self, BufReader, Write};
use std::process::ExitCode;

fn run(args: &Grep) -> Result<u8, String> {
    let (regex, files) = args.matcher()?;
    let show_name = !args.no_filename && (args.with_filename || files.len() > 1);
    let mut output = io::stdout().lock();
    let mut matched = false;
    let mut failed = false;
    for file in files {
        let name = if file == std::path::Path::new("-") {
            "(standard input)".to_owned()
        } else {
            file.display().to_string()
        };
        let result = if file == std::path::Path::new("-") {
            args.scan(&regex, io::stdin().lock(), &mut output, &name, show_name)
        } else {
            std::fs::File::open(&file).and_then(|file| {
                args.scan(&regex, BufReader::new(file), &mut output, &name, show_name)
            })
        };
        match result {
            Ok(found) => {
                matched |= found;
                if found && args.quiet {
                    return Ok(0);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(2),
            Err(error) => {
                eprintln!("grep: {name}: {error}");
                failed = true;
            }
        }
    }
    output.flush().map_err(|error| error.to_string())?;
    Ok(if failed {
        2
    } else if matched {
        0
    } else {
        1
    })
}
fn main() -> ExitCode {
    match run(&Grep::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("grep: {error}");
            ExitCode::from(2)
        }
    }
}
