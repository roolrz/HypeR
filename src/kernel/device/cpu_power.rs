//! Kernel-facing CPU power service.

use hyper::hal::cpu_power::{
    CpuAffinityState, CpuHardwareId, CpuPower, CpuPowerCapabilities, ResumeAddress, SuspendState,
};
use hyper::platform::CpuPowerInfo;
use hyper::sync::InterruptSpinLock;

type CpuPowerLock =
    InterruptSpinLock<Option<crate::arch::ArchitectureCpuPower>, crate::arch::LocalInterruptMask>;

static CPU_POWER: CpuPowerLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    NotInitialized,
    Architecture(crate::arch::CpuPowerError),
}

impl From<crate::arch::CpuPowerError> for Error {
    fn from(error: crate::arch::CpuPowerError) -> Self {
        Self::Architecture(error)
    }
}

pub fn initialize(info: CpuPowerInfo) -> Result<CpuPowerCapabilities, Error> {
    CPU_POWER.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        let controller = crate::arch::initialize_cpu_power(info)?;
        let capabilities = controller.capabilities();
        *slot = Some(controller);
        Ok(capabilities)
    })
}

pub fn capabilities() -> Result<CpuPowerCapabilities, Error> {
    Ok(controller()?.capabilities())
}

pub fn cpu_on(target: CpuHardwareId, entry: ResumeAddress, context: u64) -> Result<(), Error> {
    controller()?
        .cpu_on(target, entry, context)
        .map_err(Error::from)
}

pub fn cpu_off() -> Result<(), Error> {
    controller()?.cpu_off().map_err(Error::from)
}

pub fn cpu_suspend(state: SuspendState, entry: ResumeAddress, context: u64) -> Result<(), Error> {
    controller()?
        .cpu_suspend(state, entry, context)
        .map_err(Error::from)
}

pub fn affinity_info(
    target: CpuHardwareId,
    lowest_affinity_level: u8,
) -> Result<CpuAffinityState, Error> {
    controller()?
        .affinity_info(target, lowest_affinity_level)
        .map_err(Error::from)
}

pub fn system_off() -> Result<(), Error> {
    controller()?.system_off().map_err(Error::from)
}

pub fn system_reset() -> Result<(), Error> {
    controller()?.system_reset().map_err(Error::from)
}

fn controller() -> Result<crate::arch::ArchitectureCpuPower, Error> {
    CPU_POWER.with(|slot| {
        let controller = *slot.as_ref().ok_or(Error::NotInitialized)?;
        Ok(controller)
    })
}
