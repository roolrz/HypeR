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
static BUILTIN_DRIVERS: &[&dyn PlatformDriver] = &[
    &hyper::drivers::serial::PL011_PLATFORM_DRIVER,
    &hyper::drivers::serial::NS16550_PLATFORM_DRIVER,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    AlreadyInitialized,
    DriverRegistration(ProbeError),
    Fdt(fdt::Error),
    Scan(ScanError),
}

pub(super) struct InitializationReport {
    pub(super) drivers: ProbeReport,
    pub(super) console: Result<Option<super::serial::Capabilities>, super::serial::Error>,
}

pub(super) fn initialize(
    boot: &super::super::boot::Initialization,
) -> Result<InitializationReport, Error> {
    let mut scanner = DeviceScanner::new(boot.essential().claims());
    // SAFETY: The DTB reservation remains in the permanent RAM linear map.
    unsafe { fdt::discover_with(boot.linear_dtb(), &mut scanner) }.map_err(Error::Fdt)?;
    let mut devices = scanner.finish().map_err(Error::Scan)?;
    let console = super::serial::initialize(boot, &devices);
    let reserved_console_base = boot.early_console().map(|console| console.base);
    devices.retain(|device| {
        reserved_console_base.is_none_or(|reserved| {
            device.registers().first().map(|range| range.start()) != Some(reserved)
        })
    });
    let mut manager = DriverManager::new();
    for &driver in BUILTIN_DRIVERS {
        manager
            .register(driver)
            .map_err(Error::DriverRegistration)?;
    }
    let drivers = manager.probe_devices(&devices, &KernelDriverServices);
    PLATFORM_BUS.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(PlatformBusState {
            _devices: devices,
            _manager: manager,
        });
        Ok(InitializationReport { drivers, console })
    })
}
