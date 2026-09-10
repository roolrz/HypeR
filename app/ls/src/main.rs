// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use hyper_ls::cli::{Ls, Sort};
use std::fs::{self, Metadata};
use std::io::{self, Write};
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Entry {
    name: String,
    metadata: Metadata,
}

fn list(path: &Path, args: &Ls, output: &mut impl Write) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let mut entries = Vec::new();
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !args.all && name.starts_with('.') {
                continue;
            }
            entries.push(Entry {
                name,
                metadata: fs::symlink_metadata(entry.path())?,
            });
        }
    } else {
        entries.push(Entry {
            name: path.to_string_lossy().into_owned(),
            metadata,
        });
    }
    entries.sort_by(|a, b| match args.sort {
        Sort::Name => a.name.cmp(&b.name),
        Sort::Size => b
            .metadata
            .len()
            .cmp(&a.metadata.len())
            .then(a.name.cmp(&b.name)),
    });
    if args.reverse {
        entries.reverse();
    }
    if !args.names_only {
        writeln!(output, "MODE                 SIZE  NAME")?;
    }
    for entry in entries {
        let directory = entry.metadata.is_dir();
        let symlink = entry.metadata.file_type().is_symlink();
        let name: String = entry.name.chars().flat_map(char::escape_default).collect();
        // Preserve printable Unicode, escaping only control characters and backslashes.
        let name = if entry.name.chars().all(|c| !c.is_control() && c != '\\') {
            entry.name
        } else {
            name
        };
        let suffix = if directory {
            "/"
        } else if symlink {
            "@"
        } else {
            ""
        };
        if args.names_only {
            writeln!(output, "{name}{suffix}")?;
        } else {
            let size = if directory {
                "-".into()
            } else if args.bytes {
                entry.metadata.len().to_string()
            } else {
                hyper_ls::readable_size(entry.metadata.len())
            };
            writeln!(
                output,
                "{} {:>14}  {name}{suffix}",
                hyper_ls::mode_text(entry.metadata.mode(), directory, symlink),
                size
            )?;
        }
    }
    Ok(())
}

fn run() -> io::Result<bool> {
    let mut args = Ls::parse();
    if args.paths.is_empty() {
        args.paths.push(PathBuf::from("."));
    }
    let mut output = io::stdout().lock();
    let mut failed = false;
    for (index, path) in args.paths.iter().enumerate() {
        if args.paths.len() > 1 {
            if index != 0 {
                writeln!(output)?;
            }
            writeln!(output, "{}:", path.display())?;
        }
        if let Err(error) = list(path, &args, &mut output) {
            if error.kind() == io::ErrorKind::BrokenPipe {
                return Err(error);
            }
            eprintln!("ls: {}: {error}", path.display());
            failed = true;
        }
    }
    output.flush()?;
    Ok(failed)
}

fn main() -> ExitCode {
    match run() {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("ls: {error}");
            ExitCode::FAILURE
        }
    }
}
