//! Kernel log production, retention, and reader API.

use core::fmt::{self, Write};

use hyper::log::{AppendError, Level, ReadError, ReadResult, RecordFlags, RingBuffer};
use hyper::sync::InterruptSpinLock;

pub(crate) mod console;

type KernelSpinLock<T> = InterruptSpinLock<T, crate::arch::LocalInterruptMask>;

const LOG_BUFFER_SIZE: usize = 1usize << hyper::config::LOG_BUF_SHIFT as usize;
const LOG_LINE_MAX: usize = hyper::config::LOG_LINE_MAX as usize;

struct FormatBuffer {
    bytes: [u8; LOG_LINE_MAX],
    length: usize,
    truncated: bool,
}

impl FormatBuffer {
    const fn new() -> Self {
        Self {
            bytes: [0; LOG_LINE_MAX],
            length: 0,
            truncated: false,
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

impl fmt::Write for FormatBuffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let available = LOG_LINE_MAX - self.length;
        let copied = available.min(text.len());
        self.bytes[self.length..self.length + copied].copy_from_slice(&text.as_bytes()[..copied]);
        self.length += copied;
        self.truncated |= copied != text.len();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Append(AppendError),
    Read(ReadError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Statistics {
    pub capacity: usize,
    pub dropped: u64,
    pub next_sequence: u64,
}

static LOG_RING: KernelSpinLock<RingBuffer<LOG_BUFFER_SIZE>> =
    KernelSpinLock::new(RingBuffer::new());

pub fn log(level: Level, arguments: fmt::Arguments<'_>) -> Result<u64, Error> {
    let mut formatted = FormatBuffer::new();
    let _ = formatted.write_fmt(arguments);
    let flags = if formatted.truncated {
        RecordFlags::TRUNCATED
    } else {
        RecordFlags::NONE
    };
    let sequence = LOG_RING
        .with(|ring| ring.append(level, formatted.as_slice(), flags))
        .map_err(Error::Append)?;
    console::flush();
    Ok(sequence)
}

/// Compatibility path for existing unclassified output.
pub fn write(arguments: fmt::Arguments<'_>) {
    let _ = log(Level::Info, arguments);
}

/// Copies one record without advancing any global reader cursor.
pub fn read(sequence: u64, output: &mut [u8]) -> Result<ReadResult, Error> {
    LOG_RING
        .with(|ring| ring.read(sequence, output))
        .map_err(Error::Read)
}

pub fn statistics() -> Statistics {
    LOG_RING.with(|ring| Statistics {
        capacity: ring.capacity(),
        dropped: ring.dropped(),
        next_sequence: ring.next_sequence(),
    })
}

pub fn set_console_loglevel(level: Level) {
    console::set_loglevel(level);
}

pub fn console_loglevel() -> Level {
    console::loglevel()
}

pub fn flush() {
    console::flush();
}

/// Reports the state of the logging backend at the end of kernel startup.
pub(crate) fn report_startup_state() {
    let statistics = statistics();
    crate::println!(
        "HypeR: kernel log ring: {} bytes, {} records dropped",
        statistics.capacity,
        statistics.dropped
    );
}

/// Emits a fatal diagnostic without waiting for a potentially interrupted
/// logging lock. Retention in the ring is best-effort; direct console output is
/// attempted independently so a lock failure cannot stall the fail-stop path.
pub fn emergency(arguments: fmt::Arguments<'_>) {
    let mut formatted = FormatBuffer::new();
    let _ = formatted.write_fmt(arguments);
    let flags = if formatted.truncated {
        RecordFlags::TRUNCATED
    } else {
        RecordFlags::NONE
    };
    let _ = LOG_RING.try_with(|ring| {
        let _ = ring.append(Level::Emergency, formatted.as_slice(), flags);
    });
    console::emergency_write(formatted.as_slice());
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(crate) fn crash_console_available() -> bool {
    console::emergency_available()
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(crate) fn crash_console_write(bytes: &[u8]) {
    console::emergency_write_raw(bytes);
}

#[cfg(CONFIG_CRASH_CONSOLE)]
pub(crate) fn crash_console_read() -> Option<u8> {
    console::emergency_read_raw()
}

#[macro_export]
macro_rules! printk {
    ($level:expr, $($argument:tt)*) => {{
        let _ = $crate::kernel::log::log($level, core::format_args!($($argument)*));
    }};
}

#[macro_export]
macro_rules! pr_emerg {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Emergency, $($argument)*) };
}

#[macro_export]
macro_rules! pr_alert {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Alert, $($argument)*) };
}

#[macro_export]
macro_rules! pr_crit {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Critical, $($argument)*) };
}

#[macro_export]
macro_rules! pr_err {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Error, $($argument)*) };
}

#[macro_export]
macro_rules! pr_warn {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Warning, $($argument)*) };
}

#[macro_export]
macro_rules! pr_notice {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Notice, $($argument)*) };
}

#[macro_export]
macro_rules! pr_info {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Info, $($argument)*) };
}

#[macro_export]
macro_rules! pr_debug {
    ($($argument:tt)*) => { $crate::printk!(hyper::log::Level::Debug, $($argument)*) };
}

#[macro_export]
macro_rules! print {
    ($($argument:tt)*) => { $crate::pr_info!($($argument)*) };
}

#[macro_export]
macro_rules! println {
    () => { $crate::pr_info!("") };
    ($($argument:tt)*) => { $crate::pr_info!($($argument)*) };
}
