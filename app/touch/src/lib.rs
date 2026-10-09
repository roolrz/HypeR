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
    /// Set time to @SECONDS since the Unix epoch (up to nine fractional digits).
    #[arg(short = 'd', long = "date", value_parser = parse_timestamp, conflicts_with = "reference")]
    pub date: Option<SystemTime>,
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

fn parse_timestamp(value: &str) -> Result<SystemTime, String> {
    let invalid = || "expected @SECONDS[.NANOSECONDS] since the Unix epoch".to_owned();
    let value = value.strip_prefix('@').ok_or_else(invalid)?;
    let (seconds, fraction) = value.split_once('.').unwrap_or((value, ""));
    if seconds.is_empty()
        || !seconds.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return Err(invalid());
    }
    let seconds: u64 = seconds.parse().map_err(|_| invalid())?;
    let nanos = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u32>().map_err(|_| invalid())? * 10_u32.pow(9 - fraction.len() as u32)
    };
    let duration = std::time::Duration::new(seconds, nanos);
    // The Native ABI transports nanoseconds in u64. Reject values the target
    // cannot represent even when the host's SystemTime has a larger range.
    if duration.as_nanos() > u128::from(u64::MAX) {
        return Err(invalid());
    }
    SystemTime::UNIX_EPOCH
        .checked_add(duration)
        .ok_or_else(invalid)
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
    let now = args.date.unwrap_or_else(SystemTime::now);
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
