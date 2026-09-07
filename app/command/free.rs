// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped physical-memory summary.

#![no_std]
#![no_main]

#[path = "format.rs"]
mod format;

use core::fmt::Write;

use format::Buffer;
use hyper_os::inspect::MemoryInspector;
use hyper_os::startup::{self, Startup};
use hyper_rt::ExitCode;
use hyper_service::stdio;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let output = match startup.take(stdio::STANDARD_OUTPUT) {
        Ok(output) => output,
        Err(_) => return ExitCode::FAILURE,
    };
    let inspector = match startup.take(startup::MEMORY_INSPECTOR) {
        Ok(handle) => MemoryInspector::from_handle(handle),
        Err(_) => return ExitCode::FAILURE,
    };
    let observation = match inspector.read() {
        Ok(value) => value,
        Err(_) => return ExitCode::FAILURE,
    };
    let mut text = Buffer::<1024>::new();
    if writeln!(
        text,
        "               total        used        free    reserved reclaimable"
    )
    .and_then(|()| {
        writeln!(
            text,
            "Mem:      {:>6} MiB  {:>6} MiB  {:>6} MiB  {:>6} MiB  {:>6} MiB",
            mib(observation.total_bytes),
            mib(observation.used_bytes),
            mib(observation.free_bytes),
            mib(observation.reserved_bytes),
            mib(observation.reclaimable_bytes),
        )
    })
    .and_then(|()| {
        writeln!(
            text,
            "Owners:   kernel={} MiB heap={} MiB tables={} MiB user={} MiB guest={} MiB other={} MiB",
            mib(observation.kernel_bytes),
            mib(observation.heap_bytes),
            mib(observation.page_table_bytes),
            mib(observation.user_bytes),
            mib(observation.guest_bytes),
            mib(observation.unattributed_bytes),
        )
    })
    .is_err()
    {
        return ExitCode::FAILURE;
    }
    if output.as_byte_channel().send(text.bytes()).is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

const fn mib(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

hyper_rt::entry!(application_main);
