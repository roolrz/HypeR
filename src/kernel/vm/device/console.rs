//! Guest console service and host byte-stream backend routing.

use hyper::sync::InterruptSpinLock;
use hyper::vm::device::pl011::{VirtualPl011, VirtualPl011Error};
use hyper::vm::interrupt::VirtualInterruptId;

use crate::arch::GuestDataAccess;

type ConsoleLock = InterruptSpinLock<Option<VirtualPl011>, crate::arch::LocalInterruptMask>;

pub const BASE: u64 = 0x0900_0000;
pub const SIZE: u64 = 0x1000;
pub const INTERRUPT: u32 = 33;

static CONSOLE: ConsoleLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    InvalidInterrupt,
    Model(VirtualPl011Error),
    NotInitialized,
    Vcpu(super::super::VcpuInterruptError),
}

impl From<VirtualPl011Error> for Error {
    fn from(error: VirtualPl011Error) -> Self {
        Self::Model(error)
    }
}

impl From<super::super::VcpuInterruptError> for Error {
    fn from(error: super::super::VcpuInterruptError) -> Self {
        Self::Vcpu(error)
    }
}

pub fn initialize() -> Result<(), Error> {
    CONSOLE.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(VirtualPl011::new());
        Ok(())
    })
}

pub const fn handles(address: u64) -> bool {
    address >= BASE && address < BASE + SIZE
}

pub fn access(access: GuestDataAccess) -> Result<Option<u64>, Error> {
    let offset = access
        .address
        .checked_sub(BASE)
        .ok_or(Error::Model(VirtualPl011Error::InvalidAccess))?;
    let outcome = CONSOLE.with(|slot| {
        let model = slot.as_mut().ok_or(Error::NotInitialized)?;
        if access.write {
            model.write(offset, access.size, access.value)
        } else {
            model.read(offset, access.size)
        }
        .map_err(Error::Model)
    })?;
    if let Some(byte) = outcome.transmitted {
        crate::kernel::log::console::write_raw_byte(byte);
    }
    update_interrupt(outcome.interrupt_asserted)?;
    Ok(outcome.value)
}

pub fn receive(byte: u8) -> Result<(), Error> {
    let asserted = CONSOLE.with(|slot| {
        let model = slot.as_mut().ok_or(Error::NotInitialized)?;
        Ok::<bool, Error>(model.receive(byte))
    })?;
    update_interrupt(asserted)
}

fn update_interrupt(asserted: bool) -> Result<(), Error> {
    let interrupt = VirtualInterruptId::new(INTERRUPT).ok_or(Error::InvalidInterrupt)?;
    let _ = super::super::vcpu::update_active_device_interrupt(interrupt, asserted)?;
    Ok(())
}
