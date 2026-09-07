// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Interactive capability-scoped CPU, memory, and Process monitor.

#![no_std]
#![no_main]

#[path = "format.rs"]
mod format;

use core::fmt::Write;

use format::Buffer;
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::inspect::{CpuInspector, Koid, MemoryInspector, ScanCursor, TaskInspector};
use hyper_os::startup::{self, Startup};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_os::{Error, Status};
use hyper_rt::ExitCode;
use hyper_service::stdio;

const REFRESH_NS: u64 = 1_000_000_000;
const MAX_THREAD_SAMPLES: usize = 256;

#[derive(Clone, Copy, Default)]
struct ThreadSample {
    process_koid: u64,
    thread_koid: u64,
    runtime_ticks: u64,
}

struct SampleSet {
    entries: [ThreadSample; MAX_THREAD_SAMPLES],
    len: usize,
    truncated: bool,
}

impl SampleSet {
    const fn new() -> Self {
        Self {
            entries: [ThreadSample {
                process_koid: 0,
                thread_koid: 0,
                runtime_ticks: 0,
            }; MAX_THREAD_SAMPLES],
            len: 0,
            truncated: false,
        }
    }

    fn runtime_delta(&self, current: ThreadSample) -> u64 {
        self.entries[..self.len]
            .iter()
            .find(|sample| sample.thread_koid == current.thread_koid)
            .map_or(0, |previous| {
                current.runtime_ticks.saturating_sub(previous.runtime_ticks)
            })
    }
}

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn run(startup: &mut Startup<'_>) -> hyper_os::Result<()> {
    hyper_os::require_core_abi()?;
    let input = startup.take(stdio::STANDARD_INPUT)?;
    let output = startup.take(stdio::STANDARD_OUTPUT)?;
    let tasks = TaskInspector::from_handle(startup.take(startup::TASK_INSPECTOR)?);
    let memory = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let cpu = CpuInspector::from_handle(startup.take(startup::CPU_INSPECTOR)?);
    let mut previous_cpu = cpu.read()?;
    let mut previous_threads = capture_threads(&tasks)?;
    loop {
        if wait_or_quit(
            &input,
            previous_cpu.captured_at_ns.saturating_add(REFRESH_NS),
        )? {
            return Ok(());
        }
        let current_cpu = cpu.read()?;
        let current_threads = capture_threads(&tasks)?;
        render(
            &output,
            &tasks,
            memory.read()?,
            previous_cpu,
            current_cpu,
            &previous_threads,
            &current_threads,
        )?;
        previous_cpu = current_cpu;
        previous_threads = current_threads;
    }
}

fn wait_or_quit(input: &OwnedHandle<ByteChannelObject>, deadline: u64) -> hyper_os::Result<bool> {
    let waits = [WaitItem::new(
        input.as_handle_ref(),
        ObjectSignals::<ByteChannelObject>::READABLE,
    )];
    match wait_many(&waits, deadline) {
        Ok(_) => {
            let mut bytes = [0_u8; 16];
            let count = input.as_byte_channel().receive(&mut bytes)?;
            Ok(bytes[..count]
                .iter()
                .any(|byte| matches!(byte, b'q' | b'Q')))
        }
        Err(Error::Status(status)) if status == Status::TIMED_OUT => Ok(false),
        Err(error) => Err(error),
    }
}

fn capture_threads(tasks: &TaskInspector) -> hyper_os::Result<SampleSet> {
    let mut samples = SampleSet::new();
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = tasks.scan_threads(position)?;
        for thread in page.entries() {
            let Some(slot) = samples.entries.get_mut(samples.len) else {
                samples.truncated = true;
                return Ok(samples);
            };
            *slot = ThreadSample {
                process_koid: thread.process_koid.map_or(0, Koid::get),
                thread_koid: thread.koid.get(),
                runtime_ticks: thread.runtime_ticks,
            };
            samples.len += 1;
        }
        cursor = page.next();
    }
    Ok(samples)
}

fn render(
    output: &OwnedHandle<ByteChannelObject>,
    tasks: &TaskInspector,
    memory: hyper_os::inspect::MemoryObservation,
    previous_cpu: hyper_os::inspect::CpuObservation,
    current_cpu: hyper_os::inspect::CpuObservation,
    previous_threads: &SampleSet,
    current_threads: &SampleSet,
) -> hyper_os::Result<()> {
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
    let mut header = Buffer::<768>::new();
    write!(
        header,
        "\x1b[2J\x1b[Htop - {} CPUs  ticks={} Hz\nCPU: user-thread ",
        current_cpu.online_cpus, current_cpu.ticks_per_second
    )
    .and_then(|()| write_percent(&mut header, user_thread, total))
    .and_then(|()| write!(header, " kthread "))
    .and_then(|()| write_percent(&mut header, kernel_thread, total))
    .and_then(|()| write!(header, " vcpu "))
    .and_then(|()| write_percent(&mut header, vcpu, total))
    .and_then(|()| write!(header, " idle "))
    .and_then(|()| write_percent(&mut header, idle, total))
    .and_then(|()| {
        writeln!(
            header,
            "\nMem: {} MiB total, {} MiB used, {} MiB free, {} MiB reserved",
            mib(memory.total_bytes),
            mib(memory.used_bytes),
            mib(memory.free_bytes),
            mib(memory.reserved_bytes),
        )
    })
    .and_then(|()| writeln!(header, "KOID       CPU      THREADS  NAME"))
    .map_err(|_| Error::InvalidResponse)?;
    output.as_byte_channel().send(header.bytes())?;

    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = tasks.scan_processes(position)?;
        for process in page.entries() {
            let (ticks, threads) = process_delta(process.koid, previous_threads, current_threads);
            let mut line = Buffer::<256>::new();
            write!(line, "{:<10} ", process.koid.get())
                .and_then(|()| write_percent(&mut line, ticks, total))
                .and_then(|()| writeln!(line, "  {:<7}  {}", threads, process.name.as_str(),))
                .map_err(|_| Error::InvalidResponse)?;
            output.as_byte_channel().send(line.bytes())?;
        }
        cursor = page.next();
    }
    if previous_threads.truncated || current_threads.truncated {
        output
            .as_byte_channel()
            .send(b"warning: thread accounting display was truncated\n")?;
    }
    output.as_byte_channel().send(b"Press q to quit.\n")
}

fn process_delta(process: Koid, previous: &SampleSet, current: &SampleSet) -> (u64, usize) {
    let mut ticks = 0_u64;
    let mut threads = 0_usize;
    for sample in &current.entries[..current.len] {
        if sample.process_koid == process.get() {
            ticks = ticks.saturating_add(previous.runtime_delta(*sample));
            threads += 1;
        }
    }
    (ticks, threads)
}

fn write_percent<const N: usize>(
    output: &mut Buffer<N>,
    value: u64,
    total: u64,
) -> core::fmt::Result {
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

hyper_rt::entry!(application_main);
