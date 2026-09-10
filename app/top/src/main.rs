// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Interactive capability-scoped CPU, memory, and Process monitor.

use clap::Parser;
use std::collections::BTreeMap;
use std::io::Write;

use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::inspect::{CpuInspector, Koid, MemoryInspector, ScanCursor, TaskInspector};
use hyper_os::startup::{self, Startup};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_os::{Error, Status};
use std::process::ExitCode;

#[derive(Clone, Copy, Default)]
struct ThreadSample {
    process_koid: u64,
    thread_koid: u64,
    runtime_ticks: u64,
}

type SampleSet = BTreeMap<u64, ThreadSample>;

fn application_main(mut startup: Startup<'_>, args: hyper_top::cli::Top) -> ExitCode {
    match run(&mut startup, &args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("top: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    startup: &mut Startup<'_>,
    args: &hyper_top::cli::Top,
) -> Result<(), Box<dyn std::error::Error>> {
    hyper_os::require_core_abi()?;
    let input = if args.batch {
        None
    } else {
        Some(hyper_rt::process::stdin()?)
    };
    let refresh = std::time::Duration::from_secs_f64(args.delay);
    let mut iterations = 0_u32;
    let mut output = std::io::stdout().lock();
    let tasks = TaskInspector::from_handle(startup.take(startup::TASK_INSPECTOR)?);
    let memory = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let cpu = CpuInspector::from_handle(startup.take(startup::CPU_INSPECTOR)?);
    let mut previous_cpu = cpu.read()?;
    let mut previous_threads = capture_threads(&tasks)?;
    loop {
        if let Some(input) = input {
            if wait_or_quit(
                input,
                previous_cpu
                    .captured_at_ns
                    .saturating_add(refresh.as_nanos() as u64),
            )? {
                return Ok(());
            }
        } else {
            std::thread::sleep(refresh);
        }
        let current_cpu = cpu.read()?;
        let current_threads = capture_threads(&tasks)?;
        if !args.batch {
            output.write_all(b"\x1b[2J\x1b[H")?;
        }
        render(
            &mut output,
            &tasks,
            memory.read()?,
            previous_cpu,
            current_cpu,
            &previous_threads,
            &current_threads,
        )?;
        if !args.batch {
            writeln!(output, "Press q or Ctrl-C to quit.")?;
            output.flush()?;
        }
        iterations = iterations.saturating_add(1);
        if args
            .iterations
            .is_some_and(|limit| iterations >= limit.get())
        {
            return Ok(());
        }
        previous_cpu = current_cpu;
        previous_threads = current_threads;
    }
}

fn wait_or_quit(input: &OwnedHandle<ByteChannelObject>, deadline: u64) -> hyper_os::Result<bool> {
    let waits = [WaitItem::new(
        input.as_handle_ref(),
        ObjectSignals::<ByteChannelObject>::READABLE
            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
    )];
    loop {
        match wait_many(&waits, deadline) {
            Ok(observation) => {
                if !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.observed)
                {
                    return Ok(true);
                }
                let mut bytes = [0_u8; hyper_os::channel::MAX_MESSAGE_BYTES];
                match input.as_byte_channel().try_receive(&mut bytes) {
                    Ok(count) => {
                        return Ok(bytes[..count]
                            .iter()
                            .any(|byte| matches!(byte, b'q' | b'Q' | 3)));
                    }
                    Err(Error::Status(Status::WOULD_BLOCK)) => continue,
                    Err(Error::Status(Status::PEER_CLOSED)) => return Ok(true),
                    Err(error) => return Err(error),
                }
            }
            Err(Error::Status(Status::TIMED_OUT)) => return Ok(false),
            Err(error) => return Err(error),
        }
    }
}

fn capture_threads(tasks: &TaskInspector) -> hyper_os::Result<SampleSet> {
    let mut samples = SampleSet::new();
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = tasks.scan_threads(position)?;
        for thread in page.entries() {
            samples.insert(
                thread.koid.get(),
                ThreadSample {
                    process_koid: thread.process_koid.map_or(0, Koid::get),
                    thread_koid: thread.koid.get(),
                    runtime_ticks: thread.runtime_ticks,
                },
            );
        }
        cursor = page.next();
    }
    Ok(samples)
}

fn render(
    output: &mut impl Write,
    tasks: &TaskInspector,
    memory: hyper_os::inspect::MemoryObservation,
    previous_cpu: hyper_os::inspect::CpuObservation,
    current_cpu: hyper_os::inspect::CpuObservation,
    previous_threads: &SampleSet,
    current_threads: &SampleSet,
) -> Result<(), Box<dyn std::error::Error>> {
    let idle = current_cpu
        .idle_ticks
        .saturating_sub(previous_cpu.idle_ticks);
    let kernel_thread = current_cpu
        .kernel_thread_ticks
        .saturating_sub(previous_cpu.kernel_thread_ticks);
    let user_thread = current_cpu
        .user_thread_ticks
        .saturating_sub(previous_cpu.user_thread_ticks);
    let vcpu = current_cpu
        .vcpu_ticks
        .saturating_sub(previous_cpu.vcpu_ticks);
    let total = idle
        .saturating_add(kernel_thread)
        .saturating_add(user_thread)
        .saturating_add(vcpu);
    write!(
        output,
        "top - {} CPUs  ticks={} Hz\nCPU: user-thread ",
        current_cpu.online_cpus, current_cpu.ticks_per_second
    )
    .and_then(|()| write_percent(output, user_thread, total))
    .and_then(|()| write!(output, " kthread "))
    .and_then(|()| write_percent(output, kernel_thread, total))
    .and_then(|()| write!(output, " vcpu "))
    .and_then(|()| write_percent(output, vcpu, total))
    .and_then(|()| write!(output, " idle "))
    .and_then(|()| write_percent(output, idle, total))
    .and_then(|()| {
        writeln!(
            output,
            "\nMem: {} MiB total, {} MiB used, {} MiB free, {} MiB reserved",
            mib(memory.total_bytes),
            mib(memory.used_bytes),
            mib(memory.free_bytes),
            mib(memory.reserved_bytes),
        )
    })
    .and_then(|()| writeln!(output, "KOID       CPU      THREADS  NAME"))
    .map_err(|_| Error::InvalidResponse)?;

    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = tasks.scan_processes(position)?;
        for process in page.entries() {
            let (ticks, threads) = process_delta(process.koid, previous_threads, current_threads);
            write!(output, "{:<10} ", process.koid.get())
                .and_then(|()| write_percent(output, ticks, total))
                .and_then(|()| writeln!(output, "  {:<7}  {}", threads, process.name.as_str(),))
                .map_err(|_| Error::InvalidResponse)?;
        }
        cursor = page.next();
    }
    output.flush()?;
    Ok(())
}

fn process_delta(process: Koid, previous: &SampleSet, current: &SampleSet) -> (u64, usize) {
    let mut ticks = 0_u64;
    let mut threads = 0_usize;
    for sample in current.values() {
        if sample.process_koid == process.get() {
            ticks = ticks.saturating_add(previous.get(&sample.thread_koid).map_or(0, |old| {
                sample.runtime_ticks.saturating_sub(old.runtime_ticks)
            }));
            threads += 1;
        }
    }
    (ticks, threads)
}

fn write_percent(output: &mut impl Write, value: u64, total: u64) -> std::io::Result<()> {
    let basis_points = if total == 0 {
        0
    } else {
        (u128::from(value).saturating_mul(10_000) / u128::from(total)) as u64
    };
    write!(output, "{}.{:02}%", basis_points / 100, basis_points % 100)
}

const fn mib(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

fn main() -> ExitCode {
    let args = hyper_top::cli::Top::parse();
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup, args),
        Err(_) => ExitCode::FAILURE,
    }
}
