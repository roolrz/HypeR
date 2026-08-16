//! NS16550-compatible physical UART driver.

use alloc::boxed::Box;
use core::ptr::{read_volatile, write_volatile};

use crate::drivers::platform::{
    DriverInstance, DriverServices, PlatformDevice, PlatformDriver, ProbeError,
};
use crate::hal::console::Console;

const RBR_THR_DLL: usize = 0;
const IER_DLM: usize = 1;
const IIR_FCR: usize = 2;
const LCR: usize = 3;
const MCR: usize = 4;
const LSR: usize = 5;
const MSR: usize = 6;
const SCR: usize = 7;

const IIR_NO_INTERRUPT: u8 = 1;
const IIR_IDENTIFICATION_MASK: u8 = 0x0e;
const FCR_ENABLE: u8 = 1;
const FCR_CLEAR_RECEIVE: u8 = 1 << 1;
const FCR_CLEAR_TRANSMIT: u8 = 1 << 2;
const FCR_TRIGGER_SHIFT: u8 = 6;
const LCR_STOP: u8 = 1 << 2;
const LCR_PARITY_ENABLE: u8 = 1 << 3;
const LCR_EVEN_PARITY: u8 = 1 << 4;
const LCR_STICK_PARITY: u8 = 1 << 5;
const LCR_BREAK: u8 = 1 << 6;
const LCR_DLAB: u8 = 1 << 7;
const MCR_DTR: u8 = 1;
const MCR_RTS: u8 = 1 << 1;
const MCR_OUT1: u8 = 1 << 2;
const MCR_OUT2: u8 = 1 << 3;
const MCR_LOOPBACK: u8 = 1 << 4;
const LSR_DATA_READY: u8 = 1;
const LSR_OVERRUN_ERROR: u8 = 1 << 1;
const LSR_PARITY_ERROR: u8 = 1 << 2;
const LSR_FRAMING_ERROR: u8 = 1 << 3;
const LSR_BREAK_INTERRUPT: u8 = 1 << 4;
const LSR_THR_EMPTY: u8 = 1 << 5;
const LSR_TRANSMITTER_EMPTY: u8 = 1 << 6;
const LSR_FIFO_ERROR: u8 = 1 << 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    BaudRateOutOfRange,
    InvalidAccess,
    InvalidClock,
}

/// Width and spacing of the eight standard UART registers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioAccess {
    Byte { register_shift: u8 },
    Word { register_shift: u8 },
}

impl MmioAccess {
    pub const BYTE: Self = Self::Byte { register_shift: 0 };
    pub const WORD: Self = Self::Word { register_shift: 2 };

    pub const fn register_shift(self) -> u8 {
        match self {
            Self::Byte { register_shift } | Self::Word { register_shift } => register_shift,
        }
    }

    const fn valid(self, base: usize) -> bool {
        let shift = self.register_shift();
        shift < usize::BITS as u8
            && match self {
                Self::Byte { .. } => true,
                Self::Word { .. } => base.is_multiple_of(4) && shift >= 2,
            }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopBits {
    One,
    Two,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parity {
    None,
    Odd,
    Even,
    StickOne,
    StickZero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineConfig {
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
}

impl LineConfig {
    pub const EIGHT_N_ONE: Self = Self {
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: Parity::None,
    };

    const fn register_value(self) -> u8 {
        let data = match self.data_bits {
            DataBits::Five => 0,
            DataBits::Six => 1,
            DataBits::Seven => 2,
            DataBits::Eight => 3,
        };
        let stop = match self.stop_bits {
            StopBits::One => 0,
            StopBits::Two => LCR_STOP,
        };
        let parity = match self.parity {
            Parity::None => 0,
            Parity::Odd => LCR_PARITY_ENABLE,
            Parity::Even => LCR_PARITY_ENABLE | LCR_EVEN_PARITY,
            Parity::StickOne => LCR_PARITY_ENABLE | LCR_STICK_PARITY,
            Parity::StickZero => LCR_PARITY_ENABLE | LCR_EVEN_PARITY | LCR_STICK_PARITY,
        };
        data | stop | parity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FifoTrigger {
    OneByte = 0,
    FourBytes = 1,
    EightBytes = 2,
    FourteenBytes = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptMask(u8);

impl InterruptMask {
    pub const NONE: Self = Self(0);
    pub const RECEIVED_DATA: Self = Self(1);
    pub const TRANSMIT_EMPTY: Self = Self(1 << 1);
    pub const LINE_STATUS: Self = Self(1 << 2);
    pub const MODEM_STATUS: Self = Self(1 << 3);
    pub const RUNTIME_INPUT: Self = Self(Self::RECEIVED_DATA.0 | Self::LINE_STATUS.0);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptIdentification {
    None,
    ModemStatus,
    TransmitEmpty,
    ReceivedData,
    LineStatus,
    CharacterTimeout,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceivedByte {
    pub byte: u8,
    pub overrun_error: bool,
    pub parity_error: bool,
    pub framing_error: bool,
    pub break_interrupt: bool,
    pub fifo_error: bool,
}

impl ReceivedByte {
    pub const fn has_error(self) -> bool {
        self.overrun_error
            || self.parity_error
            || self.framing_error
            || self.break_interrupt
            || self.fifo_error
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModemStatus(u8);

impl ModemStatus {
    pub const fn raw(self) -> u8 {
        self.0
    }

    pub const fn clear_to_send(self) -> bool {
        self.0 & (1 << 4) != 0
    }

    pub const fn data_set_ready(self) -> bool {
        self.0 & (1 << 5) != 0
    }

    pub const fn ring_indicator(self) -> bool {
        self.0 & (1 << 6) != 0
    }

    pub const fn carrier_detect(self) -> bool {
        self.0 & (1 << 7) != 0
    }
}

/// Shareable handle for one permanently mapped NS16550 register bank.
#[derive(Clone, Copy)]
pub struct Ns16550 {
    base: usize,
    access: MmioAccess,
}

impl Ns16550 {
    /// Creates a handle for a byte-wide, contiguous interface.
    ///
    /// # Safety
    ///
    /// `base` must be a permanent device mapping owned by the caller.
    pub const unsafe fn from_mmio_base(base: usize) -> Self {
        Self {
            base,
            access: MmioAccess::BYTE,
        }
    }

    /// Creates a handle for an explicitly described register interface.
    ///
    /// # Safety
    ///
    /// The complete register window must be mapped with device attributes and
    /// register ownership must be externally serialized.
    pub const unsafe fn from_mmio(base: usize, access: MmioAccess) -> Result<Self, Error> {
        if !access.valid(base) {
            return Err(Error::InvalidAccess);
        }
        let Some(last) = 7usize.checked_shl(access.register_shift() as u32) else {
            return Err(Error::AddressOverflow);
        };
        if base.checked_add(last).is_none() {
            return Err(Error::AddressOverflow);
        }
        Ok(Self { base, access })
    }

    pub const fn mmio_base(self) -> usize {
        self.base
    }

    pub const fn mmio_access(self) -> MmioAccess {
        self.access
    }

    pub fn configure(
        &self,
        input_clock_hz: u32,
        baud_rate: u32,
        line: LineConfig,
        receive_trigger: FifoTrigger,
    ) -> Result<(), Error> {
        let divisor = baud_divisor(input_clock_hz, baud_rate)?;
        self.set_interrupt_mask(InterruptMask::NONE);
        self.write(LCR, LCR_DLAB);
        self.write(RBR_THR_DLL, divisor as u8);
        self.write(IER_DLM, (divisor >> 8) as u8);
        self.write(LCR, line.register_value());
        self.configure_fifo(true, receive_trigger, true, true);
        self.set_modem_outputs(true, true, false, true, false);
        self.clear_pending_state();
        Ok(())
    }

    /// Takes input ownership while preserving firmware baud and line format.
    pub fn enable_runtime_input(&self, receive_trigger: FifoTrigger) {
        self.set_interrupt_mask(InterruptMask::NONE);
        self.configure_fifo(true, receive_trigger, true, false);
        self.clear_pending_state();
        self.set_irq_output(true);
        self.set_interrupt_mask(InterruptMask::RUNTIME_INPUT);
    }

    pub fn set_interrupt_mask(&self, mask: InterruptMask) {
        let line = self.read(LCR);
        self.write(LCR, line & !LCR_DLAB);
        self.write(IER_DLM, mask.raw() & 0x0f);
        self.write(LCR, line & !LCR_DLAB);
    }

    pub fn interrupt_identification(&self) -> InterruptIdentification {
        let value = self.read(IIR_FCR);
        if value & IIR_NO_INTERRUPT != 0 {
            return InterruptIdentification::None;
        }
        match value & IIR_IDENTIFICATION_MASK {
            0x00 => InterruptIdentification::ModemStatus,
            0x02 => InterruptIdentification::TransmitEmpty,
            0x04 => InterruptIdentification::ReceivedData,
            0x06 => InterruptIdentification::LineStatus,
            0x0c => InterruptIdentification::CharacterTimeout,
            other => InterruptIdentification::Unknown(other),
        }
    }

    pub fn configure_fifo(
        &self,
        enabled: bool,
        receive_trigger: FifoTrigger,
        clear_receive: bool,
        clear_transmit: bool,
    ) {
        self.write(
            IIR_FCR,
            if enabled { FCR_ENABLE } else { 0 }
                | if clear_receive { FCR_CLEAR_RECEIVE } else { 0 }
                | if clear_transmit {
                    FCR_CLEAR_TRANSMIT
                } else {
                    0
                }
                | ((receive_trigger as u8) << FCR_TRIGGER_SHIFT),
        );
    }

    pub fn try_read(&self) -> Option<ReceivedByte> {
        let status = self.read(LSR);
        if status & LSR_DATA_READY == 0 {
            return None;
        }
        Some(ReceivedByte {
            byte: self.read(RBR_THR_DLL),
            overrun_error: status & LSR_OVERRUN_ERROR != 0,
            parity_error: status & LSR_PARITY_ERROR != 0,
            framing_error: status & LSR_FRAMING_ERROR != 0,
            break_interrupt: status & LSR_BREAK_INTERRUPT != 0,
            fifo_error: status & LSR_FIFO_ERROR != 0,
        })
    }

    pub fn try_write(&self, byte: u8) -> bool {
        if self.read(LSR) & LSR_THR_EMPTY == 0 {
            return false;
        }
        self.write(RBR_THR_DLL, byte);
        true
    }

    pub fn wait_until_transmitted(&self) {
        while self.read(LSR) & LSR_TRANSMITTER_EMPTY == 0 {
            core::hint::spin_loop();
        }
    }

    pub fn set_break(&self, enabled: bool) {
        let mut control = self.read(LCR);
        if enabled {
            control |= LCR_BREAK;
        } else {
            control &= !LCR_BREAK;
        }
        self.write(LCR, control);
    }

    pub fn set_modem_outputs(&self, dtr: bool, rts: bool, out1: bool, out2: bool, loopback: bool) {
        self.write(
            MCR,
            if dtr { MCR_DTR } else { 0 }
                | if rts { MCR_RTS } else { 0 }
                | if out1 { MCR_OUT1 } else { 0 }
                | if out2 { MCR_OUT2 } else { 0 }
                | if loopback { MCR_LOOPBACK } else { 0 },
        );
    }

    /// Controls the conventional OUT2 interrupt gate without changing other
    /// modem outputs. Platforms that do not implement the gate ignore it.
    pub fn set_irq_output(&self, enabled: bool) {
        let control = self.read(MCR);
        self.write(
            MCR,
            if enabled {
                control | MCR_OUT2
            } else {
                control & !MCR_OUT2
            },
        );
    }

    pub fn modem_status(&self) -> ModemStatus {
        ModemStatus(self.read(MSR))
    }

    pub fn write_scratch(&self, value: u8) {
        self.write(SCR, value);
    }

    pub fn read_scratch(&self) -> u8 {
        self.read(SCR)
    }

    fn clear_pending_state(&self) {
        let _ = self.read(LSR);
        let _ = self.read(RBR_THR_DLL);
        let _ = self.read(IIR_FCR);
        let _ = self.read(MSR);
    }

    fn register_address(&self, register: usize) -> usize {
        self.base + (register << self.access.register_shift())
    }

    fn read(&self, register: usize) -> u8 {
        let address = self.register_address(register);
        unsafe {
            match self.access {
                MmioAccess::Byte { .. } => read_volatile(address as *const u8),
                MmioAccess::Word { .. } => read_volatile(address as *const u32) as u8,
            }
        }
    }

    fn write(&self, register: usize, value: u8) {
        let address = self.register_address(register);
        unsafe {
            match self.access {
                MmioAccess::Byte { .. } => write_volatile(address as *mut u8, value),
                MmioAccess::Word { .. } => write_volatile(address as *mut u32, u32::from(value)),
            }
        }
    }
}

impl Console for Ns16550 {
    fn write_byte(&self, byte: u8) {
        while !self.try_write(byte) {
            core::hint::spin_loop();
        }
    }
}

// SAFETY: MMIO operations are volatile; subsystem owners serialize compound
// configuration and interrupt-mask changes.
unsafe impl Send for Ns16550 {}
unsafe impl Sync for Ns16550 {}

impl DriverInstance for Ns16550 {
    fn suspend(&mut self) -> Result<(), ProbeError> {
        self.set_interrupt_mask(InterruptMask::NONE);
        Ok(())
    }
}

pub struct Ns16550PlatformDriver;

impl PlatformDriver for Ns16550PlatformDriver {
    fn name(&self) -> &'static str {
        "ns16550"
    }

    fn compatible_table(&self) -> &'static [&'static str] {
        &["ns16550a", "ns16550", "uart8250", "snps,dw-apb-uart"]
    }

    fn probe(
        &self,
        device: &PlatformDevice,
        services: &dyn DriverServices,
    ) -> Result<Box<dyn DriverInstance>, ProbeError> {
        let registers = device.registers().first().ok_or(ProbeError::Resource)?;
        let register_shift = match optional_u32(device, "reg-shift")? {
            Some(value) => u8::try_from(value).map_err(|_| ProbeError::Unsupported)?,
            None => 0,
        };
        let width = optional_u32(device, "reg-io-width")?.unwrap_or(1);
        let access = match width {
            1 => MmioAccess::Byte { register_shift },
            4 => MmioAccess::Word { register_shift },
            _ => return Err(ProbeError::Unsupported),
        };
        let window = 8u64
            .checked_shl(u32::from(register_shift))
            .ok_or(ProbeError::Resource)?;
        if registers.size() < window {
            return Err(ProbeError::Resource);
        }
        let base = services
            .map_mmio(registers.start())
            .ok_or(ProbeError::Resource)?;
        // SAFETY: Successful probing transfers this complete translated MMIO
        // resource to one platform-driver instance.
        let uart = unsafe { Ns16550::from_mmio(base, access) }.map_err(|_| ProbeError::Resource)?;
        Ok(Box::new(uart))
    }
}

fn optional_u32(device: &PlatformDevice, name: &str) -> Result<Option<u32>, ProbeError> {
    let Some(value) = device.property(name) else {
        return Ok(None);
    };
    let bytes: [u8; 4] = value.try_into().map_err(|_| ProbeError::Unsupported)?;
    Ok(Some(u32::from_be_bytes(bytes)))
}

pub static PLATFORM_DRIVER: Ns16550PlatformDriver = Ns16550PlatformDriver;

fn baud_divisor(input_clock_hz: u32, baud_rate: u32) -> Result<u16, Error> {
    if input_clock_hz == 0 {
        return Err(Error::InvalidClock);
    }
    if baud_rate == 0 {
        return Err(Error::BaudRateOutOfRange);
    }
    let denominator = u64::from(baud_rate) * 16;
    let divisor = (u64::from(input_clock_hz) + denominator / 2) / denominator;
    u16::try_from(divisor)
        .ok()
        .filter(|divisor| *divisor != 0)
        .ok_or(Error::BaudRateOutOfRange)
}
