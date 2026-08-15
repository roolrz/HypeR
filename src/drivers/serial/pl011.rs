use alloc::boxed::Box;
use core::ptr::{read_volatile, write_volatile};

use crate::drivers::platform::{
    DriverInstance, DriverServices, PlatformDevice, PlatformDriver, ProbeError,
};
use crate::hal::console::Console;

const DATA_REGISTER: usize = 0x000;
const FLAG_REGISTER: usize = 0x018;
const FLAG_TRANSMIT_FIFO_FULL: u32 = 1 << 5;

/// ARM PrimeCell PL011 UART using an already configured firmware instance.
///
/// Early boot intentionally preserves the baud rate and line setup installed
/// by QEMU/firmware. A later serial subsystem may take ownership and reconfigure
/// the device after clocks and pin control are represented in the HAL.
#[derive(Clone, Copy)]
pub struct Pl011 {
    base: usize,
}

impl Pl011 {
    /// Creates a driver for an MMIO range discovered from trusted platform data.
    ///
    /// # Safety
    ///
    /// `base` must identify a mapped PL011 register block for the lifetime of
    /// the driver and must not be concurrently owned by another driver.
    pub const unsafe fn from_mmio_base(base: usize) -> Self {
        Self { base }
    }

    fn read_register(&self, offset: usize) -> u32 {
        // SAFETY: The constructor requires a valid, permanently mapped PL011.
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write_register(&self, offset: usize, value: u32) {
        // SAFETY: The constructor requires a valid, permanently mapped PL011.
        unsafe { write_volatile((self.base + offset) as *mut u32, value) }
    }
}

// SAFETY: PL011 output is synchronized by its FIFO state and the global console
// lock. The address is immutable after construction.
unsafe impl Send for Pl011 {}
unsafe impl Sync for Pl011 {}

impl Console for Pl011 {
    fn write_byte(&self, byte: u8) {
        while self.read_register(FLAG_REGISTER) & FLAG_TRANSMIT_FIFO_FULL != 0 {
            core::hint::spin_loop();
        }
        self.write_register(DATA_REGISTER, u32::from(byte));
    }
}

impl DriverInstance for Pl011 {}

pub struct Pl011PlatformDriver;

impl PlatformDriver for Pl011PlatformDriver {
    fn name(&self) -> &'static str {
        "pl011"
    }

    fn compatible_table(&self) -> &'static [&'static str] {
        &["arm,pl011"]
    }

    fn probe(
        &self,
        device: &PlatformDevice,
        services: &dyn DriverServices,
    ) -> Result<Box<dyn DriverInstance>, ProbeError> {
        let registers = device.registers().first().ok_or(ProbeError::Resource)?;
        let base = services
            .map_mmio(registers.start())
            .ok_or(ProbeError::Resource)?;
        // SAFETY: The platform bus gives one successful driver exclusive
        // ownership of this translated MMIO resource.
        Ok(Box::new(unsafe { Pl011::from_mmio_base(base) }))
    }
}

pub static PLATFORM_DRIVER: Pl011PlatformDriver = Pl011PlatformDriver;
