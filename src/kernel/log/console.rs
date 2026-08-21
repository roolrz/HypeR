//! Console filtering and serialized log draining.

use core::fmt::Write;

use hyper::drivers::console::{ConsoleDevice, EmergencyConsoleHandle};
use hyper::hal::console::{Console, ConsoleWriter};
use hyper::log::{Level, ReadResult, Record, RecordFlags};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicBool, AtomicFlag, AtomicUsize, Ordering};

type KernelSpinLock<T> = InterruptSpinLock<T, crate::arch::irq::LocalMask>;

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
static EMERGENCY_CONSOLE: AtomicUsize = AtomicUsize::new(0);
static EMERGENCY_CONSOLE_METADATA: AtomicUsize = AtomicUsize::new(0);
static EMERGENCY_CONSOLE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static FLUSHING: AtomicFlag = AtomicFlag::new(false);
static FLUSH_REQUESTED: AtomicBool = AtomicBool::new(false);
static EMERGENCY_MODE: AtomicBool = AtomicBool::new(false);

pub fn install(console: ConsoleDevice) {
    let emergency = console.emergency_handle();
    CONSOLE.with(|state| {
        publish_emergency_handle(Some(emergency));
        state.device = Some(console);
    });
    flush();
}

/// Retires the identity-mapped console before runtime promotion.
///
/// This is a one-way boot transition. Failure between retirement and the next
/// [`install`] remains diagnosable through the log ring, but must not access a
/// virtual address whose bootstrap mapping no longer exists.
pub(crate) fn retire_bootstrap() {
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
    flush();
}

pub fn loglevel() -> Level {
    CONSOLE.with(|state| state.maximum_level)
}

/// Drains all records eligible for the current console loglevel.
pub fn flush() {
    if EMERGENCY_MODE.load(Ordering::Acquire) {
        return;
    }
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

/// Stops normal ring draining once fatal diagnostics switch to direct output.
///
/// Emergency records remain retained for post-mortem readers, but allowing an
/// unrelated CPU to drain them would print every fatal line a second time.
pub(super) fn enter_emergency_mode() {
    EMERGENCY_MODE.store(true, Ordering::Release);
}

/// Writes a best-effort fatal message without waiting for kernel log locks.
pub(super) fn emergency_write(message: &[u8]) {
    let Some(device) = emergency_device() else {
        return;
    };
    device.write_bytes(b"<0>[exception] ");
    device.write_bytes(message);
    if !message.ends_with(b"\n") {
        device.write_bytes(b"\n");
    }
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(super) fn emergency_available() -> bool {
    emergency_device().is_some()
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(super) fn emergency_write_raw(bytes: &[u8]) {
    if let Some(device) = emergency_device() {
        device.write_bytes(bytes);
    }
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(super) fn emergency_read_raw() -> Option<u8> {
    emergency_device().and_then(|device| device.try_read_byte())
}

fn emergency_device() -> Option<ConsoleDevice> {
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
    unsafe { ConsoleDevice::from_emergency_handle(handle, crate::arch::platform::port_io()) }
}

/// Writes one guest-console byte through the selected host console without
/// inserting a kernel log prefix.
pub(crate) fn write_raw_byte(byte: u8) {
    CONSOLE.with(|state| {
        if let Some(device) = state.device {
            device.write_byte(byte);
        }
    });
}

fn drain() {
    let mut message = [0u8; LOG_LINE_MAX];
    loop {
        if EMERGENCY_MODE.load(Ordering::Acquire) {
            return;
        }
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
