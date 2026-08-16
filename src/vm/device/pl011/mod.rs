//! Architecture-neutral PL011 device model for a virtual machine.

use crate::drivers::serial::pl011_registers as reg;

const FIFO_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualPl011Error {
    InvalidAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualPl011Access {
    pub value: Option<u64>,
    pub transmitted: Option<u8>,
    pub interrupt_asserted: bool,
}

impl VirtualPl011Access {
    const fn read(value: u32, interrupt_asserted: bool) -> Self {
        Self {
            value: Some(value as u64),
            transmitted: None,
            interrupt_asserted,
        }
    }

    const fn write(transmitted: Option<u8>, interrupt_asserted: bool) -> Self {
        Self {
            value: None,
            transmitted,
            interrupt_asserted,
        }
    }
}

/// PL011 register state. Host byte-stream routing deliberately lives outside
/// the model so the same device can target a physical UART, a driver domain,
/// or a future virtio console backend.
pub struct VirtualPl011 {
    receive_fifo: [u16; FIFO_CAPACITY],
    receive_head: usize,
    receive_length: usize,
    receive_status: u32,
    integer_baud: u32,
    fractional_baud: u32,
    line_control: u32,
    control: u32,
    fifo_levels: u32,
    interrupt_mask: u32,
    receive_timeout: bool,
    dma_control: u32,
}

impl VirtualPl011 {
    pub const fn new() -> Self {
        Self {
            receive_fifo: [0; FIFO_CAPACITY],
            receive_head: 0,
            receive_length: 0,
            receive_status: 0,
            integer_baud: 13,
            fractional_baud: 1,
            line_control: reg::LCR_H_FEN | (3 << reg::LCR_H_WLEN_SHIFT),
            control: reg::CR_UARTEN | reg::CR_TXE | reg::CR_RXE,
            fifo_levels: 0,
            interrupt_mask: 0,
            receive_timeout: false,
            dma_control: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn receive(&mut self, byte: u8) -> bool {
        if self.control & (reg::CR_UARTEN | reg::CR_RXE) != (reg::CR_UARTEN | reg::CR_RXE) {
            return self.interrupt_asserted();
        }
        let capacity = self.active_fifo_capacity();
        if self.receive_length == capacity {
            self.receive_status |= reg::RSR_OE;
        } else {
            let tail = (self.receive_head + self.receive_length) % FIFO_CAPACITY;
            self.receive_fifo[tail] = u16::from(byte);
            self.receive_length += 1;
            self.receive_timeout = true;
        }
        self.interrupt_asserted()
    }

    pub fn read(
        &mut self,
        offset: u64,
        size: usize,
    ) -> Result<VirtualPl011Access, VirtualPl011Error> {
        if !valid_access(offset, size) {
            return Err(VirtualPl011Error::InvalidAccess);
        }
        let value = match offset as usize {
            reg::DR => self.read_data(),
            reg::RSR_ECR => self.receive_status,
            reg::FR => self.flags(),
            reg::ILPR => 0,
            reg::IBRD => self.integer_baud,
            reg::FBRD => self.fractional_baud,
            reg::LCR_H => self.line_control,
            reg::CR => self.control,
            reg::IFLS => self.fifo_levels,
            reg::IMSC => self.interrupt_mask,
            reg::RIS => self.raw_interrupt_status(),
            reg::MIS => self.masked_interrupt_status(),
            reg::DMACR => self.dma_control,
            reg::PERIPH_ID0 => reg::PERIPH_ID0_VALUE,
            reg::PERIPH_ID1 => reg::PERIPH_ID1_VALUE,
            reg::PERIPH_ID2 => reg::PERIPH_ID2_VALUE,
            reg::PERIPH_ID3 => reg::PERIPH_ID3_VALUE,
            reg::PCELL_ID0 => reg::PCELL_ID0_VALUE,
            reg::PCELL_ID1 => reg::PCELL_ID1_VALUE,
            reg::PCELL_ID2 => reg::PCELL_ID2_VALUE,
            reg::PCELL_ID3 => reg::PCELL_ID3_VALUE,
            _ => return Err(VirtualPl011Error::InvalidAccess),
        };
        Ok(VirtualPl011Access::read(value, self.interrupt_asserted()))
    }

    pub fn write(
        &mut self,
        offset: u64,
        size: usize,
        value: u64,
    ) -> Result<VirtualPl011Access, VirtualPl011Error> {
        if !valid_access(offset, size) {
            return Err(VirtualPl011Error::InvalidAccess);
        }
        let value = value as u32;
        let transmitted = match offset as usize {
            reg::DR => self.write_data(value),
            reg::RSR_ECR => {
                self.receive_status = 0;
                None
            }
            reg::IBRD => {
                self.integer_baud = value & 0xffff;
                None
            }
            reg::FBRD => {
                self.fractional_baud = value & 0x3f;
                None
            }
            reg::LCR_H => {
                self.write_line_control(value);
                None
            }
            reg::CR => {
                self.control = value & reg::CR_MASK;
                None
            }
            reg::IFLS => {
                self.fifo_levels = value & reg::IFLS_MASK;
                None
            }
            reg::IMSC => {
                self.interrupt_mask = value & reg::INT_ALL;
                None
            }
            reg::ICR => {
                self.clear_interrupts(value);
                None
            }
            reg::DMACR => {
                self.dma_control = value & reg::DMACR_MASK;
                None
            }
            reg::ILPR => None,
            _ => return Err(VirtualPl011Error::InvalidAccess),
        };
        Ok(VirtualPl011Access::write(
            transmitted,
            self.interrupt_asserted(),
        ))
    }

    pub fn interrupt_asserted(&self) -> bool {
        self.masked_interrupt_status() != 0
    }

    fn read_data(&mut self) -> u32 {
        let Some(value) = self.receive_fifo.get(self.receive_head).copied() else {
            return 0;
        };
        if self.receive_length == 0 {
            return 0;
        }
        self.receive_head = (self.receive_head + 1) % FIFO_CAPACITY;
        self.receive_length -= 1;
        self.receive_timeout = false;
        u32::from(value)
    }

    fn write_data(&self, value: u32) -> Option<u8> {
        let enabled = reg::CR_UARTEN | reg::CR_TXE;
        (self.control & enabled == enabled).then_some((value & reg::DR_DATA_MASK) as u8)
    }

    fn write_line_control(&mut self, value: u32) {
        let value = value
            & (reg::LCR_H_BRK
                | reg::LCR_H_PEN
                | reg::LCR_H_EPS
                | reg::LCR_H_STP2
                | reg::LCR_H_FEN
                | reg::LCR_H_WLEN_MASK
                | reg::LCR_H_SPS);
        if (self.line_control ^ value) & reg::LCR_H_FEN != 0 {
            self.receive_head = 0;
            self.receive_length = 0;
            self.receive_timeout = false;
        }
        self.line_control = value;
    }

    fn flags(&self) -> u32 {
        let mut flags = reg::FR_TXFE | reg::FR_CTS;
        if self.receive_length == 0 {
            flags |= reg::FR_RXFE;
        }
        if self.receive_length == self.active_fifo_capacity() {
            flags |= reg::FR_RXFF;
        }
        flags
    }

    fn raw_interrupt_status(&self) -> u32 {
        let mut status = reg::INT_TX;
        if self.receive_length >= self.receive_trigger_level() {
            status |= reg::INT_RX;
        }
        if self.receive_timeout && self.receive_length != 0 {
            status |= reg::INT_RT;
        }
        if self.receive_status & reg::RSR_FE != 0 {
            status |= reg::INT_FE;
        }
        if self.receive_status & reg::RSR_PE != 0 {
            status |= reg::INT_PE;
        }
        if self.receive_status & reg::RSR_BE != 0 {
            status |= reg::INT_BE;
        }
        if self.receive_status & reg::RSR_OE != 0 {
            status |= reg::INT_OE;
        }
        status
    }

    fn masked_interrupt_status(&self) -> u32 {
        self.raw_interrupt_status() & self.interrupt_mask
    }

    fn clear_interrupts(&mut self, mask: u32) {
        if mask & reg::INT_RT != 0 {
            self.receive_timeout = false;
        }
        if mask & reg::INT_ERROR_MASK != 0 {
            self.receive_status = 0;
        }
    }

    fn active_fifo_capacity(&self) -> usize {
        if self.line_control & reg::LCR_H_FEN != 0 {
            FIFO_CAPACITY
        } else {
            1
        }
    }

    fn receive_trigger_level(&self) -> usize {
        if self.line_control & reg::LCR_H_FEN == 0 {
            return 1;
        }
        match (self.fifo_levels >> reg::IFLS_RX_SHIFT) & reg::IFLS_FIELD_MASK {
            0 => FIFO_CAPACITY / 8,
            1 => FIFO_CAPACITY / 4,
            2 => FIFO_CAPACITY / 2,
            3 => FIFO_CAPACITY * 3 / 4,
            _ => FIFO_CAPACITY * 7 / 8,
        }
    }
}

impl Default for VirtualPl011 {
    fn default() -> Self {
        Self::new()
    }
}

const fn valid_access(offset: u64, size: usize) -> bool {
    matches!(size, 1 | 2 | 4) && offset & (size as u64 - 1) == 0
}
