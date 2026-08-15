//! Console filtering and serialized log draining.

use core::fmt::Write;

use hyper::drivers::console::ConsoleDevice;
use hyper::hal::console::{Console, ConsoleWriter};
use hyper::log::{Level, ReadResult, Record, RecordFlags};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicBool, AtomicFlag, Ordering};

type KernelSpinLock<T> = InterruptSpinLock<T, crate::arch::LocalInterruptMask>;

const LOG_LINE_MAX: usize = hyper::config::LOG_LINE_MAX as usize;
const CONSOLE_LOGLEVEL: Level = configured_console_level();

struct ConsoleState {
    device: Option<ConsoleDevice>,
    next_sequence: u64,
    maximum_level: Level,
}

impl ConsoleState {
    const fn new() -> Self {
        Self {
            device: None,
            next_sequence: 0,
            maximum_level: CONSOLE_LOGLEVEL,
        }
    }
}

static CONSOLE: KernelSpinLock<ConsoleState> = KernelSpinLock::new(ConsoleState::new());
static FLUSHING: AtomicFlag = AtomicFlag::new(false);
static FLUSH_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn install(console: ConsoleDevice) {
    CONSOLE.with(|state| state.device = Some(console));
    flush();
}

/// Changes only console filtering; every severity remains in the ring.
pub fn set_loglevel(level: Level) {
    CONSOLE.with(|state| state.maximum_level = level);
    flush();
}

pub fn loglevel() -> Level {
    CONSOLE.with(|state| state.maximum_level)
}

/// Drains all records eligible for the current console loglevel.
pub fn flush() {
    FLUSH_REQUESTED.store(true, Ordering::Release);
    loop {
        if !FLUSHING.try_acquire() {
            return;
        }
        FLUSH_REQUESTED.store(false, Ordering::Release);
        drain();
        FLUSHING.release();
        if !FLUSH_REQUESTED.swap(false, Ordering::AcqRel) {
            return;
        }
    }
}

/// Writes a best-effort fatal message without waiting for kernel log locks.
pub(super) fn emergency_write(message: &[u8]) {
    let device = CONSOLE.try_with(|state| state.device).flatten();
    let Some(device) = device else {
        return;
    };
    device.write_bytes(b"<0>[exception] ");
    device.write_bytes(message);
    if !message.ends_with(b"\n") {
        device.write_bytes(b"\n");
    }
}

fn drain() {
    let mut message = [0u8; LOG_LINE_MAX];
    loop {
        let (device, sequence, maximum_level) =
            CONSOLE.with(|state| (state.device, state.next_sequence, state.maximum_level));
        let Some(device) = device else {
            return;
        };
        match super::read(sequence, &mut message) {
            Ok(ReadResult::Record(record)) => {
                advance(record.sequence.wrapping_add(1));
                if record.level <= maximum_level {
                    write_record(&device, record, &message[..record.copied]);
                }
            }
            Ok(ReadResult::Overrun {
                oldest_sequence,
                missed,
            }) => {
                advance(oldest_sequence);
                let mut writer = ConsoleWriter(&device);
                let _ = writeln!(writer, "<4>[log] {missed} record(s) lost");
            }
            Ok(ReadResult::Empty { .. }) => return,
            Err(error) => {
                let mut writer = ConsoleWriter(&device);
                let _ = writeln!(writer, "<2>[log] ring read failure: {error:?}");
                return;
            }
        }
    }
}

fn advance(sequence: u64) {
    CONSOLE.with(|state| {
        if state.next_sequence < sequence {
            state.next_sequence = sequence;
        }
    });
}

fn write_record(console: &dyn Console, record: Record, message: &[u8]) {
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

const fn configured_console_level() -> Level {
    match Level::from_u8(hyper::config::CONSOLE_LOGLEVEL as u8) {
        Some(level) => level,
        None => Level::Info,
    }
}
