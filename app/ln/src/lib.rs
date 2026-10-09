// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::io::Write;
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::symlink;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Default, Parser)]
#[command(name = "ln", about = "Create a hard link, or a symbolic link with -s")]
pub struct Args {
    /// Treat a symlink to a directory as the link name, not as a directory.
    #[arg(short = 'n', long)]
    pub no_dereference: bool,
    /// Report the newly created link.
    #[arg(short = 'v', long)]
    pub verbose: bool,
    #[arg(short = 's', long)]
    pub symbolic: bool,
    /// Treat LINK as the link name even when it names a directory.
    #[arg(short = 'T', long)]
    pub no_target_directory: bool,
    pub target: PathBuf,
    pub link: PathBuf,
}

pub fn link(target: &Path, name: &Path, symbolic: bool) -> io::Result<()> {
    if symbolic {
        symlink(target, name)
    } else {
        fs::hard_link(target, name)
    }
}

pub fn run(args: Args) -> bool {
    let link_is_symlink =
        fs::symlink_metadata(&args.link).is_ok_and(|m| m.file_type().is_symlink());
    let name = if !args.no_target_directory
        && !(args.no_dereference && link_is_symlink)
        && args.link.is_dir()
    {
        match args.target.file_name() {
            Some(name) => args.link.join(name),
            None => {
                eprintln!("ln: target has no file name");
                return false;
            }
        }
    } else {
        args.link
    };
    match link(&args.target, &name, args.symbolic).and_then(|()| {
        if args.verbose {
            writeln!(
                io::stdout().lock(),
                "{} -> {}",
                name.display(),
                args.target.display()
            )
        } else {
            Ok(())
        }
    }) {
        Ok(()) => true,
        Err(error) => {
            eprintln!(
                "ln: {} -> {}: {error}",
                name.display(),
                args.target.display()
            );
            false
        }
    }
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
