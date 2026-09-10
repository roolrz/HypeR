// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Directory listing through the Native Rust standard-library adapter.

use clap::Parser;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_ls::cli::Ls::parse();
    let path = args.directory.as_deref().unwrap_or(".");
    if !hyper_ls::directory_path::is_descendant(path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ls requires a relative directory without parent components",
        )
        .into());
    }
    let mut output = std::io::stdout().lock();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let suffix = if kind.is_dir() {
            "/"
        } else if kind.is_symlink() {
            "@"
        } else {
            ""
        };
        writeln!(output, "{}{suffix}", entry.file_name().to_string_lossy())?;
    }
    Ok(())
}
