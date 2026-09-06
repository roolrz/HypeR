// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native Process and Thread listing.

#![no_std]
#![no_main]

#[path = "format.rs"]
mod format;

use core::fmt::Write;

use format::Buffer;
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::inspect::{Koid, ProcessObservation, ScanCursor, TaskInspector, ThreadObservation};
use hyper_os::startup::{self, Startup};
use hyper_rt::ExitCode;
use hyper_service::stdio;

type Output = OwnedHandle<ByteChannelObject>;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let output = match startup.take(stdio::STANDARD_OUTPUT) {
        Ok(output) => output,
        Err(_) => return ExitCode::FAILURE,
    };
    let inspector = match startup.take(startup::TASK_INSPECTOR) {
        Ok(handle) => TaskInspector::from_handle(handle),
        Err(_) => return ExitCode::FAILURE,
    };
    let include_threads = match parse_options(&startup) {
        Ok(value) => value,
        Err(()) => {
            let _ = output.as_byte_channel().send(b"usage: ps [-T|--threads]\n");
            return ExitCode::FAILURE;
        }
    };
    match list_tasks(&inspector, &output, include_threads) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn parse_options(startup: &Startup<'_>) -> Result<bool, ()> {
    match startup.argument_count() {
        1 => Ok(false),
        2 => match startup.argument(1).map_err(|_| ())? {
            "-T" | "--threads" => Ok(true),
            _ => Err(()),
        },
        _ => Err(()),
    }
}

fn list_tasks(
    inspector: &TaskInspector,
    output: &Output,
    include_threads: bool,
) -> hyper_os::Result<()> {
    output
        .as_byte_channel()
        .send(b"TYPE     KOID       OWNER      NAME                 STATE\n")?;
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = inspector.scan_processes(position)?;
        for process in page.entries() {
            write_process(output, process)?;
            if include_threads {
                write_process_threads(inspector, output, process.koid)?;
            }
        }
        cursor = page.next();
    }
    if include_threads {
        write_kernel_threads(inspector, output)?;
    }
    Ok(())
}

fn write_process(output: &Output, process: &ProcessObservation) -> hyper_os::Result<()> {
    let mut line = Buffer::<256>::new();
    write!(
        line,
        "process  {:<10} -          {:<20} {} threads={} pending={}",
        process.koid.get(),
        process.name.as_str(),
        process.phase.name(),
        process.active_threads,
        process.pending_threads
    )
    .map_err(|_| hyper_os::Error::InvalidResponse)?;
    if let Some(reason) = process.terminal_reason {
        write!(line, " terminal={}", reason.name())
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
    }
    writeln!(line).map_err(|_| hyper_os::Error::InvalidResponse)?;
    output.as_byte_channel().send(line.bytes())
}

fn write_process_threads(
    inspector: &TaskInspector,
    output: &Output,
    process: Koid,
) -> hyper_os::Result<()> {
    scan_threads(inspector, |thread| {
        if thread.process_koid == Some(process) {
            write_thread(output, thread)
        } else {
            Ok(())
        }
    })
}

fn write_kernel_threads(inspector: &TaskInspector, output: &Output) -> hyper_os::Result<()> {
    scan_threads(inspector, |thread| {
        if thread.process_koid.is_none() {
            write_thread(output, thread)
        } else {
            Ok(())
        }
    })
}

fn scan_threads(
    inspector: &TaskInspector,
    mut visit: impl FnMut(&ThreadObservation) -> hyper_os::Result<()>,
) -> hyper_os::Result<()> {
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = inspector.scan_threads(position)?;
        for thread in page.entries() {
            visit(thread)?;
        }
        cursor = page.next();
    }
    Ok(())
}

fn write_thread(output: &Output, thread: &ThreadObservation) -> hyper_os::Result<()> {
    let mut line = Buffer::<256>::new();
    writeln!(
        line,
        "  thread {:<10} {:<10} {:<20} {}/{}",
        thread.koid.get(),
        Owner(thread.process_koid),
        thread.name.as_str(),
        thread.role.name(),
        thread.registry_phase.name(),
    )
    .map_err(|_| hyper_os::Error::InvalidResponse)?;
    output.as_byte_channel().send(line.bytes())
}

struct Owner(Option<Koid>);

impl core::fmt::Display for Owner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let width = formatter.width().unwrap_or(0);
        match self.0 {
            Some(koid) => write!(formatter, "{:<width$}", koid.get()),
            None => write!(formatter, "{:<width$}", "-"),
        }
    }
}

hyper_rt::entry!(application_main);
