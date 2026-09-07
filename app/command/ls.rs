// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native directory listing command.

#![no_std]
#![no_main]

#[path = "directory_path.rs"]
mod directory_path;
#[path = "format.rs"]
mod format;

use core::fmt::Write;

use directory_path::is_descendant;
use format::Buffer;
use hyper_os::fs::{Directory, DirectoryEntry, DirectoryEntryKind, DirectoryRights};
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::startup::Startup;
use hyper_rt::ExitCode;
use hyper_service::{process, stdio};

type Output = OwnedHandle<ByteChannelObject>;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let output = match startup.take(stdio::STANDARD_OUTPUT) {
        Ok(output) => output,
        Err(_) => return ExitCode::FAILURE,
    };
    let error = match startup.take(stdio::STANDARD_ERROR) {
        Ok(error) => error,
        Err(_) => return ExitCode::FAILURE,
    };
    let current = match startup.take(process::WORKING_DIRECTORY) {
        Ok(handle) => Directory::from_handle(handle),
        Err(_) => return fail(&error, b"ls: working directory is unavailable\n"),
    };

    let result = match startup.argument_count() {
        1 => list_directory(&current, &output),
        2 => match startup.argument(1) {
            Ok(path) if is_descendant(path) => current
                .open_directory(path, DirectoryRights::READ)
                .and_then(|directory| list_directory(&directory, &output)),
            _ => return fail(&error, b"usage: ls [relative-directory]\n"),
        },
        _ => return fail(&error, b"usage: ls [relative-directory]\n"),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => fail(&error, b"ls: cannot read directory\n"),
    }
}

fn list_directory(directory: &Directory, output: &Output) -> hyper_os::Result<()> {
    let mut reader = directory.reader();
    while let Some(page) = reader.next_page()? {
        for entry in page.entries() {
            write_entry(output, entry)?;
        }
    }
    Ok(())
}

fn write_entry(output: &Output, entry: &DirectoryEntry) -> hyper_os::Result<()> {
    let name =
        core::str::from_utf8(entry.name_bytes()).map_err(|_| hyper_os::Error::InvalidResponse)?;
    let suffix = match entry.kind() {
        DirectoryEntryKind::Directory => "/",
        DirectoryEntryKind::Symlink => "@",
        DirectoryEntryKind::File | DirectoryEntryKind::Other => "",
    };
    let mut line = Buffer::<300>::new();
    writeln!(line, "{name}{suffix}").map_err(|_| hyper_os::Error::InvalidResponse)?;
    output.as_byte_channel().send(line.bytes())
}

fn fail(error: &Output, message: &[u8]) -> ExitCode {
    let _ = error.as_byte_channel().send(message);
    ExitCode::FAILURE
}

hyper_rt::entry!(application_main);
