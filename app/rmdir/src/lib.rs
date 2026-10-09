// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Parser)]
#[command(name = "rmdir", about = "Remove empty directories")]
pub struct Args {
    /// Remove the directory and then its empty ancestors; stop at the first error.
    #[arg(short = 'p', long)]
    pub parents: bool,
    /// Report every removed directory.
    #[arg(short = 'v', long)]
    pub verbose: bool,
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}

fn remove(
    path: &Path,
    parents: bool,
    verbose: bool,
    output: &mut impl io::Write,
) -> io::Result<()> {
    if path.as_os_str().is_empty() || path == Path::new(".") || path.parent().is_none() {
        return Err(io::Error::other(
            "refusing an empty, current-directory or root operand",
        ));
    }
    // Keep the user's lexical path: canonicalization could cross a symlink and
    // remove ancestors outside the requested path. Never climb through '..'.
    if parents
        && path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err(io::Error::other("-p does not accept '..' components"));
    }
    for directory in path.ancestors() {
        if directory.as_os_str().is_empty()
            || directory == Path::new(".")
            || directory.parent().is_none()
        {
            break;
        }
        fs::remove_dir(directory).map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", directory.display()))
        })?;
        if verbose {
            writeln!(output, "removed directory {}", directory.display())?;
        }
        if !parents {
            break;
        }
    }
    Ok(())
}

pub fn run(args: Args) -> bool {
    let mut success = true;
    for path in args.paths {
        if let Err(error) = remove(&path, args.parents, args.verbose, &mut io::stdout().lock()) {
            eprintln!("rmdir: {}: {error}", path.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
