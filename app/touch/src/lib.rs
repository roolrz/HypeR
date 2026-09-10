// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::fs::{self, FileTimes, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Parser)]
#[command(
    name = "touch",
    about = "Update file timestamps, creating missing files without truncating existing data"
)]
pub struct Args {
    /// Change only access time (unless -m is also given).
    #[arg(short = 'a')]
    pub access: bool,
    /// Change only modification time (unless -a is also given).
    #[arg(short = 'm')]
    pub modification: bool,
    #[arg(short = 'c', long)]
    pub no_create: bool,
    /// Copy timestamps from this file instead of using the current time.
    #[arg(short = 'r', long)]
    pub reference: Option<PathBuf>,
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}

pub fn touch(path: &Path, no_create: bool, times: FileTimes) -> io::Result<()> {
    match fs::metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if no_create {
                return Ok(());
            }
            // Do not truncate an existing file if another creator won the race.
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)?;
        }
        Err(error) => return Err(error),
    }
    #[cfg(target_os = "hyper")]
    {
        std::os::hyper::fs::set_times(path, times)
    }
    #[cfg(unix)]
    {
        fs::File::open(path)?.set_times(times)
    }
}

fn times(args: &Args) -> io::Result<FileTimes> {
    let reference = args.reference.as_ref().map(fs::metadata).transpose()?;
    let now = SystemTime::now();
    let mut times = FileTimes::new();
    if args.access || !args.modification {
        times = times.set_accessed(match &reference {
            Some(m) => m.accessed()?,
            None => now,
        });
    }
    if args.modification || !args.access {
        times = times.set_modified(match &reference {
            Some(m) => m.modified()?,
            None => now,
        });
    }
    Ok(times)
}

pub fn run(args: Args) -> bool {
    let times = match times(&args) {
        Ok(times) => times,
        Err(error) => {
            eprintln!("touch: reference: {error}");
            return false;
        }
    };
    let mut success = true;
    for path in args.paths {
        if let Err(error) = touch(&path, args.no_create, times) {
            eprintln!("touch: {}: {error}", path.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
