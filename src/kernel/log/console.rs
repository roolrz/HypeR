// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Console ownership, filtering, and fatal-output access.

use core::fmt::Write;

use hyper::drivers::console::{ConsoleDevice, EmergencyConsoleHandle};
use hyper::hal::console::{Console, ConsoleWriter};
use hyper::log::{
    DrainBarrierError, DrainBarrierRegistration, DrainBarrierSet, DrainBarrierStatus,
    DrainBarrierToken, EmergencyQuiescence, EmergencyWriteGate, Level, Record, RecordFlags,
    RuntimeByteAccess,
};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicUsize, Ordering};

type KernelSpinLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

const CONSOLE_LOGLEVEL: Level = configured_console_level();
const FLUSH_BARRIER_SLOTS: usize = crate::kernel::task::scheduler::THREAD_CAPACITY;
const EMERGENCY_QUIESCENCE_POLLS: usize = 4096;
const EMERGENCY_UART_ATTEMPTS: usize = 4096;

struct ConsoleState {
    device: Option<ConsoleDevice>,
    next_sequence: u64,
    maximum_level: Level,
    barriers: DrainBarrierSet<FLUSH_BARRIER_SLOTS>,
    barrier_waiters: [crate::kernel::task::WaitQueue; FLUSH_BARRIER_SLOTS],
}

impl ConsoleState {
    const fn new() -> Self {
        Self {
            device: None,
            next_sequence: 0,
            maximum_level: CONSOLE_LOGLEVEL,
            barriers: DrainBarrierSet::new(),
            barrier_waiters: [const { crate::kernel::task::WaitQueue::new() }; FLUSH_BARRIER_SLOTS],
        }
    }
}

static CONSOLE: KernelSpinLock<ConsoleState> = KernelSpinLock::new(ConsoleState::new());
static EMERGENCY_CONSOLE: AtomicUsize = AtomicUsize::new(0);
static EMERGENCY_CONSOLE_METADATA: AtomicUsize = AtomicUsize::new(0);
static EMERGENCY_CONSOLE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static WRITE_GATE: EmergencyWriteGate = EmergencyWriteGate::new();

pub fn install(console: ConsoleDevice) {
    let emergency = console.emergency_handle();
    CONSOLE.with(|state| {
        publish_emergency_handle(Some(emergency));
        state.device = Some(console);
    });
    super::drain::request();
}

/// Retires the identity-mapped console before runtime promotion.
///
/// This is a one-way boot transition. Failure between retirement and the next
/// [`install`] remains diagnosable through the log ring, but must not access a
/// virtual address whose bootstrap mapping no longer exists.
pub(crate) fn retire_bootstrap() {
    super::drain::flush_boot();
    CONSOLE.with(|state| {
        publish_emergency_handle(None);
        state.device = None;
    });
}

/// Publishes both machine words as one sequence-validated snapshot.
///
/// `CONSOLE` serializes writers. Sequential consistency is intentional here:
/// installation is a boot-only slow path, while a fatal reader must never
/// combine the virtual base from one mapping with another driver's metadata.
fn publish_emergency_handle(handle: Option<EmergencyConsoleHandle>) {
    // `CONSOLE` serializes every publisher, so this writer does not need an
    // atomic read-modify-write. Avoiding one also keeps early-console
    // publication independent of outlined atomic RMW helpers.
    let sequence = EMERGENCY_CONSOLE_SEQUENCE.load(Ordering::SeqCst);
    EMERGENCY_CONSOLE_SEQUENCE.store(sequence.wrapping_add(1), Ordering::SeqCst);
    let handle = match handle {
        Some(handle) => handle,
        None => EmergencyConsoleHandle {
            base: 0,
            metadata: 0,
        },
    };
    EMERGENCY_CONSOLE.store(handle.base, Ordering::SeqCst);
    EMERGENCY_CONSOLE_METADATA.store(handle.metadata, Ordering::SeqCst);
    EMERGENCY_CONSOLE_SEQUENCE.store(sequence.wrapping_add(2), Ordering::SeqCst);
}

/// Changes only console filtering; every severity remains in the ring.
pub fn set_loglevel(level: Level) {
    CONSOLE.with(|state| state.maximum_level = level);
    super::drain::request();
}

pub fn loglevel() -> Level {
    CONSOLE.with(|state| state.maximum_level)
}

/// Stops normal ring draining once fatal diagnostics switch to direct output.
///
/// Emergency records remain retained for post-mortem readers, but allowing an
/// unrelated CPU to drain them would print every fatal line a second time.
pub(super) fn enter_emergency_mode() -> EmergencyQuiescence {
    // A normal transaction is one nonblocking status read plus, at most, one
    // byte write, so a remote owner should quiesce well inside this fixed
    // bound. A timeout fails closed: the retained crash record remains
    // available, but direct UART output stays disabled rather than racing a
    // possibly stalled MMIO transaction.
    let current_cpu = crate::kernel::cpu::current_index()
        .map(|cpu| cpu.get())
        .unwrap_or(usize::MAX);
    WRITE_GATE.retire_normal_writer(current_cpu, EMERGENCY_QUIESCENCE_POLLS)
}

/// Writes a best-effort fatal message without waiting for kernel log locks.
pub(super) fn emergency_write(message: &[u8]) {
    let Some(device) = emergency_device() else {
        return;
    };
    let mut attempts = EMERGENCY_UART_ATTEMPTS;
    emergency_write_bytes(&device, b"<0>[exception] ", &mut attempts);
    emergency_write_bytes(&device, message, &mut attempts);
    if !message.ends_with(b"\n") {
        emergency_write_bytes(&device, b"\n", &mut attempts);
    }
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(super) fn emergency_available() -> bool {
    emergency_device().is_some()
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(super) fn emergency_write_raw(bytes: &[u8]) {
    if let Some(device) = emergency_device() {
        let mut attempts = EMERGENCY_UART_ATTEMPTS;
        emergency_write_bytes(&device, bytes, &mut attempts);
    }
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(super) fn emergency_read_raw() -> Option<u8> {
    emergency_device().and_then(|device| device.try_read_byte())
}

fn emergency_device() -> Option<ConsoleDevice> {
    if !WRITE_GATE.emergency_enabled() {
        return None;
    }
    let sequence = EMERGENCY_CONSOLE_SEQUENCE.load(Ordering::SeqCst);
    if sequence & 1 != 0 {
        // A synchronous failure may interrupt the installing CPU. Do not spin
        // waiting for a writer that cannot resume on this execution path.
        return None;
    }
    let handle = EmergencyConsoleHandle {
        base: EMERGENCY_CONSOLE.load(Ordering::SeqCst),
        metadata: EMERGENCY_CONSOLE_METADATA.load(Ordering::SeqCst),
    };
    if EMERGENCY_CONSOLE_SEQUENCE.load(Ordering::SeqCst) != sequence {
        return None;
    }
    // SAFETY: install publishes only a currently live bootstrap or permanent
    // mapping, and retirement clears the snapshot before invalidating the
    // bootstrap address. Crash handling is the sole lock-free consumer.
    unsafe { ConsoleDevice::from_emergency_handle(handle, crate::hal::platform::port_io()) }
}

/// Enqueues one guest-console byte for the sole runtime console writer.
pub(crate) fn write_raw_byte(byte: u8) {
    super::drain::enqueue_raw(byte);
}

#[derive(Clone, Copy)]
pub(super) struct OutputSnapshot {
    pub(super) device: ConsoleDevice,
    pub(super) next_sequence: u64,
    pub(super) maximum_level: Level,
}

pub(super) fn output_snapshot() -> Option<OutputSnapshot> {
    if WRITE_GATE.is_retired() {
        return None;
    }
    CONSOLE.with(|state| {
        state.device.map(|device| OutputSnapshot {
            device,
            next_sequence: state.next_sequence,
            maximum_level: state.maximum_level,
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeByteWrite {
    Accepted,
    WouldBlock,
    Retired,
}

/// Attempts one runtime byte while excluding emergency ownership transition.
pub(super) fn try_write_runtime_byte(device: ConsoleDevice, byte: u8) -> RuntimeByteWrite {
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        return RuntimeByteWrite::WouldBlock;
    };
    match WRITE_GATE.try_begin_normal_byte(cpu.get()) {
        RuntimeByteAccess::Acquired(_permit) => {
            if device.try_write_byte(byte) {
                RuntimeByteWrite::Accepted
            } else {
                RuntimeByteWrite::WouldBlock
            }
        }
        RuntimeByteAccess::Busy => RuntimeByteWrite::WouldBlock,
        RuntimeByteAccess::Retired => RuntimeByteWrite::Retired,
    }
}

/// Writes until the fixed attempt budget is consumed, preserving CRLF policy.
fn emergency_write_bytes(console: &ConsoleDevice, bytes: &[u8], attempts: &mut usize) {
    for &byte in bytes {
        if byte == b'\n' && !emergency_write_byte(console, b'\r', attempts) {
            return;
        }
        if !emergency_write_byte(console, byte, attempts) {
            return;
        }
    }
}

fn emergency_write_byte(console: &ConsoleDevice, byte: u8, attempts: &mut usize) -> bool {
    while *attempts != 0 {
        *attempts -= 1;
        if console.try_write_byte(byte) {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

pub(super) fn advance(sequence: u64) -> Result<(), ConsoleProgressError> {
    CONSOLE.with(|state| {
        if state.next_sequence < sequence {
            state.next_sequence = sequence;
            state.barriers.advance(sequence);
        }
        wake_completed_barriers(state).map_err(ConsoleProgressError::Scheduler)
    })
}

pub(super) fn advance_overrun(sequence: u64, missed: u64) -> Result<(), ConsoleProgressError> {
    CONSOLE.with(|state| {
        if state.next_sequence < sequence {
            state
                .barriers
                .advance_overrun(state.next_sequence, sequence, missed)
                .map_err(ConsoleProgressError::Barrier)?;
            state.next_sequence = sequence;
        }
        wake_completed_barriers(state).map_err(ConsoleProgressError::Scheduler)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConsoleProgressError {
    Barrier(DrainBarrierError),
    Scheduler(crate::kernel::task::scheduler::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleFlushOutcome {
    Drained,
    Overrun { missed: u64 },
    NoConsole,
    Emergency,
}

pub(super) enum FlushBarrierRegistration {
    Complete(ConsoleFlushOutcome),
    Pending(FlushBarrier),
}

pub(crate) struct FlushBarrier {
    token: Option<DrainBarrierToken>,
}

/// Registers a finite log-ring watermark while the console cursor is stable.
///
/// Lock order is permanently `CONSOLE -> LOG_RING`. Producers release
/// `LOG_RING` before requesting a drain, and the worker releases it before
/// advancing `CONSOLE`, so the reverse nested order does not exist.
pub(super) fn register_flush_barrier() -> Result<FlushBarrierRegistration, DrainBarrierError> {
    CONSOLE.with(|state| {
        if WRITE_GATE.is_retired() {
            return Ok(FlushBarrierRegistration::Complete(
                ConsoleFlushOutcome::Emergency,
            ));
        }
        if state.device.is_none() {
            return Ok(FlushBarrierRegistration::Complete(
                ConsoleFlushOutcome::NoConsole,
            ));
        }
        let target_sequence = super::statistics().next_sequence;
        match state
            .barriers
            .register(state.next_sequence, target_sequence)?
        {
            DrainBarrierRegistration::Complete => Ok(FlushBarrierRegistration::Complete(
                ConsoleFlushOutcome::Drained,
            )),
            DrainBarrierRegistration::Pending(token) => {
                Ok(FlushBarrierRegistration::Pending(FlushBarrier {
                    token: Some(token),
                }))
            }
        }
    })
}

pub(super) fn wait_for_drain(
    mut barrier: FlushBarrier,
) -> Result<ConsoleFlushOutcome, crate::kernel::sync::Error> {
    use crate::kernel::task::scheduler::{self, PrepareWait};
    use crate::kernel::task::{WaitMobility, WaitOutcome};

    scheduler::ensure_sleepable()?;
    loop {
        let token = barrier.token_or_invariant();
        // SAFETY: The retained IRQ mask is consumed by the park transition or
        // dropped before this function resumes ordinary Thread execution.
        let (prepared, interrupt_mask) = unsafe {
            CONSOLE.with_mask_retained(|state| {
                let status = barrier_status_or_invariant(state, token);
                match status {
                    DrainBarrierStatus::Pending => {
                        let registration = scheduler::begin_wait(WaitMobility::Migratable)?;
                        scheduler::prepare_registered_park_locked(
                            &state.barrier_waiters[token.slot()],
                            registration,
                        )
                        .map(Some)
                    }
                    _ => Ok(None),
                }
            })
        };
        let Some(prepared) = prepared? else {
            drop(interrupt_mask);
            let status = CONSOLE.with(|state| barrier_status_or_invariant(state, token));
            let outcome = match status {
                DrainBarrierStatus::Drained => ConsoleFlushOutcome::Drained,
                DrainBarrierStatus::Overrun { missed } => ConsoleFlushOutcome::Overrun { missed },
                DrainBarrierStatus::Pending => continue,
            };
            barrier.release();
            return Ok(outcome);
        };
        let outcome = match prepared {
            PrepareWait::Park(commit) => {
                scheduler::complete_park(scheduler::retain_park_mask(commit, interrupt_mask))
            }
            PrepareWait::Completed(outcome) => {
                drop(interrupt_mask);
                outcome
            }
        };
        if outcome != WaitOutcome::Notified {
            return Err(crate::kernel::sync::Error::WaitInterrupted(outcome));
        }
    }
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn register_pending_flush_barrier_for_test() -> Result<FlushBarrier, DrainBarrierError> {
    CONSOLE.with(
        |state| match state.barriers.register(state.next_sequence, u64::MAX)? {
            DrainBarrierRegistration::Pending(token) => Ok(FlushBarrier { token: Some(token) }),
            DrainBarrierRegistration::Complete => {
                barrier_invariant("test flush barrier unexpectedly completed during registration")
            }
        },
    )
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn wait_pending_flush_barrier_for_test(
    barrier: FlushBarrier,
) -> Result<ConsoleFlushOutcome, crate::kernel::sync::Error> {
    wait_for_drain(barrier)
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn flush_barrier_waiter_count_for_test(
    slot: usize,
) -> Result<usize, crate::kernel::task::scheduler::Error> {
    CONSOLE.with(|state| match state.barrier_waiters.get(slot) {
        Some(waiters) => waiters.len(),
        None => Err(crate::kernel::task::scheduler::Error::InvalidWaitRegistration),
    })
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn cancel_flush_barrier_waiter_for_test(
    slot: usize,
    thread: crate::kernel::task::thread::ThreadId,
) -> Result<bool, crate::kernel::task::scheduler::Error> {
    CONSOLE.with(|state| match state.barrier_waiters.get(slot) {
        Some(waiters) => waiters.cancel(thread),
        None => Err(crate::kernel::task::scheduler::Error::InvalidWaitRegistration),
    })
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn active_flush_barrier_count_for_test() -> usize {
    CONSOLE.with(|state| state.barriers.active_count())
}

impl FlushBarrier {
    fn token_or_invariant(&self) -> DrainBarrierToken {
        match self.token {
            Some(token) => token,
            None => barrier_invariant("flush barrier token consumed twice"),
        }
    }

    fn release(&mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        let result = CONSOLE.with(|state| state.barriers.release(token));
        if result.is_err() {
            barrier_invariant("flush barrier release used a stale token")
        }
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn slot_for_test(&self) -> usize {
        self.token_or_invariant().slot()
    }
}

impl Drop for FlushBarrier {
    fn drop(&mut self) {
        self.release();
    }
}

fn barrier_status_or_invariant(
    state: &ConsoleState,
    token: DrainBarrierToken,
) -> DrainBarrierStatus {
    match state.barriers.status(token) {
        Ok(status) => status,
        Err(_) => barrier_invariant("flush barrier observation used a stale token"),
    }
}

fn wake_completed_barriers(
    state: &mut ConsoleState,
) -> Result<(), crate::kernel::task::scheduler::Error> {
    if state.barriers.active_count() == 0 || !crate::kernel::task::is_ready() {
        return Ok(());
    }
    for index in 0..FLUSH_BARRIER_SLOTS {
        if state.barriers.take_completion_notification(index) {
            let _ = crate::kernel::task::scheduler::wake_all(&state.barrier_waiters[index])?;
        }
    }
    Ok(())
}

fn barrier_invariant(message: &str) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {message}"))
}

pub(super) fn write_record(console: &dyn Console, record: Record, message: &[u8]) {
    let mut writer = ConsoleWriter(console);
    let _ = write!(writer, "<{}>[{:06}] ", record.level as u8, record.sequence);
    console.write_bytes(message);
    if record.flags.contains(RecordFlags::TRUNCATED) || record.copied != record.length {
        console.write_bytes(b" [truncated]");
    }
    if !message.ends_with(b"\n") {
        console.write_bytes(b"\n");
    }
}

pub(super) fn write_overrun(console: &ConsoleDevice, missed: u64) {
    let mut writer = ConsoleWriter(console);
    let _ = writeln!(writer, "<4>[log] {missed} record(s) lost");
}

pub(super) fn write_ring_failure(console: &ConsoleDevice, error: super::Error) {
    let mut writer = ConsoleWriter(console);
    let _ = writeln!(writer, "<2>[log] ring read failure: {error:?}");
}

pub(super) fn write_raw(console: &ConsoleDevice, bytes: &[u8]) {
    console.write_bytes(bytes);
}

pub(super) fn write_raw_overflow(console: &ConsoleDevice, dropped: u64) {
    let mut writer = ConsoleWriter(console);
    let _ = writeln!(writer, "<4>[console] {dropped} guest console byte(s) lost");
}

const fn configured_console_level() -> Level {
    match Level::from_u8(hyper::config::CONSOLE_LOGLEVEL as u8) {
        Some(level) => level,
        None => Level::Info,
    }
}
