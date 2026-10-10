// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native object and process-handle inspection.

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Parser;
use hyper_handle::{cli::Handle, output, query};
use hyper_os::inspect::{ObjectInspector, TaskInspector};
use hyper_os::startup;
use std::io;

fn run() -> io::Result<()> {
    let args = Handle::parse();
    let mut out = io::stdout().lock();
    if args.list_kinds || args.list_rights {
        return output::catalog(&args, &mut out);
    }
    let mut startup = hyper_rt::process::startup().map_err(io::Error::other)?;
    hyper_os::require_core_abi().map_err(io::Error::other)?;
    let objects =
        ObjectInspector::from_handle(startup.take(startup::OBJECT_INSPECTOR).map_err(|error| {
            io::Error::other(format!("object-inspector capability unavailable: {error}"))
        })?);
    let tasks = startup
        .take_optional(startup::TASK_INSPECTOR)
        .map_err(io::Error::other)?
        .map(TaskInspector::from_handle);
    query::inspect(
        &args,
        &objects,
        tasks.as_ref(),
        &mut out,
        &mut io::stderr().lock(),
    )
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("handle: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
