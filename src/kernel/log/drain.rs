// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deferred normal-console ownership and bounded drain policy.

use core::fmt::Write;

use hyper::drivers::console::ConsoleDevice;
use hyper::log::{
    ByteRing, DeferredDrain, DrainBarrierError, DrainDisposition, OutputBuffer, OutputError,
    ReadResult, Record, RecordFlags,
};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicU8, Ordering};

use crate::kernel::sync::Completion;
use crate::kernel::task::scheduler;

type RawQueueLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

const BOOT: u8 = 0;
const RUNTIME: u8 = 1;
const EMERGENCY: u8 = 2;
const LOG_RECORDS_PER_BATCH: usize = 32;
const RAW_QUEUE_CAPACITY: usize = 4096;
const RAW_BYTES_PER_BATCH: usize = 256;
const LOG_LINE_MAX: usize = hyper::config::LOG_LINE_MAX as usize;
const OUTPUT_BUFFER_SIZE: usize = LOG_LINE_MAX * 2 + 128;
const UART_BYTES_PER_BATCH: usize = 256;

static MODE: AtomicU8 = AtomicU8::new(BOOT);
static WORK: DeferredDrain = DeferredDrain::new();
static WAKE: Completion = Completion::new();
static RAW_QUEUE: RawQueueLock<RawQueueState> = InterruptSpinLock::new(RawQueueState::new());

struct RawQueueState {
    bytes: ByteRing<RAW_QUEUE_CAPACITY>,
    reported_dropped: u64,
}

#[derive(Clone, Copy)]
enum PendingCommit {
    None,
    Log { sequence: u64, missed: u64 },
    RawByte(u8),
    RawOverflow { total_dropped: u64 },
    RingFailure,
}

struct WorkerOutput {
    bytes: OutputBuffer<OUTPUT_BUFFER_SIZE>,
    commit: PendingCommit,
    device: Option<ConsoleDevice>,
    prefer_raw: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BatchOutcome {
    Idle,
    More,
    Backpressured,
    Emergency,
}

impl WorkerOutput {
    const fn new() -> Self {
        Self {
            bytes: OutputBuffer::new(),
            commit: PendingCommit::None,
            device: None,
            prefer_raw: false,
        }
    }
}

impl RawQueueState {
    const fn new() -> Self {
        Self {
            bytes: ByteRing::new(),
            reported_dropped: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    AlreadyInitialized,
    Scheduler(scheduler::Error),
    WorkerOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlushOutcome {
    Drained,
    Overrun { missed: u64 },
    NoConsole,
    Emergency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlushError {
    NotRuntime,
    Barrier(DrainBarrierError),
    Wait(crate::kernel::sync::Error),
}

impl From<scheduler::Error> for InitializationError {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

/// Creates the sole normal-runtime UART writer and changes producer ownership.
///
/// The worker is made runnable before Runtime publication. The bootstrap
/// Thread remains current throughout this call, so it cannot observe partially
/// initialized deferred state through a context switch.
pub(super) fn initialize() -> Result<(), InitializationError> {
    if MODE.load(Ordering::Acquire) != BOOT {
        return Err(InitializationError::AlreadyInitialized);
    }
    let worker = scheduler::kthread_create("klogd", worker_entry, 0)?;
    if !WORK.claim_initial_worker() {
        return Err(InitializationError::WorkerOwnership);
    }
    if !scheduler::thread_ready(worker)? {
        return Err(InitializationError::Scheduler(
            scheduler::Error::InvalidThreadState,
        ));
    }
    MODE.compare_exchange(BOOT, RUNTIME, Ordering::Release, Ordering::Acquire)
        .map_err(|_| InitializationError::AlreadyInitialized)?;
    let _ = WORK.request();
    Ok(())
}

/// Publishes normal output without entering scheduler or console-driver code.
pub(super) fn request() {
    let mode = MODE.load(Ordering::Acquire);
    if mode == BOOT {
        drain_boot_synchronously();
        return;
    }
    if mode == RUNTIME && WORK.request() {
        prompt_local_cpu();
    }
}

/// Converts a durable producer request into one scheduler wake at IRQ entry.
///
/// Interrupt registry dispatch has already returned before this seam runs, so
/// Completion cannot nest inside a registered interrupt handler or its lock.
pub(crate) fn service_irq_prompt() {
    if MODE.load(Ordering::Acquire) != RUNTIME
        || !WORK.consume_prompt()
        || !WORK.claim_notification()
    {
        return;
    }
    if let Err(error) = WAKE.complete() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: deferred log wake invariant failed: {error:?}"
        ));
    }
}

/// Waits in schedulable Thread context for records present at call time.
pub(super) fn flush_sync() -> Result<FlushOutcome, FlushError> {
    if MODE.load(Ordering::Acquire) != RUNTIME {
        return if MODE.load(Ordering::Acquire) == EMERGENCY {
            Ok(FlushOutcome::Emergency)
        } else {
            Err(FlushError::NotRuntime)
        };
    }
    scheduler::ensure_sleepable()
        .map_err(crate::kernel::sync::Error::from)
        .map_err(FlushError::Wait)?;
    let registration = super::console::register_flush_barrier().map_err(FlushError::Barrier)?;
    let outcome = match registration {
        super::console::FlushBarrierRegistration::Complete(outcome) => outcome,
        super::console::FlushBarrierRegistration::Pending(barrier) => {
            request();
            super::console::wait_for_drain(barrier).map_err(FlushError::Wait)?
        }
    };
    match outcome {
        super::console::ConsoleFlushOutcome::Drained => Ok(FlushOutcome::Drained),
        super::console::ConsoleFlushOutcome::Overrun { missed } => {
            Ok(FlushOutcome::Overrun { missed })
        }
        super::console::ConsoleFlushOutcome::NoConsole => Ok(FlushOutcome::NoConsole),
        super::console::ConsoleFlushOutcome::Emergency => Ok(FlushOutcome::Emergency),
    }
}

/// Sends only a hardware/event prompt; the durable work bit remains authoritative.
fn prompt_local_cpu() {
    match crate::kernel::cpu::current_index() {
        Some(cpu) => crate::kernel::irq::reschedule::notify(cpu),
        None => crate::hal::cpu::send_event(),
    }
}

/// Enqueues a guest-console byte without touching the physical UART.
pub(super) fn enqueue_raw(byte: u8) {
    if MODE.load(Ordering::Acquire) == EMERGENCY {
        return;
    }
    RAW_QUEUE.with(|queue| {
        let _ = queue.bytes.push(byte);
    });
    request();
}

/// Gives fatal output permanent priority over normal runtime draining.
pub(super) fn enter_emergency_mode() {
    MODE.store(EMERGENCY, Ordering::Release);
}

extern "C" fn worker_entry(_argument: usize) {
    let mut output = WorkerOutput::new();
    loop {
        if MODE.load(Ordering::Acquire) == EMERGENCY {
            scheduler::exit_current();
        }

        WORK.begin_batch();
        match drain_runtime_batch(&mut output) {
            BatchOutcome::Emergency => scheduler::exit_current(),
            BatchOutcome::Backpressured => {
                // Retain the exact pending output but leave the run queue until
                // a later periodic IRQ observes the prompt. A wedged UART can
                // therefore consume at most one bounded attempt per IRQ.
                WORK.defer_until_irq();
                wait_for_worker_prompt();
            }
            BatchOutcome::More => {
                if let Err(error) = scheduler::cond_resched() {
                    worker_failure("reschedule", error)
                }
            }
            BatchOutcome::Idle => match WORK.finish_batch(false) {
                DrainDisposition::Continue => {
                    if let Err(error) = scheduler::cond_resched() {
                        worker_failure("reschedule", error)
                    }
                }
                DrainDisposition::Wait => wait_for_worker_prompt(),
            },
        }
    }
}

fn wait_for_worker_prompt() {
    if let Err(error) = WAKE.wait() {
        worker_failure("wait", error)
    }
}

fn worker_failure(operation: &str, error: impl core::fmt::Debug) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: deferred log worker {operation} failed: {error:?}"
    ))
}

fn drain_boot_synchronously() {
    while drain_boot_log_records() || drain_boot_raw_bytes() {}
}

fn drain_runtime_batch(output: &mut WorkerOutput) -> BatchOutcome {
    if MODE.load(Ordering::Acquire) == EMERGENCY {
        return BatchOutcome::Emergency;
    }
    let mut accepted = 0;
    while accepted < UART_BYTES_PER_BATCH {
        if output.bytes.is_empty() {
            let Some(snapshot) = super::console::output_snapshot() else {
                return if MODE.load(Ordering::Acquire) == EMERGENCY {
                    BatchOutcome::Emergency
                } else {
                    BatchOutcome::Idle
                };
            };
            if !prepare_runtime_output(output, snapshot) {
                return if MODE.load(Ordering::Acquire) == EMERGENCY {
                    BatchOutcome::Emergency
                } else {
                    BatchOutcome::Idle
                };
            }
        }
        let Some(device) = output.device else {
            worker_failure("output device", "missing frame owner")
        };
        let budget = UART_BYTES_PER_BATCH - accepted;
        let mut retired = false;
        let progress =
            output
                .bytes
                .try_write(
                    budget,
                    |byte| match super::console::try_write_runtime_byte(device, byte) {
                        super::console::RuntimeByteWrite::Accepted => true,
                        super::console::RuntimeByteWrite::WouldBlock => false,
                        super::console::RuntimeByteWrite::Retired => {
                            retired = true;
                            false
                        }
                    },
                );
        if retired {
            return BatchOutcome::Emergency;
        }
        accepted += progress.accepted;
        if !progress.complete {
            return if progress.blocked {
                BatchOutcome::Backpressured
            } else {
                BatchOutcome::More
            };
        }
        commit_runtime_output(output);
    }
    BatchOutcome::More
}

fn prepare_runtime_output(
    output: &mut WorkerOutput,
    snapshot: super::console::OutputSnapshot,
) -> bool {
    output.bytes.clear();
    output.commit = PendingCommit::None;
    let prepared = match output.prefer_raw {
        true => {
            if prepare_raw_output(output) {
                true
            } else {
                prepare_log_output(output, snapshot)
            }
        }
        false => {
            if prepare_log_output(output, snapshot) {
                true
            } else {
                prepare_raw_output(output)
            }
        }
    };
    if prepared {
        output.device = Some(snapshot.device);
        output.prefer_raw = !output.prefer_raw;
    }
    prepared
}

fn prepare_log_output(
    output: &mut WorkerOutput,
    mut snapshot: super::console::OutputSnapshot,
) -> bool {
    let mut message = [0u8; LOG_LINE_MAX];
    for _ in 0..LOG_RECORDS_PER_BATCH {
        match super::read(snapshot.next_sequence, &mut message) {
            Ok(ReadResult::Record(record)) => {
                if record.level > snapshot.maximum_level {
                    publish_log_progress(record.sequence.wrapping_add(1), 0);
                    let Some(next) = super::console::output_snapshot() else {
                        return false;
                    };
                    snapshot = next;
                    continue;
                }
                if prepare_record(&mut output.bytes, record, &message[..record.copied]).is_err() {
                    worker_failure("record formatting", OutputError::Full)
                }
                output.commit = PendingCommit::Log {
                    sequence: record.sequence.wrapping_add(1),
                    missed: 0,
                };
                return true;
            }
            Ok(ReadResult::Overrun {
                oldest_sequence,
                missed,
            }) => {
                if prepare_overrun(&mut output.bytes, missed).is_err() {
                    worker_failure("overrun formatting", OutputError::Full)
                }
                output.commit = PendingCommit::Log {
                    sequence: oldest_sequence,
                    missed,
                };
                return true;
            }
            Ok(ReadResult::Empty { .. }) => return false,
            Err(error) => {
                if prepare_ring_failure(&mut output.bytes, error).is_err() {
                    worker_failure("ring-failure formatting", OutputError::Full)
                }
                output.commit = PendingCommit::RingFailure;
                return true;
            }
        }
    }
    // A run of filtered records still consumes only a bounded batch. Retain a
    // durable internal request so the worker continues without external help.
    let _ = WORK.request();
    false
}

fn prepare_raw_output(output: &mut WorkerOutput) -> bool {
    let (byte, dropped, reported) = RAW_QUEUE.with(|queue| {
        (
            queue.bytes.front(),
            queue.bytes.dropped(),
            queue.reported_dropped,
        )
    });
    if dropped > reported {
        if prepare_raw_overflow(&mut output.bytes, dropped - reported).is_err() {
            worker_failure("raw-overflow formatting", OutputError::Full)
        }
        output.commit = PendingCommit::RawOverflow {
            total_dropped: dropped,
        };
        return true;
    }
    let Some(byte) = byte else {
        return false;
    };
    if output
        .bytes
        .push_console_bytes(core::slice::from_ref(&byte))
        .is_err()
    {
        worker_failure("raw-byte formatting", OutputError::Full)
    }
    output.commit = PendingCommit::RawByte(byte);
    true
}

fn commit_runtime_output(output: &mut WorkerOutput) {
    match output.commit {
        PendingCommit::None => worker_failure("output commit", "missing commit owner"),
        PendingCommit::Log { sequence, missed } => publish_log_progress(sequence, missed),
        PendingCommit::RawByte(expected) => RAW_QUEUE.with(|queue| {
            if queue.bytes.pop_front() != Some(expected) {
                worker_failure("raw-byte commit", "queue head changed")
            }
        }),
        PendingCommit::RawOverflow { total_dropped } => RAW_QUEUE.with(|queue| {
            if queue.reported_dropped < total_dropped {
                queue.reported_dropped = total_dropped;
            }
        }),
        PendingCommit::RingFailure => worker_failure(
            "ring integrity",
            "ring read failed after diagnostic publication",
        ),
    }
    output.bytes.clear();
    output.commit = PendingCommit::None;
    output.device = None;
}

fn publish_log_progress(sequence: u64, missed: u64) {
    let result = if missed == 0 {
        super::console::advance(sequence)
    } else {
        super::console::advance_overrun(sequence, missed)
    };
    if let Err(error) = result {
        worker_failure("record progress publication", error)
    }
}

fn prepare_record(
    output: &mut OutputBuffer<OUTPUT_BUFFER_SIZE>,
    record: Record,
    message: &[u8],
) -> Result<(), OutputError> {
    write!(output, "<{}>[{:06}] ", record.level as u8, record.sequence)
        .map_err(|_| OutputError::Full)?;
    output.push_console_bytes(message)?;
    if record.flags.contains(RecordFlags::TRUNCATED) || record.copied != record.length {
        output.push_console_bytes(b" [truncated]")?;
    }
    if !message.ends_with(b"\n") {
        output.push_console_bytes(b"\n")?;
    }
    Ok(())
}

fn prepare_overrun(
    output: &mut OutputBuffer<OUTPUT_BUFFER_SIZE>,
    missed: u64,
) -> Result<(), OutputError> {
    write!(output, "<4>[log] {missed} record(s) lost").map_err(|_| OutputError::Full)?;
    output.push_console_bytes(b"\n")
}

fn prepare_ring_failure(
    output: &mut OutputBuffer<OUTPUT_BUFFER_SIZE>,
    error: super::Error,
) -> Result<(), OutputError> {
    write!(output, "<2>[log] ring read failure: {error:?}").map_err(|_| OutputError::Full)?;
    output.push_console_bytes(b"\n")
}

fn prepare_raw_overflow(
    output: &mut OutputBuffer<OUTPUT_BUFFER_SIZE>,
    dropped: u64,
) -> Result<(), OutputError> {
    write!(output, "<4>[console] {dropped} guest console byte(s) lost")
        .map_err(|_| OutputError::Full)?;
    output.push_console_bytes(b"\n")
}

fn drain_boot_log_records() -> bool {
    let mut message = [0u8; LOG_LINE_MAX];
    for _ in 0..LOG_RECORDS_PER_BATCH {
        let Some(output) = super::console::output_snapshot() else {
            return false;
        };
        match super::read(output.next_sequence, &mut message) {
            Ok(ReadResult::Record(record)) => {
                if record.level <= output.maximum_level {
                    super::console::write_record(&output.device, record, &message[..record.copied]);
                }
                publish_log_progress(record.sequence.wrapping_add(1), 0);
            }
            Ok(ReadResult::Overrun {
                oldest_sequence,
                missed,
            }) => {
                super::console::write_overrun(&output.device, missed);
                publish_log_progress(oldest_sequence, missed);
            }
            Ok(ReadResult::Empty { .. }) => return false,
            Err(error) => {
                super::console::write_ring_failure(&output.device, error);
                return false;
            }
        }
    }
    true
}

fn drain_boot_raw_bytes() -> bool {
    let Some(output) = super::console::output_snapshot() else {
        return false;
    };
    let mut bytes = [0u8; RAW_BYTES_PER_BATCH];
    let (count, more, newly_dropped) = RAW_QUEUE.with(|queue| {
        let count = queue.bytes.pop_into(&mut bytes);
        let more = !queue.bytes.is_empty();
        let dropped = queue.bytes.dropped().saturating_sub(queue.reported_dropped);
        queue.reported_dropped = queue.bytes.dropped();
        (count, more, dropped)
    });
    if newly_dropped != 0 {
        super::console::write_raw_overflow(&output.device, newly_dropped);
    }
    if count != 0 {
        super::console::write_raw(&output.device, &bytes[..count]);
    }
    more
}

/// Drains retained boot records before retiring an identity-mapped console.
pub(super) fn flush_boot() {
    if MODE.load(Ordering::Acquire) == BOOT {
        drain_boot_synchronously();
    }
}
