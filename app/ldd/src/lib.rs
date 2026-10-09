// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Read-only Native ELF dependency diagnostics; inspected files never execute.

pub mod cli;
mod elf;
mod graph;
mod output;

use std::io::{self, Write};

/// Reports every operand, preserving failures without discarding later files.
/// Output failure stops the scan, including a downstream pipe closing early.
pub fn run(args: &cli::Ldd, output: &mut impl Write, errors: &mut impl Write) -> io::Result<bool> {
    let mut failed = false;
    for (index, path) in args.files.iter().enumerate() {
        if index != 0 {
            writeln!(output)?;
        }
        writeln!(output, "{}:", output::escaped(&path.to_string_lossy()))?;
        match graph::inspect(path, args.library_dir.as_deref(), args.direct) {
            Ok(graph) => {
                failed |= graph.failed();
                output::write(&graph, args.tree, args.verbose, output)?;
            }
            Err(error) => {
                output.flush()?;
                let message = format!(
                    "ldd: {}: {}\n",
                    output::escaped(&path.to_string_lossy()),
                    output::escaped(&error.to_string())
                );
                errors.write_all(message.as_bytes())?;
                failed = true;
            }
        }
    }
    output.flush()?;
    Ok(failed)
}

#[cfg(test)]
#[path = "../tests/fixtures.rs"]
mod fixtures;
