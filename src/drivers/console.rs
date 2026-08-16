use crate::hal::console::Console;
use crate::hal::io::PortIo;
use crate::platform::{ConsoleInfo, ConsoleKind, ConsoleRegisterAccess, chosen::CommandLine};

use super::serial::{MmioAccess, Ns16550, Ns16550Error, Pl011};

/// A platform-selected early console driver.
#[derive(Clone, Copy)]
pub enum ConsoleDevice {
    Pl011(Pl011),
    Ns16550(Ns16550),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EarlyConsoleError {
    InvalidAddress,
    MissingAddress,
    UnsupportedDriver,
}

#[derive(Clone, Copy)]
pub struct EmergencyConsoleHandle {
    pub base: usize,
    pub metadata: usize,
}

/// Parses the explicit-address subset of Linux's `earlycon=` command line.
///
/// Supported drivers are PL011 and NS16550/8250. NS16550 accepts Linux's
/// byte-wide `mmio` and word-wide, word-spaced `mmio32` selectors. Firmware is
/// expected to have configured the UART before entry, so optional
/// serial-format fields are not consumed.
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
    let driver = fields.next().ok_or(EarlyConsoleError::UnsupportedDriver)?;
    let mut address = fields.next().ok_or(EarlyConsoleError::MissingAddress)?;
    let access = matches!(address, "mmio" | "mmio32" | "io").then_some(address);
    if access.is_some() {
        address = fields.next().ok_or(EarlyConsoleError::MissingAddress)?;
    }
    let address = parse_address(address).ok_or(EarlyConsoleError::InvalidAddress)?;
    if address == 0 || address & 3 != 0 {
        return Err(EarlyConsoleError::InvalidAddress);
    }
    let (kind, access) = match driver {
        "pl011" => (ConsoleKind::Pl011, ConsoleRegisterAccess::Native),
        "uart8250" | "ns16550" => (
            ConsoleKind::Ns16550,
            if access == Some("io") {
                ConsoleRegisterAccess::Port
            } else if access == Some("mmio32") {
                ConsoleRegisterAccess::Mmio32 { register_shift: 2 }
            } else {
                ConsoleRegisterAccess::Mmio8 { register_shift: 0 }
            },
        ),
        _ => return Err(EarlyConsoleError::UnsupportedDriver),
    };
    Ok(Some(ConsoleInfo {
        kind,
        base: address,
        access,
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
            Self::Ns16550(device) => device.write_byte(byte),
        }
    }
}

impl ConsoleDevice {
    #[cfg(CONFIG_CRASH_CONSOLE)]
    pub fn try_read_byte(&self) -> Option<u8> {
        match self {
            Self::Pl011(device) => device.try_read().map(|received| received.byte),
            Self::Ns16550(device) => device.try_read().map(|received| received.byte),
        }
    }

    /// Encodes a lock-free handle retained by fatal console paths.
    pub const fn emergency_handle(self) -> EmergencyConsoleHandle {
        match self {
            Self::Pl011(device) => EmergencyConsoleHandle {
                base: device.mmio_base(),
                metadata: 1,
            },
            Self::Ns16550(device) => {
                let (kind, shift) = match device.mmio_access() {
                    Some(MmioAccess::Byte { register_shift }) => (2, register_shift),
                    Some(MmioAccess::Word { register_shift }) => (6, register_shift),
                    None => (3, 0),
                };
                EmergencyConsoleHandle {
                    base: device.mmio_base(),
                    metadata: kind | ((shift as usize) << 3),
                }
            }
        }
    }

    /// Reconstructs a console from a handle returned by [`Self::emergency_handle`].
    ///
    /// # Safety
    ///
    /// The encoded MMIO mapping must still be valid and exclusively owned by
    /// the console subsystem.
    pub unsafe fn from_emergency_handle(
        handle: EmergencyConsoleHandle,
        port_io: Option<PortIo>,
    ) -> Option<Self> {
        match handle.metadata & 3 {
            1 => Some(Self::Pl011(unsafe { Pl011::from_mmio_base(handle.base) })),
            2 => {
                let shift = (handle.metadata >> 3) as u8;
                let access = if handle.metadata & (1 << 2) == 0 {
                    MmioAccess::Byte {
                        register_shift: shift,
                    }
                } else {
                    MmioAccess::Word {
                        register_shift: shift,
                    }
                };
                match unsafe { Ns16550::from_mmio(handle.base, access) } {
                    Ok(device) => Some(Self::Ns16550(device)),
                    Err(_) => None,
                }
            }
            3 => match (u16::try_from(handle.base), port_io) {
                (Ok(base), Some(io)) => match unsafe { Ns16550::from_port(base, io) } {
                    Ok(device) => Some(Self::Ns16550(device)),
                    Err(_) => None,
                },
                _ => None,
            },
            _ => None,
        }
    }
}

/// Binds a discovered console description to its mapped register address.
///
/// # Safety
///
/// `mapped_base` must map the register range described by `info` with Device
/// memory attributes, and that range must have a single driver owner.
pub unsafe fn bind(
    info: ConsoleInfo,
    mapped_base: usize,
    port_io: Option<PortIo>,
) -> Result<ConsoleDevice, Ns16550Error> {
    match info.kind {
        ConsoleKind::Pl011 => {
            // SAFETY: The caller transfers the validated MMIO range to the
            // selected driver implementation.
            Ok(ConsoleDevice::Pl011(unsafe {
                Pl011::from_mmio_base(mapped_base)
            }))
        }
        ConsoleKind::Ns16550 => {
            if info.access == ConsoleRegisterAccess::Port {
                let port = u16::try_from(info.base).map_err(|_| Ns16550Error::InvalidAccess)?;
                let io = port_io.ok_or(Ns16550Error::InvalidAccess)?;
                return unsafe { Ns16550::from_port(port, io) }.map(ConsoleDevice::Ns16550);
            }
            let access = match info.access {
                ConsoleRegisterAccess::Mmio8 { register_shift } => {
                    MmioAccess::Byte { register_shift }
                }
                ConsoleRegisterAccess::Mmio32 { register_shift } => {
                    MmioAccess::Word { register_shift }
                }
                ConsoleRegisterAccess::Native => MmioAccess::BYTE,
                ConsoleRegisterAccess::Port => return Err(Ns16550Error::InvalidAccess),
            };
            // SAFETY: The caller transfers the validated MMIO mapping.
            unsafe { Ns16550::from_mmio(mapped_base, access) }.map(ConsoleDevice::Ns16550)
        }
    }
}
