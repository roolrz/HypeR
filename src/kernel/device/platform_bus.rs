//! Platform-device enumeration and driver binding orchestration.

use alloc::vec::Vec;

use hyper::{
    drivers::platform::{
        DeviceScanner, DriverManager, DriverServices, PlatformDevice, PlatformDriver, ProbeError,
        ProbeReport, ScanError,
    },
    platform::fdt,
    sync::InterruptSpinLock,
};

type KernelSpinLock<T> = InterruptSpinLock<T, crate::arch::LocalInterruptMask>;

struct KernelDriverServices;

impl DriverServices for KernelDriverServices {
    fn map_mmio(&self, physical_address: u64) -> Option<usize> {
        crate::kernel::mm::memory::mmio_address(physical_address)
    }
}

struct PlatformBusState {
    _devices: Vec<PlatformDevice>,
    _manager: DriverManager,
}

static PLATFORM_BUS: KernelSpinLock<Option<PlatformBusState>> = KernelSpinLock::new(None);
static BUILTIN_DRIVERS: &[&dyn PlatformDriver] = &[&hyper::drivers::serial::PLATFORM_DRIVER];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    DriverRegistration(ProbeError),
    Fdt(fdt::Error),
    Scan(ScanError),
}

pub fn initialize(linear_dtb: usize, claims: &[Option<fdt::NodeId>]) -> Result<ProbeReport, Error> {
    let mut scanner = DeviceScanner::new(claims);
    // SAFETY: The DTB reservation remains in the permanent RAM linear map.
    unsafe { fdt::discover_with(linear_dtb, &mut scanner) }.map_err(Error::Fdt)?;
    let devices = scanner.finish().map_err(Error::Scan)?;
    let mut manager = DriverManager::new();
    for &driver in BUILTIN_DRIVERS {
        manager
            .register(driver)
            .map_err(Error::DriverRegistration)?;
    }
    let report = manager.probe_devices(&devices, &KernelDriverServices);
    PLATFORM_BUS.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(PlatformBusState {
            _devices: devices,
            _manager: manager,
        });
        Ok(report)
    })
}
