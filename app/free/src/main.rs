// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped physical-memory summary.

use clap::Parser;
use hyper_os::inspect::MemoryInspector;
use hyper_os::startup;
use std::io::Write;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_free::cli::Free::parse();
    let quantity = |bytes: u64| hyper_free::format_bytes(bytes, args.bytes);
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let observation = inspector.read()?;
    let cache = if observation.cache_sample_complete {
        quantity(observation.reclaimable_bytes)
    } else {
        String::from("—")
    };
    let mut output = std::io::stdout().lock();
    writeln!(
        output,
        "          {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
        "total", "used", "free", "cache", "buffers"
    )?;
    writeln!(
        output,
        "Mem:      {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
        quantity(observation.total_bytes),
        quantity(observation.used_bytes),
        quantity(observation.free_bytes),
        cache,
        quantity(observation.buffered_bytes),
    )?;
    writeln!(
        output,
        "Reserved: {}  Reclaimable: {}",
        quantity(observation.reserved_bytes),
        cache
    )?;
    writeln!(
        output,
        "Owners:   kernel={} heap={} tables={} user={} guest={} other={}",
        quantity(observation.kernel_bytes),
        quantity(observation.heap_bytes),
        quantity(observation.page_table_bytes),
        quantity(observation.user_bytes),
        quantity(observation.guest_bytes),
        quantity(observation.unattributed_bytes)
    )?;
    Ok(())
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
