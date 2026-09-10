// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Debug, Parser)]
#[command(name = "rmdir", about = "Remove empty directories")]
pub struct Args {
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}

pub fn run(args: Args) -> bool {
    let mut success = true;
    for path in args.paths {
        if let Err(error) = fs::remove_dir(&path) {
            eprintln!("rmdir: {}: {error}", path.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
