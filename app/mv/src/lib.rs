// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Default, Parser)]
#[command(
    name = "mv",
    about = "Move or rename files and directories within a filesystem"
)]
pub struct Args {
    /// Treat DEST as an exact path instead of moving inside a destination directory.
    #[arg(short = 'T', long)]
    pub no_target_directory: bool,
    /// Report each successful move.
    #[arg(short = 'v', long)]
    pub verbose: bool,
    /// Source paths followed by the destination. Multiple sources require a directory.
    #[arg(required = true, num_args = 2..)]
    pub paths: Vec<PathBuf>,
}

pub fn move_one(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

pub fn run(args: Args) -> bool {
    let Some((destination, sources)) = args.paths.split_last() else {
        return false;
    };
    let directory = !args.no_target_directory && destination.is_dir();
    if sources.len() > 1 && !directory {
        eprintln!(
            "mv: {}: multiple sources require a destination directory",
            destination.display()
        );
        return false;
    }
    let mut success = true;
    for source in sources {
        let target = if directory {
            match source.file_name() {
                Some(name) => destination.join(name),
                None => {
                    eprintln!("mv: {}: source has no file name", source.display());
                    success = false;
                    continue;
                }
            }
        } else {
            destination.clone()
        };
        if let Err(error) = move_one(source, &target).and_then(|()| {
            if args.verbose {
                writeln!(
                    io::stdout().lock(),
                    "{} -> {}",
                    source.display(),
                    target.display()
                )
            } else {
                Ok(())
            }
        }) {
            eprintln!("mv: {} -> {}: {error}", source.display(), target.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
