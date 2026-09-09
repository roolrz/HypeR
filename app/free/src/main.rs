// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped physical-memory summary.

use clap::Parser;
use hyper_os::inspect::MemoryInspector;
use hyper_os::startup;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    hyper_free::cli::Free::parse();
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
        "Mem:      {:>6} MiB  {:>6} MiB  {:>6} MiB  {:>6} MiB  {:>6} MiB",
        mib(observation.total_bytes),
        mib(observation.used_bytes),
        mib(observation.free_bytes),
        mib(observation.reserved_bytes),
        mib(observation.reclaimable_bytes)
    )?;
    writeln!(
        output,
        "Owners:   kernel={} MiB heap={} MiB tables={} MiB user={} MiB guest={} MiB other={} MiB",
        mib(observation.kernel_bytes),
        mib(observation.heap_bytes),
        mib(observation.page_table_bytes),
        mib(observation.user_bytes),
        mib(observation.guest_bytes),
        mib(observation.unattributed_bytes)
    )?;
    Ok(())
}

const fn mib(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}
