// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native directory listing command.

use clap::Parser;
use std::io::Write;

use hyper_ls::directory_path::is_descendant;
use hyper_os::fs::{Directory, DirectoryEntry, DirectoryEntryKind, DirectoryRights};
use hyper_service::process;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_ls::cli::Ls::parse();
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let current = Directory::from_handle(startup.take(process::WORKING_DIRECTORY)?);
    let directory = match args.directory {
        None => current,
        Some(path) if is_descendant(&path) => {
            current.open_directory(&path, DirectoryRights::READ)?
        }
        Some(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ls requires a relative directory without parent components",
            )
            .into());
        }
    };
    list_directory(&directory, &mut std::io::stdout().lock())
}

fn list_directory(
    directory: &Directory,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = directory.reader();
    while let Some(page) = reader.next_page()? {
        for entry in page.entries() {
            write_entry(output, entry)?;
        }
    }
    Ok(())
}

fn write_entry(
    output: &mut impl Write,
    entry: &DirectoryEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    let name =
        std::str::from_utf8(entry.name_bytes()).map_err(|_| hyper_os::Error::InvalidResponse)?;
    let suffix = match entry.kind() {
        DirectoryEntryKind::Directory => "/",
        DirectoryEntryKind::Symlink => "@",
        DirectoryEntryKind::File | DirectoryEntryKind::Other => "",
    };
    writeln!(output, "{name}{suffix}")?;
    Ok(())
}
