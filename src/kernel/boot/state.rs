use hyper::platform::ConsoleInfo;
use hyper::platform::PhysicalRange;
use hyper::platform::PlatformInfo;
use hyper::sync::InterruptSpinLock;

use crate::kernel::mm::PreparedMemory;

pub struct BootState {
    pub platform: PlatformInfo,
    pub essential: crate::arch::platform::EssentialInfo,
    pub early_console: Option<ConsoleInfo>,
    pub memory: PreparedMemory,
    pub dtb_address: u64,
    pub image_physical_start: u64,
    pub initial_ramdisk: PhysicalRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInstalled,
    NotInstalled,
}

type KernelSpinLock<T> = InterruptSpinLock<T, crate::arch::irq::LocalMask>;

static BOOT_STATE: KernelSpinLock<Option<BootState>> = KernelSpinLock::new(None);

pub fn install(state: BootState) -> Result<(), Error> {
    BOOT_STATE.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInstalled);
        }
        *slot = Some(state);
        Ok(())
    })
}

pub fn with<R>(operation: impl FnOnce(&BootState) -> R) -> Result<R, Error> {
    BOOT_STATE.with(|slot| match slot.as_ref() {
        Some(state) => Ok(operation(state)),
        None => Err(Error::NotInstalled),
    })
}
