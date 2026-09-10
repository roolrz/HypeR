// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped physical-memory summary.

use clap::Parser;
use hyper_os::inspect::MemoryInspector;
use hyper_os::startup;
use std::io::Write;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_free::cli::Free::parse();
    let unit = if args.bytes { "B" } else { "MiB" };
    let quantity = |bytes: u64| if args.bytes { bytes } else { mib(bytes) };
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let observation = inspector.read()?;
    let mut output = std::io::stdout().lock();
    writeln!(
        output,
        "               total        used        free    reserved reclaimable"
    )?;
    writeln!(
        output,
        "Mem:      {:>6} {unit}  {:>6} {unit}  {:>6} {unit}  {:>6} {unit}  {:>6} {unit}",
        quantity(observation.total_bytes),
        quantity(observation.used_bytes),
        quantity(observation.free_bytes),
        quantity(observation.reserved_bytes),
        quantity(observation.reclaimable_bytes)
    )?;
    writeln!(
        output,
        "Owners:   kernel={} {unit} heap={} {unit} tables={} {unit} user={} {unit} guest={} {unit} other={} {unit}",
        quantity(observation.kernel_bytes),
        quantity(observation.heap_bytes),
        quantity(observation.page_table_bytes),
        quantity(observation.user_bytes),
        quantity(observation.guest_bytes),
        quantity(observation.unattributed_bytes)
    )?;
    Ok(())
}

const fn mib(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("free: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
