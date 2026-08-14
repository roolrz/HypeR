use crate::hal::console::Console;
use crate::platform::{ConsoleInfo, ConsoleKind};

use super::serial::Pl011;

/// A platform-selected early console driver.
#[derive(Clone, Copy)]
pub enum ConsoleDevice {
    Pl011(Pl011),
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
