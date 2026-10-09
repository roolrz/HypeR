// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped physical-memory summary.

use clap::Parser;
use hyper_os::inspect::MemoryInspector;
use hyper_os::startup;
use std::io::Write;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_free::cli::Free::parse();
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let mut output = std::io::stdout().lock();
    for sample in 0..args.count.get() {
        if sample != 0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(args.seconds));
            writeln!(output)?;
        }
        snapshot(&args, inspector.read()?, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

fn snapshot(
    args: &hyper_free::cli::Free,
    observation: hyper_os::inspect::MemoryObservation,
    output: &mut impl Write,
) -> std::io::Result<()> {
    let quantity = |bytes| args.quantity(bytes);
    let cache = if observation.cache_sample_complete {
        quantity(observation.reclaimable_bytes)
    } else {
        String::from("—")
    };
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
