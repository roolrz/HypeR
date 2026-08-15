use crate::hal::console::Console;
use crate::platform::{ConsoleInfo, ConsoleKind, chosen::CommandLine};

use super::serial::Pl011;

/// A platform-selected early console driver.
#[derive(Clone, Copy)]
pub enum ConsoleDevice {
    Pl011(Pl011),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EarlyConsoleError {
    InvalidAddress,
    MissingAddress,
    UnsupportedDriver,
}

/// Parses the explicit-address subset of Linux's `earlycon=` command line.
///
/// Supported forms are `earlycon=pl011,<address>` and
/// `earlycon=pl011,mmio32,<address>`. Firmware is expected to have configured
/// the UART before entry, so optional serial-format fields are not consumed.
pub fn early_console(
    command_line: Option<&CommandLine>,
) -> Result<Option<ConsoleInfo>, EarlyConsoleError> {
    let Some(value) = command_line.and_then(|arguments| arguments.value("earlycon")) else {
        return Ok(None);
    };
    if value == "off" {
        return Ok(None);
    }
    let mut fields = value.split(',');
    if fields.next() != Some("pl011") {
        return Err(EarlyConsoleError::UnsupportedDriver);
    }
    let mut address = fields.next().ok_or(EarlyConsoleError::MissingAddress)?;
    if matches!(address, "mmio" | "mmio32") {
        address = fields.next().ok_or(EarlyConsoleError::MissingAddress)?;
    }
    let address = parse_address(address).ok_or(EarlyConsoleError::InvalidAddress)?;
    if address == 0 || address & 3 != 0 {
        return Err(EarlyConsoleError::InvalidAddress);
    }
    Ok(Some(ConsoleInfo {
        kind: ConsoleKind::Pl011,
        base: address,
    }))
}

fn parse_address(value: &str) -> Option<u64> {
    let (digits, radix) = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(digits) => (digits, 16),
        None => (value, 10),
    };
    if digits.is_empty() {
        return None;
    }
    u64::from_str_radix(digits, radix).ok()
}

impl Console for ConsoleDevice {
    fn write_byte(&self, byte: u8) {
        match self {
            Self::Pl011(device) => device.write_byte(byte),
        }
    }
}

/// Binds a discovered console description to its mapped register address.
///
/// # Safety
///
/// `mapped_base` must map the register range described by `info` with Device
/// memory attributes, and that range must have a single driver owner.
pub unsafe fn bind(info: ConsoleInfo, mapped_base: usize) -> ConsoleDevice {
    match info.kind {
        ConsoleKind::Pl011 => {
            // SAFETY: The caller transfers the validated MMIO range to the
            // selected driver implementation.
            ConsoleDevice::Pl011(unsafe { Pl011::from_mmio_base(mapped_base) })
        }
    }
}
