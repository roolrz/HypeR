//! Kernel binding for the reusable legacy-PC virtual-device models.

use hyper::sync::InterruptSpinLock;
use hyper::vm::device::x86_legacy::{Error as ModelError, LegacyPcDevices, PendingInterrupt};

type DeviceLock = InterruptSpinLock<Option<LegacyPcDevices>, crate::arch::LocalInterruptMask>;

static DEVICES: DeviceLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Model(ModelError),
    NotInitialized,
}

impl From<ModelError> for Error {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

pub fn initialize() -> Result<(), Error> {
    DEVICES.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(LegacyPcDevices::new());
        Ok(())
    })
}

pub fn access(port: u16, size: usize, write: bool, value: u32) -> Result<Option<u32>, Error> {
    let outcome = DEVICES.with(|slot| {
        slot.as_mut()
            .ok_or(Error::NotInitialized)?
            .access(port, size, write, value)
            .map_err(Error::Model)
    })?;
    if let Some(byte) = outcome.transmitted {
        crate::kernel::vm::write_guest_console_byte(byte);
    }
    Ok(outcome.value)
}

pub fn pending_interrupt(timer_pending: bool) -> Result<Option<PendingInterrupt>, Error> {
    DEVICES.with(|slot| {
        slot.as_ref()
            .map(|devices| devices.pending_interrupt(timer_pending))
            .ok_or(Error::NotInitialized)
    })
}
