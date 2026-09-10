// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Parser)]
#[command(
    name = "rm",
    about = "Remove files; recursive removal does not follow symbolic links"
)]
pub struct Args {
    #[arg(short = 'r', visible_short_alias = 'R', long)]
    pub recursive: bool,
    /// Ignore missing paths (other errors are still reported).
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(required_unless_present = "force")]
    pub paths: Vec<PathBuf>,
}

pub fn remove(path: &Path, recursive: bool) -> io::Result<()> {
    // Reject dot operands before normalization, including trailing separators.
    let raw = path.as_os_str().as_encoded_bytes();
    let last = raw.rsplit(|b| *b == b'/').find(|part| !part.is_empty());
    if matches!(last, Some(b"." | b"..")) {
        return Err(io::Error::other("refusing to remove '.' or '..'"));
    }
    // Strip trailing separators so a directory symlink remains a link operand.
    let normalized: PathBuf = path.components().collect();
    remove_normalized(&normalized, recursive)
}

fn remove_normalized(path: &Path, recursive: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        if !recursive {
            return Err(io::Error::other("is a directory (use -r)"));
        }
        if fs::canonicalize(path)?.parent().is_none() {
            return Err(io::Error::other("refusing to remove the filesystem root"));
        }
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn run(args: Args) -> bool {
    let mut success = true;
    for path in args.paths {
        if let Err(error) = remove(&path, args.recursive) {
            if args.force && error.kind() == io::ErrorKind::NotFound {
                continue;
            }
            eprintln!("rm: {}: {error}", path.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
