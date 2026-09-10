// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Parser)]
#[command(name = "mkdir", about = "Create directories")]
pub struct Args {
    /// Create missing parents and accept existing directories.
    #[arg(short = 'p', long)]
    pub parents: bool,
    /// Octal permissions for newly created final directories; existing directories are unchanged.
    #[arg(short = 'm', long, value_parser = parse_mode)]
    pub mode: Option<u32>,
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}

fn parse_mode(value: &str) -> Result<u32, String> {
    if value.is_empty() || value.len() > 4 || !value.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return Err("expected an octal mode from 0000 to 7777".into());
    }
    u32::from_str_radix(value, 8).map_err(|e| e.to_string())
}

pub fn create(path: &Path, parents: bool, mode: Option<u32>) -> io::Result<()> {
    if parents && let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    match fs::create_dir(path) {
        Ok(()) => {
            if let Some(mode) = mode {
                fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
            }
            Ok(())
        }
        Err(error) if parents && error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn run(args: Args) -> bool {
    let mut success = true;
    for path in args.paths {
        if let Err(error) = create(&path, args.parents, args.mode) {
            eprintln!("mkdir: {}: {error}", path.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
