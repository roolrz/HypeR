// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native Process and Thread listing.

use clap::Parser;
use std::io::Write;

use hyper_os::inspect::{Koid, ProcessObservation, ScanCursor, TaskInspector, ThreadObservation};
use hyper_os::startup;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_ps::cli::Ps::parse();
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = TaskInspector::from_handle(startup.take(startup::TASK_INSPECTOR)?);
    list_tasks(&inspector, &mut std::io::stdout().lock(), args.threads)
}

fn list_tasks(
    inspector: &TaskInspector,
    output: &mut impl Write,
    include_threads: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    output.write_all(b"TYPE     KOID       OWNER      NAME                 STATE\n")?;
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

fn write_process(
    output: &mut impl Write,
    process: &ProcessObservation,
) -> Result<(), Box<dyn std::error::Error>> {
    write!(
        output,
        "process  {:<10} -          {:<20} {} threads={} pending={}",
        process.koid.get(),
        process.name.as_str(),
        process.phase.name(),
        process.active_threads,
        process.pending_threads
    )?;
    if let Some(reason) = process.terminal_reason {
        write!(output, " terminal={}", reason.name())?;
    }
    writeln!(output)?;
    Ok(())
}

fn write_process_threads(
    inspector: &TaskInspector,
    output: &mut impl Write,
    process: Koid,
) -> Result<(), Box<dyn std::error::Error>> {
    scan_threads(inspector, |thread| {
        if thread.process_koid == Some(process) {
            write_thread(output, thread)
        } else {
            Ok(())
        }
    })
}

fn write_kernel_threads(
    inspector: &TaskInspector,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut visit: impl FnMut(&ThreadObservation) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
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

fn write_thread(
    output: &mut impl Write,
    thread: &ThreadObservation,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(
        output,
        "  thread {:<10} {:<10} {:<20} {}/{}",
        thread.koid.get(),
        Owner(thread.process_koid),
        thread.name.as_str(),
        thread.role.name(),
        thread.registry_phase.name(),
    )?;
    Ok(())
}

struct Owner(Option<Koid>);

impl std::fmt::Display for Owner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let width = formatter.width().unwrap_or(0);
        match self.0 {
            Some(koid) => write!(formatter, "{:<width$}", koid.get()),
            None => write!(formatter, "{:<width$}", "-"),
        }
    }
}
